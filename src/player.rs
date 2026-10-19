//! Makro-Wiedergabe über ein virtuelles Tastatur-und-Maus-Gerät.

use crate::macros::{Action, Macro, playable_key};
use evdev::uinput::VirtualDevice;
use evdev::{AttributeSet, EventType, InputEvent, KeyCode, RelativeAxisCode};
use serde::Serialize;
use std::collections::BTreeSet;
use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

pub const PLAYER_DEVICE_NAME: &str = "tuxhub-autoclicker-macro";

pub struct Keyboard {
    dev: VirtualDevice,
}

impl Keyboard {
    pub fn new() -> io::Result<Self> {
        let keys =
            AttributeSet::from_iter((0..0x118u16).filter(|c| playable_key(*c)).map(KeyCode::new));
        let axes = AttributeSet::from_iter([
            RelativeAxisCode::REL_X,
            RelativeAxisCode::REL_Y,
            RelativeAxisCode::REL_WHEEL,
            RelativeAxisCode::REL_HWHEEL,
        ]);
        let dev = VirtualDevice::builder()?
            .name(PLAYER_DEVICE_NAME)
            .with_keys(&keys)?
            .with_relative_axes(&axes)?
            .build()?;
        Ok(Self { dev })
    }

    pub fn dev_nodes(&mut self) -> io::Result<Vec<PathBuf>> {
        self.dev.enumerate_dev_nodes_blocking()?.collect()
    }

    fn key(&mut self, code: u16, down: bool) -> io::Result<()> {
        self.dev
            .emit(&[InputEvent::new(EventType::KEY.0, code, down as i32)])
    }

    fn rel(&mut self, pairs: &[(RelativeAxisCode, i32)]) -> io::Result<()> {
        let evs: Vec<InputEvent> = pairs
            .iter()
            .filter(|(_, v)| *v != 0)
            .map(|(a, v)| InputEvent::new(EventType::RELATIVE.0, a.0, *v))
            .collect();
        if evs.is_empty() {
            Ok(())
        } else {
            self.dev.emit(&evs)
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PlayStatus {
    pub playing: Option<String>,
    pub round: u32,
    pub text: String,
}

pub type PlayStatusFn = Arc<dyn Fn(&PlayStatus) + Send + Sync>;

struct Shared {
    kb: Mutex<Keyboard>,
    stop: Mutex<bool>,
    wake: Condvar,
    on_status: PlayStatusFn,
}

impl Shared {
    fn wait(&self, dur: Duration) -> bool {
        let guard = self.stop.lock().unwrap();
        let (guard, _) = self.wake.wait_timeout_while(guard, dur, |s| !*s).unwrap();
        *guard
    }
}

pub struct Player {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

impl Player {
    pub fn new(kb: Keyboard, on_status: PlayStatusFn) -> Self {
        Self {
            shared: Arc::new(Shared {
                kb: Mutex::new(kb),
                stop: Mutex::new(false),
                wake: Condvar::new(),
                on_status,
            }),
            thread: None,
        }
    }

    pub fn is_playing(&self) -> bool {
        self.thread.as_ref().is_some_and(|t| !t.is_finished())
    }

    /// Spielt ein Makro ab; ein laufendes wird vorher gestoppt.
    pub fn play(&mut self, m: Macro) {
        self.stop();
        *self.shared.stop.lock().unwrap() = false;
        let shared = self.shared.clone();
        self.thread = Some(std::thread::spawn(move || run(&shared, &m)));
    }

    pub fn stop(&mut self) {
        *self.shared.stop.lock().unwrap() = true;
        self.shared.wake.notify_all();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.stop();
    }
}

fn run(shared: &Shared, m: &Macro) {
    let status = |round: u32, playing: bool, text: String| {
        (shared.on_status)(&PlayStatus {
            playing: playing.then(|| m.id.clone()),
            round,
            text,
        });
    };
    let speed = m.speed.clamp(0.1, 10.0);
    let mut held = BTreeSet::new();
    let mut round = 0u32;
    let result: io::Result<bool> = (|| {
        loop {
            round += 1;
            let of = if m.repeat == 0 {
                "∞".to_string()
            } else {
                m.repeat.to_string()
            };
            status(
                round,
                true,
                format!("Spielt „{}“ – Runde {round}/{of}", m.name),
            );
            for step in &m.steps {
                let wait = Duration::from_secs_f64(step.delay_ms as f64 / 1000.0 / speed);
                if !wait.is_zero() && shared.wait(wait) {
                    return Ok(false);
                }
                if *shared.stop.lock().unwrap() {
                    return Ok(false);
                }
                let mut kb = shared.kb.lock().unwrap();
                match step.action {
                    Action::Key { code, down } if playable_key(code) => {
                        kb.key(code, down)?;
                        if down {
                            held.insert(code);
                        } else {
                            held.remove(&code);
                        }
                    }
                    Action::Key { .. } | Action::Wait => {}
                    Action::Move { dx, dy } => {
                        kb.rel(&[(RelativeAxisCode::REL_X, dx), (RelativeAxisCode::REL_Y, dy)])?
                    }
                    Action::Scroll { dx, dy } => kb.rel(&[
                        (RelativeAxisCode::REL_WHEEL, dy),
                        (RelativeAxisCode::REL_HWHEEL, dx),
                    ])?,
                }
            }
            if m.repeat != 0 && round >= m.repeat {
                return Ok(true);
            }
            // Leeres Makro in Endlosschleife würde sonst die CPU verbrennen.
            if m.duration_ms() == 0 && shared.wait(Duration::from_millis(10)) {
                return Ok(false);
            }
        }
    })();

    // Nie eine gedrückte Taste zurücklassen.
    let mut kb = shared.kb.lock().unwrap();
    for code in held {
        let _ = kb.key(code, false);
    }
    drop(kb);
    match result {
        Ok(true) => status(round, false, format!("„{}“ fertig", m.name)),
        Ok(false) => status(round, false, format!("„{}“ gestoppt", m.name)),
        Err(e) => status(round, false, format!("Fehler: {e}")),
    }
}
