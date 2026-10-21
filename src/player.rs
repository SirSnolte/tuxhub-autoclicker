//! Macro playback through a virtual keyboard and mouse device.

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

pub const PLAYER_DEVICE_NAME: &str = "tuxly-autoclicker-macro";

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

/// Playback state. User interfaces build localized text from `phase`;
/// `text` is an English fallback.
#[derive(Debug, Clone, Serialize)]
pub struct PlayStatus {
    /// Id of the macro being played, if any.
    pub playing: Option<String>,
    /// `playing`, `finished`, `stopped` or `error`.
    pub phase: &'static str,
    pub name: String,
    pub round: u32,
    /// Total rounds, 0 = endless.
    pub rounds: u32,
    pub error: Option<String>,
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

    /// Plays a macro; a macro that is already playing is stopped first.
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
    let status = |round: u32, phase: &'static str, error: Option<String>| {
        let text = match phase {
            "playing" if m.repeat == 0 => format!("Playing \"{}\", round {round}", m.name),
            "playing" => format!("Playing \"{}\", round {round} of {}", m.name, m.repeat),
            "finished" => format!("\"{}\" finished", m.name),
            "stopped" => format!("\"{}\" stopped", m.name),
            _ => format!("Error: {}", error.as_deref().unwrap_or("unknown")),
        };
        (shared.on_status)(&PlayStatus {
            playing: (phase == "playing").then(|| m.id.clone()),
            phase,
            name: m.name.clone(),
            round,
            rounds: m.repeat,
            error,
            text,
        });
    };
    let speed = m.speed.clamp(0.1, 10.0);
    let mut held = BTreeSet::new();
    let mut round = 0u32;
    let result: io::Result<bool> = (|| {
        loop {
            round += 1;
            status(round, "playing", None);
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
            // Prevent an empty endless macro from spinning the CPU.
            if m.duration_ms() == 0 && shared.wait(Duration::from_millis(10)) {
                return Ok(false);
            }
        }
    })();

    // Never leave a key pressed.
    let mut kb = shared.kb.lock().unwrap();
    for code in held {
        let _ = kb.key(code, false);
    }
    drop(kb);
    match result {
        Ok(true) => status(round, "finished", None),
        Ok(false) => status(round, "stopped", None),
        Err(e) => status(round, "error", Some(e.to_string())),
    }
}
