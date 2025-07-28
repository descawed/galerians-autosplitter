use std::io::Write;
use std::net::{TcpStream, ToSocketAddrs};
use std::thread;
use std::time::Duration;

use anyhow::{bail, Result};

use crate::game::Game;

const CONNECTION_RETRY_DURATION: Duration = Duration::from_millis(1000);

const SECOND_ROOM: (u16, u16) = (0, 1);
const FINAL_BOSS_ROOM: (u16, u16) = (8, 7);

const CMD_SPLIT: &[u8] = b"startorsplit\r\n";
const CMD_RESET: &[u8] = b"reset\r\n";

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

#[derive(Debug)]
pub struct AutoSplitter {
    live_split_connection: TcpStream,
    game: Game,
    run_state: RunState,
    last_room: (u16, u16),
}

impl AutoSplitter {
    pub fn create(game: Game, live_split_addr: impl ToSocketAddrs) -> Result<Self> {
        let Some(live_split_addr) = live_split_addr.to_socket_addrs()?.next() else {
            bail!("No LiveSplit address was provided");
        };

        let mut warned_about_connection = false;
        let live_split_connection = loop {
            match TcpStream::connect(live_split_addr) {
                Ok(conn) => break conn,
                Err(e) => {
                    if !warned_about_connection {
                        warned_about_connection = true;
                        log::error!("Failed to connect to LiveSplit: {e}. Retrying...");
                    }

                    thread::sleep(CONNECTION_RETRY_DURATION);
                }
            }
        };

        log::info!("Successfully connected to LiveSplit");

        Ok(Self {
            live_split_connection,
            game,
            run_state: RunState::NotStarted,
            last_room: (0, 0),
        })
    }

    fn live_split(&mut self, command: &[u8]) -> Result<()> {
        self.live_split_connection.write_all(command)?;
        Ok(())
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
        self.live_split(CMD_SPLIT)
    }

    pub fn reset(&mut self) -> Result<()> {
        if self.run_state.is_started() {
            self.live_split(CMD_RESET)?;
            self.run_state = RunState::NotStarted;
        }

        Ok(())
    }

    pub fn update(&mut self) -> Result<()> {
        // TODO: sync run_state from LiveSplit at appropriate points, since the player could reset
        //  without us knowing
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
        let current_room = self.current_room();
        if self.last_room != current_room {
            // player changed rooms; split
            // TODO: only split if the player went the right way
            log::debug!("Room change: map = {}, room = {}", self.last_room.0, self.last_room.1);
            self.split()?;
            self.last_room = current_room;
        } else if current_room == FINAL_BOSS_ROOM {
            // if we're in the final boss room, start watching flags to see when the player beats
            // the game
            if self.game.has_defeated_final_boss() {
                self.run_state = RunState::Finished;
                self.split()?;
                log::info!("Run completed!");
            }
        }

        Ok(())
    }
}