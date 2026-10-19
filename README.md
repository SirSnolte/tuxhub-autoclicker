# tuxhub-autoclicker

Autoclicker für Linux – funktioniert unter **Wayland und X11** und in jedem
Spiel, weil die Klicks über ein virtuelles Eingabegerät (`/dev/uinput`) direkt
vom Kernel kommen.

Eigenständig nutzbar oder bequem im [tuxhub](https://github.com/SirSnolte/tuxhub)
(Einstellungen, Start/Stopp, globaler Hotkey F8).

> In Online-Spielen mit Anti-Cheat kann ein Autoclicker gegen die
> Nutzungsbedingungen verstoßen. Nutzung auf eigenes Risiko.

## Funktionen

- Linke, rechte oder mittlere Maustaste
- Einzelklick, Doppelklick oder Gedrückthalten
- Intervall ab 1 ms, zufällige Abweichung (Jitter)
- Klick-Limit und Startverzögerung

## Kommandozeile

```bash
tuxhub-autoclicker click --interval 100 --button left --delay 3
tuxhub-autoclicker click -i 50 -m double -n 200
tuxhub-autoclicker setup
```

`Strg+C` stoppt sofort. `setup` richtet einmalig eine udev-Regel ein, damit
der angemeldete Nutzer `/dev/uinput` verwenden darf (fragt nach dem Passwort).
Wer Steam installiert hat, hat diesen Zugriff meist schon.

## Bauen

```bash
cargo build --release
cargo test        # die uinput-Tests belegen das virtuelle Gerät exklusiv – es kommen keine echten Klicks an
```

## Lizenz

MIT
