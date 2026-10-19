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
- **Makros:** Tastatur und Maus aufnehmen (F9), abspielen mit Wiederholungen
  und Geschwindigkeit, im Editor Schritte und Wartezeiten bearbeiten, eigener
  Hotkey pro Makro
- **F10** stoppt alles sofort; beim Stopp werden gedrückte Tasten losgelassen
- Optionale eigene **Stopp-Taste** für den Klicker (z. B. `CTRL+q` oder `F7`),
  direkt in der Oberfläche festlegbar. Tasten, die man zum Tippen braucht,
  gehen nur mit Strg/Alt/Super – sonst würden sie systemweit abgefangen.

## Kommandozeile

```bash
tuxhub-autoclicker click --interval 100 --button left --delay 3
tuxhub-autoclicker click -i 50 -m double -n 200
tuxhub-autoclicker record "Erz farmen"     # Strg+C beendet die Aufnahme
tuxhub-autoclicker play "Erz farmen" -r 10 -s 1.5
tuxhub-autoclicker macros
tuxhub-autoclicker setup
```

Makros liegen als JSON in `~/.local/share/tuxhub-autoclicker/macros/`.
Zum Aufnehmen braucht dein Nutzer Lesezugriff auf `/dev/input` (Gruppe
`input`). Gelesen wird nur während einer Aufnahme – trotzdem keine Passwörter
eintippen, solange sie läuft.

`Strg+C` stoppt sofort. `setup` richtet einmalig eine udev-Regel ein, damit
der angemeldete Nutzer `/dev/uinput` verwenden darf (fragt nach dem Passwort).
Wer Steam installiert hat, hat diesen Zugriff meist schon.

## Bauen

```bash
cargo build --release
cargo test -- --test-threads=1   # Geräte-Tests: Wiedergabe exklusiv gegrabbt, Aufnahme nur über F24 eines Testgeräts
```

## Lizenz

MIT
