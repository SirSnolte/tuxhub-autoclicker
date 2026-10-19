//! Aufnahme von Tastatur und Maus über `/dev/input/event*`.
//!
//! Liest nur mit (kein Grab) – die Eingaben gehen also ganz normal weiter an
//! Spiel und Desktop. Gelesen wird ausschließlich während einer Aufnahme.

use crate::macros::{RawEvent, RawKind};
use evdev::{Device, EventSummary, KeyCode, RelativeAxisCode};
use std::io;
use std::os::fd::AsRawFd;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;

pub struct Recorder {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<Vec<RawEvent>>>,
    pub devices: Vec<String>,
}

impl Recorder {
    /// Öffnet alle lesbaren Tastaturen und Mäuse und beginnt die Aufnahme.
    pub fn start(record_moves: bool) -> io::Result<Self> {
        Self::start_filtered(record_moves, |_| true)
    }

    /// Wie [`start`](Self::start), aber nur Geräte, deren Name `filter` erfüllt.
    pub fn start_filtered(record_moves: bool, filter: impl Fn(&str) -> bool) -> io::Result<Self> {
        let mut devs = Vec::new();
        let mut names = Vec::new();
        for (_, dev) in evdev::enumerate() {
            let name = dev.name().unwrap_or("").to_string();
            // Eigene virtuelle Geräte nicht mit aufnehmen.
            if name.starts_with(crate::device::DEVICE_NAME) || !filter(&name) {
                continue;
            }
            let Some(keys) = dev.supported_keys() else {
                continue;
            };
            let keyboard = keys.contains(KeyCode::KEY_A) && keys.contains(KeyCode::KEY_SPACE);
            let mouse = keys.contains(KeyCode::BTN_LEFT)
                && dev
                    .supported_relative_axes()
                    .is_some_and(|r| r.contains(RelativeAxisCode::REL_X));
            if !keyboard && !mouse {
                continue;
            }
            dev.set_nonblocking(true)?;
            names.push(name);
            devs.push(dev);
        }
        if devs.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Keine Tastatur oder Maus lesbar. Ist dein Nutzer in der Gruppe `input`? \
                 (sudo usermod -aG input $USER, danach neu anmelden)",
            ));
        }

        let stop = Arc::new(AtomicBool::new(false));
        let stop2 = stop.clone();
        let thread = std::thread::spawn(move || read_loop(devs, &stop2, record_moves));
        Ok(Self {
            stop,
            thread: Some(thread),
            devices: names,
        })
    }

    /// Beendet die Aufnahme und liefert die rohen Ereignisse.
    pub fn finish(mut self) -> Vec<RawEvent> {
        self.stop.store(true, Ordering::SeqCst);
        self.thread
            .take()
            .and_then(|t| t.join().ok())
            .unwrap_or_default()
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

fn read_loop(mut devs: Vec<Device>, stop: &AtomicBool, record_moves: bool) -> Vec<RawEvent> {
    let mut out = Vec::new();
    let mut fds: Vec<libc::pollfd> = devs
        .iter()
        .map(|d| libc::pollfd {
            fd: d.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        })
        .collect();

    while !stop.load(Ordering::SeqCst) {
        // Mit Timeout, damit `stop` regelmäßig geprüft wird.
        let n = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, 50) };
        if n <= 0 {
            continue;
        }
        for (dev, pfd) in devs.iter_mut().zip(fds.iter_mut()) {
            if pfd.revents & libc::POLLIN == 0 {
                continue;
            }
            pfd.revents = 0;
            let Ok(events) = dev.fetch_events() else {
                continue;
            };
            for ev in events {
                let time = ev.timestamp();
                let kind = match ev.destructure() {
                    // value 2 = Autorepeat, das erzeugt das System beim Abspielen selbst
                    EventSummary::Key(_, code, v @ (0 | 1)) => RawKind::Key {
                        code: code.code(),
                        down: v == 1,
                    },
                    EventSummary::RelativeAxis(_, RelativeAxisCode::REL_X, v) if record_moves => {
                        RawKind::Move { dx: v, dy: 0 }
                    }
                    EventSummary::RelativeAxis(_, RelativeAxisCode::REL_Y, v) if record_moves => {
                        RawKind::Move { dx: 0, dy: v }
                    }
                    EventSummary::RelativeAxis(_, RelativeAxisCode::REL_WHEEL, v) => {
                        RawKind::Scroll { dx: 0, dy: v }
                    }
                    EventSummary::RelativeAxis(_, RelativeAxisCode::REL_HWHEEL, v) => {
                        RawKind::Scroll { dx: v, dy: 0 }
                    }
                    _ => continue,
                };
                if let RawKind::Key { code, .. } = kind
                    && !crate::macros::playable_key(code)
                {
                    continue;
                }
                out.push(RawEvent { time, kind });
            }
        }
    }
    // Geräte kommen in unterschiedlicher Reihenfolge an – nach Zeit sortieren.
    out.sort_by_key(|e| e.time);
    out
}
