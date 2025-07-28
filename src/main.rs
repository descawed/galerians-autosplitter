use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use anyhow::Result;
use clap::Parser;

mod game;
use game::{Game, GameVersion};
mod shmem;
use shmem::GameMemory;
mod split;
use split::AutoSplitter;

const EMULATOR_RETRY_DURATION: Duration = Duration::from_millis(5000);

#[derive(Parser, Debug)]
struct Cli {
    /// Port that the LiveSplit server is running on
    #[arg(short, long, default_value_t = 16834)]
    live_split_port: u16,
    /// Path to the emulator's shared memory file. If not provided, it will be searched for automatically.
    #[arg(short, long)]
    shared_memory_path: Option<PathBuf>,
    /// How often to update the state of the game in milliseconds
    #[arg(short, long, default_value_t = 15)]
    update_frequency: u64,
}

fn main() -> Result<()> {
    colog::init();

    let args = Cli::parse();

    // locate emulator memory
    let mut warned_about_shmem = false;
    let game_mem = match args.shared_memory_path {
        Some(path) => GameMemory::from_shmem(&path)?,
        None => {
            loop {
                if let Some(game_mem) = GameMemory::discover()? {
                    break game_mem;
                }

                if !warned_about_shmem {
                    warned_about_shmem = true;
                    log::error!("Failed to locate emulator memory. Retrying...");
                }
                thread::sleep(EMULATOR_RETRY_DURATION);
            }
        }
    };

    // detect loaded game version
    // TODO: re-check this periodically in case the player loads another game without closing the program
    let game = loop {
        if let Some(version) = GameVersion::detect(&game_mem) {
            break Game::new(version, game_mem);
        }

        thread::sleep(EMULATOR_RETRY_DURATION);
    };

    // create autosplitter
    let mut splitter = AutoSplitter::create(game, ("localhost", args.live_split_port))?;
    let update_duration = Duration::from_millis(args.update_frequency);

    loop {
        splitter.update()?;
        thread::sleep(update_duration);
    }
}
