use std::path::{Path, PathBuf};

pub fn directory() -> PathBuf {
    static DIRECTORY: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    DIRECTORY
        .get_or_init(|| {
            std::env::var_os("VELUNO_PROFILE_DIR")
                .or_else(|| std::env::var_os("FELARIS_PROFILE_DIR"))
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    let base = std::env::var_os("LOCALAPPDATA")
                        .or_else(|| std::env::var_os("APPDATA"))
                        .map(PathBuf::from)
                        .unwrap_or_else(std::env::temp_dir);
                    migrate_directory(&base)
                })
        })
        .clone()
}

fn migrate_directory(base: &Path) -> PathBuf {
    let current = base.join("Veluno");
    let legacy = base.join("Felaris");
    if !current.exists() && legacy.is_dir() && std::fs::rename(&legacy, &current).is_err() {
        return legacy;
    }
    current
}

pub(crate) fn temporary_path(path: &Path) -> PathBuf {
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(".tmp");
    PathBuf::from(temporary)
}

pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temporary = temporary_path(path);
    let mut file = std::fs::File::create(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    let result = std::fs::rename(&temporary, path);
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

/// Returns the guard even after a panic poisoned the mutex; the stored values stay usable.
pub fn lock<T>(mutex: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

pub fn allowed_url(value: &str) -> bool {
    normalize_url(value).is_some()
}

pub fn is_internal(value: &str) -> bool {
    matches!(
        value,
        "veluno://newtab" | "veluno://blank" | "veluno://settings" | "veluno://history"
    )
}

pub fn normalize_url(value: &str) -> Option<String> {
    if let Some(page) = value.strip_prefix("felaris://") {
        let migrated = format!("veluno://{page}");
        return is_internal(&migrated).then_some(migrated);
    }
    if is_internal(value) {
        return Some(value.into());
    }
    let mut url = url::Url::parse(value).ok()?;
    if matches!(url.scheme(), "http" | "https")
        && url.host_str() == Some("drive.google.com")
        && (url.path() == "/" || url.path().is_empty())
        && url.query().is_none()
        && url.fragment().is_none()
    {
        url.set_path("/drive");
        return Some(url.into());
    }
    matches!(url.scheme(), "http" | "https" | "file" | "blob").then(|| url.into())
}

pub fn web_origin(value: &str) -> Option<String> {
    let url = url::Url::parse(value).ok()?;
    matches!(url.scheme(), "http" | "https").then(|| url.origin().ascii_serialization())
}

pub fn is_auth_url(value: &str) -> bool {
    let Ok(url) = url::Url::parse(value) else {
        return false;
    };
    if !matches!(url.scheme(), "http" | "https") {
        return false;
    }
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    let path = url.path().to_ascii_lowercase();
    let query = url.query().unwrap_or_default().to_ascii_lowercase();
    matches!(
        host.as_str(),
        "accounts.google.com"
            | "login.microsoftonline.com"
            | "login.live.com"
            | "appleid.apple.com"
            | "id.atlassian.com"
    ) || host.starts_with("auth.")
        || host.starts_with("login.")
        || host.starts_with("sso.")
        || host.starts_with("id.")
        || host.ends_with(".auth0.com")
        || host.ends_with(".okta.com")
        || host.ends_with(".clerk.accounts.dev")
        || [
            "/oauth",
            "/authorize",
            "/login",
            "/signin",
            "/sign-in",
            "/auth/",
            "/sso",
            "/session",
            "/callback",
            "/confirm",
            "/installations/",
        ]
        .iter()
        .any(|needle| path.contains(needle))
        || [
            "client_id=",
            "redirect_uri=",
            "response_type=",
            "oauth_token=",
            "code_challenge=",
        ]
        .iter()
        .any(|needle| query.contains(needle))
}

pub fn startup_url(value: &str) -> Option<String> {
    if let Some(url) = normalize_url(value) {
        return Some(url);
    }
    let path = Path::new(value);
    if path.is_absolute() {
        return url::Url::from_file_path(path).ok().map(Into::into);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poisoned_locks_keep_their_data() {
        let values = std::sync::Mutex::new(vec![1, 2, 3]);
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = values.lock().unwrap();
            panic!("poison the lock");
        }));
        assert!(values.is_poisoned());
        assert_eq!(*lock(&values), vec![1, 2, 3]);
    }

    #[test]
    fn legacy_profile_moves_without_overwriting_a_current_profile() {
        let base = std::env::temp_dir().join(format!("veluno-migration-{}", std::process::id()));
        let legacy = base.join("Felaris");
        let current = base.join("Veluno");
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::write(legacy.join("session.json"), b"saved tabs").unwrap();
        assert_eq!(migrate_directory(&base), current);
        assert!(!legacy.exists());
        assert_eq!(
            std::fs::read(current.join("session.json")).unwrap(),
            b"saved tabs"
        );
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::write(legacy.join("session.json"), b"other tabs").unwrap();
        assert_eq!(migrate_directory(&base), current);
        assert_eq!(
            std::fs::read(current.join("session.json")).unwrap(),
            b"saved tabs"
        );
        assert_eq!(
            std::fs::read(legacy.join("session.json")).unwrap(),
            b"other tabs"
        );
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn legacy_internal_urls_are_migrated_without_accepting_unknown_pages() {
        for page in ["newtab", "blank", "settings", "history"] {
            assert_eq!(
                normalize_url(&format!("felaris://{page}")),
                Some(format!("veluno://{page}"))
            );
        }
        assert!(normalize_url("felaris://other").is_none());
        assert!(normalize_url("felaris://settings/../../other").is_none());
    }

    #[test]
    fn atomic_writes_replace_the_previous_file() {
        let directory = std::env::temp_dir().join(format!("veluno-atomic-{}", std::process::id()));
        let path = directory.join("session.json");
        write_atomic(&path, b"first").unwrap();
        write_atomic(&path, b"second").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"second");
        assert!(!temporary_path(&path).exists());
        assert_ne!(
            temporary_path(&directory.join("session.json")),
            temporary_path(&directory.join("session.bak"))
        );
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn external_links_reject_commands_and_accept_windows_html_paths() {
        assert!(startup_url("https://example.com/a?b=c").is_some());
        assert!(startup_url("javascript:alert(1)").is_none());
        assert!(startup_url("--install").is_none());
        #[cfg(windows)]
        assert_eq!(
            startup_url(r"C:\Browser tests\hello.html").as_deref(),
            Some("file:///C:/Browser%20tests/hello.html")
        );
    }

    #[test]
    fn navigation_urls_match_browser_normalization() {
        assert_eq!(
            normalize_url("HTTPS://Example.COM").as_deref(),
            Some("https://example.com/")
        );
        assert_eq!(
            normalize_url("https://example.com/café").as_deref(),
            Some("https://example.com/caf%C3%A9")
        );
        assert_eq!(
            normalize_url("veluno://newtab").as_deref(),
            Some("veluno://newtab")
        );
        assert_eq!(
            normalize_url("veluno://settings").as_deref(),
            Some("veluno://settings")
        );
        assert_eq!(
            normalize_url("https://drive.google.com").as_deref(),
            Some("https://drive.google.com/drive")
        );
        assert_eq!(
            normalize_url("https://drive.google.com/").as_deref(),
            Some("https://drive.google.com/drive")
        );
        assert_eq!(
            normalize_url("https://drive.google.com/drive/my-drive").as_deref(),
            Some("https://drive.google.com/drive/my-drive")
        );
        assert_eq!(
            normalize_url("blob:https://drive.google.com/7b1486be-4607-d53d-9f93-841b7c776bc5")
                .as_deref(),
            Some("blob:https://drive.google.com/7b1486be-4607-d53d-9f93-841b7c776bc5")
        );
        assert_eq!(
            normalize_url("blob:null/8a8e96d5-bb10-4eb2-b3a2-ec53bf8c0044").as_deref(),
            Some("blob:null/8a8e96d5-bb10-4eb2-b3a2-ec53bf8c0044")
        );
        assert!(normalize_url("veluno://other").is_none());
    }

    #[test]
    fn auth_urls_detect_oauth_login_and_installation_flows() {
        assert!(is_auth_url(
            "https://github.com/settings/installations/173231206"
        ));
        assert!(is_auth_url(
            "https://github.com/login/oauth/authorize?client_id=abc"
        ));
        assert!(is_auth_url(
            "https://accounts.google.com/o/oauth2/v2/auth?response_type=code"
        ));
        assert!(!is_auth_url("https://doc.rust-lang.org/book/"));
        assert!(is_auth_url("https://example.com/api/callback?code=1"));
        assert!(!is_auth_url("https://example.com/dashboard"));
        assert!(!is_auth_url("http://127.0.0.1:8080/popup"));
    }
}
