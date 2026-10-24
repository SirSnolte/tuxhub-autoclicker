//! Persistent settings, shared by the service, the window, tuxhub and the
//! command line. Only the service writes them.

use crate::clicker;
use crate::macros::safe_trigger;
use serde::{Deserialize, Serialize};
use std::io;
use std::path::{Path, PathBuf};

pub const APP_DIR: &str = "tuxhub-autoclicker";

/// Settings that tuxhub stored before the tool kept its own. They are
/// imported once, when the tool has no settings yet.
const HUB_SETTINGS: &str = "io.github.sirsnolte.tuxhub/settings/autoclicker.json";

pub const MAX_INTERVAL_MS: u64 = 60_000;
pub const MAX_JITTER_PCT: u32 = 50;
pub const MAX_LIMIT: u64 = 1_000_000;
pub const MAX_START_DELAY_S: u64 = 30;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    #[serde(flatten)]
    pub clicker: clicker::Config,
    /// Record mouse movements in macros, not only keys, clicks and the wheel.
    pub record_moves: bool,
    /// Optional hotkey that only stops the clicker, empty for none.
    pub stop_shortcut: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            clicker: clicker::Config::default(),
            record_moves: true,
            stop_shortcut: String::new(),
        }
    }
}

impl Config {
    pub fn path() -> io::Result<PathBuf> {
        Ok(crate::paths::config_home()?
            .join(APP_DIR)
            .join("config.json"))
    }

    /// Loads the configuration. A missing or unreadable file yields the defaults.
    pub fn load() -> Self {
        match (Self::path(), crate::paths::config_home()) {
            (Ok(path), Ok(config)) => Self::load_from(&path, &config.join(HUB_SETTINGS)),
            _ => Self::default(),
        }
    }

    /// Loads `path`, or imports and saves the settings of tuxhub at `hub`
    /// if `path` does not exist yet.
    pub fn load_from(path: &Path, hub: &Path) -> Self {
        if path.exists() {
            return std::fs::read_to_string(path)
                .ok()
                .and_then(|s| Self::parse(&s))
                .unwrap_or_default();
        }
        let Some(imported) = std::fs::read_to_string(hub)
            .ok()
            .and_then(|s| Self::parse(&s))
        else {
            return Self::default();
        };
        if let Err(e) = imported.save_to(path) {
            eprintln!("could not save the imported settings: {e}");
        }
        imported
    }

    fn parse(text: &str) -> Option<Self> {
        serde_json::from_str::<Self>(text)
            .ok()
            .map(Self::normalized)
    }

    pub fn save(&self) -> io::Result<()> {
        self.save_to(&Self::path()?)
    }

    fn save_to(&self, path: &Path) -> io::Result<()> {
        std::fs::create_dir_all(path.parent().unwrap())?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(self).unwrap() + "\n")?;
        std::fs::rename(tmp, path)
    }

    /// Clamps values instead of failing, for files edited by hand.
    fn normalized(mut self) -> Self {
        let c = &mut self.clicker;
        c.interval_ms = c.interval_ms.clamp(1, MAX_INTERVAL_MS);
        c.jitter_pct = c.jitter_pct.min(MAX_JITTER_PCT);
        c.limit = c.limit.min(MAX_LIMIT);
        c.start_delay_s = c.start_delay_s.min(MAX_START_DELAY_S);
        self.stop_shortcut = self.stop_shortcut.trim().to_string();
        if !self.stop_shortcut.is_empty() && !safe_trigger(&self.stop_shortcut) {
            self.stop_shortcut.clear();
        }
        self
    }

    /// Strict validation for changes requested through the interface.
    pub fn validate(&self) -> Result<(), String> {
        let c = &self.clicker;
        if !(1..=MAX_INTERVAL_MS).contains(&c.interval_ms) {
            return Err(format!("the interval must be 1 to {MAX_INTERVAL_MS} ms"));
        }
        if c.jitter_pct > MAX_JITTER_PCT {
            return Err(format!(
                "the random variation is limited to {MAX_JITTER_PCT} %"
            ));
        }
        if c.limit > MAX_LIMIT {
            return Err(format!("the number of clicks is limited to {MAX_LIMIT}"));
        }
        if c.start_delay_s > MAX_START_DELAY_S {
            return Err(format!(
                "the start delay is limited to {MAX_START_DELAY_S} s"
            ));
        }
        if !self.stop_shortcut.is_empty() && !safe_trigger(&self.stop_shortcut) {
            return Err(format!(
                "the stop hotkey {:?} is not allowed",
                self.stop_shortcut
            ));
        }
        Ok(())
    }

    /// Applies the fields present in `patch` and validates the result.
    pub fn merged(&self, patch: serde_json::Value) -> Result<Self, String> {
        let mut value = serde_json::to_value(self).unwrap();
        let (Some(obj), Some(patch)) = (value.as_object_mut(), patch.as_object()) else {
            return Err("expected an object".into());
        };
        for (k, v) in patch {
            if !obj.contains_key(k) {
                return Err(format!("unknown setting {k:?}"));
            }
            obj.insert(k.clone(), v.clone());
        }
        let mut cfg: Config =
            serde_json::from_value(value).map_err(|e| format!("invalid settings: {e}"))?;
        cfg.stop_shortcut = cfg.stop_shortcut.trim().to_string();
        cfg.validate()?;
        Ok(cfg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "tuxhub-autoclicker-config-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn file_is_flat_and_round_trips() {
        let cfg = Config::default();
        let value = serde_json::to_value(&cfg).unwrap();
        assert_eq!(value["interval_ms"], 100);
        assert_eq!(value["record_moves"], true);
        assert_eq!(Config::parse(&value.to_string()), Some(cfg));
    }

    #[test]
    fn hand_edited_values_are_clamped() {
        let cfg = Config::parse(
            r#"{"interval_ms": 0, "jitter_pct": 99, "limit": 5000000, "start_delay_s": 100, "stop_shortcut": " q "}"#,
        )
        .unwrap();
        assert_eq!(cfg.clicker.interval_ms, 1);
        assert_eq!(cfg.clicker.jitter_pct, MAX_JITTER_PCT);
        assert_eq!(cfg.clicker.limit, MAX_LIMIT);
        assert_eq!(cfg.clicker.start_delay_s, MAX_START_DELAY_S);
        assert_eq!(cfg.stop_shortcut, "", "a typing key is dropped");
    }

    #[test]
    fn merge_changes_only_the_given_fields() {
        let cfg = Config::default()
            .merged(json!({ "button": "right", "stop_shortcut": " CTRL+F7 " }))
            .unwrap();
        assert_eq!(cfg.clicker.button, crate::device::Button::Right);
        assert_eq!(cfg.clicker.interval_ms, 100);
        assert_eq!(cfg.stop_shortcut, "CTRL+F7");
        assert!(Config::default().merged(json!({ "limit": -1 })).is_err());
        assert!(Config::default().merged(json!({ "nope": 1 })).is_err());
    }

    #[test]
    fn imports_the_hub_settings_only_without_own_settings() {
        let dir = temp("import");
        let (own, hub) = (dir.join("own/config.json"), dir.join("hub.json"));
        std::fs::write(&hub, r#"{"interval_ms": 42, "other": true}"#).unwrap();
        assert_eq!(Config::load_from(&own, &hub).clicker.interval_ms, 42);
        assert!(own.is_file());

        std::fs::write(&hub, r#"{"interval_ms": 7}"#).unwrap();
        assert_eq!(
            Config::load_from(&own, &hub).clicker.interval_ms,
            42,
            "own settings win"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn missing_or_broken_files_yield_defaults() {
        let dir = temp("broken");
        let own = dir.join("config.json");
        assert_eq!(
            Config::load_from(&own, &dir.join("none")),
            Config::default()
        );
        std::fs::write(&own, "{ broken").unwrap();
        assert_eq!(
            Config::load_from(&own, &dir.join("none")),
            Config::default()
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
