//! Reads bookmarks from Chromium-based browsers installed for this Windows user.
//! Only local files are read; nothing is sent anywhere.

use crate::state::Bookmark;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// (id, display name, base folder variable, path under it, whether it holds named profiles)
const SOURCES: [(&str, &str, &str, &str, bool); 6] = [
    (
        "chrome",
        "Google Chrome",
        "LOCALAPPDATA",
        r"Google\Chrome\User Data",
        true,
    ),
    (
        "edge",
        "Microsoft Edge",
        "LOCALAPPDATA",
        r"Microsoft\Edge\User Data",
        true,
    ),
    (
        "brave",
        "Brave",
        "LOCALAPPDATA",
        r"BraveSoftware\Brave-Browser\User Data",
        true,
    ),
    (
        "vivaldi",
        "Vivaldi",
        "LOCALAPPDATA",
        r"Vivaldi\User Data",
        true,
    ),
    (
        "opera",
        "Opera",
        "APPDATA",
        r"Opera Software\Opera Stable",
        false,
    ),
    (
        "opera_gx",
        "Opera GX",
        "APPDATA",
        r"Opera Software\Opera GX Stable",
        false,
    ),
];

/// Browsers on this computer that have at least one bookmarks file, as (id, name).
pub fn available() -> Vec<(&'static str, &'static str)> {
    SOURCES
        .iter()
        .filter(|(id, ..)| !files(id).is_empty())
        .map(|(id, name, ..)| (*id, *name))
        .collect()
}

pub fn name(id: &str) -> Option<&'static str> {
    SOURCES
        .iter()
        .find(|(key, ..)| *key == id)
        .map(|(_, name, ..)| *name)
}

/// Every web bookmark from all of the browser's profiles, folders flattened, first copy kept.
/// The second value counts links that were not web pages, such as `javascript:` entries.
pub fn read(id: &str) -> Result<(Vec<Bookmark>, usize), String> {
    let name = name(id).ok_or("Unknown browser.")?;
    let files = files(id);
    if files.is_empty() {
        return Err(format!("No {name} bookmarks were found on this computer."));
    }
    let mut bookmarks = Vec::new();
    let mut skipped = 0;
    let mut read_any = false;
    for file in files {
        // The browser may be running; its bookmarks file is still readable.
        if let Ok(json) = std::fs::read_to_string(&file)
            && let Ok(tree) = serde_json::from_str::<Value>(&json)
        {
            read_any = true;
            collect(&tree, &mut bookmarks, &mut skipped);
        }
    }
    if !read_any {
        return Err(format!("Veluno could not read the {name} bookmarks file."));
    }
    let mut seen = std::collections::HashSet::new();
    bookmarks.retain(|bookmark| seen.insert(bookmark.url.clone()));
    Ok((bookmarks, skipped))
}

fn files(id: &str) -> Vec<PathBuf> {
    let Some((_, _, variable, folder, profiles)) = SOURCES.iter().find(|(key, ..)| *key == id)
    else {
        return Vec::new();
    };
    let Some(base) = std::env::var_os(variable).map(|base| PathBuf::from(base).join(folder)) else {
        return Vec::new();
    };
    profile_files(&base, *profiles)
}

fn profile_files(base: &Path, profiles: bool) -> Vec<PathBuf> {
    if !profiles {
        let file = base.join("Bookmarks");
        return if file.is_file() {
            vec![file]
        } else {
            Vec::new()
        };
    }
    let Ok(entries) = std::fs::read_dir(base) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = entries
        .flatten()
        .filter(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            name == "Default" || name.starts_with("Profile ")
        })
        .map(|entry| entry.path().join("Bookmarks"))
        .filter(|file| file.is_file())
        .collect();
    // Default first, then numbered profiles in order, so duplicates keep the main profile's title.
    found.sort_by_key(|file| {
        let profile = file
            .parent()
            .and_then(Path::file_name)
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let number = profile
            .strip_prefix("Profile ")
            .and_then(|n| n.parse::<u32>().ok());
        (profile != "Default", number.unwrap_or(u32::MAX), profile)
    });
    found
}

/// Walks the `roots` of a Chromium bookmarks file in bar, other, then mobile order.
fn collect(tree: &Value, out: &mut Vec<Bookmark>, skipped: &mut usize) {
    let Some(roots) = tree.get("roots").and_then(Value::as_object) else {
        return;
    };
    for key in ["bookmark_bar", "other", "synced"] {
        if let Some(root) = roots.get(key) {
            walk(root, out, skipped, 0);
        }
    }
}

fn walk(node: &Value, out: &mut Vec<Bookmark>, skipped: &mut usize, depth: usize) {
    if depth > 64 {
        return;
    }
    match node.get("type").and_then(Value::as_str) {
        Some("url") => {
            let url = node.get("url").and_then(Value::as_str).unwrap_or_default();
            let web =
                url::Url::parse(url).is_ok_and(|url| matches!(url.scheme(), "http" | "https"));
            if !web || crate::profile::normalize_url(url).is_none() {
                *skipped += 1;
            }
            if web && let Some(url) = crate::profile::normalize_url(url) {
                let title = node
                    .get("name")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|title| !title.is_empty())
                    .unwrap_or(&url)
                    .to_owned();
                out.push(Bookmark { url, title });
            }
        }
        _ => {
            for child in node
                .get("children")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                walk(child, out, skipped, depth + 1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folders_flatten_and_non_web_links_are_skipped() {
        let tree = serde_json::json!({"roots": {
            "bookmark_bar": {"type": "folder", "children": [
                {"type": "url", "name": "Example", "url": "https://example.com/"},
                {"type": "folder", "name": "Work", "children": [
                    {"type": "url", "name": "", "url": "http://intranet.test/a"},
                    {"type": "url", "name": "Script", "url": "javascript:alert(1)"},
                    {"type": "url", "name": "Local", "url": "file:///C:/secret.txt"}
                ]}
            ]},
            "other": {"type": "folder", "children": [
                {"type": "url", "name": "Chrome page", "url": "chrome://settings"}
            ]},
            "synced": {"type": "folder", "children": [
                {"type": "url", "name": "Phone", "url": "https://m.example.com/"}
            ]}
        }});
        let mut out = Vec::new();
        let mut skipped = 0;
        collect(&tree, &mut out, &mut skipped);
        assert_eq!(
            skipped, 3,
            "script, file, and chrome links are reported, not silently dropped"
        );
        let urls: Vec<_> = out.iter().map(|b| b.url.as_str()).collect();
        assert_eq!(
            urls,
            [
                "https://example.com/",
                "http://intranet.test/a",
                "https://m.example.com/"
            ]
        );
        assert_eq!(out[0].title, "Example");
        assert_eq!(out[1].title, "http://intranet.test/a");
    }

    #[test]
    fn profiles_are_found_default_first() {
        let base = std::env::temp_dir().join(format!("veluno-import-{}", std::process::id()));
        for profile in ["Profile 10", "Default", "Profile 2", "System Profile"] {
            std::fs::create_dir_all(base.join(profile)).unwrap();
            std::fs::write(base.join(profile).join("Bookmarks"), "{}").unwrap();
        }
        let names: Vec<_> = profile_files(&base, true)
            .iter()
            .map(|file| {
                file.parent()
                    .unwrap()
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        assert_eq!(names, ["Default", "Profile 2", "Profile 10"]);
        assert!(profile_files(&base.join("missing"), false).is_empty());
        let _ = std::fs::remove_dir_all(base);
    }
}
