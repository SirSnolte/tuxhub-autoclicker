use clap::{Parser, Subcommand};
use std::sync::{Arc, Mutex};
use tuxhub_autoclicker::clicker::{Clicker, Config, Mode, Status};
use tuxhub_autoclicker::device::{Button, Mouse, UDEV_RULE_PATH, setup_access};

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
