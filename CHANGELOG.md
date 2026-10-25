# Changelog

All notable changes are documented in this file. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/).

## [0.5.0] - 2026-10-25

Requires tuxhub 0.7.0 or newer.

### Changed

- tuxhub talks to the service over its live interface, the socket
  `$XDG_RUNTIME_DIR/tuxhub/autoclicker.sock`, and starts the service when
  needed. The window and the command line use the same socket. tuxhub's
  connection counts as a client: when the last client goes, clicking stops
  at once and the service exits after two seconds, as before.
- The service offers its basic settings (interval, random variation, button,
  mode, number of clicks, start delay, recording of mouse movements) and the
  actions `toggle`, `panic` and `setup` to every live client; changes reach
  all of them at once. Macros and the stop hotkey stay in the tool's own
  interface.
- A file that is not a socket in place of the socket is never removed; the
  service refuses to start instead. A line over 1 MiB, or one that is not a
  JSON object, closes only that connection.

### Removed

- tuxhub mode (`--tuxhub`) and the `configure` request, used by tuxhub 0.5
  and older.

## [0.4.0] - 2026-10-24

### Added

- Own window: `tuxhub-autoclicker` without arguments opens the same interface
  as in tuxhub, and tuxhub can open it from its menu.
- `install` and `uninstall` for an installation without tuxhub, with a
  launcher in the application menu and an icon. tuxhub detects such an
  installation.

### Changed

- Settings, devices and macros belong to a background service shared by the
  window and tuxhub. It stops everything and exits when the last of them
  closes. The settings move from tuxhub to
  `~/.config/tuxhub-autoclicker/config.json`; settings saved in tuxhub are
  imported once.

### Fixed

- The taskbar matches the window to its launcher and icon: the window reports
  its app id to the compositor.
- The page inside tuxhub no longer shows a stray pixel at the top left.

## [0.3.1] - 2026-10-24

### Changed

- The anti-cheat warning and the recording privacy notice can be closed and
  stay hidden afterwards.

## [0.3.0] - 2026-10-23

First public release.

### Added

- Autoclicker with single clicks, double clicks and holding, interval,
  random variation, click limit and start delay.
- Keyboard and mouse macros: recording, playback with repetitions and speed,
  and an editor for steps and delays.
- Integration with tuxhub: localized interface, settings, global hotkeys for
  clicking, recording and stopping, an optional stop hotkey and one hotkey
  per macro.
- Command line interface for clicking, recording and playback.

### Changed

- Interface rebuilt with Web Awesome and the tuxhub theme, so it matches
  tuxhub. Settings appear as rows with a hint below each label.
- Deleting a macro, scaling delays and error messages use the interface's own
  dialog and notice instead of browser dialogs.
- Follows the light or dark appearance reported by tuxhub.

[0.4.0]: https://github.com/SirSnolte/tuxhub-autoclicker/releases/tag/v0.4.0
[0.3.1]: https://github.com/SirSnolte/tuxhub-autoclicker/releases/tag/v0.3.1
[0.3.0]: https://github.com/SirSnolte/tuxhub-autoclicker/releases/tag/v0.3.0
