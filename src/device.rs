//! Virtuelle Maus über `/dev/uinput`.
//!
//! Der Kernel behandelt sie wie eine echte Maus – deshalb funktionieren die
//! Klicks unter Wayland, X11 und in jedem Spiel.

use evdev::uinput::VirtualDevice;
use evdev::{AttributeSet, EventType, InputEvent, KeyCode, RelativeAxisCode};
use serde::{Deserialize, Serialize};
use std::io;
use std::path::PathBuf;

pub const DEVICE_NAME: &str = "tuxhub-autoclicker";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Button {
    Left,
    Right,
    Middle,
}

impl Button {
    pub fn code(self) -> KeyCode {
        match self {
            Button::Left => KeyCode::BTN_LEFT,
            Button::Right => KeyCode::BTN_RIGHT,
            Button::Middle => KeyCode::BTN_MIDDLE,
        }
    }
}

pub struct Mouse {
    dev: VirtualDevice,
}

impl Mouse {
    pub fn new() -> io::Result<Self> {
        let keys =
            AttributeSet::from_iter([KeyCode::BTN_LEFT, KeyCode::BTN_RIGHT, KeyCode::BTN_MIDDLE]);
        // Ohne Bewegungsachsen erkennt libinput das Gerät nicht als Zeigegerät.
        let axes = AttributeSet::from_iter([
            RelativeAxisCode::REL_X,
            RelativeAxisCode::REL_Y,
            RelativeAxisCode::REL_WHEEL,
        ]);
        let dev = VirtualDevice::builder()
            .map_err(explain)?
            .name(DEVICE_NAME)
            .with_keys(&keys)?
            .with_relative_axes(&axes)?
            .build()
            .map_err(explain)?;
        Ok(Self { dev })
    }

    /// Pfade der `/dev/input/event*`-Knoten dieses Geräts.
    pub fn dev_nodes(&mut self) -> io::Result<Vec<PathBuf>> {
        self.dev.enumerate_dev_nodes_blocking()?.collect()
    }

    pub fn set(&mut self, button: Button, pressed: bool) -> io::Result<()> {
        self.dev.emit(&[InputEvent::new(
            EventType::KEY.0,
            button.code().code(),
            pressed as i32,
        )])
    }
}

fn explain(e: io::Error) -> io::Error {
    if e.kind() == io::ErrorKind::PermissionDenied || e.kind() == io::ErrorKind::NotFound {
        io::Error::new(
            e.kind(),
            format!(
                "Kein Zugriff auf /dev/uinput ({e}). Einmalig einrichten mit \
                 `tuxhub-autoclicker setup` bzw. der Aktion „Zugriff einrichten“ im Hub."
            ),
        )
    } else {
        e
    }
}

/// udev-Regel, die dem angemeldeten Nutzer Zugriff auf `/dev/uinput` gibt.
pub const UDEV_RULE_PATH: &str = "/etc/udev/rules.d/60-tuxhub-uinput.rules";
pub const UDEV_RULE: &str =
    "KERNEL==\"uinput\", SUBSYSTEM==\"misc\", TAG+=\"uaccess\", OPTIONS+=\"static_node=uinput\"\n";

/// Richtet die udev-Regel per `pkexec` ein (fragt grafisch nach dem Passwort).
pub fn setup_access() -> io::Result<()> {
    let script = format!(
        "printf '%s' '{UDEV_RULE}' > {UDEV_RULE_PATH} && modprobe uinput && \
         udevadm control --reload-rules && udevadm trigger --name-match=uinput"
    );
    let status = std::process::Command::new("pkexec")
        .args(["sh", "-c", &script])
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(
            "Einrichtung abgebrochen oder fehlgeschlagen",
        ))
    }
}
