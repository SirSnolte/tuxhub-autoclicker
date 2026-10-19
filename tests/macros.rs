//! Makro-Tests mit echten uinput-Geräten.
//!
//! - Wiedergabe: Das Makro-Gerät wird per EVIOCGRAB exklusiv belegt – nichts
//!   erreicht den Desktop.
//! - Aufnahme: Grab ist hier nicht möglich (der Recorder muss mitlesen). Der
//!   Test sendet deshalb nur F24 auf einem eigenen Testgerät und der Recorder
//!   liest ausschließlich dieses Gerät.
//!
//! Ohne Zugriff auf /dev/uinput bzw. /dev/input werden die Tests übersprungen.

use evdev::uinput::VirtualDevice;
use evdev::{AttributeSet, Device, EventSummary, EventType, InputEvent, KeyCode, RelativeAxisCode};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};
use tuxhub_autoclicker::macros::{self, Action, Macro, Step};
use tuxhub_autoclicker::player::{Keyboard, PLAYER_DEVICE_NAME, PlayStatus, Player};
use tuxhub_autoclicker::recorder::Recorder;

#[derive(Debug, PartialEq)]
enum Got {
    Key(u16, i32),
    Rel(u16, i32),
}

/// Öffnet den event-Knoten mit genau diesem Namen (wartet auf udev-Rechte).
fn open_named(nodes: impl Fn() -> Vec<std::path::PathBuf>, name: &str) -> Option<Device> {
    let end = Instant::now() + Duration::from_secs(3);
    while Instant::now() < end {
        for n in nodes() {
            if !n
                .file_name()
                .is_some_and(|f| f.to_string_lossy().starts_with("event"))
            {
                continue;
            }
            if let Ok(d) = Device::open(&n)
                && d.name() == Some(name)
            {
                return Some(d);
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    None
}

fn player_with_grabbed_reader() -> Option<(Player, mpsc::Receiver<Got>, mpsc::Receiver<PlayStatus>)>
{
    let kb = match Keyboard::new() {
        Ok(k) => k,
        Err(e) => {
            eprintln!("übersprungen: {e}");
            return None;
        }
    };
    let kb = Arc::new(Mutex::new(kb));
    let kb2 = kb.clone();
    let Some(mut dev) = open_named(
        move || kb2.lock().unwrap().dev_nodes().unwrap_or_default(),
        PLAYER_DEVICE_NAME,
    ) else {
        eprintln!("übersprungen: Makro-Gerät nicht lesbar");
        return None;
    };
    dev.grab()
        .expect("Grab muss klappen, sonst kämen echte Eingaben an");
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        while let Ok(evs) = dev.fetch_events() {
            for ev in evs {
                let got = match ev.destructure() {
                    EventSummary::Key(_, c, v) => Got::Key(c.code(), v),
                    EventSummary::RelativeAxis(_, a, v) => Got::Rel(a.0, v),
                    _ => continue,
                };
                if tx.send(got).is_err() {
                    return;
                }
            }
        }
    });
    let (stx, srx) = mpsc::channel();
    let kb = Arc::into_inner(kb).unwrap().into_inner().unwrap();
    let player = Player::new(
        kb,
        Arc::new(move |s: &PlayStatus| {
            let _ = stx.send(s.clone());
        }),
    );
    Some((player, rx, srx))
}

fn collect<T>(rx: &mpsc::Receiver<T>, n: usize, timeout: Duration) -> Vec<T> {
    let end = Instant::now() + timeout;
    let mut out = Vec::new();
    while out.len() < n {
        match rx.recv_timeout(end.saturating_duration_since(Instant::now())) {
            Ok(v) => out.push(v),
            Err(_) => break,
        }
    }
    out
}

fn mk(steps: Vec<Step>, repeat: u32) -> Macro {
    Macro {
        id: "test".into(),
        name: "Test".into(),
        steps,
        repeat,
        speed: 1.0,
        shortcut: None,
    }
}

#[test]
fn plays_keys_moves_and_scroll_twice() {
    let Some((mut player, rx, _)) = player_with_grabbed_reader() else {
        return;
    };
    let a = KeyCode::KEY_A.code();
    player.play(mk(
        vec![
            Step {
                delay_ms: 0,
                action: Action::Key {
                    code: a,
                    down: true,
                },
            },
            Step {
                delay_ms: 10,
                action: Action::Key {
                    code: a,
                    down: false,
                },
            },
            Step {
                delay_ms: 5,
                action: Action::Move { dx: 7, dy: -3 },
            },
            Step {
                delay_ms: 5,
                action: Action::Scroll { dx: 0, dy: 1 },
            },
        ],
        2,
    ));
    let got = collect(&rx, 10, Duration::from_secs(3));
    let x = RelativeAxisCode::REL_X.0;
    let y = RelativeAxisCode::REL_Y.0;
    let wheel = RelativeAxisCode::REL_WHEEL.0;
    let round = [
        Got::Key(a, 1),
        Got::Key(a, 0),
        Got::Rel(x, 7),
        Got::Rel(y, -3),
        Got::Rel(wheel, 1),
    ];
    assert_eq!(&got[..5], &round);
    assert_eq!(&got[5..], &round);
    assert!(
        collect(&rx, 1, Duration::from_millis(200)).is_empty(),
        "nach 2 Runden ist Schluss"
    );
}

#[test]
fn stop_releases_held_keys() {
    let Some((mut player, rx, _)) = player_with_grabbed_reader() else {
        return;
    };
    let shift = KeyCode::KEY_LEFTSHIFT.code();
    player.play(mk(
        vec![
            Step {
                delay_ms: 0,
                action: Action::Key {
                    code: shift,
                    down: true,
                },
            },
            Step {
                delay_ms: 60_000,
                action: Action::Wait,
            },
        ],
        1,
    ));
    assert_eq!(
        collect(&rx, 1, Duration::from_secs(2)),
        [Got::Key(shift, 1)]
    );
    let t = Instant::now();
    player.stop();
    assert!(t.elapsed() < Duration::from_millis(500));
    assert_eq!(
        collect(&rx, 1, Duration::from_secs(2)),
        [Got::Key(shift, 0)],
        "Shift muss losgelassen werden"
    );
}

#[test]
fn endless_loop_stops() {
    let Some((mut player, rx, status)) = player_with_grabbed_reader() else {
        return;
    };
    let b = KeyCode::KEY_B.code();
    player.play(mk(
        vec![
            Step {
                delay_ms: 2,
                action: Action::Key {
                    code: b,
                    down: true,
                },
            },
            Step {
                delay_ms: 2,
                action: Action::Key {
                    code: b,
                    down: false,
                },
            },
        ],
        0,
    ));
    assert_eq!(
        collect(&rx, 20, Duration::from_secs(3)).len(),
        20,
        "läuft endlos weiter"
    );
    player.stop();
    assert!(!player.is_playing());
    let last = collect(&status, 100, Duration::from_millis(300))
        .pop()
        .unwrap();
    assert!(
        last.playing.is_none() && last.text.contains("gestoppt"),
        "{last:?}"
    );
}

#[test]
fn records_from_test_device_only() {
    const NAME: &str = "tuxhub-recorder-test";
    let f24 = KeyCode::KEY_F24;
    let mut fake = match VirtualDevice::builder().and_then(|b| {
        b.name(NAME)
            .with_keys(&AttributeSet::from_iter([
                f24,
                KeyCode::KEY_A,
                KeyCode::KEY_SPACE,
            ]))?
            .build()
    }) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("übersprungen: {e}");
            return;
        }
    };
    // Warten, bis udev den Knoten lesbar gemacht hat.
    let nodes: Vec<_> = fake
        .enumerate_dev_nodes_blocking()
        .unwrap()
        .flatten()
        .collect();
    if open_named(|| nodes.clone(), NAME).is_none() {
        eprintln!("übersprungen: Testgerät nicht lesbar (Gruppe input?)");
        return;
    }

    let rec = Recorder::start_filtered(true, |n| n == NAME).expect("Recorder");
    assert_eq!(rec.devices, [NAME]);
    std::thread::sleep(Duration::from_millis(100));
    let key = |v| InputEvent::new(EventType::KEY.0, f24.code(), v);
    fake.emit(&[key(1)]).unwrap();
    std::thread::sleep(Duration::from_millis(120));
    fake.emit(&[key(0)]).unwrap();
    std::thread::sleep(Duration::from_millis(100));
    let steps = macros::to_steps(&rec.finish());

    assert_eq!(steps.len(), 2, "{steps:?}");
    assert_eq!(
        steps[0].action,
        Action::Key {
            code: f24.code(),
            down: true
        }
    );
    assert_eq!(
        steps[1].action,
        Action::Key {
            code: f24.code(),
            down: false
        }
    );
    assert!(
        (100..=250).contains(&steps[1].delay_ms),
        "Zeitabstand {} ms",
        steps[1].delay_ms
    );
}
