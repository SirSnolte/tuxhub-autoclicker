//! tuxhub-Modus: JSON-RPC über stdin/stdout, eine Nachricht pro Zeile.
//!
//! Methoden: `configure` (Einstellungen), `action` (`start`, `toggle`, `stop`,
//! `setup`), `status`, `shutdown`. Statusänderungen gehen als
//! `{"method":"status","params":{…}}` an den Hub.

use crate::clicker::{Clicker, Config, Status};
use crate::device::{Mouse, setup_access};
use serde_json::{Value, json};
use std::io::{BufRead, Write};
use std::sync::{Arc, Mutex};

pub fn serve() -> std::io::Result<()> {
    let out = Arc::new(Mutex::new(std::io::stdout()));
    let send = {
        let out = out.clone();
        move |msg: Value| {
            let mut o = out.lock().unwrap();
            let _ = writeln!(o, "{msg}");
            let _ = o.flush();
        }
    };

    let last = Arc::new(Mutex::new(Status {
        running: false,
        clicks: 0,
        text: "Bereit".into(),
    }));
    let on_status = {
        let send = send.clone();
        let last = last.clone();
        Arc::new(move |s: &Status| {
            *last.lock().unwrap() = s.clone();
            send(json!({ "method": "status", "params": s }));
        })
    };

    // Das Gerät gleich beim Start anlegen: libinput braucht einen Moment, bis
    // es ein neues Gerät erkennt, und so ist der erste Klick nicht verloren.
    let mut clicker: Option<Clicker> = None;
    let mut device_error: Option<String> = None;
    match Mouse::new() {
        Ok(m) => clicker = Some(Clicker::new(m, on_status.clone())),
        Err(e) => device_error = Some(e.to_string()),
    }
    let mut cfg = Config::default();
    on_status(&Status {
        running: false,
        clicks: 0,
        text: device_error.clone().unwrap_or_else(|| "Bereit".into()),
    });

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

        let result: Result<Value, String> = match method {
            "shutdown" => break,
            "configure" => serde_json::from_value::<Config>(params)
                .map(|c| {
                    cfg = c;
                    json!(true)
                })
                .map_err(|e| format!("Ungültige Einstellungen: {e}")),
            "status" => Ok(json!(*last.lock().unwrap())),
            "action" => {
                let action = params.get("id").and_then(Value::as_str).unwrap_or_default();
                if action == "setup" {
                    match setup_access().and_then(|_| Mouse::new()) {
                        Ok(m) => {
                            clicker = Some(Clicker::new(m, on_status.clone()));
                            device_error = None;
                            on_status(&Status {
                                running: false,
                                clicks: 0,
                                text: "Zugriff eingerichtet – bereit".into(),
                            });
                            Ok(json!(true))
                        }
                        Err(e) => Err(e.to_string()),
                    }
                } else if let Some(c) = clicker.as_mut() {
                    match action {
                        "start" => c.start(&cfg, true),
                        "toggle" => c.toggle(&cfg, false),
                        "stop" => c.stop(),
                        _ => {}
                    }
                    if matches!(action, "start" | "toggle" | "stop") {
                        Ok(json!(c.is_running()))
                    } else {
                        Err(format!("Unbekannte Aktion {action:?}"))
                    }
                } else {
                    Err(device_error.clone().unwrap_or_default())
                }
            }
            other => Err(format!("Unbekannte Methode {other:?}")),
        };

        if let Some(id) = id {
            send(match result {
                Ok(r) => json!({ "id": id, "result": r }),
                Err(e) => json!({ "id": id, "error": { "message": e } }),
            });
        }
    }
    // stdin zu (Hub beendet) → Klicken sofort stoppen.
    drop(clicker);
    Ok(())
}
