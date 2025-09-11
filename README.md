# Galerians autosplitter

An autosplitter for use with LiveSplit when speedrunning the PSX game Galerians. Supports both DuckStation and 
PCSX-Redux on Windows and Linux. Works with the North American and Japanese versions of the game.

## Basic Usage

- Download the latest release from the [Releases page](https://github.com/descawed/galerians-autosplitter/releases)
- Run the `galerians-autosplitter` executable
- Open one of the included splits in LiveSplit
- Right-click on LiveSplit and select Control > Start TCP Server (you'll need to do this every time you start LiveSplit)
- If you're using DuckStation, go to Settings > Advanced and enable the "Export Shared Memory" setting (only needs to be
  done once; may require a restart of DuckStation to take effect)
- The app will automatically detect LiveSplit and the emulator once they're running and you've enabled the options
  above. It will show a message when this happens so you know it's working.
- Timer starts on New Game and ends on the last hit in the Dorothy fight in accordance with the SRC category rules.
  If at any point during a run you return to the main menu, the run will reset. The timer will *not* reset automatically
  after you complete a run; you'll need to reset manually if you want to go again.

## How It Works

This autosplitter is a standalone application rather than an ASL script or LiveSplit plugin. The main reason I did it
this way is that I needed something that worked on Linux, and I kept running into obstacles trying to achieve this
with a "normal" autosplitter. LiveSplit runs fine under Wine, but getting an ASL script inside the Wine LiveSplit to
monitor a native Linux emulator process would be a challenge (not to mention that writing ASL scripts for emulators is
just generally a pain). LiveSplit One has native Linux support, and its auto splitting runtime (asr) actually has
built-in support for most major emulators, but that functionality is unfortunately Windows-only at the moment.

The app takes advantage of a feature supported by both DuckStation and PCSX-Redux that puts the emulated RAM in shared
memory that other processes can access. This makes reading the game memory super simple and avoids having to know
anything about the emulator internals or worry about version differences moving things around. I don't know yet if there
are any other PSX emulators that have this feature, but if there are, adding support for them should be very
straightforward.

## Advanced Usage

Although it's not normally necessary, there are a few options you can use to customize the autosplitter's behavior.
The autosplitter is a command-line application, meaning you need to run it from a command prompt if you want to set any
of these options. The options are described below. You can also use the command `galerians-autosplitter --help` to see
the list of available options.

The autosplitter uses LiveSplit's server feature to communicate with LiveSplit and tell it when to start, split, or
reset. The server has to be started every time you start LiveSplit as described above. It defaults to connecting on
LiveSplit's default port, 16834, but you can use the `-l`/`--live-split-port` option to specify a different port if you
need to. By default, the autosplitter will check the game state and update LiveSplit every 15 ms, but you can control
this duration with the `-u`/`--update-frequency` option.

The autosplitter supports three splitting strategies:

- `all-doors` - splits on every door
- `route-doors` - splits on doors, but only if the door is the next door expected in the proper route. This helps avoid
  spurious splits if you have to take a detour or if you mistakenly go the wrong way.
- `key-events` - splits on a series of key progression events that I've selected. This includes most key item pickups in
  stages A and B, progression events in stage C, and boss fights. This split type has significantly fewer splits than 
  `route-doors`, with 46 compared to its 177. Like `route-doors`, a split is only triggered if the event is the next
  event expected in the proper route.

The included split files contain a custom variable indicating which split type they're intended for, so if you're using
those splits, it's generally not necessary to specify the split type explicitly; it will be detected after connecting to
LiveSplit with an appropriate split file loaded. If for some reason you do need to explicitly specify the split type,
this can be done with the `-p`/`--split-type` option. If the splits you're using don't contain the custom variable
indicating which split type to use, and you don't specify a split type with this option, the autosplitter will print a
warning and default to `all-doors`.

## Known Issues

- When you do stuff manually in LiveSplit (e.g., manually resetting), it can take the autosplitter a few seconds to
  notice, so if you're rapidly toggling things in LiveSplit and taking actions in-game, it's possible you could get
  some weird behavior.