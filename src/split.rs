use std::path::Path;
use std::thread;
use std::time::Duration;

use anyhow::Result;

use crate::game::{Game, GameCheck, GameVersion};
use crate::lss::{LiveSplit, TimerPhase};
use crate::shmem::GameMemory;

const CONNECTION_RETRY_DURATION: Duration = Duration::from_millis(1000);
const EMULATOR_RETRY_DURATION: Duration = Duration::from_millis(5000);

const SECOND_ROOM: (u16, u16) = (0, 1);
const FINAL_BOSS_ROOM: (u16, u16) = (8, 7);

const LIVE_SPLIT_KEEP_ALIVE: i32 = 334; // ~5 seconds at the default update frequency
const EMULATOR_KEEP_ALIVE: i32 = 334;
const GAME_KEEP_ALIVE: i32 = 200; // ~3 seconds at the default update frequency

#[derive(Debug, Clone)]
struct KeepAliveCounter {
    period: i32,
    remaining: i32,
}

impl KeepAliveCounter {
    const fn new(period: i32) -> Self {
        Self { period, remaining: period }
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

    fn get_delay(&self, update_frequency: Duration) -> Duration {
        match self {
            Self::EmulatorPending | Self::GamePending => EMULATOR_RETRY_DURATION,
            Self::LiveSplitPending => CONNECTION_RETRY_DURATION,
            _ => update_frequency,
        }
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

fn wait_for_version(game_memory: GameMemory) -> Game {
    log::info!("Waiting for game to be loaded...");
    loop {
        if let Some(version) = GameVersion::detect(&game_memory) {
            return Game::new(version, game_memory);
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
}

impl AutoSplitter {
    pub fn create(shared_memory_path: Option<&Path>, update_frequency: Duration, live_split_port: u16) -> Result<Self> {
        let live_split = wait_for_live_split(live_split_port);
        let game_memory = wait_for_emulator(shared_memory_path)?;
        let game = wait_for_version(game_memory);

        log::info!("Autosplitter is ready to go");

        Ok(Self {
            connection_state: ConnectionState::Connected,
            update_frequency,
            live_split,
            game,
            run_state: RunState::NotStarted,
            last_room: (0, 0),
            live_split_keep_alive: KeepAliveCounter::new(LIVE_SPLIT_KEEP_ALIVE),
            emulator_keep_alive: KeepAliveCounter::new(EMULATOR_KEEP_ALIVE),
            game_keep_alive: KeepAliveCounter::new(GAME_KEEP_ALIVE),
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
        // don't try to reset if we've lost the LiveSplit connection because it will just immediately
        // fail
        if self.live_split.is_connected() {
            self.reset()
        } else {
            Ok(())
        }
    }

    fn delay(&self) {
        let delay = self.connection_state.get_delay(self.update_frequency);
        thread::sleep(delay);
    }

    fn sync_with_live_split(&mut self) -> Result<()> {
        self.run_state = match self.live_split.get_timer_phase()? {
            TimerPhase::NotRunning => RunState::NotStarted,
            TimerPhase::Ended => RunState::Finished,
            _ => if self.live_split.get_split_index()? == 0 {
                RunState::Intro
            } else {
                RunState::Active
            },
        };

        Ok(())
    }

    pub fn update(&mut self) -> Result<()> {
        loop {
            match self.connection_state {
                ConnectionState::LiveSplitPending => self.wait_for_live_split(),
                ConnectionState::EmulatorPending => self.wait_for_emulator()?,
                ConnectionState::GamePending => self.wait_for_game(),
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

    fn wait_for_game(&mut self) {
        if self.game.search_for_game().is_valid() {
            self.game_keep_alive.reset();
            self.connection_state.advance();
        }
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
                log::warn!("Lost emulator; resetting and waiting for emulator...");
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
                    log::warn!("Game unloaded; resetting and waiting for a recognized game to be loaded...");
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
                self.split()
            } else {
                Ok(())
            };
        } else if self.run_state != RunState::Active {
            // if the run is inactive and nothing is going on with the menu, there's nothing for us to do
            return Ok(());
        }

        // the run is active, so check for player progression
        if self.last_room == FINAL_BOSS_ROOM {
            // if we're in the final boss room, start watching flags to see when the player beats
            // the game. we'll also stop watching for room changes, since there's no way out of
            // here but to win.
            if self.game.has_defeated_final_boss() {
                self.run_state = RunState::Finished;
                self.split()?;
                log::info!("Run completed!");
            }
        } else {
            let current_room = self.current_room();
            if self.last_room != current_room {
                // player changed rooms; split
                // TODO: only split if the player went the right way
                log::debug!("Room change: map = {}, room = {}", self.last_room.0, self.last_room.1);
                self.split()?;
                self.last_room = current_room;
            }
        }

        Ok(())
    }
}