use std::path::Path;
use std::thread;
use std::time::Duration;

use anyhow::Result;

use crate::SplitType;
use crate::game::{Game, GameCheck, GameVersion};
use crate::lss::{LiveSplit, TimerPhase};
use crate::shmem::GameMemory;
use crate::splits::Event;

const CONNECTION_RETRY_DURATION: Duration = Duration::from_millis(1000);
const EMULATOR_RETRY_DURATION: Duration = Duration::from_millis(5000);

const SECOND_ROOM: (u16, u16) = (0, 1);
const FINAL_BOSS_ROOM: (u16, u16) = (8, 7);

const LIVE_SPLIT_KEEP_ALIVE: i32 = 334; // ~5 seconds at the default update frequency
const EMULATOR_KEEP_ALIVE: i32 = 334;
const GAME_KEEP_ALIVE: i32 = 200; // ~3 seconds at the default update frequency

const SPLIT_TYPE_VARIABLE_NAME: &str = "GaleriansSplitType";

#[derive(Debug, Clone)]
struct KeepAliveCounter {
    period: i32,
    remaining: i32,
}

impl KeepAliveCounter {
    const fn new(period: i32) -> Self {
        Self { period, remaining: period }
    }
    
    const fn with_trigger_on_start(mut self) -> Self {
        self.remaining = 0;
        self
    }

    const fn should_check(&mut self) -> bool {
        self.remaining -= 1;
        if self.remaining < 0 {
            self.remaining = self.period;
            true
        } else {
            false
        }
    }

    const fn reset(&mut self) {
        self.remaining = self.period;
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum ConnectionState {
    LiveSplitPending,
    EmulatorPending,
    GamePending,
    Connected,
}

impl ConnectionState {
    fn advance(&mut self) {
        *self = match self {
            Self::LiveSplitPending => Self::EmulatorPending,
            Self::EmulatorPending => Self::GamePending,
            _ => Self::Connected,
        };
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum RunState {
    NotStarted,
    Intro,
    Active,
    Finished,
}

impl RunState {
    const fn is_started(&self) -> bool {
        !matches!(self, Self::NotStarted)
    }

    const fn is_active(&self) -> bool {
        matches!(self, Self::Intro | Self::Active)
    }
}

fn wait_for_live_split(port: u16) -> LiveSplit {
    log::info!("Waiting for LiveSplit server...");
    loop {
        if let Ok(live_split) = LiveSplit::create(port) {
            return live_split;
        }

        thread::sleep(CONNECTION_RETRY_DURATION);
    }
}

fn wait_for_emulator(path: Option<&Path>) -> Result<GameMemory> {
    Ok(match path {
        Some(path) => GameMemory::from_shmem(path, None)?,
        None => {
            log::info!("Waiting for emulator...");
            loop {
                if let Some(game_mem) = GameMemory::discover()? {
                    break game_mem;
                }

                thread::sleep(EMULATOR_RETRY_DURATION);
            }
        }
    })
}

fn wait_for_version(mut game_memory: GameMemory) -> Result<Game> {
    log::info!("Waiting for game to be loaded...");
    loop {
        // make sure we don't lose the emulator while we're waiting for the game
        if !game_memory.check_pulse() {
            log::warn!("Lost emulator");
            game_memory = wait_for_emulator(None)?;
            log::info!("Waiting for game to be loaded...");
        }

        if let Some(version) = GameVersion::detect(&game_memory) {
            return Ok(Game::new(version, game_memory));
        }

        thread::sleep(EMULATOR_RETRY_DURATION);
    }
}

#[derive(Debug)]
pub struct AutoSplitter {
    connection_state: ConnectionState,
    update_frequency: Duration,
    live_split: LiveSplit,
    game: Game,
    run_state: RunState,
    last_room: (u16, u16),
    live_split_keep_alive: KeepAliveCounter,
    emulator_keep_alive: KeepAliveCounter,
    game_keep_alive: KeepAliveCounter,
    requested_split_type: Option<SplitType>,
    effective_split_type: Option<SplitType>,
    last_reported_split_type: Option<SplitType>,
    splits: Option<&'static [Event]>,
}

impl AutoSplitter {
    pub fn create(shared_memory_path: Option<&Path>, update_frequency: Duration, live_split_port: u16, requested_split_type: Option<SplitType>) -> Result<Self> {
        let live_split = wait_for_live_split(live_split_port);
        let game_memory = wait_for_emulator(shared_memory_path)?;
        let game = wait_for_version(game_memory)?;

        log::info!("Autosplitter is ready to go");

        Ok(Self {
            connection_state: ConnectionState::Connected,
            update_frequency,
            live_split,
            game,
            run_state: RunState::NotStarted,
            last_room: (0, 0),
            // need to trigger LiveSplit sync on first update so split type is set
            live_split_keep_alive: KeepAliveCounter::new(LIVE_SPLIT_KEEP_ALIVE).with_trigger_on_start(),
            emulator_keep_alive: KeepAliveCounter::new(EMULATOR_KEEP_ALIVE),
            game_keep_alive: KeepAliveCounter::new(GAME_KEEP_ALIVE),
            requested_split_type,
            effective_split_type: None,
            last_reported_split_type: None,
            splits: None,
        })
    }

    fn current_room(&self) -> (u16, u16) {
        (self.game.map_id(), self.game.room_id())
    }

    pub fn split(&mut self) -> Result<()> {
        if self.run_state == RunState::Finished {
            return Ok(());
        }

        if self.run_state == RunState::NotStarted {
            self.run_state = RunState::Intro;
            self.last_room = (0, 0);
        }
        self.live_split.split()
    }

    pub fn reset(&mut self) -> Result<()> {
        if self.run_state.is_started() {
            self.live_split.reset()?;
            self.run_state = RunState::NotStarted;
        }

        Ok(())
    }

    fn conn_fail(&mut self, new_state: ConnectionState) -> Result<()> {
        self.connection_state = new_state;

        match self.connection_state {
            ConnectionState::EmulatorPending => log::warn!("Lost emulator; resetting and waiting for emulator..."),
            ConnectionState::GamePending => log::warn!("Game unloaded; resetting and waiting for a recognized game to be loaded..."),
            _ => (),
        }

        // don't try to reset if we've lost the LiveSplit connection because it will just immediately
        // fail
        if self.live_split.is_connected() {
            self.reset()
        } else {
            Ok(())
        }
    }

    fn delay(&self) {
        let delay = match self.connection_state {
            ConnectionState::EmulatorPending | ConnectionState::GamePending => EMULATOR_RETRY_DURATION,
            ConnectionState::LiveSplitPending => CONNECTION_RETRY_DURATION,
            _ => self.update_frequency,
        };
        thread::sleep(delay);
    }

    fn set_split_type(&mut self, split_type: SplitType) {
        self.effective_split_type = Some(split_type);
        self.splits = split_type.splits();
    }
    
    fn get_live_split_split_type(&mut self) -> Result<Option<SplitType>> {
        let Some(str_split_type) = self.live_split.get_custom_variable_value(SPLIT_TYPE_VARIABLE_NAME)? else {
            return Ok(None);
        };
        
        let Ok(split_type) = SplitType::try_from(str_split_type.as_str()) else {
            log::warn!("LiveSplit reported unrecognized split type {str_split_type}; ignoring");
            return Ok(None);
        };
        
        Ok(Some(split_type))
    }
    
    fn sync_split_type(&mut self) -> Result<()> {
        let live_split_split_type = self.get_live_split_split_type()?;

        match (self.requested_split_type, self.effective_split_type, live_split_split_type) {
            (None, None, None) => {
                log::warn!("No split type was specified by either the user or the splits; defaulting to all-doors");
                self.set_split_type(SplitType::AllDoors);
            }
            (None, None, Some(split_type)) => {
                log::info!("Split type {} detected from LiveSplit splits", split_type.as_str());
                self.set_split_type(split_type);
            }
            (None, Some(old_split_type), Some(new_split_type)) => {
                if old_split_type != new_split_type {
                    log::info!("LiveSplit splits were changed; new split type is {}. Resetting", new_split_type.as_str());
                    self.set_split_type(new_split_type);
                    self.reset()?;
                }
            }
            (_, Some(old_split_type), None) => {
                if self.last_reported_split_type.is_some() {
                    log::warn!(
                        "LiveSplit splits were changed but the new split type could not be detected. Continuing to use old split type {}",
                        old_split_type.as_str(),
                    );
                }
            }
            (Some(requested_split_type), None, None) => self.set_split_type(requested_split_type),
            (Some(requested_split_type), None, Some(reported_split_type)) => {
                if requested_split_type != reported_split_type {
                    log::warn!(
                        "User requested split type {} but LiveSplit reported split type {}. Going with user choice {}",
                        requested_split_type.as_str(),
                        reported_split_type.as_str(),
                        requested_split_type.as_str(),
                    );
                }
                self.set_split_type(requested_split_type);
            }
            (Some(requested_split_type), Some(_), Some(reported_split_type)) => {
                if self.last_reported_split_type != Some(reported_split_type) && requested_split_type != reported_split_type {
                    log::warn!(
                        "LiveSplit splits were changed and the new LiveSplit-reported split type {} does not match the user-requested split type {}. Continuing to use user-requested split type {}",
                        reported_split_type.as_str(),
                        requested_split_type.as_str(),
                        requested_split_type.as_str(),
                    );
                }
            }
        }
        
        self.last_reported_split_type = live_split_split_type;

        Ok(())
    }

    fn sync_with_live_split(&mut self) -> Result<()> {
        self.run_state = match self.live_split.get_timer_phase()? {
            TimerPhase::NotRunning => RunState::NotStarted,
            TimerPhase::Ended => RunState::Finished,
            _ => if self.run_state != RunState::Active && self.live_split.get_split_index()? == 0 {
                RunState::Intro
            } else {
                RunState::Active
            },
        };

        self.sync_split_type()?;

        Ok(())
    }

    pub fn update(&mut self) -> Result<()> {
        loop {
            match self.connection_state {
                ConnectionState::LiveSplitPending => self.wait_for_live_split(),
                ConnectionState::EmulatorPending => self.wait_for_emulator()?,
                ConnectionState::GamePending => self.wait_for_game()?,
                ConnectionState::Connected => self.update_splits()?,
            }

            self.delay();
        }
    }

    fn wait_for_live_split(&mut self) {
        if self.live_split.try_reconnect().is_ok() && self.live_split.is_connected() {
            // now that we're reconnected, sync up
            if let Err(e) = self.sync_with_live_split() {
                if !self.live_split.is_connected() {
                    // the connection failed again. do not advance.
                    return;
                }

                // if we got an error back but the connection is still live, that should mean the
                // server sent us bogus data. that's weird, but we'll just ignore it for now.
                log::warn!("Failed to sync with LiveSplit: {e}. Attempting to continue anyway.");
            }

            self.live_split_keep_alive.reset();
            self.connection_state.advance();
        }
    }

    fn wait_for_emulator(&mut self) -> Result<()> {
        if self.game.search_for_emulator()? {
            self.emulator_keep_alive.reset();
            self.connection_state.advance();
        }

        Ok(())
    }

    fn wait_for_game(&mut self) -> Result<()> {
        // make sure we don't lose the emulator while we're waiting for the game
        if !self.game.check_emulator() {
            return self.conn_fail(ConnectionState::EmulatorPending);
        }

        if self.game.search_for_game().is_valid() {
            self.game_keep_alive.reset();
            self.connection_state.advance();
        }

        Ok(())
    }

    fn check_split_event(&mut self) -> Result<bool> {
        let split_index = self.live_split.get_split_index()?;
        if split_index < 0 {
            return Ok(false);
        }

        let Some(event) = self.splits.and_then(|s| s.get(split_index as usize)) else {
            return Ok(false);
        };

        Ok(match event {
            Event::Room(map, room) => (*map as u16, *room) == self.current_room(),
            Event::Room2((map1, room1), (map2, room2)) => {
                let current_room = self.current_room();
                (*map1 as u16, *room1) == current_room || (*map2 as u16, *room2) == current_room
            }
            Event::Flag(stage, flag) => self.game.flag(*stage, *flag),
            Event::Item(item) => self.game.has_item(*item),
        })
    }

    fn update_splits(&mut self) -> Result<()> {
        if self.live_split_keep_alive.should_check() {
            // make sure the LiveSplit connection is still good and our run state is in sync with theirs
            if self.sync_with_live_split().is_err() && !self.live_split.is_connected() {
                // we lost the LiveSplit connection
                return self.conn_fail(ConnectionState::LiveSplitPending);
            }
        }

        if self.emulator_keep_alive.should_check() {
            // make sure the emulator is still running
            if !self.game.check_emulator() {
                return self.conn_fail(ConnectionState::EmulatorPending);
            }
        }

        if self.game_keep_alive.should_check() {
            // make sure the user hasn't changed the game out from under us
            match self.game.check_version() {
                GameCheck::Same => {}
                GameCheck::Changed => {
                    // if we just changed to a different game, any run we had in progress is no longer
                    // meaningful, so reset
                    log::info!("Game changed to {}; resetting", self.game.version_name());
                    return self.reset();
                }
                GameCheck::Unknown => {
                    // we lost the game - reset and go back to a waiting state
                    return self.conn_fail(ConnectionState::GamePending);
                }
            }
        }

        if self.run_state.is_active() && self.game.is_at_main_menu() {
            // we died or reset; the run is over
            log::info!("Reset");
            return self.reset();
        } else if !self.run_state.is_active() && self.game.is_new_game_start() {
            // a new run has been started
            if self.run_state == RunState::Finished {
                self.reset()?;
            }
            log::info!("Run starting");
            return self.split();
        } else if self.run_state == RunState::Intro {
            // I don't want to rely on the map and room IDs being set to sensible values before the
            // first room is actually loaded. so, immediately after new game start, we won't track
            // any room changes until we see the player reach the second room of the game, which
            // should indicate that they're now progressing normally.
            return if self.current_room() == SECOND_ROOM {
                log::debug!("Player reached second room");
                self.run_state = RunState::Active;
                self.last_room = SECOND_ROOM;
                // if we're splitting on all doors, split now
                if self.splits.is_none() {
                    self.split()
                } else {
                    Ok(())
                }
            } else {
                Ok(())
            };
        } else if self.run_state != RunState::Active {
            // if the run is inactive and nothing is going on with the menu, there's nothing for us to do
            return Ok(());
        }

        // the run is active, so check for player progression
        let current_room = self.current_room();
        if self.last_room == FINAL_BOSS_ROOM {
            // if we're in the final boss room, start watching flags to see when the player beats
            // the game. we'll also stop watching for room changes, since there's no way out of
            // here but to win.
            if self.game.has_defeated_final_boss() {
                self.split()?;
                self.run_state = RunState::Finished;
                log::info!("Run completed!");
            }
        } else if self.splits.is_some() {
            if self.check_split_event()? {
                self.split()?;
            }
        } else if self.last_room != current_room {
            // player changed rooms; split
            log::debug!("Room change: map = {}, room = {}", self.last_room.0, self.last_room.1);
            self.split()?;
        }

        self.last_room = current_room;

        Ok(())
    }
}