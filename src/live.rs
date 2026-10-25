//! tuxhub live interface (protocol 1): the schema of the basic settings and
//! the mapping between its keys and [`Config`]. Macros and the stop hotkey
//! stay in the tool's own interface.

use crate::config::{Config, MAX_INTERVAL_MS, MAX_JITTER_PCT, MAX_LIMIT, MAX_START_DELAY_S};
use serde_json::{Map, Value, json};

pub const PROTOCOL: u64 = 1;
/// Id of the tool in tuxhub, the name of the socket.
pub const TOOL_ID: &str = "autoclicker";

/// Keys of the schema, in the order of the page.
pub const KEYS: [&str; 7] = [
    "interval_ms",
    "jitter_pct",
    "button",
    "mode",
    "limit",
    "start_delay_s",
    "record_moves",
];

/// A refused request; `field` names the setting it is about.
#[derive(Debug, Clone, PartialEq)]
pub struct Failure {
    pub message: String,
    pub field: Option<String>,
}

impl From<String> for Failure {
    fn from(message: String) -> Self {
        Self {
            message,
            field: None,
        }
    }
}

impl From<&str> for Failure {
    fn from(message: &str) -> Self {
        message.to_string().into()
    }
}

fn on_field(field: &str, message: impl Into<String>) -> Failure {
    Failure {
        message: message.into(),
        field: Some(field.to_string()),
    }
}

/// Answer to `hello`.
pub fn hello(params: &Value) -> Result<Value, Failure> {
    match params.get("protocol").and_then(Value::as_u64) {
        Some(PROTOCOL) => Ok(json!({
            "protocol": PROTOCOL,
            "name": "Autoclicker",
            "version": env!("CARGO_PKG_VERSION"),
        })),
        _ => Err(format!("only protocol {PROTOCOL} is supported").into()),
    }
}

/// Schema of the basic settings, in German for `de` and in English otherwise.
pub fn schema(locale: &str) -> Value {
    let de = locale == "de";
    let t = |en: &'static str, de_text: &'static str| if de { de_text } else { en };
    json!({
        "sections": [
            {
                "id": "clicking",
                "label": t("Clicking", "Klicken"),
                "fields": [
                    {
                        "key": "interval_ms",
                        "type": "number",
                        "label": t("Interval", "Intervall"),
                        "help": t("Time between two clicks", "Zeit zwischen zwei Klicks"),
                        "min": 1,
                        "max": MAX_INTERVAL_MS,
                        "step": 1,
                        "unit": "ms",
                        "default": 100,
                    },
                    {
                        "key": "jitter_pct",
                        "type": "number",
                        "label": t("Random variation", "Zufällige Abweichung"),
                        "help": t("Varies the interval by up to this share", "Ändert das Intervall um bis zu diesen Anteil"),
                        "min": 0,
                        "max": MAX_JITTER_PCT,
                        "step": 1,
                        "unit": "%",
                        "default": 0,
                    },
                    {
                        "key": "button",
                        "type": "select",
                        "label": t("Mouse button", "Maustaste"),
                        "options": [
                            { "value": "left", "label": t("Left", "Links") },
                            { "value": "right", "label": t("Right", "Rechts") },
                            { "value": "middle", "label": t("Middle", "Mitte") },
                        ],
                        "default": "left",
                    },
                    {
                        "key": "mode",
                        "type": "select",
                        "label": t("Mode", "Modus"),
                        "options": [
                            { "value": "click", "label": t("Single clicks", "Einzelklicks") },
                            { "value": "double", "label": t("Double clicks", "Doppelklicks") },
                            { "value": "hold", "label": t("Hold the button", "Taste gedrückt halten") },
                        ],
                        "default": "click",
                    },
                    {
                        "key": "limit",
                        "type": "number",
                        "label": t("Number of clicks", "Anzahl der Klicks"),
                        "help": t("0 means until stopped", "0 heißt bis zum Stoppen"),
                        "min": 0,
                        "max": MAX_LIMIT,
                        "step": 1,
                        "default": 0,
                    },
                    {
                        "key": "start_delay_s",
                        "type": "number",
                        "label": t("Start delay", "Startverzögerung"),
                        "help": t("Time to move the pointer before clicking starts", "Zeit, um den Zeiger vor dem Klicken zu bewegen"),
                        "min": 0,
                        "max": MAX_START_DELAY_S,
                        "step": 1,
                        "unit": "s",
                        "default": 3,
                    },
                ],
            },
            {
                "id": "macros",
                "label": t("Macros", "Makros"),
                "fields": [
                    {
                        "key": "record_moves",
                        "type": "toggle",
                        "label": t("Record mouse movements", "Mausbewegungen aufnehmen"),
                        "help": t("Otherwise only keys, clicks and the wheel", "Sonst nur Tasten, Klicks und das Mausrad"),
                        "default": true,
                    },
                ],
            },
        ],
        "actions": [
            { "id": "toggle", "label": t("Start or stop clicking", "Klicken starten oder stoppen") },
            { "id": "panic", "label": t("Stop everything", "Alles stoppen") },
            { "id": "setup", "label": t("Set up input device access", "Zugriff auf Eingabegeräte einrichten") },
        ],
    })
}

/// The values of the schema keys.
pub fn values(cfg: &Config) -> Map<String, Value> {
    let all = serde_json::to_value(cfg).unwrap_or_default();
    KEYS.iter()
        .map(|k| (k.to_string(), all.get(*k).cloned().unwrap_or(Value::Null)))
        .collect()
}

/// The configuration after `set {key, value}`, checked like every change.
pub fn set(cfg: &Config, params: &Value) -> Result<(Config, Value), Failure> {
    let key = params
        .get("key")
        .and_then(Value::as_str)
        .ok_or("`key` is missing")?;
    if !KEYS.contains(&key) {
        return Err(on_field(key, format!("unknown setting {key:?}")));
    }
    let value = params.get("value").cloned().unwrap_or(Value::Null);
    let fits = match key {
        "record_moves" => value.is_boolean(),
        "button" => matches!(value.as_str(), Some("left" | "right" | "middle")),
        "mode" => matches!(value.as_str(), Some("click" | "double" | "hold")),
        _ => value.as_u64().is_some(),
    };
    if !fits {
        return Err(on_field(key, format!("invalid value for {key:?}")));
    }
    let next = cfg
        .merged(json!({ key: value }))
        .map_err(|e| on_field(key, e))?;
    let applied = values(&next).remove(key).unwrap_or(Value::Null);
    Ok((next, applied))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields(schema: &Value) -> Vec<&Value> {
        schema["sections"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|s| s["fields"].as_array().unwrap())
            .collect()
    }

    fn texts(v: &Value, out: &mut Vec<String>) {
        match v {
            Value::Object(o) => {
                for (k, v) in o {
                    if matches!(k.as_str(), "label" | "help") {
                        out.push(v.as_str().unwrap().to_string());
                    } else {
                        texts(v, out);
                    }
                }
            }
            Value::Array(a) => a.iter().for_each(|v| texts(v, out)),
            _ => {}
        }
    }

    /// Edge case 7: complete in both languages, English for unknown ones.
    #[test]
    fn schema_is_complete_in_german_and_english() {
        let (en, de) = (schema("en"), schema("de"));
        let keys: Vec<&str> = fields(&en)
            .iter()
            .map(|f| f["key"].as_str().unwrap())
            .collect();
        assert_eq!(keys, KEYS);
        let (mut en_texts, mut de_texts) = (Vec::new(), Vec::new());
        texts(&en, &mut en_texts);
        texts(&de, &mut de_texts);
        assert_eq!(en_texts.len(), de_texts.len());
        for (e, d) in en_texts.iter().zip(&de_texts) {
            assert!(!e.is_empty() && !d.is_empty());
            assert_ne!(e, d, "{e:?} is not translated");
        }
        assert_eq!(schema("fr"), en);
        assert_eq!(schema(""), en);
        let defaults = values(&Config::default());
        for f in fields(&en) {
            assert_eq!(
                f["default"],
                defaults[f["key"].as_str().unwrap()],
                "{}",
                f["key"]
            );
        }
    }

    #[test]
    fn values_have_exactly_the_schema_keys() {
        let v = values(&Config::default());
        assert_eq!(v.len(), KEYS.len());
        assert!(v.values().all(|v| !v.is_null()));
    }

    /// Edge case 4: every refused value names its field and changes nothing.
    #[test]
    fn set_refuses_unknown_keys_wrong_types_and_values_out_of_range() {
        let cfg = Config::default();
        for (params, field) in [
            (
                json!({ "key": "stop_shortcut", "value": "F7" }),
                "stop_shortcut",
            ),
            (json!({ "key": "nope", "value": 1 }), "nope"),
            (json!({ "key": "record_moves", "value": 1 }), "record_moves"),
            (json!({ "key": "button", "value": "side" }), "button"),
            (json!({ "key": "mode", "value": "triple" }), "mode"),
            (
                json!({ "key": "interval_ms", "value": "100" }),
                "interval_ms",
            ),
            (json!({ "key": "interval_ms", "value": 0 }), "interval_ms"),
            (json!({ "key": "interval_ms", "value": 2.5 }), "interval_ms"),
            (
                json!({ "key": "jitter_pct", "value": MAX_JITTER_PCT + 1 }),
                "jitter_pct",
            ),
            (json!({ "key": "limit", "value": -1 }), "limit"),
            (json!({ "key": "limit", "value": MAX_LIMIT + 1 }), "limit"),
            (
                json!({ "key": "start_delay_s", "value": MAX_START_DELAY_S + 1 }),
                "start_delay_s",
            ),
        ] {
            let e = set(&cfg, &params).unwrap_err();
            assert_eq!(e.field.as_deref(), Some(field), "{params}");
        }
        assert!(
            set(&cfg, &json!({ "value": 1 }))
                .unwrap_err()
                .field
                .is_none()
        );
    }

    #[test]
    fn set_returns_the_new_configuration_and_value() {
        let (cfg, v) = set(
            &Config::default(),
            &json!({ "key": "interval_ms", "value": 250 }),
        )
        .unwrap();
        assert_eq!((cfg.clicker.interval_ms, v), (250, json!(250)));
        let (cfg, v) = set(&cfg, &json!({ "key": "button", "value": "right" })).unwrap();
        assert_eq!(v, json!("right"));
        assert_eq!(values(&cfg)["interval_ms"], json!(250));
    }

    #[test]
    fn hello_needs_protocol_1() {
        assert_eq!(
            hello(&json!({ "protocol": 1 })).unwrap()["protocol"],
            json!(1)
        );
        assert!(hello(&json!({ "protocol": 2 })).is_err());
        assert!(hello(&json!({})).is_err());
    }
}
