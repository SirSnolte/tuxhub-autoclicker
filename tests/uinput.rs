//! End-to-end tests with a real uinput device.
//!
//! The test grabs the device exclusively (EVIOCGRAB) before any click is
//! emitted, so no click ever reaches the desktop. Without access to
//! /dev/uinput or /dev/input the tests are skipped.

use evdev::{Device, EventType, KeyCode};
use std::sync::Arc;
use std::sync::mpsc;
use std::time::{Duration, Instant};
use tuxhub_autoclicker::clicker::{Clicker, Config, Mode};
use tuxhub_autoclicker::device::{Button, DEVICE_NAME, Mouse};

/// Creates the mouse and a grabbed reader that collects key events (code, value).
fn setup() -> Option<(Mouse, mpsc::Receiver<(u16, i32)>)> {
    let mut mouse = match Mouse::new() {
        Ok(m) => m,
        Err(e) => {
            eprintln!("skipped: {e}");
            return None;
        }
    };
    let mut dev = match open_own_node(&mut mouse) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("skipped: {e}");
            return None;
        }
    };
    dev.grab()
        .expect("the grab must succeed, otherwise real clicks would reach the desktop");
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        while let Ok(events) = dev.fetch_events() {
            for ev in events {
                if ev.event_type() == EventType::KEY && tx.send((ev.code(), ev.value())).is_err() {
                    return;
                }
            }
        }
    });
    Some((mouse, rx))
}

/// Opens the event node of the virtual device. Checks the name so a physical
/// device is never grabbed by accident, and waits until udev has applied the
/// permissions of the new node.
fn open_own_node(mouse: &mut Mouse) -> Result<Device, String> {
    let end = Instant::now() + Duration::from_secs(3);
    let mut last_err = String::from("no event node found");
    while Instant::now() < end {
        let nodes = mouse.dev_nodes().map_err(|e| e.to_string())?;
        for node in nodes.iter().filter(|n| {
            n.file_name()
                .is_some_and(|f| f.to_string_lossy().starts_with("event"))
        }) {
            match Device::open(node) {
                Ok(d) if d.name() == Some(DEVICE_NAME) => return Ok(d),
                Ok(d) => {
                    last_err = format!("{} is {:?}, not the test device", node.display(), d.name())
                }
                Err(e) => last_err = format!("{}: {e}", node.display()),
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Err(last_err)
}

fn collect(rx: &mpsc::Receiver<(u16, i32)>, n: usize, timeout: Duration) -> Vec<(u16, i32)> {
    let end = Instant::now() + timeout;
    let mut out = Vec::new();
    while out.len() < n {
        let left = end.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left) {
            Ok(ev) => out.push(ev),
            Err(_) => break,
        }
    }
    out
}

#[test]
fn clicks_reach_device_and_stop_at_limit() {
    let Some((mouse, rx)) = setup() else { return };
    let mut c = Clicker::new(mouse, Arc::new(|_| {}));
    let cfg = Config {
        interval_ms: 5,
        limit: 5,
        start_delay_s: 0,
        button: Button::Right,
        ..Default::default()
    };
    c.start(&cfg, false);
    let evs = collect(&rx, 10, Duration::from_secs(3));
    let right = KeyCode::BTN_RIGHT.code();
    assert_eq!(evs, [(right, 1), (right, 0)].repeat(5));
    // Nothing follows after the limit.
    assert!(collect(&rx, 1, Duration::from_millis(200)).is_empty());
    assert!(!c.is_running());
}

#[test]
fn double_click_and_stop() {
    let Some((mouse, rx)) = setup() else { return };
    let mut c = Clicker::new(mouse, Arc::new(|_| {}));
    let cfg = Config {
        interval_ms: 50,
        mode: Mode::Double,
        limit: 2,
        start_delay_s: 0,
        ..Default::default()
    };
    c.start(&cfg, false);
    let evs = collect(&rx, 8, Duration::from_secs(3));
    assert_eq!(evs.len(), 8, "2 double clicks = 4 presses and releases");
}

#[test]
fn hold_releases_on_stop() {
    let Some((mouse, rx)) = setup() else { return };
    let mut c = Clicker::new(mouse, Arc::new(|_| {}));
    let cfg = Config {
        mode: Mode::Hold,
        start_delay_s: 0,
        ..Default::default()
    };
    c.start(&cfg, false);
    let left = KeyCode::BTN_LEFT.code();
    assert_eq!(collect(&rx, 1, Duration::from_secs(2)), [(left, 1)]);
    let t = Instant::now();
    c.stop();
    assert!(
        t.elapsed() < Duration::from_millis(500),
        "stop must take effect immediately"
    );
    assert_eq!(collect(&rx, 1, Duration::from_secs(2)), [(left, 0)]);
}

#[test]
fn stop_during_start_delay_clicks_nothing() {
    let Some((mouse, rx)) = setup() else { return };
    let mut c = Clicker::new(mouse, Arc::new(|_| {}));
    let cfg = Config {
        start_delay_s: 2,
        ..Default::default()
    };
    c.start(&cfg, true);
    std::thread::sleep(Duration::from_millis(300));
    c.stop();
    assert!(collect(&rx, 1, Duration::from_millis(2500)).is_empty());
}
