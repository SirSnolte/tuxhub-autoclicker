use clap::{Parser, Subcommand};
use std::sync::{Arc, Mutex};
use tuxly_autoclicker::clicker::{Clicker, Config, Mode, Status};
use tuxly_autoclicker::device::{Button, Mouse, UDEV_RULE_PATH, setup_access};
use tuxly_autoclicker::macros::{self, Macro, Store};
use tuxly_autoclicker::player::{Keyboard, PlayStatus, Player};
use tuxly_autoclicker::recorder::Recorder;

/// Autoclicker and input macros for Linux. Works on Wayland and X11.
#[derive(Parser)]
#[command(version, about)]
struct Cli {
    /// tuxly mode: JSON-RPC over standard input and output (used by tuxly)
    #[arg(long, hide = true)]
    tuxly: bool,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Click until Ctrl+C is pressed or the limit is reached
    Click {
        /// Time between two clicks in milliseconds
        #[arg(short, long, default_value_t = 100)]
        interval: u64,
        #[arg(short, long, value_enum, default_value_t = Button::Left)]
        button: Button,
        #[arg(short, long, value_enum, default_value_t = Mode::Click)]
        mode: Mode,
        /// Number of clicks, 0 = unlimited
        #[arg(short = 'n', long, default_value_t = 0)]
        count: u64,
        /// Random variation of the interval in percent
        #[arg(short, long, default_value_t = 0)]
        jitter: u32,
        /// Seconds before starting
        #[arg(short, long, default_value_t = 3)]
        delay: u64,
    },
    /// Record a macro until Ctrl+C is pressed
    Record {
        /// Name of the macro
        name: String,
        /// Do not record mouse movements (keys, clicks and wheel only)
        #[arg(long)]
        no_moves: bool,
    },
    /// Play a macro
    Play {
        /// Id or name of the macro
        name: String,
        /// Repetitions (0 = endless); defaults to the value stored in the macro
        #[arg(short, long)]
        repeat: Option<u32>,
        /// Speed (1.0 = original); defaults to the value stored in the macro
        #[arg(short, long)]
        speed: Option<f64>,
        /// Seconds before starting
        #[arg(short, long, default_value_t = 3)]
        delay: u64,
    },
    /// List saved macros
    Macros,
    /// Grant access to /dev/uinput (asks for the password)
    Setup,
}

fn main() {
    let cli = Cli::parse();
    let result = if cli.tuxly {
        tuxly_autoclicker::rpc::serve()
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
                setup_access().map(|()| println!("Done: created {UDEV_RULE_PATH}."))
            }
            None => {
                use clap::CommandFactory;
                Cli::command().print_help().map(|()| println!())
            }
        }
    };
    if let Err(e) = result {
        eprintln!("Error: {e}");
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
    eprintln!("Press Ctrl+C to stop.");
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
    eprintln!("Recording from: {}", rec.devices.join(", "));
    eprintln!("Press Ctrl+C to stop recording.");
    let (tx, rx) = std::sync::mpsc::channel();
    ctrlc::set_handler(move || {
        let _ = tx.send(());
    })
    .map_err(std::io::Error::other)?;
    let _ = rx.recv();
    let steps = macros::to_steps(&rec.finish());
    if steps.is_empty() {
        return Err(std::io::Error::other("nothing was recorded"));
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
        "\nSaved as \"{}\" (id {}, {} steps, {:.1} s).",
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
        .ok_or_else(|| std::io::Error::other(format!("macro {name:?} not found")))
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
        eprint!("\r\x1b[KStarting in {left} s");
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
        println!("No macros yet. Record one with: tuxly-autoclicker record <name>");
    }
    for m in list {
        println!(
            "{:<24} {:<20} {:>5} steps {:>7.1} s{}",
            m.id,
            m.name,
            m.steps.len(),
            m.duration_ms() as f64 / 1000.0,
            m.shortcut.map(|s| format!("  [{s}]")).unwrap_or_default()
        );
    }
    Ok(())
}
