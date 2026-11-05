use crate::{profile, settings};
use serde::{Deserialize, Serialize};
use std::sync::{
    Mutex, MutexGuard, OnceLock,
    atomic::{AtomicBool, Ordering},
};

const MAX_ENTRIES: usize = 2000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub url: String,
    pub title: String,
    pub time: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub favicon: Option<String>,
}

static DIRTY: AtomicBool = AtomicBool::new(false);

fn log() -> MutexGuard<'static, Vec<Entry>> {
    static LOG: OnceLock<Mutex<Vec<Entry>>> = OnceLock::new();
    LOG.get_or_init(|| {
        let loaded = std::fs::read_to_string(path())
            .ok()
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default();
        Mutex::new(loaded)
    })
    .lock()
    .expect("history mutex should never be poisoned")
}

fn path() -> std::path::PathBuf {
    profile::directory().join("history.json")
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
        })
}

/// Writes the log if it changed; the maintenance thread calls this so visits never block on disk.
pub fn flush() {
    static WRITING: Mutex<()> = Mutex::new(());
    let _writing = WRITING
        .lock()
        .expect("history flush mutex should never be poisoned");
    if !DIRTY.swap(false, Ordering::AcqRel) {
        return;
    }
    if let Ok(bytes) = serde_json::to_vec(&*log())
        && profile::write_atomic(&path(), &bytes).is_err()
    {
        DIRTY.store(true, Ordering::Release);
    }
}

fn changed() {
    DIRTY.store(true, Ordering::Release);
}

/// Newest first; reloading the page at the top of the list only refreshes it.
fn record(entries: &mut Vec<Entry>, url: &str, title: &str, time: u64, favicon: Option<&str>) {
    let title = if title.is_empty() { url } else { title };
    match entries.first_mut() {
        Some(latest) if latest.url == url => {
            latest.title = title.into();
            latest.time = time;
            if let Some(favicon) = favicon {
                latest.favicon = Some(favicon.into());
            }
        }
        _ => {
            entries.insert(
                0,
                Entry {
                    url: url.into(),
                    title: title.into(),
                    time,
                    favicon: favicon.map(str::to_owned),
                },
            );
            entries.truncate(MAX_ENTRIES);
        }
    }
}

pub fn visit(url: &str, title: &str, favicon: Option<&str>) {
    if !matches!(
        url::Url::parse(url)
            .map(|u| u.scheme().to_owned())
            .as_deref(),
        Ok("http" | "https")
    ) || !settings::get().save_history
    {
        return;
    }
    let mut entries = log();
    record(&mut entries, url, title, now(), favicon);
    changed();
}

/// The page title usually arrives after the visit is recorded.
pub fn retitle(url: &str, title: &str) {
    let mut entries = log();
    if let Some(latest) = entries.first_mut().filter(|latest| latest.url == url)
        && latest.title != title
    {
        latest.title = title.into();
        changed();
    }
}

pub fn set_favicon(url: &str, favicon: &str) {
    if !settings::get().save_history {
        return;
    }
    if update_favicon(&mut log(), url, favicon) {
        changed();
    }
}

fn update_favicon(entries: &mut [Entry], url: &str, favicon: &str) -> bool {
    let mut updated = false;
    for entry in entries.iter_mut().filter(|entry| entry.url == url) {
        if entry.favicon.as_deref() != Some(favicon) {
            entry.favicon = Some(favicon.into());
            updated = true;
        }
    }
    updated
}

pub fn all() -> Vec<Entry> {
    log().clone()
}

pub fn remove(url: &str) {
    let mut entries = log();
    entries.retain(|entry| entry.url != url);
    changed();
}

pub fn clear() {
    let mut entries = log();
    entries.clear();
    changed();
}

/// Removes visits from the last `range_ms` milliseconds; `None` removes everything.
pub fn clear_recent(range_ms: Option<u64>) {
    let mut entries = log();
    retain_older(
        &mut entries,
        range_ms.map(|range| now().saturating_sub(range)),
    );
    changed();
}

fn retain_older(entries: &mut Vec<Entry>, cutoff: Option<u64>) {
    match cutoff {
        Some(cutoff) => entries.retain(|entry| entry.time < cutoff),
        None => entries.clear(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newest_visit_leads_and_repeats_only_refresh() {
        let mut entries = Vec::new();
        record(&mut entries, "https://a.test/", "A", 1, None);
        record(&mut entries, "https://b.test/", "", 2, None);
        record(&mut entries, "https://b.test/", "B", 3, None);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].title, "B");
        assert_eq!(entries[0].time, 3);
        assert_eq!(entries[1].url, "https://a.test/");
    }

    #[test]
    fn recent_clearing_keeps_older_visits() {
        let mut entries = Vec::new();
        for (n, time) in [10, 20, 30].into_iter().enumerate() {
            record(&mut entries, &format!("https://{n}.test/"), "t", time, None);
        }
        retain_older(&mut entries, Some(20));
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].time, 10);
        retain_older(&mut entries, None);
        assert!(entries.is_empty());
    }

    #[test]
    fn favicons_survive_serialization_and_late_updates() {
        let mut entries: Vec<Entry> =
            serde_json::from_str(r#"[{"url":"https://a.test/","title":"A","time":1}]"#).unwrap();
        assert_eq!(entries[0].favicon, None);
        let icon = "https://a.test/assets/logo.png";
        assert!(update_favicon(&mut entries, "https://a.test/", icon));
        assert!(!update_favicon(&mut entries, "https://a.test/", icon));
        assert!(!update_favicon(&mut entries, "https://other.test/", icon));
        record(&mut entries, "https://a.test/", "A", 2, None);
        assert_eq!(entries[0].favicon.as_deref(), Some(icon));
        record(&mut entries, "https://b.test/", "B", 3, Some(icon));
        let restored: Vec<Entry> =
            serde_json::from_slice(&serde_json::to_vec(&entries).unwrap()).unwrap();
        assert_eq!(restored, entries);
    }

    #[test]
    fn history_is_capped() {
        let mut entries = Vec::new();
        for n in 0..MAX_ENTRIES + 5 {
            record(
                &mut entries,
                &format!("https://{n}.test/"),
                "t",
                n as u64,
                None,
            );
        }
        assert_eq!(entries.len(), MAX_ENTRIES);
        assert_eq!(entries[0].url, format!("https://{}.test/", MAX_ENTRIES + 4));
    }
}
