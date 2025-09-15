# Galerians autosplitter

An autosplitter for use with LiveSplit when speedrunning the PSX game Galerians. Supports both DuckStation and 
PCSX-Redux on Windows and Linux. Works with the North American and Japanese versions of the game. Also supports console;
see the [Console](#Console) section below.

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

## Console

The autosplitter can also be used when playing the game on console. This works by watching the video capture and using
the pre-rendered backgrounds to determine which room you're in. This feature should currently be considered somewhat
experimental. I've tested it successfully over the course of a full (but segmented) run on my own capture setup, but I
can't guarantee it will work with all capture setups. I've also only tested it on Linux, although it should in theory
work on Windows.

The first time you use this feature with a new capture device, you'll have to go through a calibration process. This
involves starting a new game, getting to the first screen where you have control of Rion, and then letting the
autosplitter determine the best way to crop and scale the capture to get the best match to the background. You'll be
prompted to perform this process automatically. After this has been done once, the calibration settings will be saved
so you don't need to do it again next time. If you ever do need to recalibrate, you can do so with the
`--force-calibrate` option (see [Console Options](#Console-Options) below for more details).

The biggest hurdle to using this feature is that you're probably going to want to be recording your capture at the same
time, but in general, only one application can be using the capture device at a time. It's possible to work around this
by using a third-party tool to mirror the capture device. For Linux, I've included a `share_capture.sh` script (in the
`scripts` directory of the repo) that can be used to do this. It uses `v4l2loopback` and `ffmpeg` (so you'll need to
have those installed) to mirror the capture to two virtual devices, one for the autosplitter and one for the recording
software. The script takes three arguments: the index of the input capture device, the index of the output capture
device for the recording software, and the index of the output capture device for the autosplitter. For example, with
my capture card at `/dev/video0`, I use the script like this:

`sudo ./share_capture.sh 0 10 11` (it does need to be run as root in order to control `v4l2loopback`)

This creates `/dev/video10` (named `Galerians OBS share`) which I record in OBS, and `/dev/video11` (named
`Galerians autosplitter share`) which I use with the autosplitter via `galerians-autosplitter -c 11`. Note that you need
to leave this script running as long as you're using these virtual devices. Also note that the mirroring process can
introduce some latency in the capture, although it's generally very small. The script supports some additional
parameters if you need to configure any video settings; run it with no arguments to see the list of options.

I have no idea what would be involved in setting up a similar pipeline on Windows. An alternative low-tech solution
would be to set up a camera to record your screen IRL and then allow the autosplitter exclusive use of the capture card.

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
- `route-doors-console` - like `route-doors` but when running on console. Compared to the emulator version, some splits
  corresponding to FMVs in stage C have been removed since we don't have a reliable way to detect them.

The included split files contain a custom variable indicating which split type they're intended for, so if you're using
those splits, it's generally not necessary to specify the split type explicitly; it will be detected after connecting to
LiveSplit with an appropriate split file loaded. If for some reason you do need to explicitly specify the split type,
this can be done with the `-p`/`--split-type` option. If the splits you're using don't contain the custom variable
indicating which split type to use, and you don't specify a split type with this option, the autosplitter will print a
warning and default to `all-doors`.

### Console Options

The following options only apply to console runs:

By default, the autosplitter uses the first video capture device it finds. If you want to use a different device, you
can use the `-c`/`--capture-device` option. This option takes a number indicating the index of the capture device to
use. On Linux, the index corresponds to `/dev/videoN`, where `N` is the device index. You can use the
`v4l2-ctl --list-devices` command to find the index of the device you want to use. I'm not sure how to determine the
index of a particular device on Windows.

If you need to recalibrate the autosplitter's capture settings, you can use the `-f`/`--force-calibrate` option.
Calibration settings are recorded by device index, and which device is at a particular index can change depending on
which port you plug it into or in which order you connect devices, so it may be necessary to use this option if the
autosplitter is trying to apply saved settings to the wrong device.

## Known Issues

- When you do stuff manually in LiveSplit (e.g., manually resetting), it can take the autosplitter a few seconds to
  notice, so if you're rapidly toggling things in LiveSplit and taking actions in-game, it's possible you could get
  some weird behavior.
- The console autosplitter detects the start of a new game by watching for a fade to black on the main menu, and it
  detects the end of a run by watching for a fade to black during the Dorothy fight. This can lead to false positives.
  For example, if you die during the Dorothy fight, the autosplitter will think you've completed the run. Likewise,
  selecting any option on the main menu, or waiting long enough for the trailer to start playing, will all be
  interpreted as the start of a run. The run will be automatically reset upon returning to the main menu.