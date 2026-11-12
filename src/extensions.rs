use crate::{profile, updater};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

static INSTALLING: AtomicBool = AtomicBool::new(false);
static FAILED: AtomicBool = AtomicBool::new(false);

const RELEASE_API: &str = "https://api.github.com/repos/gorhill/uBlock/releases/latest";
const DOWNLOAD_PREFIX: &str = "https://github.com/gorhill/uBlock/releases/download/";

/// Lists uBlock Origin ships switched off that Brave turns on by default: cookie notices,
/// mobile app banners, and tracking parameters in links.
const EXTRA_LISTS: [&str; 4] = [
    "fanboy-cookiemonster",
    "ublock-cookies-easylist",
    "adguard-mobile-app-banners",
    "adguard-spyware-url",
];

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    digest: Option<String>,
}

/// Every folder in here is loaded by WebView2 as an unpacked extension.
pub fn directory() -> PathBuf {
    profile::directory().join("extensions")
}

/// Computes Chromium's 32-character extension ID for an unpacked extension path.
pub fn unpacked_id(path: &std::path::Path) -> String {
    let normalized = path
        .to_string_lossy()
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_owned();
    let mut bytes = Vec::with_capacity(normalized.len() * 2);
    for unit in normalized.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    let digest = Sha256::digest(&bytes);
    let mut id = String::with_capacity(32);
    for byte in &digest[..16] {
        id.push(char::from(b'a' + (byte >> 4)));
        id.push(char::from(b'a' + (byte & 0x0f)));
    }
    id
}

fn ublock_directory() -> PathBuf {
    directory().join("ublock-origin")
}

/// Earlier Veluno versions installed uBlock Origin Lite here.
fn lite_directory() -> PathBuf {
    directory().join("ublock-origin-lite")
}

/// WebView2 holds the installed copy open, so a newer release waits here until the next start.
fn pending_directory() -> PathBuf {
    profile::directory()
        .join("extensions-pending")
        .join("ublock-origin")
}

pub fn ublock_installed() -> bool {
    ublock_directory().join("manifest.json").is_file()
}

/// WebView2 cannot switch extensions on or off after startup, so the first answer sticks.
pub fn enabled_at_startup() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| crate::settings::get().ad_blocker)
}

pub fn active_at_startup() -> bool {
    static ACTIVE: OnceLock<bool> = OnceLock::new();
    *ACTIVE.get_or_init(|| enabled_at_startup() && ublock_installed())
}

pub fn status() -> &'static str {
    if ublock_installed() {
        "installed"
    } else if INSTALLING.load(Ordering::SeqCst) {
        "installing"
    } else if FAILED.load(Ordering::SeqCst) {
        "failed"
    } else {
        "missing"
    }
}

/// Runs before any tab exists. Swaps in a staged update, and retires uBlock Origin Lite once
/// uBlock Origin can replace it, so upgraded profiles are never left without either.
pub fn prepare() {
    let pending = pending_directory();
    if pending.join("manifest.json").is_file() {
        let previous = profile::directory().join("extensions-previous");
        let _ = std::fs::remove_dir_all(&previous);
        let _ = std::fs::create_dir_all(directory());
        let moved =
            !ublock_directory().exists() || std::fs::rename(ublock_directory(), &previous).is_ok();
        if moved && std::fs::rename(&pending, ublock_directory()).is_err() {
            let _ = std::fs::rename(&previous, ublock_directory());
        }
        let _ = std::fs::remove_dir_all(&previous);
        let _ = std::fs::remove_dir_all(&pending);
    }
    if ublock_installed() {
        let _ = std::fs::remove_dir_all(lite_directory());
    }
}

/// Downloads in the background unless already installed or in progress.
pub fn install_in_background(done: impl FnOnce(Result<(), String>) + Send + 'static) {
    if ublock_installed() || INSTALLING.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(move || {
        let result = download_ublock(&ublock_directory(), None).map(|_| ());
        FAILED.store(result.is_err(), Ordering::SeqCst);
        INSTALLING.store(false, Ordering::SeqCst);
        done(result);
    });
}

/// Stages a newer uBlock Origin release for the next start. Its filter lists update on their own.
pub fn update_in_background() {
    if !ublock_installed() || INSTALLING.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(|| {
        let _ = download_ublock(&pending_directory(), installed_version());
        INSTALLING.store(false, Ordering::SeqCst);
    });
}

fn installed_version() -> Option<String> {
    let manifest = std::fs::read_to_string(ublock_directory().join("manifest.json")).ok()?;
    let manifest: serde_json::Value = serde_json::from_str(&manifest).ok()?;
    Some(manifest.get("version")?.as_str()?.to_string())
}

/// uBlock Origin selects every list not marked `off` the first time it starts. It also swaps its
/// list catalog for the online copy, which resets that selection to its own defaults, so the
/// catalog is pinned to the bundled file. The lists themselves still update online.
fn enable_extra_lists(assets: &str) -> Result<String, String> {
    let mut assets: serde_json::Value =
        serde_json::from_str(assets).map_err(|error| error.to_string())?;
    for id in EXTRA_LISTS {
        if let Some(list) = assets.get_mut(id).and_then(|list| list.as_object_mut()) {
            list.remove("off");
        }
    }
    let catalog = assets
        .get_mut("assets.json")
        .and_then(|catalog| catalog.as_object_mut())
        .ok_or("uBlock Origin's list catalog has an unexpected format.")?;
    catalog.remove("cdnURLs");
    catalog.insert(
        "contentURL".into(),
        serde_json::json!(["assets/assets.json"]),
    );
    serde_json::to_string(&assets).map_err(|error| error.to_string())
}

fn configure(extension: &Path) -> Result<(), String> {
    let path = extension.join("assets").join("assets.json");
    let assets = std::fs::read_to_string(&path).map_err(|error| error.to_string())?;
    std::fs::write(&path, enable_extra_lists(&assets)?).map_err(|error| error.to_string())
}

/// The release zip wraps the extension in a `uBlock0.chromium` folder.
fn extension_root(unpacked: &Path) -> Option<PathBuf> {
    if unpacked.join("manifest.json").is_file() {
        return Some(unpacked.to_path_buf());
    }
    std::fs::read_dir(unpacked)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .find(|path| path.join("manifest.json").is_file())
}

/// Returns false when the latest release is already `installed`.
fn download_ublock(target: &Path, installed: Option<String>) -> Result<bool, String> {
    let json = updater::run_curl(&[
        "-s",
        "-f",
        "-L",
        "--max-time",
        "15",
        "-H",
        "User-Agent: Veluno-Browser",
        RELEASE_API,
    ])
    .ok_or("Could not reach GitHub to download the ad blocker.")?;
    let release: Release = serde_json::from_str(&json).map_err(|error| error.to_string())?;
    if installed.is_some_and(|version| version == release.tag_name) {
        return Ok(false);
    }
    let asset = release
        .assets
        .into_iter()
        .find(|asset| {
            asset.name.ends_with(".chromium.zip")
                && asset.browser_download_url.starts_with(DOWNLOAD_PREFIX)
        })
        .ok_or("The ad blocker release has no Chromium package.")?;
    let expected = asset
        .digest
        .as_deref()
        .and_then(|digest| digest.strip_prefix("sha256:"))
        .ok_or("The ad blocker release has no checksum.")?;

    // Outside `directory()` so WebView2 never sees a half-installed extension.
    let work = profile::directory().join("extensions-tmp");
    let _ = std::fs::remove_dir_all(&work);
    let unpacked = work.join("unpacked");
    std::fs::create_dir_all(&unpacked).map_err(|error| error.to_string())?;
    let zip = work.join("ublock.zip");
    let result = (|| {
        updater::download(&asset.browser_download_url, &zip)?;
        if !updater::sha256(&zip)?.eq_ignore_ascii_case(expected) {
            return Err("The ad blocker download did not match its checksum.".to_string());
        }
        let status = updater::hidden_command(updater::system_binary("System32\\tar.exe"))
            .arg("-xf")
            .arg(&zip)
            .arg("-C")
            .arg(&unpacked)
            .status()
            .map_err(|error| format!("Could not unpack the ad blocker: {error}"))?;
        let root = extension_root(&unpacked)
            .filter(|_| status.success())
            .ok_or("The ad blocker package could not be unpacked.")?;
        configure(&root)?;
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let _ = std::fs::remove_dir_all(target);
        std::fs::rename(&root, target).map_err(|error| error.to_string())
    })();
    let _ = std::fs::remove_dir_all(&work);
    result.map(|()| true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extra_lists_are_switched_on_and_others_stay_off() {
        let mut assets = serde_json::json!({
            "assets.json": {
                "content": "internal",
                "cdnURLs": ["https://cdn.test/assets.json"],
                "contentURL": ["https://raw.test/assets.json", "assets/assets.json"],
            },
            "easylist": { "content": "filters" },
            "adguard-social": { "content": "filters", "off": true },
        });
        for id in EXTRA_LISTS {
            assets[id] = serde_json::json!({ "content": "filters", "off": true });
        }
        let enabled: serde_json::Value =
            serde_json::from_str(&enable_extra_lists(&assets.to_string()).unwrap()).unwrap();
        for id in EXTRA_LISTS {
            assert!(enabled[id].get("off").is_none(), "{id} should be on");
        }
        assert_eq!(enabled["adguard-social"]["off"], true);
        assert!(enabled["assets.json"].get("cdnURLs").is_none());
        assert_eq!(
            enabled["assets.json"]["contentURL"],
            serde_json::json!(["assets/assets.json"])
        );
    }

    #[test]
    fn lists_dropped_upstream_are_skipped() {
        assert!(enable_extra_lists(r#"{"assets.json": {}, "easylist": {}}"#).is_ok());
        assert!(enable_extra_lists("not json").is_err());
    }

    #[test]
    fn unpacked_extension_ids_match_chromium_path_hashing() {
        use std::path::Path;
        assert_eq!(
            unpacked_id(Path::new(
                r"C:\Users\Admin\AppData\Local\Veluno\extensions\ublock-origin-lite\"
            )),
            "cjohkdcfmgoadbgijhcgnlaklopibldn"
        );
        assert_eq!(
            unpacked_id(Path::new(
                "C:/Users/Admin/AppData/Local/Felaris/extensions/ublock-origin-lite"
            )),
            "gmoloiomomdgacakcijolgcinbjgfohb"
        );
    }
}
