//! tuxhub mode: JSON-RPC over standard input and output, one message per line.
//!
//! Methods:
//! - `configure`, `status`, `shutdown`
//! - `action` with `id`: `start`, `stop`, `toggle`, `setup`, `record`
//!   (start or stop recording), `panic` (stop everything), `macro:<id>`
//!   (play or stop a macro)
//! - `macros.list`, `macros.get`, `macros.save`, `macros.delete`,
//!   `macros.play`, `macros.stop`, `record.start`, `record.stop`, `keys`
//!
//! Notifications sent to tuxhub: `status` (clicker), `macro-status`,
//! `macros-changed` and `shortcuts` (runtime hotkeys).

use crate::clicker::{Clicker, Config, Phase, Status};
use crate::device::{Mouse, setup_access};
use crate::macros::{self, Macro, Store};
use crate::player::{Keyboard, PlayStatus, Player};
use crate::recorder::Recorder;
use serde::Deserialize;
use serde_json::{Value, json};
use std::io::{BufRead, Write};
use std::sync::{Arc, Mutex};

type Send = Arc<dyn Fn(Value) + std::marker::Send + Sync>;

#[derive(Deserialize)]
#[serde(default)]
struct MacroSettings {
    record_moves: bool,
    /// Optional hotkey that stops the clicker (empty = none).
    stop_shortcut: String,
}

impl Default for MacroSettings {
    fn default() -> Self {
        Self {
            record_moves: true,
            stop_shortcut: String::new(),
        }
    }
}

struct State {
    send: Send,
    cfg: Config,
    macro_cfg: MacroSettings,
    clicker: Option<Clicker>,
    player: Option<Player>,
    recorder: Option<Recorder>,
    device_error: Option<String>,
    store: Store,
    last_status: Arc<Mutex<Status>>,
}

pub fn serve() -> std::io::Result<()> {
    let out = Mutex::new(std::io::stdout());
    let send: Send = Arc::new(move |msg: Value| {
        let mut o = out.lock().unwrap();
        let _ = writeln!(o, "{msg}");
        let _ = o.flush();
    });

    let mut st = State {
        send: send.clone(),
        cfg: Config::default(),
        macro_cfg: MacroSettings::default(),
        clicker: None,
        player: None,
        recorder: None,
        device_error: None,
        store: Store::open()?,
        last_status: Arc::new(Mutex::new(Status::new(Phase::Ready, 0, 0, None))),
    };
    // Create the devices right away: libinput needs a moment to pick up new
    // devices, and this way the first click is not lost.
    st.create_devices();
    st.clicker_status(match &st.device_error {
        Some(e) => Status::new(Phase::Error, 0, 0, Some(e.clone())),
        None => Status::new(Phase::Ready, 0, 0, None),
    });
    st.send_shortcuts();

    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        let Ok(msg) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let id = msg.get("id").cloned();
        let method = msg
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let params = msg.get("params").cloned().unwrap_or(Value::Null);
        if method == "shutdown" {
            break;
        }
        let result = st.handle(method, params);
        if let Some(id) = id {
            send(match result {
                Ok(r) => json!({ "id": id, "result": r }),
                Err(e) => json!({ "id": id, "error": { "message": e } }),
            });
        }
    }
    // Standard input closed (tuxhub exited): stop everything immediately.
    st.panic();
    Ok(())
}

impl State {
    fn create_devices(&mut self) {
        let on_status = {
            let send = self.send.clone();
            let last = self.last_status.clone();
            Arc::new(move |s: &Status| {
                *last.lock().unwrap() = s.clone();
                send(json!({ "method": "status", "params": s }));
            })
        };
        let on_play = {
            let send = self.send.clone();
            Arc::new(move |s: &PlayStatus| {
                let mut params = json!(s);
                params["recording"] = json!(false);
                send(json!({ "method": "macro-status", "params": params }));
            })
        };
        match Mouse::new().and_then(|m| Ok((m, Keyboard::new()?))) {
            Ok((mouse, kb)) => {
                self.clicker = Some(Clicker::new(mouse, on_status));
                self.player = Some(Player::new(kb, on_play));
                self.device_error = None;
            }
            Err(e) => self.device_error = Some(e.to_string()),
        }
    }

    fn clicker_status(&self, s: Status) {
        *self.last_status.lock().unwrap() = s.clone();
        (self.send)(json!({ "method": "status", "params": s }));
    }

    /// Recording state for the interface. `phase` is `recording`, `saved`,
    /// `empty` or `discarded`; `text` is an English fallback.
    fn macro_status(&self, phase: &str, devices: usize, saved: Option<&Macro>) {
        let text = match (phase, saved) {
            ("recording", _) => format!("Recording from {devices} devices"),
            ("saved", Some(m)) => format!("\"{}\" saved with {} steps", m.name, m.steps.len()),
            ("empty", _) => "Nothing was recorded".to_string(),
            _ => "Recording discarded".to_string(),
        };
        (self.send)(json!({ "method": "macro-status", "params": {
            "recording": phase == "recording",
            "playing": null,
            "phase": phase,
            "devices": devices,
            "name": saved.map(|m| m.name.clone()),
            "steps": saved.map(|m| m.steps.len()),
            "text": text,
        }}));
    }

    /// Reports the runtime hotkeys (stop key and macro hotkeys) to tuxhub.
    fn send_shortcuts(&self) {
        let stop = self.macro_cfg.stop_shortcut.trim();
        let stop = (!stop.is_empty() && macros::safe_trigger(stop))
            .then(|| json!({ "id": "stop", "label": "Stop clicking", "shortcut": stop }));
        let list: Vec<Value> = stop
            .into_iter()
            .chain(self.store.list().into_iter().filter_map(|m| {
                let s = m.shortcut?;
                Some(json!({ "id": format!("macro:{}", m.id), "label": format!("Macro: {}", m.name), "shortcut": s }))
            }))
            .collect();
        (self.send)(json!({ "method": "shortcuts", "params": list }));
    }

    fn macros_changed(&self) {
        (self.send)(json!({ "method": "macros-changed", "params": null }));
        self.send_shortcuts();
    }

    fn device_err(&self) -> String {
        self.device_error
            .clone()
            .unwrap_or_else(|| "the input device is not available".into())
    }

    fn panic(&mut self) {
        if let Some(c) = self.clicker.as_mut() {
            c.stop();
        }
        if let Some(p) = self.player.as_mut() {
            p.stop();
        }
        if let Some(r) = self.recorder.take() {
            drop(r.finish());
            self.macro_status("discarded", 0, None);
        }
    }

    fn record_start(&mut self) -> Result<Value, String> {
        if self.recorder.is_some() {
            return Err("a recording is already running".into());
        }
        let rec = Recorder::start(self.macro_cfg.record_moves).map_err(|e| e.to_string())?;
        let n = rec.devices.len();
        self.recorder = Some(rec);
        self.macro_status("recording", n, None);
        Ok(json!(true))
    }

    fn record_stop(&mut self, name: Option<String>) -> Result<Value, String> {
        let rec = self.recorder.take().ok_or("no recording is running")?;
        let steps = macros::to_steps(&rec.finish());
        if steps.is_empty() {
            self.macro_status("empty", 0, None);
            return Err("nothing was recorded".into());
        }
        let name = name
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| format!("Recording {}", self.store.list().len() + 1));
        let m = self.store.save(Macro {
            id: self.store.new_id(&name),
            name,
            steps,
            repeat: 1,
            speed: 1.0,
            shortcut: None,
        })?;
        self.macro_status("saved", 0, Some(&m));
        self.macros_changed();
        Ok(json!(m))
    }

    fn play(&mut self, id: &str) -> Result<Value, String> {
        let m = self.store.load(id)?;
        let p = self
            .player
            .as_mut()
            .ok_or_else(|| self.device_error.clone().unwrap_or_default())?;
        p.play(m);
        Ok(json!(true))
    }

    fn handle(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let str_param = |key: &str| params.get(key).and_then(Value::as_str).map(str::to_owned);
        match method {
            "configure" => {
                self.cfg = serde_json::from_value(params.clone()).map_err(|e| format!("invalid settings: {e}"))?;
                let new: MacroSettings = serde_json::from_value(params).unwrap_or_default();
                if !new.stop_shortcut.trim().is_empty() && !macros::safe_trigger(new.stop_shortcut.trim()) {
                    return Err(format!("the stop hotkey {:?} is not allowed", new.stop_shortcut));
                }
                let changed = new.stop_shortcut != self.macro_cfg.stop_shortcut;
                self.macro_cfg = new;
                if changed {
                    self.send_shortcuts();
                }
                Ok(json!(true))
            }
            "status" => Ok(json!(*self.last_status.lock().unwrap())),
            "action" => {
                let action = str_param("id").unwrap_or_default();
                self.action(&action)
            }
            "keys" => Ok(json!(macros::all_keys())),
            "macros.list" => Ok(json!(
                self.store
                    .list()
                    .into_iter()
                    .map(|m| json!({
                        "id": m.id, "name": m.name, "steps": m.steps.len(), "duration_ms": m.duration_ms(),
                        "repeat": m.repeat, "speed": m.speed, "shortcut": m.shortcut,
                    }))
                    .collect::<Vec<_>>()
            )),
            "macros.get" => Ok(json!(self.store.load(&str_param("id").unwrap_or_default())?)),
            "macros.save" => {
                let mut m: Macro = serde_json::from_value(params).map_err(|e| format!("invalid macro: {e}"))?;
                if m.id.is_empty() {
                    m.id = self.store.new_id(&m.name);
                }
                let m = self.store.save(m)?;
                self.macros_changed();
                Ok(json!(m))
            }
            "macros.delete" => {
                self.store.delete(&str_param("id").unwrap_or_default())?;
                self.macros_changed();
                Ok(json!(true))
            }
            "macros.play" => self.play(&str_param("id").unwrap_or_default()),
            "macros.stop" => {
                if let Some(p) = self.player.as_mut() {
                    p.stop();
                }
                Ok(json!(true))
            }
            "record.start" => self.record_start(),
            "record.stop" => self.record_stop(str_param("name")),
            other => Err(format!("unknown method {other:?}")),
        }
    }

    fn action(&mut self, action: &str) -> Result<Value, String> {
        if let Some(id) = action.strip_prefix("macro:") {
            // Macro hotkey: play the macro, or stop playback if one is running.
            if self.player.as_ref().is_some_and(Player::is_playing) {
                self.player.as_mut().unwrap().stop();
                return Ok(json!(false));
            }
            return self.play(id);
        }
        match action {
            "setup" => {
                setup_access().map_err(|e| e.to_string())?;
                self.create_devices();
                if let Some(e) = &self.device_error {
                    return Err(e.clone());
                }
                self.clicker_status(Status::new(Phase::Ready, 0, 0, None));
                Ok(json!(true))
            }
            "record" => {
                if self.recorder.is_some() {
                    self.record_stop(None)
                } else {
                    self.record_start()
                }
            }
            "panic" => {
                self.panic();
                Ok(json!(true))
            }
            "start" | "toggle" | "stop" => {
                let err = self.device_err();
                let c = self.clicker.as_mut().ok_or(err)?;
                match action {
                    "start" => c.start(&self.cfg, true),
                    "toggle" => c.toggle(&self.cfg, false),
                    _ => c.stop(),
                }
                Ok(json!(c.is_running()))
            }
            other => Err(format!("unknown action {other:?}")),
        }
    }
}
