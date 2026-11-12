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
    pub sidebar_right: bool,
    pub data_saver: bool,
    pub performance_mode: bool,
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
            sidebar_right: false,
            data_saver: false,
            performance_mode: false,
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
    crate::profile::lock(current()).clone()
}

pub fn engine() -> (&'static str, &'static str) {
    let settings = crate::profile::lock(current());
    let (_, name, url) = ENGINES
        .iter()
        .find(|(key, ..)| *key == settings.search_engine)
        .unwrap_or(&ENGINES[0]);
    (name, url)
}

/// Whether the sidebar sits on the right edge of the window.
pub fn sidebar_right() -> bool {
    crate::profile::lock(current()).sidebar_right
}

pub fn accent() -> &'static str {
    let settings = crate::profile::lock(current());
    ACCENTS
        .iter()
        .find(|(key, ..)| *key == settings.accent)
        .unwrap_or(&ACCENTS[0])
        .2
}

pub fn data_saver() -> bool {
    std::env::var_os("VELUNO_DATA_SAVER").is_some() || crate::profile::lock(current()).data_saver
}

pub fn performance_mode() -> bool {
    std::env::var_os("VELUNO_PERFORMANCE_MODE").is_some()
        || crate::profile::lock(current()).performance_mode
        || is_low_end_device()
}

#[cfg(windows)]
pub fn is_low_end_device() -> bool {
    use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    let mut status = MEMORYSTATUSEX {
        dwLength: u32::try_from(std::mem::size_of::<MEMORYSTATUSEX>()).unwrap_or(0),
        ..Default::default()
    };
    // SAFETY: `status.dwLength` is set to the struct's size, as the API requires.
    // `status` is a valid, writable out-parameter.
    if unsafe { GlobalMemoryStatusEx(&mut status) }.is_ok() {
        // Less than or equal to 4GB RAM or less than 512MB available physical memory
        status.ullTotalPhys <= 4 * 1024 * 1024 * 1024 || status.ullAvailPhys <= 512 * 1024 * 1024
    } else {
        false
    }
}

#[cfg(not(windows))]
pub fn is_low_end_device() -> bool {
    false
}

pub fn set(key: &str, value: &Value) -> Result<Settings, String> {
    let mut settings = get();
    match (key, value) {
        ("search_engine", Value::String(id)) if ENGINES.iter().any(|(key, ..)| key == id) => {
            settings.search_engine.clone_from(id);
        }
        ("ad_blocker", Value::Bool(on)) => settings.ad_blocker = *on,
        ("auto_update", Value::Bool(on)) => settings.auto_update = *on,
        ("save_history", Value::Bool(on)) => settings.save_history = *on,
        ("tracking_prevention", Value::String(level))
            if TRACKING_LEVELS.contains(&level.as_str()) =>
        {
            settings.tracking_prevention.clone_from(level);
        }
        ("save_passwords", Value::Bool(on)) => settings.save_passwords = *on,
        ("autofill", Value::Bool(on)) => settings.autofill = *on,
        ("sidebar_right", Value::Bool(on)) => settings.sidebar_right = *on,
        ("data_saver", Value::Bool(on)) => settings.data_saver = *on,
        ("performance_mode", Value::Bool(on)) => settings.performance_mode = *on,
        ("accent", Value::String(id)) if ACCENTS.iter().any(|(key, ..)| key == id) => {
            settings.accent.clone_from(id);
        }
        _ => return Err(format!("Unknown setting: {key}")),
    }
    save(&settings)?;
    crate::profile::lock(current()).clone_from(&settings);
    Ok(settings)
}

fn save(settings: &Settings) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(settings).map_err(|error| error.to_string())?;
    crate::profile::write_atomic(&path(), &bytes).map_err(|error| error.to_string())
}

/// Restores every setting to its default. Tabs, bookmarks, and history are untouched.
pub fn reset() -> Result<Settings, String> {
    reset_with(save)
}

fn reset_with(write: impl FnOnce(&Settings) -> Result<(), String>) -> Result<Settings, String> {
    let defaults = Settings::default();
    write(&defaults)?;
    crate::profile::lock(current()).clone_from(&defaults);
    Ok(defaults)
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
        assert!(!settings.sidebar_right);
        assert!(!settings.data_saver);
        assert!(!settings.performance_mode);
    }

    #[test]
    fn resetting_writes_and_returns_every_default() {
        let mut written = None;
        let defaults = reset_with(|settings| {
            written = Some(settings.clone());
            Ok(())
        })
        .unwrap();
        let written = written.unwrap();
        assert_eq!(written.accent, "neutral");
        assert_eq!(written.search_engine, "brave");
        assert!(written.save_history && written.ad_blocker && written.auto_update);
        assert_eq!(defaults.tracking_prevention, "balanced");
        assert!(!defaults.save_passwords && defaults.autofill);
        assert!(!defaults.sidebar_right && !defaults.data_saver && !defaults.performance_mode);
        assert!(reset_with(|_| Err("disk full".into())).is_err());
    }

    #[test]
    fn unknown_engines_and_keys_are_rejected() {
        assert!(set("search_engine", &Value::String("evil".into())).is_err());
        assert!(set("tracking_prevention", &Value::String("none".into())).is_err());
        assert!(set("save_passwords", &Value::String("yes".into())).is_err());
        assert!(set("nope", &Value::Bool(true)).is_err());
        assert!(set("accent", &Value::String("red;}*{x".into())).is_err());
        assert!(set("ad_blocker", &Value::String("yes".into())).is_err());
        assert!(set("data_saver", &Value::String("true".into())).is_err());
        assert!(set("performance_mode", &Value::String("true".into())).is_err());
    }
}
