use clap::{Parser, Subcommand};
use std::sync::{Arc, Mutex};
use tuxhub_autoclicker::clicker::{Clicker, Config, Mode, Status};
use tuxhub_autoclicker::device::{Button, Mouse, UDEV_RULE_PATH, setup_access};
use tuxhub_autoclicker::macros::{self, Macro, Store};
use tuxhub_autoclicker::player::{Keyboard, PlayStatus, Player};
use tuxhub_autoclicker::recorder::Recorder;

/// Autoclicker für Linux – funktioniert unter Wayland und X11.
#[derive(Parser)]
#[command(version, about)]
struct Cli {
    /// Hub-Modus: JSON-RPC über stdin/stdout (wird von tuxhub genutzt)
    #[arg(long, hide = true)]
    tuxhub: bool,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Klicken, bis Strg+C gedrückt wird oder das Limit erreicht ist
    Click {
        /// Zeit zwischen zwei Klicks in Millisekunden
        #[arg(short, long, default_value_t = 100)]
        interval: u64,
        #[arg(short, long, value_enum, default_value_t = Button::Left)]
        button: Button,
        #[arg(short, long, value_enum, default_value_t = Mode::Click)]
        mode: Mode,
        /// Anzahl Klicks, 0 = unbegrenzt
        #[arg(short = 'n', long, default_value_t = 0)]
        count: u64,
        /// Zufällige Abweichung des Intervalls in Prozent
        #[arg(short, long, default_value_t = 0)]
        jitter: u32,
        /// Sekunden bis zum Start
        #[arg(short, long, default_value_t = 3)]
        delay: u64,
    },
    /// Makro aufnehmen, bis Strg+C gedrückt wird
    Record {
        /// Name des Makros
        name: String,
        /// Mausbewegungen nicht aufnehmen (nur Tasten, Klicks und Mausrad)
        #[arg(long)]
        no_moves: bool,
    },
    /// Makro abspielen
    Play {
        /// ID oder Name des Makros
        name: String,
        /// Wiederholungen (0 = endlos), sonst Wert aus dem Makro
        #[arg(short, long)]
        repeat: Option<u32>,
        /// Geschwindigkeit (1.0 = original), sonst Wert aus dem Makro
        #[arg(short, long)]
        speed: Option<f64>,
        /// Sekunden bis zum Start
        #[arg(short, long, default_value_t = 3)]
        delay: u64,
    },
    /// Gespeicherte Makros auflisten
    Macros,
    /// Zugriff auf /dev/uinput einrichten (fragt nach dem Passwort)
    Setup,
}

fn main() {
    let cli = Cli::parse();
    let result = if cli.tuxhub {
        tuxhub_autoclicker::rpc::serve()
    } else {
        match cli.command {
            Some(Command::Click {
                interval,
                button,
                mode,
                count,
                jitter,
                delay,
            }) => click(Config {
                interval_ms: interval,
                jitter_pct: jitter,
                button,
                mode,
                limit: count,
                start_delay_s: delay,
            }),
            Some(Command::Record { name, no_moves }) => record(&name, !no_moves),
            Some(Command::Play {
                name,
                repeat,
                speed,
                delay,
            }) => play(&name, repeat, speed, delay),
            Some(Command::Macros) => list_macros(),
            Some(Command::Setup) => {
                setup_access().map(|()| println!("Fertig: {UDEV_RULE_PATH} angelegt."))
            }
            None => {
                use clap::CommandFactory;
                Cli::command().print_help().map(|()| println!())
            }
        }
    };
    if let Err(e) = result {
        eprintln!("Fehler: {e}");
        std::process::exit(1);
    }
}

fn click(cfg: Config) -> std::io::Result<()> {
    let mouse = Mouse::new()?;
    let done = Arc::new((Mutex::new(false), std::sync::Condvar::new()));
    let on_status = {
        let done = done.clone();
        Arc::new(move |s: &Status| {
            eprint!("\r\x1b[K{}", s.text);
            if !s.running {
                eprintln!();
                *done.0.lock().unwrap() = true;
                done.1.notify_all();
            }
        })
    };
    let clicker = Arc::new(Mutex::new(Clicker::new(mouse, on_status)));
    {
        let clicker = clicker.clone();
        ctrlc::set_handler(move || clicker.lock().unwrap().stop())
            .map_err(std::io::Error::other)?;
    }
    eprintln!("Strg+C zum Stoppen.");
    clicker.lock().unwrap().start(&cfg, true);
    let guard = done.0.lock().unwrap();
    let _guard = done.1.wait_while(guard, |d| !*d).unwrap();
    Ok(())
}

fn store() -> std::io::Result<Store> {
    Store::open()
}

fn record(name: &str, moves: bool) -> std::io::Result<()> {
    let rec = Recorder::start(moves)?;
    eprintln!("Nimmt auf von: {}", rec.devices.join(", "));
    eprintln!("Strg+C beendet die Aufnahme.");
    let (tx, rx) = std::sync::mpsc::channel();
    ctrlc::set_handler(move || {
        let _ = tx.send(());
    })
    .map_err(std::io::Error::other)?;
    let _ = rx.recv();
    let steps = macros::to_steps(&rec.finish());
    if steps.is_empty() {
        return Err(std::io::Error::other("Nichts aufgenommen"));
    }
    let store = store()?;
    let m = store
        .save(Macro {
            id: store.new_id(name),
            name: name.into(),
            steps,
            repeat: 1,
            speed: 1.0,
            shortcut: None,
        })
        .map_err(std::io::Error::other)?;
    eprintln!(
        "\nGespeichert als „{}“ (ID {}, {} Schritte, {:.1} s).",
        m.name,
        m.id,
        m.steps.len(),
        m.duration_ms() as f64 / 1000.0
    );
    Ok(())
}

fn find_macro(store: &Store, name: &str) -> std::io::Result<Macro> {
    store
        .load(name)
        .ok()
        .or_else(|| {
            store
                .list()
                .into_iter()
                .find(|m| m.name.eq_ignore_ascii_case(name))
        })
        .ok_or_else(|| std::io::Error::other(format!("Makro {name:?} nicht gefunden")))
}

fn play(name: &str, repeat: Option<u32>, speed: Option<f64>, delay: u64) -> std::io::Result<()> {
    let mut m = find_macro(&store()?, name)?;
    if let Some(r) = repeat {
        m.repeat = r;
    }
    if let Some(s) = speed {
        m.speed = s;
    }
    m.validate().map_err(std::io::Error::other)?;
    let (tx, rx) = std::sync::mpsc::channel::<bool>();
    let done = tx.clone();
    let player = Arc::new(Mutex::new(Player::new(
        Keyboard::new()?,
        Arc::new(move |s: &PlayStatus| {
            eprint!("\r\x1b[K{}", s.text);
            if s.playing.is_none() {
                eprintln!();
                let _ = done.send(true);
            }
        }),
    )));
    {
        let player = player.clone();
        ctrlc::set_handler(move || {
            player.lock().unwrap().stop();
            let _ = tx.send(true);
        })
        .map_err(std::io::Error::other)?;
    }
    for left in (1..=delay).rev() {
        eprint!("\r\x1b[KStartet in {left} s …");
        if rx.recv_timeout(std::time::Duration::from_secs(1)).is_ok() {
            eprintln!();
            return Ok(());
        }
    }
    player.lock().unwrap().play(m);
    let _ = rx.recv();
    Ok(())
}

fn list_macros() -> std::io::Result<()> {
    let list = store()?.list();
    if list.is_empty() {
        println!("Noch keine Makros. Aufnehmen mit: tuxhub-autoclicker record <name>");
    }
    for m in list {
        println!(
            "{:<24} {:<20} {:>5} Schritte {:>7.1} s{}",
            m.id,
            m.name,
            m.steps.len(),
            m.duration_ms() as f64 / 1000.0,
            m.shortcut.map(|s| format!("  [{s}]")).unwrap_or_default()
        );
    }
    Ok(())
}
