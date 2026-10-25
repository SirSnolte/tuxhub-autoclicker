//! The background service, its live interface and the installation, run as separate
//! processes with their own XDG directories, so nothing touches the real
//! session. No test starts clicking: the clicker only counts down.

use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

const EXE: &str = env!("CARGO_BIN_EXE_tuxhub-autoclicker");
const APP_ID: &str = "io.github.sirsnolte.TuxhubAutoclicker";

/// Isolated home, XDG directories and a short runtime directory (socket
/// paths are limited to 108 bytes).
struct Env {
    root: PathBuf,
    runtime: PathBuf,
}

impl Env {
    fn new(name: &str) -> Self {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let tag = format!("{}-{n}", std::process::id());
        let root = std::env::temp_dir().join(format!("tuxhub-autoclicker-{name}-{tag}"));
        let user_run = PathBuf::from(format!("/run/user/{}", unsafe { libc::getuid() }));
        let runtime = if user_run.is_dir() {
            user_run.join(format!("tac-{tag}"))
        } else {
            root.join("run")
        };
        for dir in [&root, &runtime] {
            let _ = std::fs::remove_dir_all(dir);
            std::fs::create_dir_all(dir).unwrap();
        }
        Self { root, runtime }
    }

    fn home(&self) -> PathBuf {
        self.root.join("home")
    }
    fn config(&self) -> PathBuf {
        self.root.join("config")
    }
    fn data(&self) -> PathBuf {
        self.root.join("data")
    }
    fn socket(&self) -> PathBuf {
        self.runtime.join("tuxhub/autoclicker.sock")
    }

    fn command(&self, exe: &Path) -> Command {
        let mut cmd = Command::new(exe);
        cmd.env("HOME", self.home())
            .env("XDG_CONFIG_HOME", self.config())
            .env("XDG_DATA_HOME", self.data())
            .env("XDG_RUNTIME_DIR", &self.runtime)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        cmd
    }

    /// Starts the service in the foreground.
    fn daemon(&self) -> Guard {
        let child = self.command(Path::new(EXE)).arg("daemon").spawn().unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while UnixStream::connect(self.socket()).is_err() {
            assert!(Instant::now() < deadline, "the service did not start");
            std::thread::sleep(Duration::from_millis(20));
        }
        Guard(child)
    }

    fn connect(&self) -> Conn {
        let stream = UnixStream::connect(self.socket()).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        Conn {
            reader: BufReader::new(stream.try_clone().unwrap()),
            stream,
            next: 0,
            notes: Vec::new(),
        }
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.runtime);
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// Kills a process that a failed test left running.
struct Guard(Child);

impl Guard {
    fn exits_within(&mut self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if self.0.try_wait().unwrap().is_some() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        false
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// A connection to the service that keeps the notifications it skips.
struct Conn {
    stream: UnixStream,
    reader: BufReader<UnixStream>,
    next: u64,
    notes: Vec<Value>,
}

impl Conn {
    /// The next line, or `None` once the connection is closed.
    fn try_read(&mut self) -> Option<Value> {
        let mut line = String::new();
        match self.reader.read_line(&mut line) {
            Ok(0) | Err(_) => None,
            Ok(_) => Some(serde_json::from_str(&line).unwrap()),
        }
    }

    /// The error object of a refused request.
    fn refused(&mut self, method: &str, params: Value) -> Value {
        self.next += 1;
        let id = self.next;
        writeln!(
            self.stream,
            "{}",
            json!({ "id": id, "method": method, "params": params })
        )
        .unwrap();
        loop {
            let msg = self.read();
            if msg["id"] == json!(id) {
                return msg.get("error").cloned().expect("the request was accepted");
            }
            self.notes.push(msg);
        }
    }

    fn hello(&mut self) -> Value {
        self.call(
            "hello",
            json!({ "protocol": 1, "client": "test", "locale": "en" }),
        )
        .unwrap()
    }

    /// Notifications named `method` received within `wait`, including
    /// skipped ones.
    fn notes(&mut self, method: &str, wait: Duration) -> Vec<Value> {
        self.stream.set_read_timeout(Some(wait)).unwrap();
        while let Some(msg) = self.try_read() {
            self.notes.push(msg);
        }
        self.stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        self.notes
            .iter()
            .filter(|n| n["method"] == method)
            .map(|n| n["params"].clone())
            .collect()
    }

    fn read(&mut self) -> Value {
        let mut line = String::new();
        assert!(
            self.reader.read_line(&mut line).unwrap() > 0,
            "connection closed"
        );
        serde_json::from_str(&line).unwrap()
    }

    fn call(&mut self, method: &str, params: Value) -> Result<Value, String> {
        self.next += 1;
        let id = self.next;
        writeln!(
            self.stream,
            "{}",
            json!({ "id": id, "method": method, "params": params })
        )
        .unwrap();
        loop {
            let msg = self.read();
            if msg["id"] == json!(id) {
                return match msg.get("error") {
                    Some(e) => Err(e["message"].as_str().unwrap_or_default().to_string()),
                    None => Ok(msg["result"].clone()),
                };
            }
            self.notes.push(msg);
        }
    }

    /// Waits for the next notification `method`, including skipped ones.
    fn notification(&mut self, method: &str) -> Value {
        if let Some(i) = self.notes.iter().position(|m| m["method"] == method) {
            return self.notes.remove(i)["params"].clone();
        }
        loop {
            let msg = self.read();
            if msg["method"] == method {
                return msg["params"].clone();
            }
        }
    }
}

#[test]
fn settings_are_shared_validated_and_saved() {
    let env = Env::new("settings");
    let _service = env.daemon();
    let mut a = env.connect();
    let mut b = env.connect();

    let cfg = a.call("config.get", Value::Null).unwrap();
    assert_eq!(cfg["interval_ms"], 100);
    assert_eq!(cfg["record_moves"], true);
    assert_eq!(cfg["stop_shortcut"], "");

    let cfg = a
        .call(
            "config.set",
            json!({ "interval_ms": 250, "mode": "double" }),
        )
        .unwrap();
    assert_eq!(cfg["interval_ms"], 250);
    assert_eq!(cfg["button"], "left", "unchanged fields stay");
    let seen = b.notification("config");
    assert_eq!(seen["interval_ms"], 250);
    assert_eq!(seen["mode"], "double");

    let saved: Value = serde_json::from_str(
        &std::fs::read_to_string(env.config().join("tuxhub-autoclicker/config.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(saved["interval_ms"], 250);

    for bad in [
        json!({ "interval_ms": 0 }),
        json!({ "jitter_pct": 51 }),
        json!({ "start_delay_s": 31 }),
        json!({ "mode": "triple" }),
        json!({ "unknown": 1 }),
        // A typing key as hotkey would fire while chatting.
        json!({ "stop_shortcut": "a" }),
        json!("not an object"),
    ] {
        assert!(a.call("config.set", bad.clone()).is_err(), "accepted {bad}");
    }
    assert_eq!(
        a.call("config.get", Value::Null).unwrap()["interval_ms"],
        250
    );

    a.call("config.set", json!({ "stop_shortcut": "CTRL+F7" }))
        .unwrap();
    let shortcuts = b.notification("shortcuts");
    assert_eq!(shortcuts[0]["id"], "stop");
    assert_eq!(shortcuts[0]["shortcut"], "CTRL+F7");
    assert_eq!(a.call("shortcuts", Value::Null).unwrap(), shortcuts);
}

#[test]
fn settings_survive_a_restart() {
    let env = Env::new("restart");
    {
        let mut service = env.daemon();
        let mut a = env.connect();
        a.call("config.set", json!({ "limit": 77 })).unwrap();
        a.call("quit", Value::Null).unwrap();
        assert!(service.exits_within(Duration::from_secs(3)));
    }
    let _service = env.daemon();
    let mut a = env.connect();
    assert_eq!(a.call("config.get", Value::Null).unwrap()["limit"], 77);
}

#[test]
fn imports_the_settings_tuxhub_stored_once() {
    let env = Env::new("import");
    let hub = env.config().join("io.github.sirsnolte.tuxhub/settings");
    std::fs::create_dir_all(&hub).unwrap();
    std::fs::write(
        hub.join("autoclicker.json"),
        r#"{"interval_ms": 42, "mode": "hold", "stop_shortcut": "F7", "something_else": 1}"#,
    )
    .unwrap();
    let _service = env.daemon();
    let mut a = env.connect();
    let cfg = a.call("config.get", Value::Null).unwrap();
    assert_eq!(cfg["interval_ms"], 42);
    assert_eq!(cfg["mode"], "hold");
    assert_eq!(cfg["stop_shortcut"], "F7");
    assert!(
        env.config()
            .join("tuxhub-autoclicker/config.json")
            .is_file(),
        "the import is saved, so it happens only once"
    );
}

#[test]
fn a_second_instance_ends_at_once() {
    let env = Env::new("second");
    let _service = env.daemon();
    let out = env.command(Path::new(EXE)).arg("daemon").output().unwrap();
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("already running"));
    // Edge case 1: the socket of the running service stays.
    assert_eq!(env.connect().hello()["protocol"], 1);
}

#[test]
fn exits_when_nobody_connects() {
    let env = Env::new("unused");
    let mut service = env.daemon();
    assert!(service.exits_within(Duration::from_secs(6)));
    assert!(!env.socket().exists());
}

#[test]
fn stops_and_exits_when_the_last_client_disconnects() {
    let env = Env::new("last");
    let mut service = env.daemon();
    let mut a = env.connect();
    let mut b = env.connect();
    let devices = a.call("status", Value::Null).unwrap()["phase"] != "error";
    if devices {
        // A long start delay: the clicker counts down and never clicks.
        a.call("config.set", json!({ "start_delay_s": 30 }))
            .unwrap();
        assert_eq!(a.call("action", json!({ "id": "start" })).unwrap(), true);
        assert_eq!(b.notification("status")["phase"], "countdown");
    } else {
        eprintln!("no access to /dev/uinput: only checking the exit");
    }

    drop(a);
    std::thread::sleep(Duration::from_millis(300));
    let status = b.call("status", Value::Null).unwrap();
    if devices {
        assert_eq!(status["running"], true, "another client is still connected");
    }

    drop(b);
    assert!(
        service.exits_within(Duration::from_secs(6)),
        "the service keeps running without clients"
    );
    assert!(!env.socket().exists());
}

#[test]
fn answers_the_live_protocol() {
    let env = Env::new("protocol");
    let _service = env.daemon();
    let mut c = env.connect();
    let hello = c.hello();
    assert_eq!(hello["protocol"], 1);
    assert_eq!(hello["name"], "Autoclicker");
    assert_eq!(hello["version"], env!("CARGO_PKG_VERSION"));
    // Right after `hello` the runtime hotkeys, for tuxhub.
    assert!(c.notes("shortcuts", Duration::from_millis(300))[0].is_array());
    assert!(c.call("hello", json!({ "protocol": 2 })).is_err());

    let de = c.call("schema", json!({ "locale": "de" })).unwrap();
    assert_eq!(de["sections"][0]["label"], "Klicken");
    let fr = c.call("schema", json!({ "locale": "fr" })).unwrap();
    assert_eq!(fr["sections"][0]["label"], "Clicking");
    let values = c.call("get", json!({})).unwrap();
    assert_eq!(
        values,
        json!({ "values": {
            "interval_ms": 100, "jitter_pct": 0, "button": "left", "mode": "click",
            "limit": 0, "start_delay_s": 3, "record_moves": true
        } })
    );
    // The tool's own methods are unchanged; `configure` is gone.
    assert!(c.call("macros.list", json!({})).unwrap().is_array());
    assert!(c.call("configure", json!({})).is_err());
}

#[test]
fn socket_and_folder_are_private() {
    use std::os::unix::fs::PermissionsExt;
    let env = Env::new("modes");
    let _service = env.daemon();
    let _c = env.connect();
    let dir = std::fs::metadata(env.socket().parent().unwrap()).unwrap();
    let sock = std::fs::metadata(env.socket()).unwrap();
    assert_eq!(dir.permissions().mode() & 0o777, 0o700);
    assert_eq!(sock.permissions().mode() & 0o777, 0o600);
}

/// Edge case 2: a socket file that nobody listens on is replaced.
#[test]
fn a_dead_socket_is_replaced() {
    let env = Env::new("dead");
    std::fs::create_dir_all(env.socket().parent().unwrap()).unwrap();
    drop(std::os::unix::net::UnixListener::bind(env.socket()).unwrap());
    let _service = env.daemon();
    assert_eq!(env.connect().hello()["protocol"], 1);
}

/// Edge case 3: a regular file or a symbolic link in place of the socket is
/// never removed; the service refuses to start.
#[test]
fn foreign_files_at_the_socket_path_are_kept() {
    for kind in ["file", "symlink"] {
        let env = Env::new(kind);
        std::fs::create_dir_all(env.socket().parent().unwrap()).unwrap();
        let target = env.root.join("target");
        std::fs::write(&target, "keep").unwrap();
        if kind == "file" {
            std::fs::write(env.socket(), "keep").unwrap();
        } else {
            std::os::unix::fs::symlink(&target, env.socket()).unwrap();
        }
        let out = env.command(Path::new(EXE)).arg("daemon").output().unwrap();
        assert!(!out.status.success(), "{kind}");
        let meta = std::fs::symlink_metadata(env.socket()).unwrap();
        assert_eq!(meta.file_type().is_symlink(), kind == "symlink");
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "keep");
    }
}

/// Edge case 4: refused values name the field and change nothing.
#[test]
fn set_refuses_invalid_values() {
    let env = Env::new("refuse");
    let _service = env.daemon();
    let mut c = env.connect();
    c.hello();
    let before = c.call("config.get", Value::Null).unwrap();
    for (key, value) in [
        ("stop_shortcut", json!("F7")),
        ("button", json!(1)),
        ("interval_ms", json!(0)),
    ] {
        let e = c.refused("set", json!({ "key": key, "value": value }));
        assert_eq!(e["field"], key, "{e}");
        assert!(e["message"].as_str().is_some_and(|m| !m.is_empty()));
    }
    assert_eq!(c.call("config.get", Value::Null).unwrap(), before);
    assert!(!env.config().join("tuxhub-autoclicker/config.json").exists());
}

/// Edge case 5: the connection that sets a value gets no `changed`, the
/// others do; the value is saved.
#[test]
fn set_notifies_the_other_connections() {
    let env = Env::new("set");
    let _service = env.daemon();
    let (mut a, mut b) = (env.connect(), env.connect());
    a.hello();
    b.hello();
    let applied = a
        .call("set", json!({ "key": "interval_ms", "value": 250 }))
        .unwrap();
    assert_eq!(applied, json!({ "value": 250 }));
    let changed = b.notes("changed", Duration::from_millis(500));
    assert_eq!(changed.len(), 1);
    assert_eq!(changed[0]["values"]["interval_ms"], 250);
    assert_eq!(b.notes("config", Duration::from_millis(10)).len(), 1);
    assert!(a.notes("changed", Duration::from_millis(300)).is_empty());
    let saved: Value = serde_json::from_str(
        &std::fs::read_to_string(env.config().join("tuxhub-autoclicker/config.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(saved["interval_ms"], 250);
}

/// Edge case 6: a change from the window reaches every live connection.
#[test]
fn config_set_notifies_every_connection() {
    let env = Env::new("window");
    let _service = env.daemon();
    let (mut window, mut hub) = (env.connect(), env.connect());
    hub.hello();
    window
        .call("config.set", json!({ "interval_ms": 500 }))
        .unwrap();
    let changed = hub.notes("changed", Duration::from_millis(500));
    assert_eq!(changed.len(), 1);
    assert_eq!(changed[0]["values"]["interval_ms"], 500);
    assert_eq!(window.notes("changed", Duration::from_millis(300)).len(), 1);
    // A change outside the schema sends no `changed`.
    window
        .call("config.set", json!({ "stop_shortcut": "F7" }))
        .unwrap();
    assert_eq!(hub.notes("changed", Duration::from_millis(300)).len(), 1);
}

/// Edge case 8: `quit` answers, ends the service and removes the socket.
#[test]
fn quit_removes_the_socket() {
    let env = Env::new("quit");
    let mut service = env.daemon();
    let mut c = env.connect();
    c.hello();
    assert_eq!(c.call("quit", json!({})).unwrap(), true);
    assert!(service.exits_within(Duration::from_secs(3)));
    assert!(!env.socket().exists());
}

/// Edge case 9: a line over 1 MiB, or one that is not a JSON object, closes
/// only that connection.
#[test]
fn invalid_lines_close_only_their_connection() {
    let env = Env::new("invalid");
    let _service = env.daemon();
    let mut other = env.connect();
    other.hello();
    for line in [
        format!(
            "{{\"id\":1,\"method\":\"status\",\"params\":\"{}\"}}",
            "x".repeat(1 << 20)
        ),
        "[1, 2]".to_string(),
        "not json".to_string(),
    ] {
        let mut bad = env.connect();
        let _ = bad.stream.write_all(format!("{line}\n").as_bytes());
        loop {
            match bad.try_read() {
                // Notifications may arrive before the connection closes.
                Some(msg) if msg.get("id").is_none() => continue,
                Some(msg) => panic!("answered {msg}"),
                None => break,
            }
        }
    }
    assert_eq!(
        other.call("get", json!({})).unwrap()["values"]["mode"],
        "click"
    );
}

/// Edge case 10: when the live connection of tuxhub, the last client, goes
/// away, clicking stops at once.
#[test]
fn clicking_stops_when_the_live_connection_goes() {
    let env = Env::new("live-gone");
    let _service = env.daemon();
    let mut hub = env.connect();
    hub.hello();
    if hub.call("status", Value::Null).unwrap()["phase"] == "error" {
        eprintln!("no access to /dev/uinput: nothing can click");
        return;
    }
    // A long start delay: the clicker counts down and never clicks.
    hub.call("set", json!({ "key": "start_delay_s", "value": 30 }))
        .unwrap();
    assert_eq!(hub.call("action", json!({ "id": "toggle" })).unwrap(), true);
    drop(hub);
    // Back within the idle time: the service still runs, the clicker does not.
    std::thread::sleep(Duration::from_millis(300));
    let mut window = env.connect();
    let status = window.call("status", Value::Null).unwrap();
    assert_eq!(status["running"], false, "{status}");
    assert_ne!(status["phase"], "countdown");
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

#[test]
fn installs_and_uninstalls_for_the_current_user() {
    let env = Env::new("install");
    // An extracted release folder.
    let release = env.root.join("release/tuxhub-autoclicker");
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    std::fs::create_dir_all(&release).unwrap();
    std::fs::copy(EXE, release.join("tuxhub-autoclicker")).unwrap();
    std::fs::copy(repo.join("tuxhub.toml"), release.join("tuxhub.toml")).unwrap();
    copy_dir(&repo.join("ui"), &release.join("ui"));
    copy_dir(&repo.join("assets"), &release.join("assets"));
    // Macros of an earlier use must survive the installation.
    let macros = env.data().join("tuxhub-autoclicker/macros");
    std::fs::create_dir_all(&macros).unwrap();
    std::fs::write(macros.join("keep.json"), "{}").unwrap();

    let out = env
        .command(&release.join("tuxhub-autoclicker"))
        .arg("install")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let dest = env.data().join("tuxhub-autoclicker/app");
    let exe = dest.join("tuxhub-autoclicker");
    assert!(exe.is_file());
    assert!(dest.join("ui/index.html").is_file());
    assert!(macros.join("keep.json").is_file());
    let launcher =
        std::fs::read_to_string(env.data().join(format!("applications/{APP_ID}.desktop"))).unwrap();
    assert!(launcher.contains(&format!("Exec=\"{}\"\n", exe.display())));
    assert!(launcher.contains(&format!("StartupWMClass={APP_ID}\n")));
    assert!(
        env.data()
            .join(format!("icons/hicolor/scalable/apps/{APP_ID}.svg"))
            .is_file()
    );
    let registration =
        std::fs::read_to_string(env.data().join("tuxhub/apps/autoclicker.toml")).unwrap();
    assert!(registration.contains("id = \"autoclicker\""));
    assert!(registration.contains(&format!("path = \"{}\"", dest.display())));
    assert_eq!(
        std::fs::read_link(env.home().join(".local/bin/tuxhub-autoclicker")).unwrap(),
        exe
    );
    assert!(
        !env.config().join("autostart").exists(),
        "the autoclicker never starts at login"
    );

    let out = env.command(&exe).arg("uninstall").output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(!dest.exists());
    assert!(
        !env.data()
            .join(format!("applications/{APP_ID}.desktop"))
            .exists()
    );
    assert!(!env.data().join("tuxhub/apps/autoclicker.toml").exists());
    assert!(!env.home().join(".local/bin/tuxhub-autoclicker").exists());
    assert!(
        macros.join("keep.json").is_file(),
        "only --purge removes macros"
    );
}
