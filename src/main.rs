use std::path::PathBuf;
use std::time::Duration;

use anyhow::{anyhow, Result};
use clap::{Parser, ValueEnum};

mod autosplitter;
use autosplitter::AutoSplitter;
mod game;
mod lss;
mod shmem;
mod splits;
use splits::{Event, KEY_EVENT_SPLITS};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum SplitType {
    /// Split on all doors
    AllDoors,
    /// Split on specific key events - key item pickups, bosses, and hotel progression events
    KeyEvents,
}

impl SplitType {
    const fn splits(&self) -> Option<&'static [Event]> {
        match self {
            Self::AllDoors => None,
            Self::KeyEvents => Some(&KEY_EVENT_SPLITS),
        }
    }

    const fn as_str(&self) -> &'static str {
        match self {
            Self::AllDoors => "all-doors",
            Self::KeyEvents => "key-events",
        }
    }
}

impl TryFrom<&str> for SplitType {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> std::result::Result<Self, Self::Error> {
        match value {
            "AllDoors" | "all-doors" => Ok(Self::AllDoors),
            "KeyEvents" | "key-events" => Ok(Self::KeyEvents),
            _ => Err(anyhow!("Unknown split type: {value}")),
        }
    }
}

#[derive(Parser, Debug)]
#[command(version, about)]
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
    /// Strategy for when to split. If not provided, it will be determined from LiveSplit's split
    /// settings if possible. If the LiveSplit split settings also don't have a valid split type,
    /// defaults to all-doors.
    #[arg(short = 'p', long, value_enum)]
    split_type: Option<SplitType>,
}

fn main() -> Result<()> {
    colog::init();

    let args = Cli::parse();

    // create autosplitter
    let update_duration = Duration::from_millis(args.update_frequency);
    let mut splitter = AutoSplitter::create(
        args.shared_memory_path.as_deref(),
        update_duration,
        args.live_split_port,
        args.split_type,
    )?;
    splitter.update()
}
