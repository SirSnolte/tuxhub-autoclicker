# tuxhub-autoclicker

Autoclicker and keyboard and mouse macros for Linux. Clicks and macro playback
use a virtual input device (`/dev/uinput`), so they work on Wayland and X11
and in every application.

The tool works on its own, with its own window and from the command line,
and integrates with [tuxhub](https://github.com/SirSnolte/tuxhub), which
shows the same interface and adds global hotkeys.

> Automated input can violate the terms of service of online games, in
> particular games with anti-cheat software. Use it at your own risk.

## Features

- Left, right or middle mouse button; single click, double click or hold
- Interval from 1 ms, random variation, click limit and start delay
- Macros: record keyboard and mouse input, play it back with repetitions and
  adjustable speed, and edit steps and delays
- Global hotkeys in tuxhub: F8 starts or stops clicking, F9 starts or stops a
  recording, F10 stops everything. Each macro and the clicker's stop action
  can have their own hotkey.
- Stopping always releases all pressed keys and buttons.

## Installation

Install it from the workshop in tuxhub, or download the archive from the
[latest release](https://github.com/SirSnolte/tuxhub-autoclicker/releases/latest),
extract it and run:

```bash
./tuxhub-autoclicker install
```

This copies the tool to `~/.local/share/tuxhub-autoclicker/app`, adds it to
the application menu and links the command to `~/.local/bin`. tuxhub detects
this installation and updates it in place. `tuxhub-autoclicker uninstall`
removes it again; `--purge` also removes the settings and the macros.

## Window and background service

`tuxhub-autoclicker` without arguments opens the window. It shows the same
interface as tuxhub. Settings, the virtual input devices and the macros
belong to a small background service, which the window and tuxhub start on
demand and share, so both always show the same state. The settings are stored
in `~/.config/tuxhub-autoclicker/config.json`.

When the last window or tuxhub closes, the service stops clicking, playback
and recording at once and exits. Global hotkeys are registered by tuxhub, so
they work while tuxhub is running.

### Permissions

Creating virtual input devices requires write access to `/dev/uinput`. Many
systems grant it already, for example when Steam is installed. Otherwise run:

```bash
tuxhub-autoclicker setup
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
tuxhub-autoclicker click --interval 100 --button left --delay 3
tuxhub-autoclicker click -i 50 -m double -n 200
tuxhub-autoclicker record "Open inventory"        # Ctrl+C ends the recording
tuxhub-autoclicker play "Open inventory" -r 10 -s 1.5
tuxhub-autoclicker macros
```

These commands run on their own, without the service. Ctrl+C stops clicking
and playback immediately. Macros are stored as JSON in
`~/.local/share/tuxhub-autoclicker/macros/`.

## Building

```bash
cargo build --release
cargo test -- --test-threads=1
```

Building the window needs the WebKitGTK 4.1 and GTK 3 development packages
(`webkit2gtk4.1-devel` on Fedora, `libwebkit2gtk-4.1-dev` on Debian and
Ubuntu).

The device tests create virtual input devices. Playback tests grab their
device exclusively, so no input reaches the desktop. The recording test emits
only F24 on a dedicated test device. Tests are skipped when the required
permissions are missing. The service tests run the service in their own XDG
directories and only let the clicker count down, never click.

## License

MIT
