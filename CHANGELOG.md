# Changelog

All notable changes are documented in this file. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/).

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

[0.3.0]: https://github.com/SirSnolte/tuxhub-autoclicker/releases/tag/v0.3.0
