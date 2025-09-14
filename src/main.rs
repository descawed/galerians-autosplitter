use std::time::Duration;

use anyhow::{anyhow, Result};
use clap::{Parser, ValueEnum};

mod autosplitter;
use autosplitter::AutoSplitter;
mod game;
mod image;
mod lss;
mod platform;
mod splits;
use splits::{Event, CONSOLE_DOOR_SPLITS, DOOR_SPLITS, KEY_EVENT_SPLITS};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum SplitType {
    /// Split on all doors
    AllDoors,
    /// Split on doors, but only when the door is the expected next door in the route
    RouteDoors,
    /// Split on specific key events - key item pickups, bosses, and hotel progression events
    KeyEvents,
    /// For console: split on all doors
    AllDoorsConsole,
    /// For console: split on doors, but only when the door is the expected next door in the route
    RouteDoorsConsole,
}

impl SplitType {
    const fn splits(&self) -> Option<&'static [Event]> {
        match self {
            Self::AllDoors | Self::AllDoorsConsole => None,
            Self::RouteDoors => Some(&DOOR_SPLITS),
            Self::KeyEvents => Some(&KEY_EVENT_SPLITS),
            Self::RouteDoorsConsole => Some(&CONSOLE_DOOR_SPLITS),
        }
    }

    const fn as_str(&self) -> &'static str {
        match self {
            Self::AllDoors => "all-doors",
            Self::RouteDoors => "route-doors",
            Self::KeyEvents => "key-events",
            Self::AllDoorsConsole => "all-doors-console",
            Self::RouteDoorsConsole => "route-doors-console",
        }
    }

    const fn is_console(&self) -> bool {
        matches!(self, Self::AllDoorsConsole | Self::RouteDoorsConsole)
    }
}

impl TryFrom<&str> for SplitType {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> std::result::Result<Self, Self::Error> {
        match value {
            "AllDoors" | "all-doors" => Ok(Self::AllDoors),
            "RouteDoors" | "route-doors" => Ok(Self::RouteDoors),
            "KeyEvents" | "key-events" => Ok(Self::KeyEvents),
            "AllDoorsConsole" | "all-doors-console" => Ok(Self::AllDoorsConsole),
            "RouteDoorsConsole" | "route-doors-console" => Ok(Self::RouteDoorsConsole),
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
    /// How often to update the state of the game in milliseconds
    #[arg(short, long, default_value_t = 15)]
    update_frequency: u64,
    /// When doing console runs, the index of the video capture device to use
    #[arg(short, long, default_value_t = 0)]
    capture_device: i32,
    /// When doing console runs, force capture calibration even if the specified video capture
    /// device has already been calibrated
    #[arg(short, long, default_value_t = false)]
    force_calibrate: bool,
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
        update_duration,
        args.live_split_port,
        args.capture_device,
        args.force_calibrate,
        args.split_type,
    )?;
    splitter.update()
}
