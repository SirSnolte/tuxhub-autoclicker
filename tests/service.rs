//! The background service, tuxhub mode and the installation, run as separate
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
        self.runtime.join("tuxhub-autoclicker.sock")
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
fn tuxhub_mode_forwards_requests_and_reports_the_state() {
    let env = Env::new("proxy");
    let mut child = env
        .command(Path::new(EXE))
        .arg("--tuxhub")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel::<Value>();
    let stdout = child.stdout.take().unwrap();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            let _ = tx.send(serde_json::from_str(&line).unwrap());
        }
    });
    let mut proxy = Guard(child);
    let mut seen = Vec::new();
    let mut wait = |pred: &dyn Fn(&Value) -> bool| -> Value {
        if let Some(i) = seen.iter().position(pred) {
            return seen.remove(i);
        }
        loop {
            let msg = rx
                .recv_timeout(Duration::from_secs(5))
                .expect("no message from tuxhub mode");
            if pred(&msg) {
                return msg;
            }
            seen.push(msg);
        }
    };

    // tuxhub shows the state and registers the hotkeys right away.
    let status = wait(&|m| m["method"] == "status");
    assert!(status["params"]["phase"].is_string());
    let shortcuts = wait(&|m| m["method"] == "shortcuts");
    assert!(shortcuts["params"].is_array());

    for (id, method) in [(1, "configure"), (2, "config.get"), (3, "macros.list")] {
        writeln!(
            stdin,
            "{}",
            json!({ "id": id, "method": method, "params": {} })
        )
        .unwrap();
    }
    assert_eq!(wait(&|m| m["id"] == 1)["result"], true);
    assert_eq!(wait(&|m| m["id"] == 2)["result"]["interval_ms"], 100);
    assert!(wait(&|m| m["id"] == 3)["result"].is_array());
    assert!(env.socket().exists(), "requests go through the service");

    // tuxhub exits: the process ends, and the service with it.
    drop(stdin);
    assert!(proxy.exits_within(Duration::from_secs(3)));
    let deadline = Instant::now() + Duration::from_secs(6);
    while env.socket().exists() {
        assert!(Instant::now() < deadline, "the service keeps running");
        std::thread::sleep(Duration::from_millis(50));
    }
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
