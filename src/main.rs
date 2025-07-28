use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use clap::Parser;

mod game;
mod lss;
mod shmem;
mod split;
use split::AutoSplitter;

#[derive(Parser, Debug)]
#[command(about)]
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

    // create autosplitter
    let update_duration = Duration::from_millis(args.update_frequency);
    let mut splitter = AutoSplitter::create(args.shared_memory_path.as_deref(), update_duration, args.live_split_port)?;
    splitter.update()
}
