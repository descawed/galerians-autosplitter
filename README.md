# Galerians autosplitter

This is an autosplitter for use with LiveSplit when speedrunning the PSX game Galerians. Currently only supports Linux.
Works with both DuckStation and PCSX-Redux. Supports the North American and Japanese versions of the game.

## Basic Usage

- Build with `cargo build`
- Run the `galerians-autosplitter` executable
- Right-click on LiveSplit and select Control > Start TCP Server (you'll need to do this every time you start LiveSplit)
- If you're using DuckStation, go to Settings > Advanced and enable the "Export Shared Memory" setting (may require a
  restart of DuckStation to take effect)
- The app should automatically detect LiveSplit and the emulator once they become available
- Timer starts on New Game and ends on the last hit in the Dorothy fight in accordance with the SRC category rules
  (well, in theory; I haven't actually tested a full run with it yet). During the run, it's currently configured to
  split on every door. If you return to the main menu in the middle of a run (either due to dying or loading a save
  state), the run will reset.

## How It Works

This autosplitter is a standalone application rather than an ASL script or LiveSplit plugin. The main reason I did it
this way is that I needed something that worked on Linux and I kept running into obstacles trying to accomplish this
with a "normal" autosplitter. LiveSplit runs fine under Wine, but getting an ASL script inside the Wine LiveSplit to
then monitor the native Linux emulator process would be a challenge, not to mention that writing ASL scripts that work
with emulators is just generally a pain. LiveSplit One has native Linux support, and its auto splitting runtime (asr)
actually has built-in support for most major emulators that makes writing autosplitters for them very easy, but that
functionality is unfortunately Windows-only at the moment.

The app takes advantage of a feature that both DuckStation and PCSX-Redux have (although it has to be turned on in
DuckStation) where they put the emulated RAM in shared memory that other processes can access if they choose to. This
makes reading the game memory super simple and avoids having to know anything about the emulator internals or worry
about version differences moving things around. I don't know yet if there are any other PSX emulators that have this
feature, but if there are, adding support for them should be very straightforward. The app can auto-detect when a
supported emulator has made the shared memory available by scanning for matching files in /dev/shm, but you can also
explicitly specify the shared memory file with the `-s`/`--shared-memory-path` option if you need to.

The autosplitter uses LiveSplit's server feature to communicate with LiveSplit and tell it when to start, split, or
reset. The server has to be started every time you start LiveSplit as described above. It defaults to connecting on
LiveSplit's default port, 16834, but you can use the `-l`/`--live-split-port` option to specify a different port if you
need to. By default, the autosplitter will check the game state and update LiveSplit every 15ms, but you can control
this duration with the `-u`/`--update-frequency` option.

## Known Issues

- The app will break if you close or restart LiveSplit or the emulator after it connects to them. You can just restart
  the app to fix it, although if you were in the middle of a run, it won't properly recognize that after it restarts.
- If you manually reset a run, the app can get confused and get out of sync with what's happening in LiveSplit.
- If you load a different game in the emulator after the app has connected to it, it won't notice and will keep trying
  to autosplit for Galerians. Make sure to stop the autosplitter before switching to a different game.