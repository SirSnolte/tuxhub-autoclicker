//! Klick-Schleife in einem eigenen Thread.

use crate::device::{Button, Mouse};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Einzelklicks
    Click,
    /// Doppelklicks
    Double,
    /// Taste gedrückt halten, bis gestoppt wird
    Hold,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub interval_ms: u64,
    pub jitter_pct: u32,
    pub button: Button,
    pub mode: Mode,
    /// 0 = unbegrenzt
    pub limit: u64,
    pub start_delay_s: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            interval_ms: 100,
            jitter_pct: 0,
            button: Button::Left,
            mode: Mode::Click,
            limit: 0,
            start_delay_s: 3,
        }
    }
}

impl Config {
    /// Begrenzt Werte auf vernünftige Bereiche.
    pub fn sanitized(mut self) -> Self {
        self.interval_ms = self.interval_ms.clamp(1, 3_600_000);
        self.jitter_pct = self.jitter_pct.min(90);
        self.start_delay_s = self.start_delay_s.min(300);
        self
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Status {
    pub running: bool,
    pub clicks: u64,
    pub text: String,
}

pub type StatusFn = Arc<dyn Fn(&Status) + Send + Sync>;

struct Shared {
    mouse: Mutex<Mouse>,
    stop: Mutex<bool>,
    wake: Condvar,
    on_status: StatusFn,
}

impl Shared {
    /// Wartet `dur` oder bis gestoppt wird. Gibt `true` zurück, wenn gestoppt.
    fn wait(&self, dur: Duration) -> bool {
        let guard = self.stop.lock().unwrap();
        let (guard, _) = self
            .wake
            .wait_timeout_while(guard, dur, |stop| !*stop)
            .unwrap();
        *guard
    }

    fn set(&self, button: Button, pressed: bool) -> std::io::Result<()> {
        self.mouse.lock().unwrap().set(button, pressed)
    }

    fn status(&self, running: bool, clicks: u64, text: impl Into<String>) {
        (self.on_status)(&Status {
            running,
            clicks,
            text: text.into(),
        });
    }
}

pub struct Clicker {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

impl Clicker {
    pub fn new(mouse: Mouse, on_status: StatusFn) -> Self {
        Self {
            shared: Arc::new(Shared {
                mouse: Mutex::new(mouse),
                stop: Mutex::new(false),
                wake: Condvar::new(),
                on_status,
            }),
            thread: None,
        }
    }

    pub fn is_running(&self) -> bool {
        self.thread.as_ref().is_some_and(|t| !t.is_finished())
    }

    /// Startet das Klicken. `with_delay` nutzt die Startverzögerung aus der
    /// Konfiguration (für den Start per Button, nicht per Hotkey).
    pub fn start(&mut self, cfg: &Config, with_delay: bool) {
        if self.is_running() {
            return;
        }
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        *self.shared.stop.lock().unwrap() = false;
        let shared = self.shared.clone();
        let cfg = cfg.clone().sanitized();
        let delay = if with_delay { cfg.start_delay_s } else { 0 };
        self.thread = Some(std::thread::spawn(move || run(&shared, &cfg, delay)));
    }

    pub fn stop(&mut self) {
        *self.shared.stop.lock().unwrap() = true;
        self.shared.wake.notify_all();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }

    pub fn toggle(&mut self, cfg: &Config, with_delay: bool) {
        if self.is_running() {
            self.stop();
        } else {
            self.start(cfg, with_delay);
        }
    }
}

impl Drop for Clicker {
    fn drop(&mut self) {
        self.stop();
    }
}

fn run(shared: &Shared, cfg: &Config, delay_s: u64) {
    for left in (1..=delay_s).rev() {
        shared.status(true, 0, format!("Startet in {left} s …"));
        if shared.wait(Duration::from_secs(1)) {
            shared.status(false, 0, "Gestoppt");
            return;
        }
    }

    let result = match cfg.mode {
        Mode::Hold => hold(shared, cfg),
        Mode::Click | Mode::Double => click_loop(shared, cfg),
    };
    // Sicherheitshalber nie eine gedrückte Taste zurücklassen.
    let _ = shared.set(cfg.button, false);
    match result {
        Ok(clicks) => shared.status(false, clicks, format!("Gestoppt – {clicks} Klicks")),
        Err(e) => shared.status(false, 0, format!("Fehler: {e}")),
    }
}

fn hold(shared: &Shared, cfg: &Config) -> std::io::Result<u64> {
    shared.set(cfg.button, true)?;
    shared.status(true, 0, "Hält die Taste gedrückt");
    while !shared.wait(Duration::from_secs(3600)) {}
    Ok(0)
}

fn click_loop(shared: &Shared, cfg: &Config) -> std::io::Result<u64> {
    let mut rng = Rng::seeded();
    let interval = Duration::from_millis(cfg.interval_ms);
    // Wie lange die Taste unten bleibt: kurz, aber nie länger als das halbe Intervall.
    let press = Duration::from_millis(12).min(interval / 2);
    let mut clicks = 0u64;
    let mut last_status = Instant::now() - Duration::from_secs(1);

    loop {
        let presses = if cfg.mode == Mode::Double { 2 } else { 1 };
        for i in 0..presses {
            shared.set(cfg.button, true)?;
            if !press.is_zero() && shared.wait(press) {
                return Ok(clicks);
            }
            shared.set(cfg.button, false)?;
            if i + 1 < presses && shared.wait(Duration::from_millis(40)) {
                return Ok(clicks);
            }
        }
        clicks += 1;

        if cfg.limit > 0 && clicks >= cfg.limit {
            return Ok(clicks);
        }
        if last_status.elapsed() >= Duration::from_millis(200) {
            last_status = Instant::now();
            shared.status(true, clicks, format!("Klickt – {clicks} Klicks"));
        }

        let mut wait = interval.saturating_sub(press);
        if cfg.jitter_pct > 0 {
            let max = interval.as_secs_f64() * cfg.jitter_pct as f64 / 100.0;
            let delta = (rng.next_f64() * 2.0 - 1.0) * max;
            wait = Duration::from_secs_f64((wait.as_secs_f64() + delta).max(0.0));
        }
        if shared.wait(wait) {
            return Ok(clicks);
        }
    }
}

/// Kleiner xorshift-Zufallsgenerator – für Jitter reicht das völlig.
struct Rng(u64);

impl Rng {
    fn seeded() -> Self {
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9E37_79B9_7F4A_7C15);
        Self(t | 1)
    }

    fn next_f64(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_from_hub_settings() {
        let cfg: Config = serde_json::from_value(serde_json::json!({
            "interval_ms": 50, "button": "right", "mode": "double", "unknown": 1
        }))
        .unwrap();
        assert_eq!(cfg.interval_ms, 50);
        assert_eq!(cfg.button, Button::Right);
        assert_eq!(cfg.mode, Mode::Double);
        assert_eq!(cfg.start_delay_s, 3);
    }

    #[test]
    fn sanitize_limits() {
        let cfg = Config {
            interval_ms: 0,
            jitter_pct: 500,
            ..Default::default()
        }
        .sanitized();
        assert_eq!(cfg.interval_ms, 1);
        assert_eq!(cfg.jitter_pct, 90);
    }

    #[test]
    fn rng_in_range() {
        let mut r = Rng::seeded();
        for _ in 0..1000 {
            let v = r.next_f64();
            assert!((0.0..1.0).contains(&v));
        }
    }
}
