use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::{Mutex, OnceLock};

pub const ENGINES: [(&str, &str, &str); 4] = [
    (
        "brave",
        "Brave Search",
        "https://search.brave.com/search?q=",
    ),
    ("duckduckgo", "DuckDuckGo", "https://duckduckgo.com/?q="),
    ("google", "Google", "https://www.google.com/search?q="),
    ("bing", "Bing", "https://www.bing.com/search?q="),
];

/// Accent colors for Veluno's own interface: id, label, and color.
pub const ACCENTS: [(&str, &str, &str); 6] = [
    ("neutral", "Neutral", "#dedee3"),
    ("amethyst", "Amethyst", "#c4a7ff"),
    ("glacier", "Glacier", "#8ecbff"),
    ("emerald", "Emerald", "#7ee0a1"),
    ("amber", "Amber", "#f0c674"),
    ("rose", "Rose", "#ff9fb7"),
];

/// WebView2 tracking prevention levels offered in Settings.
pub const TRACKING_LEVELS: [&str; 3] = ["basic", "balanced", "strict"];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub search_engine: String,
    pub ad_blocker: bool,
    pub auto_update: bool,
    pub save_history: bool,
    pub tracking_prevention: String,
    pub save_passwords: bool,
    pub autofill: bool,
    pub accent: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            search_engine: "brave".into(),
            ad_blocker: true,
            auto_update: true,
            save_history: true,
            tracking_prevention: "balanced".into(),
            save_passwords: false,
            autofill: true,
            accent: "neutral".into(),
        }
    }
}

fn current() -> &'static Mutex<Settings> {
    static CURRENT: OnceLock<Mutex<Settings>> = OnceLock::new();
    CURRENT.get_or_init(|| {
        let loaded = std::fs::read_to_string(path())
            .ok()
            .and_then(|json| serde_json::from_str::<Settings>(&json).ok())
            .unwrap_or_default();
        Mutex::new(loaded)
    })
}

fn path() -> std::path::PathBuf {
    crate::profile::directory().join("settings.json")
}

pub fn get() -> Settings {
    current()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}

pub fn engine() -> (&'static str, &'static str) {
    let id = get().search_engine;
    let (_, name, url) = ENGINES
        .iter()
        .find(|(key, ..)| *key == id)
        .unwrap_or(&ENGINES[0]);
    (name, url)
}

pub fn accent() -> &'static str {
    let id = get().accent;
    ACCENTS
        .iter()
        .find(|(key, ..)| *key == id)
        .unwrap_or(&ACCENTS[0])
        .2
}

pub fn set(key: &str, value: &Value) -> Result<Settings, String> {
    let mut settings = get();
    match (key, value) {
        ("search_engine", Value::String(id)) if ENGINES.iter().any(|(key, ..)| key == id) => {
            settings.search_engine = id.clone()
        }
        ("ad_blocker", Value::Bool(on)) => settings.ad_blocker = *on,
        ("auto_update", Value::Bool(on)) => settings.auto_update = *on,
        ("save_history", Value::Bool(on)) => settings.save_history = *on,
        ("tracking_prevention", Value::String(level))
            if TRACKING_LEVELS.contains(&level.as_str()) =>
        {
            settings.tracking_prevention = level.clone()
        }
        ("save_passwords", Value::Bool(on)) => settings.save_passwords = *on,
        ("autofill", Value::Bool(on)) => settings.autofill = *on,
        ("accent", Value::String(id)) if ACCENTS.iter().any(|(key, ..)| key == id) => {
            settings.accent = id.clone()
        }
        _ => return Err(format!("Unknown setting: {key}")),
    }
    let bytes = serde_json::to_vec_pretty(&settings).map_err(|error| error.to_string())?;
    crate::profile::write_atomic(&path(), &bytes).map_err(|error| error.to_string())?;
    *current()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = settings.clone();
    Ok(settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_fields_fall_back_to_defaults() {
        let settings: Settings = serde_json::from_str(r#"{"search_engine":"google"}"#).unwrap();
        assert_eq!(settings.search_engine, "google");
        assert!(settings.ad_blocker && settings.auto_update && settings.save_history);
        assert_eq!(settings.tracking_prevention, "balanced");
        assert!(!settings.save_passwords && settings.autofill);
        assert_eq!(settings.accent, "neutral");
    }

    #[test]
    fn unknown_engines_and_keys_are_rejected() {
        assert!(set("search_engine", &Value::String("evil".into())).is_err());
        assert!(set("tracking_prevention", &Value::String("none".into())).is_err());
        assert!(set("save_passwords", &Value::String("yes".into())).is_err());
        assert!(set("nope", &Value::Bool(true)).is_err());
        assert!(set("accent", &Value::String("red;}*{x".into())).is_err());
        assert!(set("ad_blocker", &Value::String("yes".into())).is_err());
    }
}
