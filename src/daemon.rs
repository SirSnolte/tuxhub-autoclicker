//! The background service. It owns the settings, the virtual input devices,
//! the clicker, the macro player and the recorder.
//!
//! Exactly one service runs per user. The window, tuxhub and the command line
//! talk to it over the Unix socket of the tuxhub live interface
//! (`$XDG_RUNTIME_DIR/tuxhub/autoclicker.sock`, one JSON object per line), so
//! all of them always show the same state.
//!
//! Automated input must never outlive its interface: when the last window or
//! tuxhub disconnects, the service stops everything at once and exits shortly
//! after.
//!
//! Methods:
//! - Live interface: `hello`, `schema`, `get`, `set`, `quit`
//! - `config.get`, `config.set`, `status`, `shortcuts`, `keys`
//! - `action` with `id`: `start`, `stop`, `toggle`, `setup`, `record`
//!   (start or stop recording), `panic` (stop everything), `macro:<id>`
//!   (play or stop a macro)
//! - `macros.list`, `macros.get`, `macros.save`, `macros.delete`,
//!   `macros.play`, `macros.stop`, `record.start`, `record.stop`
//!
//! Notifications: `status` (clicker), `macro-status`, `macros-changed`,
//! `config`, `changed` (the values of the live schema) and `shortcuts`
//! (runtime hotkeys for tuxhub, also right after `hello`).

use crate::clicker::{Clicker, Phase, Status};
use crate::config::Config;
use crate::device::{Mouse, setup_access};
use crate::live::{self, Failure};
use crate::macros::{self, Macro, Store};
use crate::player::{Keyboard, PlayStatus, Player};
use crate::recorder::Recorder;
use crate::{integration, paths};
use serde_json::{Value, json};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const LOCK_NAME: &str = "tuxhub-autoclicker.lock";
/// How long the service waits for a client before it exits, after the start
/// and after the last client disconnected.
const IDLE_EXIT: Duration = Duration::from_secs(2);
/// Longest line a client may send, without the line break.
const MAX_LINE: usize = 1 << 20;

pub fn socket_path() -> io::Result<PathBuf> {
    Ok(paths::runtime_dir()?
        .join("tuxhub")
        .join(format!("{}.sock", live::TOOL_ID)))
}

/// Listens on `path`: creates its folder with mode 0700, replaces a socket
/// that nobody listens on, and never removes anything else.
fn bind(path: &Path) -> io::Result<UnixListener> {
    let dir = path
        .parent()
        .ok_or_else(|| io::Error::other("invalid socket path"))?;
    match std::fs::symlink_metadata(dir) {
        Ok(m) if m.is_dir() => {}
        Ok(_) => {
            return Err(io::Error::other(format!(
                "{} is not a folder",
                dir.display()
            )));
        }
        Err(_) => {
            std::fs::create_dir_all(dir)?;
            std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
        }
    }
    match std::fs::symlink_metadata(path) {
        Ok(m) if m.file_type().is_socket() => {
            if UnixStream::connect(path).is_ok() {
                return Err(io::Error::new(
                    io::ErrorKind::AddrInUse,
                    format!("{} is in use", path.display()),
                ));
            }
            std::fs::remove_file(path)?;
        }
        Ok(_) => {
            return Err(io::Error::other(format!(
                "{} exists and is not a socket; remove it to start the service",
                path.display()
            )));
        }
        Err(_) => {}
    }
    let server = UnixListener::bind(path)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    Ok(server)
}

enum Msg {
    Request {
        /// Connection the request came from.
        client: u64,
        method: String,
        params: Value,
        reply: Sender<Result<Value, Failure>>,
    },
    /// A client disconnected.
    Left,
    /// The device access setup finished.
    SetupDone(Result<(), String>),
    Terminate,
}

/// Runs the service until it is terminated or no client is left. Returns
/// `Ok(false)` if another instance is already running.
pub fn run() -> io::Result<bool> {
    let dir = paths::runtime_dir()?;
    std::fs::create_dir_all(&dir)?;
    let Some(lock) = try_lock(&dir.join(LOCK_NAME))? else {
        return Ok(false);
    };
    let socket = socket_path()?;
    let server = bind(&socket)?;

    let (tx, rx) = std::sync::mpsc::channel();
    {
        let tx = tx.clone();
        ctrlc::set_handler(move || {
            let _ = tx.send(Msg::Terminate);
        })
        .map_err(io::Error::other)?;
    }

    if let Err(e) = integration::refresh() {
        eprintln!("desktop integration: {e}");
    }
    let clients = Clients::default();
    {
        let (clients, tx) = (clients.clone(), tx.clone());
        std::thread::Builder::new()
            .name("accept".into())
            .spawn(move || accept(server, clients, tx))?;
    }

    let mut service = Service::new(Config::load(), Store::open()?, clients, tx);
    service.run(rx);
    service.panic();
    let _ = std::fs::remove_file(&socket);
    drop(lock);
    Ok(true)
}

/// Takes an exclusive lock that is released when the process exits.
fn try_lock(path: &Path) -> io::Result<Option<std::fs::File>> {
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)?;
    let ok = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0;
    Ok(ok.then_some(file))
}

// ----------------------------------------------------------------- Clients

type Writer = Arc<Mutex<UnixStream>>;

#[derive(Clone, Default)]
struct Clients(Arc<Mutex<Vec<(u64, Writer)>>>);

impl Clients {
    fn add(&self, id: u64, w: Writer) {
        self.0.lock().unwrap().push((id, w));
    }

    fn remove(&self, id: u64) {
        self.0.lock().unwrap().retain(|(i, _)| *i != id);
    }

    fn is_empty(&self) -> bool {
        self.0.lock().unwrap().is_empty()
    }

    fn notify(&self, method: &str, params: Value) {
        self.notify_except(None, method, params);
    }

    /// Notifies every connection but `except`.
    fn notify_except(&self, except: Option<u64>, method: &str, params: Value) {
        let line = format!("{}\n", json!({ "method": method, "params": params }));
        self.0.lock().unwrap().retain(|(id, w)| {
            Some(*id) == except || w.lock().unwrap().write_all(line.as_bytes()).is_ok()
        });
    }

    /// Notifies connection `client` only.
    fn notify_one(&self, client: u64, method: &str, params: Value) {
        let line = format!("{}\n", json!({ "method": method, "params": params }));
        if let Some((_, w)) = self.0.lock().unwrap().iter().find(|(id, _)| *id == client) {
            let _ = w.lock().unwrap().write_all(line.as_bytes());
        }
    }
}

fn accept(server: UnixListener, clients: Clients, tx: Sender<Msg>) {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    for stream in server.incoming() {
        let Ok(stream) = stream else { continue };
        let (Ok(read), true) = (
            stream.try_clone(),
            stream
                .set_write_timeout(Some(Duration::from_secs(1)))
                .is_ok(),
        ) else {
            continue;
        };
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let writer: Writer = Arc::new(Mutex::new(stream));
        clients.add(id, writer.clone());
        let (clients, tx) = (clients.clone(), tx.clone());
        let _ = std::thread::Builder::new()
            .name(format!("client-{id}"))
            .spawn(move || {
                serve_client(id, read, &writer, &tx);
                clients.remove(id);
                // Wakes up a client that is still blocked on its socket.
                let _ = writer.lock().unwrap().shutdown(std::net::Shutdown::Both);
                let _ = tx.send(Msg::Left);
            });
    }
}

/// Reads one line of at most [`MAX_LINE`] bytes; `None` at the end, on an
/// error or for a longer line.
fn read_line(reader: &mut impl BufRead, buf: &mut Vec<u8>) -> Option<()> {
    buf.clear();
    let n = reader
        .take(MAX_LINE as u64 + 1)
        .read_until(b'\n', buf)
        .ok()?;
    if n == 0 || buf.last() != Some(&b'\n') {
        return None;
    }
    buf.pop();
    Some(())
}

/// Answers the requests of one connection. A line that is too long or not a
/// JSON object closes it.
fn serve_client(client: u64, read: UnixStream, writer: &Writer, tx: &Sender<Msg>) {
    let mut reader = BufReader::new(read);
    let mut buf = Vec::new();
    while read_line(&mut reader, &mut buf).is_some() {
        let Ok(msg @ Value::Object(_)) = serde_json::from_slice::<Value>(&buf) else {
            break;
        };
        let id = msg.get("id").cloned();
        let method = msg
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let params = msg.get("params").cloned().unwrap_or(Value::Null);
        let (rtx, rrx) = std::sync::mpsc::channel();
        if tx
            .send(Msg::Request {
                client,
                method,
                params,
                reply: rtx,
            })
            .is_err()
        {
            break;
        }
        let result = rrx
            .recv_timeout(Duration::from_secs(4))
            .unwrap_or_else(|_| Err("the service did not respond".into()));
        if let Some(id) = id {
            let reply = match result {
                Ok(r) => json!({ "id": id, "result": r }),
                Err(Failure {
                    message,
                    field: Some(field),
                }) => json!({ "id": id, "error": { "message": message, "field": field } }),
                Err(e) => json!({ "id": id, "error": { "message": e.message } }),
            };
            if writeln!(writer.lock().unwrap(), "{reply}").is_err() {
                break;
            }
        }
    }
}

// ----------------------------------------------------------------- Service

struct Service {
    cfg: Config,
    clients: Clients,
    tx: Sender<Msg>,
    clicker: Option<Clicker>,
    player: Option<Player>,
    recorder: Option<Recorder>,
    device_error: Option<String>,
    store: Store,
    last_status: Arc<Mutex<Status>>,
}

impl Service {
    fn new(cfg: Config, store: Store, clients: Clients, tx: Sender<Msg>) -> Self {
        let mut service = Self {
            cfg,
            clients,
            tx,
            clicker: None,
            player: None,
            recorder: None,
            device_error: None,
            store,
            last_status: Arc::new(Mutex::new(Status::new(Phase::Ready, 0, 0, None))),
        };
        // Create the devices right away: libinput needs a moment to pick up
        // new devices, and this way the first click is not lost.
        service.create_devices();
        service.clicker_status(match &service.device_error {
            Some(e) => Status::new(Phase::Error, 0, 0, Some(e.clone())),
            None => Status::new(Phase::Ready, 0, 0, None),
        });
        service
    }

    /// Processes messages until the service ends.
    fn run(&mut self, rx: Receiver<Msg>) {
        // Set while no client is connected.
        let mut idle_since = Some(Instant::now());
        loop {
            let timeout = idle_since.map_or(Duration::from_secs(3600), |t| {
                (t + IDLE_EXIT).saturating_duration_since(Instant::now())
            });
            match rx.recv_timeout(timeout) {
                Ok(Msg::Terminate) | Err(RecvTimeoutError::Disconnected) => return,
                Ok(Msg::Request {
                    client,
                    method,
                    params,
                    reply,
                }) => {
                    if method == "quit" {
                        let _ = reply.send(Ok(json!(true)));
                        return;
                    }
                    let hello = method == "hello";
                    let result = self.request(client, &method, params);
                    let greeted = hello && result.is_ok();
                    let _ = reply.send(result);
                    if greeted {
                        // tuxhub registers the runtime hotkeys right away.
                        self.clients
                            .notify_one(client, "shortcuts", self.shortcuts());
                    }
                }
                Ok(Msg::SetupDone(result)) => self.setup_done(result),
                Ok(Msg::Left) | Err(RecvTimeoutError::Timeout) => {}
            }
            match idle_since {
                _ if !self.clients.is_empty() => idle_since = None,
                None => {
                    // The last client left: nobody can stop the input anymore.
                    self.panic();
                    idle_since = Some(Instant::now());
                }
                Some(t) if t.elapsed() >= IDLE_EXIT => return,
                Some(_) => {}
            }
        }
    }

    fn create_devices(&mut self) {
        let on_status = {
            let clients = self.clients.clone();
            let last = self.last_status.clone();
            Arc::new(move |s: &Status| {
                *last.lock().unwrap() = s.clone();
                clients.notify("status", json!(s));
            })
        };
        let on_play = {
            let clients = self.clients.clone();
            Arc::new(move |s: &PlayStatus| {
                let mut params = json!(s);
                params["recording"] = json!(false);
                clients.notify("macro-status", params);
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
        self.clients.notify("status", json!(s));
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
        self.clients.notify(
            "macro-status",
            json!({
                "recording": phase == "recording",
                "playing": null,
                "phase": phase,
                "devices": devices,
                "name": saved.map(|m| m.name.clone()),
                "steps": saved.map(|m| m.steps.len()),
                "text": text,
            }),
        );
    }

    /// Runtime hotkeys for tuxhub: the stop key and the macro hotkeys.
    fn shortcuts(&self) -> Value {
        let stop = self.cfg.stop_shortcut.as_str();
        let stop = (!stop.is_empty())
            .then(|| json!({ "id": "stop", "label": "Stop clicking", "shortcut": stop }));
        let list: Vec<Value> = stop
            .into_iter()
            .chain(self.store.list().into_iter().filter_map(|m| {
                let s = m.shortcut?;
                Some(json!({ "id": format!("macro:{}", m.id), "label": format!("Macro: {}", m.name), "shortcut": s }))
            }))
            .collect();
        json!(list)
    }

    fn macros_changed(&self) {
        self.clients.notify("macros-changed", Value::Null);
        self.clients.notify("shortcuts", self.shortcuts());
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

    /// Saves and applies `cfg`. `origin` is the connection that set a value
    /// over the live interface; it gets no `changed` for it.
    fn set_config(&mut self, cfg: Config, origin: Option<u64>) -> Result<(), String> {
        if cfg == self.cfg {
            return Ok(());
        }
        cfg.save()
            .map_err(|e| format!("could not save the settings: {e}"))?;
        let old = std::mem::replace(&mut self.cfg, cfg);
        let values = live::values(&self.cfg);
        if live::values(&old) != values {
            self.clients
                .notify_except(origin, "changed", json!({ "values": values }));
        }
        self.clients.notify("config", json!(self.cfg));
        if old.stop_shortcut != self.cfg.stop_shortcut {
            self.clients.notify("shortcuts", self.shortcuts());
        }
        Ok(())
    }

    fn setup_done(&mut self, result: Result<(), String>) {
        let error = result.err().or_else(|| {
            self.create_devices();
            self.device_error.clone()
        });
        self.clicker_status(match error {
            Some(e) => Status::new(Phase::Error, 0, 0, Some(e)),
            None => Status::new(Phase::Ready, 0, 0, None),
        });
    }

    fn record_start(&mut self) -> Result<Value, String> {
        if self.recorder.is_some() {
            return Err("a recording is already running".into());
        }
        let rec = Recorder::start(self.cfg.record_moves).map_err(|e| e.to_string())?;
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

    fn request(&mut self, client: u64, method: &str, params: Value) -> Result<Value, Failure> {
        match method {
            "hello" => live::hello(&params),
            "schema" => Ok(live::schema(
                params.get("locale").and_then(Value::as_str).unwrap_or("en"),
            )),
            "get" => Ok(json!({ "values": live::values(&self.cfg) })),
            "set" => {
                let (cfg, value) = live::set(&self.cfg, &params)?;
                self.set_config(cfg, Some(client))?;
                Ok(json!({ "value": value }))
            }
            _ => Ok(self.tool_request(method, params)?),
        }
    }

    /// The tool's own methods.
    fn tool_request(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let str_param = |key: &str| params.get(key).and_then(Value::as_str).map(str::to_owned);
        match method {
            "config.get" => Ok(json!(self.cfg)),
            "config.set" => {
                let cfg = self.cfg.merged(params)?;
                self.set_config(cfg, None)?;
                Ok(json!(self.cfg))
            }
            "status" => Ok(json!(*self.last_status.lock().unwrap())),
            "shortcuts" => Ok(self.shortcuts()),
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
                // The password dialog can take a while; do not block the service.
                let tx = self.tx.clone();
                std::thread::spawn(move || {
                    let result = setup_access().map_err(|e| e.to_string());
                    let _ = tx.send(Msg::SetupDone(result));
                });
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
                    "start" => c.start(&self.cfg.clicker, true),
                    "toggle" => c.toggle(&self.cfg.clicker, false),
                    _ => c.stop(),
                }
                Ok(json!(c.is_running()))
            }
            other => Err(format!("unknown action {other:?}")),
        }
    }
}
