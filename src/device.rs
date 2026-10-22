//! Virtual mouse through `/dev/uinput`.
//!
//! The kernel treats the device like a physical mouse, so clicks work on
//! Wayland, X11 and in every game.

use evdev::uinput::VirtualDevice;
use evdev::{AttributeSet, EventType, InputEvent, KeyCode, RelativeAxisCode};
use serde::{Deserialize, Serialize};
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

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
        // Without motion axes, libinput does not treat the device as a pointer.
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

    /// Paths of the `/dev/input/event*` nodes of this device.
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
                "no access to /dev/uinput ({e}). Grant access once with \
                 `tuxhub-autoclicker setup` or the \"Set up access\" action in tuxhub."
            ),
        )
    } else {
        e
    }
}

/// udev rule that gives the logged-in user access to `/dev/uinput`.
pub const UDEV_RULE_PATH: &str = "/etc/udev/rules.d/60-tuxhub-uinput.rules";
pub const UDEV_RULE: &str =
    "KERNEL==\"uinput\", SUBSYSTEM==\"misc\", TAG+=\"uaccess\", OPTIONS+=\"static_node=uinput\"\n";

/// Installs the udev rule with `pkexec`, which asks for the password in a
/// graphical dialog. The rule is passed on standard input, not interpolated
/// into the shell command.
pub fn setup_access() -> io::Result<()> {
    let script = format!(
        "cat > {UDEV_RULE_PATH} && modprobe uinput && \
         udevadm control --reload-rules && udevadm trigger --name-match=uinput"
    );
    let mut child = Command::new("pkexec")
        .args(["sh", "-c", &script])
        .stdin(Stdio::piped())
        .spawn()?;
    child
        .stdin
        .take()
        .expect("stdin is piped")
        .write_all(UDEV_RULE.as_bytes())?;
    if child.wait()?.success() {
        Ok(())
    } else {
        Err(io::Error::other("the setup was cancelled or failed"))
    }
}
