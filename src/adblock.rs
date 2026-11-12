//! Brave's ad blocking engine (adblock-rust) running Brave's default filter lists.
//!
//! Lists come from Brave's own catalog so Veluno blocks what Brave blocks by default, and are
//! rebuilt once a day. uBlock Origin runs alongside for uBlock's scriptlets, which Brave does not
//! publish in a form adblock-rust can load directly. This engine needs no extension support, so it
//! keeps blocking if WebView2 ever stops running Manifest V2 extensions such as uBlock Origin.

use crate::{profile, updater};
use adblock::Engine;
use adblock::lists::{FilterSet, ParseOptions};
use adblock::request::Request;
use adblock::resources::{PermissionMask, Resource};
use base64::{Engine as _, prelude::BASE64_STANDARD};
use serde::Deserialize;
use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, SystemTime};

const CATALOG: &str = "https://raw.githubusercontent.com/brave/adblock-resources/master/filter_lists/list_catalog.json";
const RESOURCES: &str =
    "https://raw.githubusercontent.com/brave/adblock-resources/master/dist/resources.json";
const REFRESH: Duration = Duration::from_secs(24 * 60 * 60);
const RETRY: Duration = Duration::from_secs(60 * 60);
/// Filter lists downloaded at once.
const FETCH_WORKERS: usize = 6;

/// Bumped whenever a freshly built engine lands on disk, so tabs reload it.
static GENERATION: AtomicU64 = AtomicU64::new(0);

thread_local! {
    static ENGINE: RefCell<Option<(u64, Option<Engine>)>> = const { RefCell::new(None) };
}

#[derive(Deserialize)]
struct CatalogList {
    #[serde(default)]
    default_enabled: bool,
    #[serde(default)]
    platforms: Option<Vec<String>>,
    #[serde(default)]
    permission_mask: u8,
    sources: Vec<CatalogSource>,
}

#[derive(Deserialize)]
struct CatalogSource {
    url: String,
    #[serde(default)]
    format: Option<String>,
}

pub enum Block {
    Cancel,
    /// A harmless stand-in body, such as an empty script, for pages that break without one.
    Replace {
        mime: String,
        body: Vec<u8>,
    },
}

fn directory() -> PathBuf {
    profile::directory().join("adblock")
}

fn engine_path() -> PathBuf {
    directory().join("engine.dat")
}

fn resources_path() -> PathBuf {
    directory().join("resources.json")
}

pub fn enabled() -> bool {
    crate::extensions::enabled_at_startup()
}

/// Seconds since the Unix epoch when the filter lists were last rebuilt.
pub fn updated_at() -> Option<u64> {
    let modified = std::fs::metadata(engine_path()).ok()?.modified().ok()?;
    Some(
        modified
            .duration_since(SystemTime::UNIX_EPOCH)
            .ok()?
            .as_secs(),
    )
}

fn load() -> Option<Engine> {
    let data = std::fs::read(engine_path()).ok()?;
    let mut engine = Engine::default();
    engine.deserialize(&data).ok()?;
    if let Some(resources) = std::fs::read_to_string(resources_path())
        .ok()
        .and_then(|json| serde_json::from_str::<Vec<Resource>>(&json).ok())
    {
        engine.use_resources(resources);
    }
    Some(engine)
}

fn with_engine<R>(f: impl FnOnce(&Engine) -> R) -> Option<R> {
    if !enabled() {
        return None;
    }
    ENGINE.with(|cell| {
        let generation = GENERATION.load(Ordering::SeqCst);
        let mut slot = cell.borrow_mut();
        if slot.as_ref().map(|(loaded, _)| *loaded) != Some(generation) {
            *slot = Some((generation, load()));
        }
        slot.as_ref()?.1.as_ref().map(f)
    })
}

/// `page` is the top-level document; `kind` uses adblock-rust's request type names.
pub fn check(url: &str, page: &str, kind: &str, method: &str) -> Option<Block> {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return None;
    }
    let source = if page.starts_with("http") { page } else { url };
    with_engine(|engine| {
        let request = Request::new(url, source, kind, method).ok()?;
        let result = engine.check_network_request(&request);
        if let Some(redirect) = &result.redirect
            && let Some(block) = data_url(redirect)
        {
            return Some(block);
        }
        result.should_block().then_some(Block::Cancel)
    })
    .flatten()
}

fn data_url(url: &str) -> Option<Block> {
    let (mime, data) = url.strip_prefix("data:")?.split_once(";base64,")?;
    let body = BASE64_STANDARD.decode(data).ok()?;
    Some(Block::Replace {
        mime: mime.to_string(),
        body,
    })
}

/// Hides the page's known ad slots and runs Brave's fixes for it, only on the page's own host.
pub fn cosmetic_script(url: &str) -> Option<String> {
    let host = url::Url::parse(url).ok()?.host_str()?.to_string();
    let resources = with_engine(|engine| engine.url_cosmetic_resources(url))?;
    let mut selectors: Vec<_> = resources.hide_selectors.into_iter().collect();
    selectors.sort();
    // One rule per selector: a single unsupported selector would void a combined rule.
    use std::fmt::Write;
    let mut css = String::new();
    for selector in &selectors {
        let _ = writeln!(css, "{selector}{{display:none!important}}");
    }
    if css.is_empty() && resources.injected_script.is_empty() {
        return None;
    }
    // Runs before the page parses, so scriptlets go first and the stylesheet waits for <html>.
    Some(format!(
        "(() => {{
 if (location.hostname !== {host}) return;
 try {{
{script}
 }} catch {{}}
 const css = {css};
 if (!css) return;
 const style = document.createElement('style');
 style.textContent = css;
 const add = () => (document.head || document.documentElement).append(style);
 if (document.documentElement) return add();
 new MutationObserver((_, observer) => {{
  if (!document.documentElement) return;
  observer.disconnect();
  add();
 }}).observe(document, {{ childList: true }});
}})();",
        host = serde_json::to_string(&host).ok()?,
        css = serde_json::to_string(&css).ok()?,
        script = resources.injected_script,
    ))
}

/// Rebuilds the engine at startup when the lists are a day old, then daily while Veluno runs.
pub fn update_in_background() {
    if !enabled() {
        return;
    }
    std::thread::spawn(|| {
        loop {
            let age = std::fs::metadata(engine_path())
                .and_then(|metadata| metadata.modified())
                .ok()
                .and_then(|modified| SystemTime::now().duration_since(modified).ok());
            std::thread::sleep(age.map_or(Duration::ZERO, |age| REFRESH.saturating_sub(age)));
            if rebuild().is_ok() {
                GENERATION.fetch_add(1, Ordering::SeqCst);
            } else {
                std::thread::sleep(RETRY);
            }
        }
    });
}

fn fetch(url: &str) -> Option<String> {
    updater::run_curl(&[
        "-s",
        "-f",
        "-L",
        "--max-time",
        "60",
        "--proto",
        "=https",
        "--proto-redir",
        "=https",
        "-H",
        "User-Agent: Veluno-Browser",
        url,
    ])
}

/// Downloads `urls` a few at a time. The results keep the order of `urls`.
fn fetch_all(
    urls: &[&str],
    download: impl Fn(&str) -> Option<String> + Sync,
) -> Vec<Option<String>> {
    let next = AtomicUsize::new(0);
    let mut results = vec![None; urls.len()];
    std::thread::scope(|scope| {
        let workers: Vec<_> = (0..FETCH_WORKERS.min(urls.len()))
            .map(|_| {
                scope.spawn(|| {
                    let mut fetched = Vec::new();
                    loop {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        let Some(&url) = urls.get(index) else {
                            break fetched;
                        };
                        fetched.push((index, download(url)));
                    }
                })
            })
            .collect();
        for worker in workers {
            if let Ok(fetched) = worker.join() {
                for (index, text) in fetched {
                    results[index] = text;
                }
            }
        }
    });
    results
}

fn rebuild() -> Result<(), String> {
    let catalog: Vec<CatalogList> = serde_json::from_str(
        &fetch(CATALOG).ok_or("Could not download Brave's filter list catalog.")?,
    )
    .map_err(|error| error.to_string())?;
    let sources: Vec<(ParseOptions, &str)> = catalog
        .iter()
        .filter(|list| wanted(list))
        .flat_map(|list| {
            let options = ParseOptions {
                permissions: PermissionMask::from_bits(list.permission_mask),
                ..ParseOptions::default()
            };
            list.sources
                .iter()
                .filter(|source| wanted_source(source))
                .map(move |source| (options, source.url.as_str()))
        })
        .collect();
    let urls: Vec<&str> = sources.iter().map(|(_, url)| *url).collect();
    // The resources file does not depend on the lists, so it downloads alongside them.
    let (texts, resources) = std::thread::scope(|scope| {
        let resources = scope.spawn(|| fetch(RESOURCES));
        let texts = fetch_all(&urls, fetch);
        (texts, resources.join().ok().flatten())
    });
    let mut filters = FilterSet::new(false);
    let (mut loaded, mut failed) = (0, 0);
    for ((options, _), text) in sources.iter().zip(texts) {
        match text {
            Some(text) => {
                filters.add_filter_list(text, *options);
                loaded += 1;
            }
            None => failed += 1,
        }
    }
    // Keep the previous engine rather than replace it with a mostly empty one.
    if loaded == 0 || failed > loaded {
        return Err(format!(
            "Only {loaded} of {} filter lists downloaded.",
            loaded + failed
        ));
    }
    let resources = resources
        .filter(|json| serde_json::from_str::<Vec<Resource>>(json).is_ok())
        .ok_or("Could not download Brave's ad blocking resources.")?;
    let engine = Engine::new_with_filter_set(filters);

    profile::write_atomic(&resources_path(), resources.as_bytes())
        .map_err(|error| error.to_string())?;
    profile::write_atomic(&engine_path(), &engine.serialize()).map_err(|error| error.to_string())
}

fn wanted(list: &CatalogList) -> bool {
    list.default_enabled
        && list
            .platforms
            .as_ref()
            .is_none_or(|platforms| platforms.iter().any(|platform| platform == "WINDOWS"))
}

fn wanted_source(source: &CatalogSource) -> bool {
    source
        .format
        .as_deref()
        .is_none_or(|format| format == "Standard")
        && !source.url.contains("android-specific")
        && !source.url.contains("ios-specific")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, mpsc};

    fn engine(rules: &[&str]) -> Engine {
        let mut filters = FilterSet::new(false);
        filters.add_filter_list(rules.join("\n"), ParseOptions::default());
        let mut engine = Engine::new_with_filter_set(filters);
        engine.use_resources([Resource {
            name: "noopjs".into(),
            aliases: vec!["noop.js".into()],
            kind: adblock::resources::ResourceType::Mime(
                adblock::resources::MimeType::ApplicationJavascript,
            ),
            content: BASE64_STANDARD.encode("(function(){})()"),
            dependencies: vec![],
            permission: PermissionMask::default(),
        }]);
        engine
    }

    #[test]
    fn filter_lists_download_at_once_and_keep_their_order() {
        // Each download waits for the other to start, so this passes only when both run together.
        let (a_tx, a_rx) = mpsc::channel::<()>();
        let (b_tx, b_rx) = mpsc::channel::<()>();
        let (a_rx, b_rx) = (Mutex::new(a_rx), Mutex::new(b_rx));
        let texts = fetch_all(&["a", "b"], |url| {
            let (sent, received) = if url == "a" {
                (&a_tx, &b_rx)
            } else {
                (&b_tx, &a_rx)
            };
            sent.send(()).ok()?;
            received
                .lock()
                .ok()?
                .recv_timeout(Duration::from_secs(5))
                .ok()?;
            Some(url.to_string())
        });
        assert_eq!(texts, [Some("a".into()), Some("b".into())]);
    }

    #[test]
    fn serialized_engines_still_block() {
        let built = engine(&["||ads.example^$third-party"]);
        let mut loaded = Engine::default();
        loaded.deserialize(&built.serialize()).unwrap();
        let blocked = Request::new(
            "https://ads.example/a.js",
            "https://news.test/",
            "script",
            "GET",
        )
        .unwrap();
        let first_party = Request::new(
            "https://ads.example/a.js",
            "https://ads.example/",
            "script",
            "GET",
        )
        .unwrap();
        assert!(loaded.check_network_request(&blocked).should_block());
        assert!(!loaded.check_network_request(&first_party).should_block());
    }

    #[test]
    fn redirects_become_replacement_bodies() {
        let engine = engine(&["||ads.example/gpt.js$script,redirect=noop.js"]);
        let request = Request::new(
            "https://ads.example/gpt.js",
            "https://news.test/",
            "script",
            "GET",
        )
        .unwrap();
        let result = engine.check_network_request(&request);
        let Some(Block::Replace { mime, body }) = data_url(result.redirect.as_deref().unwrap())
        else {
            panic!("expected a replacement body");
        };
        assert_eq!(mime, "application/javascript");
        assert_eq!(body, b"(function(){})()");
    }

    #[test]
    fn catalog_skips_mobile_only_lists() {
        let catalog: Vec<CatalogList> = serde_json::from_str(
            r#"[
                {"default_enabled": true, "sources": [
                    {"url": "https://lists.test/easylist.txt", "format": "Standard"},
                    {"url": "https://lists.test/brave-android-specific.txt", "format": "Standard"}
                ]},
                {"default_enabled": true, "platforms": ["IOS"], "sources": [{"url": "https://lists.test/ios.txt"}]},
                {"default_enabled": false, "sources": [{"url": "https://lists.test/optional.txt"}]}
            ]"#,
        )
        .unwrap();
        let urls: Vec<_> = catalog
            .iter()
            .filter(|list| wanted(list))
            .flat_map(|list| list.sources.iter().filter(|source| wanted_source(source)))
            .map(|source| source.url.as_str())
            .collect();
        assert_eq!(urls, ["https://lists.test/easylist.txt"]);
    }
}
