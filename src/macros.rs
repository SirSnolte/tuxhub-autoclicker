//! Makros: Format, Speicherung und Umwandlung aufgenommener Eingaben.
//!
//! Ein Makro ist eine Liste von Schritten. Jeder Schritt wartet erst
//! `delay_ms` und führt dann seine Aktion aus.

use evdev::KeyCode;
use serde::{Deserialize, Serialize};
use std::io;
use std::path::PathBuf;
use std::time::SystemTime;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Step {
    #[serde(default)]
    pub delay_ms: u32,
    #[serde(flatten)]
    pub action: Action,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Action {
    /// Taste oder Maustaste drücken (`down = true`) bzw. loslassen
    Key { code: u16, down: bool },
    /// Relative Mausbewegung
    Move { dx: i32, dy: i32 },
    /// Mausrad
    Scroll { dx: i32, dy: i32 },
    /// Nur warten
    Wait,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Macro {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub steps: Vec<Step>,
    /// Wiederholungen, 0 = endlos
    #[serde(default = "one")]
    pub repeat: u32,
    /// Abspielgeschwindigkeit, 1.0 = original
    #[serde(default = "one_f")]
    pub speed: f64,
    /// Globaler Hotkey (XDG-Format, z. B. `CTRL+F1`)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shortcut: Option<String>,
}

fn one() -> u32 {
    1
}
fn one_f() -> f64 {
    1.0
}

impl Macro {
    pub fn duration_ms(&self) -> u64 {
        self.steps.iter().map(|s| s.delay_ms as u64).sum()
    }

    pub fn validate(&mut self) -> Result<(), String> {
        if !valid_id(&self.id) {
            return Err(format!("ungültige Makro-ID {:?}", self.id));
        }
        self.name = self.name.trim().to_string();
        if self.name.is_empty() {
            self.name = self.id.clone();
        }
        if !(0.1..=10.0).contains(&self.speed) {
            return Err("Geschwindigkeit muss zwischen 0,1 und 10 liegen".into());
        }
        for (i, s) in self.steps.iter().enumerate() {
            if let Action::Key { code, .. } = s.action
                && !playable_key(code)
            {
                return Err(format!(
                    "Schritt {}: Taste {code} wird nicht unterstützt",
                    i + 1
                ));
            }
        }
        self.shortcut = self
            .shortcut
            .take()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        Ok(())
    }
}

/// Tasten, die das Wiedergabe-Gerät erzeugen kann: normale Tastatur plus Maustasten.
pub fn playable_key(code: u16) -> bool {
    (1..=248).contains(&code) || (0x110..=0x117).contains(&code)
}

pub fn key_name(code: u16) -> String {
    let name = format!("{:?}", KeyCode::new(code));
    if name.starts_with("KEY_") || name.starts_with("BTN_") {
        name
    } else {
        format!("#{code}")
    }
}

/// Alle abspielbaren Tasten mit Namen – für den Editor.
pub fn all_keys() -> Vec<(u16, String)> {
    (1..=248u16)
        .chain(0x110..=0x117)
        .map(|c| (c, key_name(c)))
        .filter(|(_, n)| !n.starts_with('#'))
        .collect()
}

pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Macht aus einem Namen eine Datei-taugliche ID.
pub fn slug(name: &str) -> String {
    let mut s = String::new();
    for c in name.to_lowercase().chars() {
        let c = match c {
            'ä' => 'a',
            'ö' => 'o',
            'ü' => 'u',
            'ß' => 's',
            c => c,
        };
        if c.is_ascii_alphanumeric() {
            s.push(c);
        } else if !s.ends_with('-') {
            s.push('-');
        }
    }
    let s = s.trim_matches('-').chars().take(40).collect::<String>();
    if s.is_empty() { "makro".into() } else { s }
}

// ------------------------------------------------------------------ Speicher

pub struct Store {
    dir: PathBuf,
}

impl Store {
    /// `$XDG_DATA_HOME/tuxhub-autoclicker/macros`
    pub fn open() -> io::Result<Self> {
        let base = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
            .ok_or_else(|| io::Error::other("HOME nicht gesetzt"))?;
        let dir = base.join("tuxhub-autoclicker/macros");
        std::fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }

    #[cfg(test)]
    pub fn at(dir: PathBuf) -> Self {
        std::fs::create_dir_all(&dir).unwrap();
        Self { dir }
    }

    fn path(&self, id: &str) -> Result<PathBuf, String> {
        if valid_id(id) {
            Ok(self.dir.join(format!("{id}.json")))
        } else {
            Err(format!("ungültige Makro-ID {id:?}"))
        }
    }

    pub fn list(&self) -> Vec<Macro> {
        let mut out: Vec<Macro> = std::fs::read_dir(&self.dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
            .filter_map(|e| serde_json::from_str(&std::fs::read_to_string(e.path()).ok()?).ok())
            .collect();
        out.sort_by_key(|m: &Macro| m.name.to_lowercase());
        out
    }

    pub fn load(&self, id: &str) -> Result<Macro, String> {
        let s = std::fs::read_to_string(self.path(id)?)
            .map_err(|_| format!("Makro {id:?} nicht gefunden"))?;
        serde_json::from_str(&s).map_err(|e| format!("Makro {id:?} ist beschädigt: {e}"))
    }

    pub fn save(&self, mut m: Macro) -> Result<Macro, String> {
        m.validate()?;
        let json = serde_json::to_string_pretty(&m).unwrap();
        std::fs::write(self.path(&m.id)?, json).map_err(|e| e.to_string())?;
        Ok(m)
    }

    pub fn delete(&self, id: &str) -> Result<(), String> {
        std::fs::remove_file(self.path(id)?).map_err(|e| e.to_string())
    }

    /// Freie ID für einen neuen Namen.
    pub fn new_id(&self, name: &str) -> String {
        let base = slug(name);
        let mut id = base.clone();
        let mut n = 2;
        while self.dir.join(format!("{id}.json")).exists() {
            id = format!("{base}-{n}");
            n += 1;
        }
        id
    }
}

// --------------------------------------------------------- Aufnahme → Schritte

/// Ein rohes, aufgenommenes Ereignis.
#[derive(Debug, Clone, Copy)]
pub struct RawEvent {
    pub time: SystemTime,
    pub kind: RawKind,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RawKind {
    Key { code: u16, down: bool },
    Move { dx: i32, dy: i32 },
    Scroll { dx: i32, dy: i32 },
}

/// Mausbewegungen innerhalb dieses Fensters werden zusammengefasst.
const MERGE_MS: u32 = 8;

/// Wandelt eine Aufnahme in Makro-Schritte um.
///
/// - Loslassen ohne vorheriges Drücken am Anfang fliegt raus (z. B. der
///   Hotkey, mit dem die Aufnahme gestartet wurde).
/// - Drücken ohne Loslassen am Ende fliegt raus (z. B. der Stopp-Hotkey).
/// - Mausbewegungen werden in kurzen Fenstern zusammengefasst.
pub fn to_steps(raw: &[RawEvent]) -> Vec<Step> {
    let mut keep = vec![true; raw.len()];

    let mut held = std::collections::HashSet::new();
    for (i, ev) in raw.iter().enumerate() {
        if let RawKind::Key { code, down } = ev.kind {
            if down {
                held.insert(code);
            } else if !held.remove(&code) {
                keep[i] = false;
            }
        }
    }
    let mut released = std::collections::HashSet::new();
    for (i, ev) in raw.iter().enumerate().rev() {
        if let RawKind::Key { code, down } = ev.kind {
            if !down {
                released.insert(code);
            } else if !released.remove(&code) {
                keep[i] = false;
            }
        }
    }

    let mut steps: Vec<Step> = Vec::new();
    let mut last: Option<SystemTime> = None;
    // Zeit seit Beginn der aktuellen Bewegungsgruppe
    let mut group_ms = u32::MAX;
    for (ev, _) in raw.iter().zip(&keep).filter(|(_, k)| **k) {
        let delay = last
            .and_then(|l| ev.time.duration_since(l).ok())
            .map(|d| d.as_millis().min(u32::MAX as u128) as u32)
            .unwrap_or(0);
        last = Some(ev.time);

        if let (
            RawKind::Move { dx, dy },
            Some(Step {
                action: Action::Move { dx: px, dy: py },
                ..
            }),
        ) = (ev.kind, steps.last_mut())
            && group_ms.saturating_add(delay) < MERGE_MS
        {
            *px += dx;
            *py += dy;
            group_ms += delay;
            // Die Wartezeit des zusammengefassten Teils geht nicht verloren.
            if let Some(s) = steps.last_mut() {
                s.delay_ms += delay;
            }
            continue;
        }
        group_ms = 0;
        let action = match ev.kind {
            RawKind::Key { code, down } => Action::Key { code, down },
            RawKind::Move { dx, dy } => Action::Move { dx, dy },
            RawKind::Scroll { dx, dy } => Action::Scroll { dx, dy },
        };
        let delay_ms = if steps.is_empty() { 0 } else { delay };
        steps.push(Step { delay_ms, action });
    }
    // Mausbewegungen nach der letzten Taste sind meist der Weg zum Stopp-Knopf.
    if let Some(last_key) = steps
        .iter()
        .rposition(|s| matches!(s.action, Action::Key { .. }))
    {
        steps.truncate(last_key + 1);
    }
    steps
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn ev(ms: u64, kind: RawKind) -> RawEvent {
        RawEvent {
            time: SystemTime::UNIX_EPOCH + Duration::from_millis(1000 + ms),
            kind,
        }
    }
    const A: u16 = 30;
    const F9: u16 = 67;

    #[test]
    fn trims_hotkey_and_keeps_timing() {
        let raw = [
            ev(
                0,
                RawKind::Key {
                    code: F9,
                    down: false,
                },
            ), // Start-Hotkey losgelassen
            ev(
                100,
                RawKind::Key {
                    code: A,
                    down: true,
                },
            ),
            ev(
                180,
                RawKind::Key {
                    code: A,
                    down: false,
                },
            ),
            ev(
                500,
                RawKind::Key {
                    code: F9,
                    down: true,
                },
            ), // Stopp-Hotkey gedrückt
        ];
        let steps = to_steps(&raw);
        assert_eq!(
            steps,
            [
                Step {
                    delay_ms: 0,
                    action: Action::Key {
                        code: A,
                        down: true
                    }
                },
                Step {
                    delay_ms: 80,
                    action: Action::Key {
                        code: A,
                        down: false
                    }
                },
            ]
        );
    }

    #[test]
    fn merges_mouse_moves() {
        let raw = [
            ev(0, RawKind::Move { dx: 1, dy: 0 }),
            ev(2, RawKind::Move { dx: 2, dy: 1 }),
            ev(4, RawKind::Move { dx: 3, dy: 1 }),
            ev(30, RawKind::Move { dx: 5, dy: 5 }),
        ];
        let steps = to_steps(&raw);
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].action, Action::Move { dx: 6, dy: 2 });
        assert_eq!(steps[1].action, Action::Move { dx: 5, dy: 5 });
        assert_eq!(steps.iter().map(|s| s.delay_ms).sum::<u32>(), 30);
    }

    #[test]
    fn drops_trailing_moves_after_last_key() {
        let raw = [
            ev(
                0,
                RawKind::Key {
                    code: A,
                    down: true,
                },
            ),
            ev(
                50,
                RawKind::Key {
                    code: A,
                    down: false,
                },
            ),
            ev(100, RawKind::Move { dx: 40, dy: 10 }),
        ];
        assert_eq!(to_steps(&raw).len(), 2);
        // Reine Bewegungs-Makros bleiben erhalten.
        assert_eq!(to_steps(&raw[2..]).len(), 1);
    }

    #[test]
    fn json_roundtrip_and_validation() {
        let mut m = Macro {
            id: "test".into(),
            name: " Test ".into(),
            steps: vec![Step {
                delay_ms: 5,
                action: Action::Key {
                    code: 0x110,
                    down: true,
                },
            }],
            repeat: 0,
            speed: 2.0,
            shortcut: Some("  ".into()),
        };
        m.validate().unwrap();
        assert_eq!(m.name, "Test");
        assert_eq!(m.shortcut, None);
        let json = serde_json::to_string(&m).unwrap();
        assert!(json.contains(r#""type":"key""#));
        let back: Macro = serde_json::from_str(&json).unwrap();
        assert_eq!(back.steps, m.steps);

        m.steps.push(Step {
            delay_ms: 0,
            action: Action::Key {
                code: 600,
                down: true,
            },
        });
        assert!(m.validate().is_err());
    }

    #[test]
    fn store_ids_and_slug() {
        assert_eq!(slug("Mein Makro: Öffnen!"), "mein-makro-offnen");
        assert_eq!(slug("!!!"), "makro");
        let dir = std::env::temp_dir().join(format!("tuxhub-macros-{}", std::process::id()));
        let store = Store::at(dir.clone());
        let id = store.new_id("Farm");
        store
            .save(Macro {
                id: id.clone(),
                name: "Farm".into(),
                steps: vec![],
                repeat: 1,
                speed: 1.0,
                shortcut: None,
            })
            .unwrap();
        assert_eq!(store.new_id("Farm"), "farm-2");
        assert_eq!(store.list().len(), 1);
        assert!(store.load("../etc").is_err());
        store.delete(&id).unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn key_names() {
        assert_eq!(key_name(30), "KEY_A");
        assert_eq!(key_name(0x110), "BTN_LEFT");
        assert!(all_keys().len() > 100);
    }
}
