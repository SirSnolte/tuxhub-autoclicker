//! tuxhub mode: forwards JSON-RPC between tuxhub (standard input and output)
//! and the background service.
//!
//! When tuxhub exits, this process ends. The service stops everything once no
//! window is connected either. If the service restarts, the connection is
//! restored transparently.

use crate::client;
use serde_json::{Value, json};
use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const STATUS_ID: &str = "tuxhub-proxy-status";
const SHORTCUTS_ID: &str = "tuxhub-proxy-shortcuts";

type Shared = Arc<Mutex<Option<UnixStream>>>;

pub fn serve() -> io::Result<()> {
    let socket: Shared = Arc::new(Mutex::new(None));
    let stdout = Arc::new(Mutex::new(io::stdout()));
    connect(&socket)?;
    {
        let (socket, stdout) = (socket.clone(), stdout.clone());
        std::thread::spawn(move || forward_replies(&socket, &stdout));
    }

    for line in io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        let Ok(msg) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if msg.get("method").and_then(Value::as_str) == Some("shutdown") {
            break;
        }
        let delivered = send(&socket, &line).is_ok()
            || (connect(&socket).is_ok() && send(&socket, &line).is_ok());
        if !delivered && let Some(id) = msg.get("id") {
            let reply = json!({ "id": id, "error": { "message": "the Autoclicker service is not available" } });
            let _ = writeln!(stdout.lock().unwrap(), "{reply}");
        }
    }
    Ok(())
}

fn connect(socket: &Shared) -> io::Result<()> {
    let stream = client::connect_or_start()?;
    // Report the current state and the runtime hotkeys right away, as tuxhub
    // shows the state on its pages and registers the hotkeys.
    let mut w = &stream;
    writeln!(w, "{}", json!({ "id": STATUS_ID, "method": "status" }))?;
    writeln!(
        w,
        "{}",
        json!({ "id": SHORTCUTS_ID, "method": "shortcuts" })
    )?;
    *socket.lock().unwrap() = Some(stream);
    Ok(())
}

fn send(socket: &Shared, line: &str) -> io::Result<()> {
    let guard = socket.lock().unwrap();
    let mut s = guard
        .as_ref()
        .ok_or_else(|| io::Error::from(io::ErrorKind::NotConnected))?;
    writeln!(s, "{line}")
}

fn forward_replies(socket: &Shared, stdout: &Mutex<io::Stdout>) {
    let mut failures = 0;
    loop {
        let reader = socket
            .lock()
            .unwrap()
            .as_ref()
            .and_then(|s| s.try_clone().ok());
        if let Some(reader) = reader {
            failures = 0;
            for line in BufReader::new(reader).lines() {
                let Ok(line) = line else { break };
                let out = match serde_json::from_str::<Value>(&line) {
                    Ok(msg) if msg.get("id") == Some(&json!(STATUS_ID)) => {
                        json!({ "method": "status", "params": msg.get("result") }).to_string()
                    }
                    Ok(msg) if msg.get("id") == Some(&json!(SHORTCUTS_ID)) => {
                        json!({ "method": "shortcuts", "params": msg.get("result") }).to_string()
                    }
                    _ => line,
                };
                let mut o = stdout.lock().unwrap();
                if writeln!(o, "{out}").and_then(|()| o.flush()).is_err() {
                    std::process::exit(0);
                }
            }
        }
        // The service went away: reconnect, starting it again if needed.
        *socket.lock().unwrap() = None;
        std::thread::sleep(Duration::from_millis(300));
        if connect(socket).is_err() {
            failures += 1;
            if failures >= 10 {
                eprintln!("the Autoclicker service is not available");
                std::process::exit(1);
            }
        }
    }
}
