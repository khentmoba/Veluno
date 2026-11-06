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

static LOG: OnceLock<Mutex<Vec<Entry>>> = OnceLock::new();

fn log() -> MutexGuard<'static, Vec<Entry>> {
    LOG.get_or_init(|| {
        let mut loaded: Vec<Entry> = std::fs::read_to_string(path())
            .ok()
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default();
        // A file from a newer build or an older cap must not grow memory without bound.
        loaded.truncate(MAX_ENTRIES);
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
    flush_with(|bytes| profile::write_atomic(&path(), bytes));
}

fn flush_with(write: impl FnOnce(&[u8]) -> std::io::Result<()>) {
    static WRITING: Mutex<()> = Mutex::new(());
    let _writing = WRITING
        .lock()
        .expect("history flush mutex should never be poisoned");
    if !DIRTY.swap(false, Ordering::AcqRel) {
        return;
    }
    // Serialize under the log lock, then write after the guard is released.
    let bytes = serde_json::to_vec(&*log());
    if let Ok(bytes) = bytes
        && write(&bytes).is_err()
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
    // An older visit to the same page moves to the top instead of leaving a duplicate row.
    let previous = entries
        .iter()
        .position(|entry| entry.url == url)
        .map(|index| entries.remove(index));
    let favicon = favicon
        .map(str::to_owned)
        .or_else(|| previous.as_ref().and_then(|entry| entry.favicon.clone()));
    entries.insert(
        0,
        Entry {
            url: url.into(),
            title: title.into(),
            time,
            favicon,
        },
    );
    entries.truncate(MAX_ENTRIES);
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

/// The page title usually arrives after the visit is recorded, so match the row by URL.
pub fn retitle(url: &str, title: &str) {
    if retitle_entry(&mut log(), url, title) {
        changed();
    }
}

fn retitle_entry(entries: &mut [Entry], url: &str, title: &str) -> bool {
    match entries.iter_mut().find(|entry| entry.url == url) {
        Some(entry) if entry.title != title => {
            entry.title = title.into();
            true
        }
        _ => false,
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
    fn revisiting_an_older_page_moves_it_up_without_duplicating() {
        let mut entries = Vec::new();
        record(&mut entries, "https://a.test/", "A", 1, None);
        record(&mut entries, "https://b.test/", "B", 2, None);
        record(
            &mut entries,
            "https://a.test/",
            "A again",
            3,
            Some("https://a.test/i.png"),
        );
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].url, "https://a.test/");
        assert_eq!(entries[0].title, "A again");
        assert_eq!(entries[0].time, 3);
        assert_eq!(entries[0].favicon.as_deref(), Some("https://a.test/i.png"));
        assert_eq!(entries[1].url, "https://b.test/");
        record(&mut entries, "https://a.test/", "A", 4, None);
        assert_eq!(entries.len(), 2, "a repeat visit still leaves one row");
        assert_eq!(
            entries[0].favicon.as_deref(),
            Some("https://a.test/i.png"),
            "the known icon survives a visit that reports none"
        );
    }

    #[test]
    fn a_late_title_reaches_its_row_after_another_visit() {
        let mut entries = Vec::new();
        record(&mut entries, "https://a.test/", "", 1, None);
        record(&mut entries, "https://b.test/", "B", 2, None);
        assert_eq!(entries[0].url, "https://b.test/");
        assert!(retitle_entry(&mut entries, "https://a.test/", "A"));
        assert_eq!(entries[1].title, "A", "the older row takes its late title");
        assert_eq!(entries[0].title, "B", "the newest row is untouched");
        assert!(!retitle_entry(&mut entries, "https://a.test/", "A"));
        assert!(!retitle_entry(&mut entries, "https://missing.test/", "X"));
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
    fn flush_releases_the_log_before_writing() {
        LOG.get_or_init(|| Mutex::new(Vec::new()));
        changed();
        let mut visits_could_proceed = false;
        flush_with(|_| {
            visits_could_proceed = LOG.get().is_some_and(|log| log.try_lock().is_ok());
            Ok(())
        });
        assert!(visits_could_proceed);
    }

    #[test]
    fn a_large_saved_log_is_capped_when_it_loads() {
        let mut entries = Vec::new();
        for n in 0..MAX_ENTRIES + 50 {
            record(
                &mut entries,
                &format!("https://{n}.test/"),
                "t",
                n as u64,
                None,
            );
        }
        let json = serde_json::to_string(&entries).unwrap();
        let mut loaded: Vec<Entry> = serde_json::from_str(&json).unwrap();
        loaded.truncate(MAX_ENTRIES);
        assert_eq!(loaded.len(), MAX_ENTRIES);
        assert_eq!(
            loaded[0].url,
            format!("https://{}.test/", MAX_ENTRIES + 49),
            "the newest rows are kept and the oldest are dropped"
        );
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
