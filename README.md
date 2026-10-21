# tuxly-autoclicker

Autoclicker and keyboard and mouse macros for Linux. Clicks and macro playback
use a virtual input device (`/dev/uinput`), so they work on Wayland and X11
and in every application.

The tool works on its own from the command line and integrates with
[tuxly](https://github.com/SirSnolte/tuxly), which provides a graphical
interface, settings and global hotkeys.

> Automated input can violate the terms of service of online games, in
> particular games with anti-cheat software. Use it at your own risk.

## Features

- Left, right or middle mouse button; single click, double click or hold
- Interval from 1 ms, random variation, click limit and start delay
- Macros: record keyboard and mouse input, play it back with repetitions and
  adjustable speed, and edit steps and delays
- Global hotkeys in tuxly: F8 starts or stops clicking, F9 starts or stops a
  recording, F10 stops everything. Each macro and the clicker's stop action
  can have their own hotkey.
- Stopping always releases all pressed keys and buttons.

## Installation

Install it from the workshop in tuxly, or download the archive from the
[latest release](https://github.com/SirSnolte/tuxly-autoclicker/releases/latest)
and extract it.

### Permissions

Creating virtual input devices requires write access to `/dev/uinput`. Many
systems grant it already, for example when Steam is installed. Otherwise run:

```bash
tuxly-autoclicker setup
```

This installs a udev rule that grants access to the logged-in user and asks
for your password once.

Recording macros requires read access to `/dev/input`, which members of the
`input` group have:

```bash
sudo usermod -aG input "$USER"   # log out and in again afterwards
```

Input devices are read only while a recording is active. Every key is stored
during a recording, so do not type passwords while recording.

## Command line

```bash
tuxly-autoclicker click --interval 100 --button left --delay 3
tuxly-autoclicker click -i 50 -m double -n 200
tuxly-autoclicker record "Open inventory"        # Ctrl+C ends the recording
tuxly-autoclicker play "Open inventory" -r 10 -s 1.5
tuxly-autoclicker macros
```

Ctrl+C stops clicking and playback immediately. Macros are stored as JSON in
`~/.local/share/tuxly-autoclicker/macros/`.

## Building

```bash
cargo build --release
cargo test -- --test-threads=1
```

The device tests create virtual input devices. Playback tests grab their
device exclusively, so no input reaches the desktop. The recording test emits
only F24 on a dedicated test device. Tests are skipped when the required
permissions are missing.

## License

MIT
