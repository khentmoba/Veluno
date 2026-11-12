use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::process::Command;

pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const GITHUB_REPO: &str = "khentmoba/Veluno";
pub const MANIFEST_URL: &str =
    "https://raw.githubusercontent.com/khentmoba/Veluno/main/version.json";
pub const GITHUB_LATEST_API: &str = "https://api.github.com/repos/khentmoba/Veluno/releases/latest";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    /// A fourth numeric segment, which Windows installers sometimes add.
    pub revision: u32,
    /// A pre-release tag such as `-beta` sorts before the release with the same numbers.
    pub prerelease: bool,
}

impl Version {
    pub fn parse(s: &str) -> Option<Self> {
        let clean = s.trim().trim_start_matches(['v', 'V']);
        // Build metadata after '+' never affects ordering.
        let core = clean.split(['-', '+']).next()?;
        let prerelease = clean
            .split_once('-')
            .is_some_and(|(_, tag)| !tag.is_empty());
        let mut parts = core.split('.');
        let major = parts.next()?.parse().ok()?;
        let minor = parts.next()?.parse().ok()?;
        let patch = parts.next().unwrap_or("0").parse().ok()?;
        let revision = parts.next().unwrap_or("0").parse().ok()?;
        if parts.next().is_some() {
            return None;
        }
        Some(Self {
            major,
            minor,
            patch,
            revision,
            prerelease,
        })
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.major, self.minor, self.patch, self.revision)
            .cmp(&(other.major, other.minor, other.patch, other.revision))
            .then_with(|| match (self.prerelease, other.prerelease) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => std::cmp::Ordering::Equal,
            })
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateManifest {
    pub version: String,
    pub release_url: String,
    pub download_url: Option<String>,
    pub summary: String,
}

#[derive(Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Deserialize)]
struct GhRelease {
    tag_name: String,
    html_url: String,
    body: Option<String>,
    assets: Option<Vec<GhAsset>>,
}

const SETUP_TEMP: &str = "veluno-update-setup.exe";

pub fn cleanup_old_version() {
    let _ = std::fs::remove_file(std::env::temp_dir().join(SETUP_TEMP));
    if let Ok(current) = std::env::current_exe() {
        let old = current.with_extension("exe.old");
        if old.exists() {
            let _ = std::fs::remove_file(old);
        }
    }
}

pub fn check_for_updates() -> Result<Option<UpdateManifest>, String> {
    // 1. Try static manifest (fast, zero rate limits)
    if let Some(manifest) = fetch_manifest(MANIFEST_URL) {
        if is_newer(&manifest.version, CURRENT_VERSION) {
            return Ok(Some(manifest));
        }
        return Ok(None);
    }

    let manifest = fetch_github_latest()
        .ok_or("Could not check for updates. Check your connection and try again.")?;
    Ok(is_newer(&manifest.version, CURRENT_VERSION).then_some(manifest))
}

pub fn is_newer(remote: &str, current: &str) -> bool {
    match (Version::parse(remote), Version::parse(current)) {
        (Some(r), Some(c)) => r > c,
        _ => false,
    }
}

pub(crate) fn run_curl(args: &[&str]) -> Option<String> {
    let output = hidden_command(system_binary("System32\\curl.exe"))
        .args(args)
        .output()
        .ok()?;
    if output.status.success() {
        String::from_utf8(output.stdout).ok()
    } else {
        None
    }
}

fn fetch_manifest(url: &str) -> Option<UpdateManifest> {
    let json_str = run_curl(&["-s", "-f", "-L", "--max-time", "6", url])?;
    serde_json::from_str::<UpdateManifest>(&json_str).ok()
}

fn fetch_github_latest() -> Option<UpdateManifest> {
    let json_str = run_curl(&[
        "-s",
        "-f",
        "-L",
        "--max-time",
        "6",
        "-H",
        "User-Agent: Veluno-Browser",
        GITHUB_LATEST_API,
    ])?;

    let release = serde_json::from_str::<GhRelease>(&json_str).ok()?;
    let exe_asset = release.assets.as_ref().and_then(|assets| {
        assets
            .iter()
            .find(|a| a.name.eq_ignore_ascii_case("veluno.exe"))
            .map(|a| a.browser_download_url.clone())
    });

    Some(UpdateManifest {
        version: release.tag_name.trim_start_matches(['v', 'V']).to_string(),
        release_url: release.html_url,
        download_url: exe_asset,
        summary: release
            .body
            .unwrap_or_else(|| "Veluno update available.".to_string()),
    })
}

pub fn download_and_apply_update(download_url: &str) -> Result<(), String> {
    let current_exe =
        std::env::current_exe().map_err(|e| format!("Cannot locate current executable: {e}"))?;
    let target_dir = current_exe
        .parent()
        .ok_or_else(|| "Invalid executable parent directory".to_string())?;
    validate_download_url(download_url)?;

    // Copies made by veluno-setup.exe update through it so Windows keeps the right version.
    let installed = target_dir.join("unins000.exe").is_file();
    let legacy = download_url.ends_with("/felaris.exe");
    let (asset, file) = if installed {
        (
            if legacy {
                "felaris-setup.exe"
            } else {
                "veluno-setup.exe"
            },
            std::env::temp_dir().join(SETUP_TEMP),
        )
    } else {
        (
            if legacy { "felaris.exe" } else { "veluno.exe" },
            target_dir.join("veluno.exe.new"),
        )
    };
    let release = download_url.rsplit_once('/').map_or("", |(base, _)| base);
    // The checksum list does not depend on the download, so it is fetched while the file downloads.
    let sums_url = format!("{release}/SHA256SUMS");
    let sums = std::thread::spawn(move || fetch_sums(&sums_url));
    download(&format!("{release}/{asset}"), &file)?;
    if let Err(error) = verify_download(sums, asset, &file) {
        let _ = std::fs::remove_file(&file);
        return Err(error);
    }
    if let Err(error) = verify_publisher(&current_exe, &file) {
        let _ = std::fs::remove_file(&file);
        return Err(error);
    }

    if installed {
        let mut command = Command::new(&file);
        command.args([
            "/VERYSILENT",
            "/SUPPRESSMSGBOXES",
            "/NORESTART",
            "/CLOSEAPPLICATIONS",
        ]);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0000_0008 | 0x0000_0200);
        }
        command
            .spawn()
            .map_err(|e| format!("Could not start the update installer: {e}"))?;
    } else {
        replace_and_launch(&current_exe, &file)?;
    }
    std::process::exit(0);
}

pub(crate) fn download(url: &str, file: &std::path::Path) -> Result<(), String> {
    let status = hidden_command(system_binary("System32\\curl.exe"))
        .args([
            "-s",
            "-f",
            "-L",
            "--max-time",
            "120",
            "--proto",
            "=https",
            "--proto-redir",
            "=https",
            "--connect-timeout",
            "10",
            "-o",
            file.to_str().ok_or("Invalid temp path")?,
            url,
        ])
        .status()
        .map_err(|e| format!("Failed to spawn curl: {e}"))?;
    if !status.success() {
        let _ = std::fs::remove_file(file);
        return Err(format!("Download failed with status: {status}"));
    }
    Ok(())
}

fn fetch_sums(sums_url: &str) -> Option<String> {
    run_curl(&[
        "-s",
        "-f",
        "-L",
        "--max-time",
        "15",
        "--proto",
        "=https",
        "--proto-redir",
        "=https",
        sums_url,
    ])
}

fn verify_download(
    sums: std::thread::JoinHandle<Option<String>>,
    asset: &str,
    file: &std::path::Path,
) -> Result<(), String> {
    let bytes = std::fs::read(file).map_err(|error| error.to_string())?;
    if bytes.len() < 500_000 {
        return Err(format!(
            "Downloaded file too small ({} bytes); update aborted",
            bytes.len()
        ));
    }
    if matches!(asset, "veluno.exe" | "felaris.exe") && !valid_executable(&bytes) {
        return Err(
            "The downloaded file is not a compatible Windows app. Your current version was kept."
                .into(),
        );
    }
    let sums = sums
        .join()
        .ok()
        .flatten()
        .ok_or("Could not fetch the release checksums. Your current version was kept.")?;
    let expected = checksum_for(&sums, asset)
        .ok_or("This release has no checksum for the update. Your current version was kept.")?;
    let actual = sha256(file)?;
    if !actual.eq_ignore_ascii_case(&expected) {
        return Err(
            "The download did not match its checksum. Your current version was kept.".into(),
        );
    }
    Ok(())
}

pub(crate) fn sha256(file: &std::path::Path) -> Result<String, String> {
    let mut reader =
        std::fs::File::open(file).map_err(|e| format!("Could not verify the download: {e}"))?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut reader, &mut hasher)
        .map_err(|e| format!("Could not verify the download: {e}"))?;
    Ok(format!("{:x}", hasher.finalize()))
}

fn checksum_for(sums: &str, asset: &str) -> Option<String> {
    sums.lines().find_map(|line| {
        let (hash, name) = line.split_once("  ")?;
        (name.trim() == asset).then(|| hash.trim().to_string())
    })
}

#[derive(Debug, PartialEq, Eq)]
enum Signer {
    Unsigned,
    Valid(String),
    Broken,
}

fn pe_bytes_have_no_certificate_table(bytes: &[u8]) -> Option<bool> {
    if bytes.len() < 64 || &bytes[..2] != b"MZ" {
        return None;
    }
    let pe = u32::from_le_bytes(bytes[60..64].try_into().ok()?) as usize;
    if bytes.get(pe..pe.checked_add(4)?)? != b"PE\0\0" {
        return None;
    }
    let opt = pe.checked_add(24)?;
    let magic = u16::from_le_bytes(bytes.get(opt..opt.checked_add(2)?)?.try_into().ok()?);
    let (rva_count_offset, data_dir_offset) = match magic {
        0x10b => (92usize, 96usize),
        0x20b => (108usize, 112usize),
        _ => return None,
    };
    let count_pos = opt.checked_add(rva_count_offset)?;
    let rva_count = u32::from_le_bytes(
        bytes
            .get(count_pos..count_pos.checked_add(4)?)?
            .try_into()
            .ok()?,
    );
    if rva_count <= 4 {
        return Some(true);
    }
    let security_pos = opt.checked_add(data_dir_offset)?.checked_add(32)?;
    let entry = bytes.get(security_pos..security_pos.checked_add(8)?)?;
    Some(entry == [0; 8])
}

fn pe_has_no_certificate_table(file: &std::path::Path) -> bool {
    use std::io::Read as _;
    let Ok(mut reader) = std::fs::File::open(file) else {
        return false;
    };
    let mut header = [0u8; 4096];
    let Ok(read) = reader.read(&mut header) else {
        return false;
    };
    pe_bytes_have_no_certificate_table(&header[..read]).unwrap_or(false)
}

fn signer(file: &std::path::Path) -> Result<Signer, String> {
    if pe_has_no_certificate_table(file) {
        return Ok(Signer::Unsigned);
    }
    use base64::Engine as _;
    const SCRIPT: &str = r#"
$signature = Get-AuthenticodeSignature -LiteralPath $env:VELUNO_SIGNED_FILE
if (-not $signature) { exit 1 }
$subject = ''
if ($signature.SignerCertificate) { $subject = $signature.SignerCertificate.Subject }
Write-Output $signature.Status.ToString()
Write-Output ([Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($subject)))
"#;
    let encoded = base64::engine::general_purpose::STANDARD.encode(
        SCRIPT
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<u8>>(),
    );
    let output = hidden_command(system_binary(
        "System32\\WindowsPowerShell\\v1.0\\powershell.exe",
    ))
    .env("VELUNO_SIGNED_FILE", file)
    .args(["-NoProfile", "-NonInteractive", "-EncodedCommand", &encoded])
    .output()
    .map_err(|error| format!("Could not verify the update's signature: {error}"))?;
    let text = String::from_utf8_lossy(&output.stdout);
    let mut lines = text.lines();
    let status = lines.next().unwrap_or_default().trim();
    let subject = lines.next().unwrap_or_default().trim();
    match status {
        "" => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(format!(
                "Could not verify the update's signature (exit {}, {}).",
                output.status,
                stderr.trim()
            ))
        }
        "NotSigned" => Ok(Signer::Unsigned),
        "Valid" => {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(subject)
                .map_err(|_| "Could not verify the update's signature.".to_string())?;
            Ok(Signer::Valid(String::from_utf8_lossy(&bytes).into_owned()))
        }
        _ => Ok(Signer::Broken),
    }
}

/// Pins an update to the publisher of a validly signed running build. Unsigned builds accept
/// unsigned or validly signed updates. A running build with a broken signature accepts nothing.
fn signer_allows_update(running: &Signer, update: &Signer) -> bool {
    match (running, update) {
        (Signer::Broken, _) => false,
        (Signer::Valid(installed), Signer::Valid(new)) => installed == new,
        (Signer::Valid(_), _) => false,
        (Signer::Unsigned, Signer::Unsigned | Signer::Valid(_)) => true,
        (Signer::Unsigned, Signer::Broken) => false,
    }
}

fn verify_publisher(current_exe: &std::path::Path, update: &std::path::Path) -> Result<(), String> {
    if signer_allows_update(&signer(current_exe)?, &signer(update)?) {
        Ok(())
    } else {
        Err("The update's signature could not be trusted. Your current version was kept.".into())
    }
}

fn replace_and_launch(
    current_exe: &std::path::Path,
    new_exe: &std::path::Path,
) -> Result<(), String> {
    let old_exe = current_exe.with_extension("exe.old");
    if old_exe.exists() {
        std::fs::remove_file(&old_exe)
            .map_err(|error| format!("Could not clear the previous update backup: {error}"))?;
    }
    std::fs::rename(current_exe, &old_exe)
        .map_err(|error| format!("Could not back up the running app: {error}"))?;
    if let Err(error) = std::fs::rename(new_exe, current_exe) {
        std::fs::rename(&old_exe, current_exe).map_err(|recovery| {
            format!(
                "Update failed ({error}); recovery failed ({recovery}). Your previous app is at {}",
                old_exe.display()
            )
        })?;
        return Err(format!(
            "Update could not be installed; your previous version was restored: {error}"
        ));
    }
    if let Err(error) = Command::new(current_exe).arg("--updated").spawn() {
        std::fs::remove_file(current_exe).and_then(|_| std::fs::rename(&old_exe, current_exe))
            .map_err(|recovery| format!("The update could not start ({error}); recovery failed ({recovery}). Your previous app is at {}", old_exe.display()))?;
        return Err(format!(
            "The update could not start; your previous version was restored: {error}"
        ));
    }
    Ok(())
}

/// A Windows program under `%SystemRoot%`. A bare name could resolve to a file in the working directory.
pub(crate) fn system_binary(relative: &str) -> std::path::PathBuf {
    let root = std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into());
    std::path::Path::new(&root).join(relative)
}

pub(crate) fn hidden_command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut command = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command
}

fn validate_download_url(value: &str) -> Result<(), String> {
    let url = url::Url::parse(value).map_err(|_| "Invalid update address")?;
    if url.scheme() != "https"
        || url.host_str() != Some("github.com")
        || !url.username().is_empty()
        || url.password().is_some()
        || ![
            "/khentmoba/Veluno/releases/download/",
            "/khentmoba/Felaris/releases/download/",
        ]
        .iter()
        .any(|prefix| url.path().starts_with(prefix))
        || !["/veluno.exe", "/felaris.exe"]
            .iter()
            .any(|asset| url.path().ends_with(asset))
        || url.port().is_some()
    {
        return Err("Update address is not an official Veluno release.".into());
    }
    Ok(())
}

fn valid_executable(bytes: &[u8]) -> bool {
    if bytes.len() < 64 || &bytes[..2] != b"MZ" {
        return false;
    }
    let offset = u32::from_le_bytes(bytes[60..64].try_into().unwrap()) as usize;
    let Some(header) = bytes.get(offset..offset.saturating_add(94)) else {
        return false;
    };
    let machine = u16::from_le_bytes([header[4], header[5]]);
    let expected = if cfg!(target_arch = "aarch64") {
        0xaa64
    } else {
        0x8664
    };
    &header[..4] == b"PE\0\0"
        && machine == expected
        && u16::from_le_bytes([header[92], header[93]]) == 2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(windows)]
    fn verify_download_checks_the_fetched_checksums() {
        let file = std::env::temp_dir().join(format!("veluno-sums-{}.bin", std::process::id()));
        std::fs::write(&file, vec![7u8; 500_000]).unwrap();
        let listed = format!("{}  veluno-setup.exe\n", sha256(&file).unwrap());
        let fetched = std::thread::spawn(move || Some(listed));
        assert!(verify_download(fetched, "veluno-setup.exe", &file).is_ok());
        let missing = std::thread::spawn(|| None);
        let error = verify_download(missing, "veluno-setup.exe", &file).unwrap_err();
        assert!(error.contains("Could not fetch the release checksums"));
        std::fs::remove_file(&file).unwrap();
    }

    #[test]
    #[cfg(windows)]
    fn failed_update_launch_restores_the_previous_binary() {
        let directory =
            std::env::temp_dir().join(format!("veluno-rollback-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let current = directory.join("veluno.exe");
        let new = directory.join("veluno.exe.new");
        std::fs::write(&current, b"previous app").unwrap();
        std::fs::write(&new, b"not executable").unwrap();
        assert!(replace_and_launch(&current, &new).is_err());
        assert_eq!(std::fs::read(&current).unwrap(), b"previous app");
        assert!(!current.with_extension("exe.old").exists());
        assert!(replace_and_launch(&current, &new).is_err());
        assert_eq!(std::fs::read(&current).unwrap(), b"previous app");
        std::fs::remove_file(current).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn updates_only_accept_official_https_app_assets() {
        for address in [
            "https://github.com/khentmoba/Felaris/releases/download/v0.3.2/felaris.exe",
            "https://github.com/khentmoba/Veluno/releases/download/v0.3.2/felaris.exe",
        ] {
            assert!(validate_download_url(address).is_ok(), "{address}");
        }
        assert!(
            validate_download_url(
                "https://github.com/khentmoba/Veluno/releases/download/v0.2.0/veluno.exe"
            )
            .is_ok()
        );
        for address in [
            "http://github.com/khentmoba/Veluno/releases/download/v0.2.0/veluno.exe",
            "https://github.com.evil.test/khentmoba/Veluno/releases/download/v0.2.0/veluno.exe",
            "https://github.com/other/project/releases/download/v0.2.0/veluno.exe",
            "https://github.com/khentmoba/Veluno/releases/download/v0.2.0/veluno-setup.exe",
        ] {
            assert!(validate_download_url(address).is_err(), "{address}");
        }
    }

    #[test]
    #[cfg(windows)]
    fn system_tools_resolve_to_windows_binaries() {
        for tool in [
            "System32\\curl.exe",
            "System32\\WindowsPowerShell\\v1.0\\powershell.exe",
            "System32\\tar.exe",
            "explorer.exe",
        ] {
            assert!(system_binary(tool).is_file(), "{tool}");
        }
    }

    #[test]
    fn sha256_matches_the_known_digest() {
        let file = std::env::temp_dir().join(format!("veluno-sha256-{}", std::process::id()));
        std::fs::write(&file, b"abc").unwrap();
        let digest = sha256(&file);
        std::fs::remove_file(&file).unwrap();
        assert_eq!(
            digest.unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn update_validation_rejects_html_and_truncated_executables() {
        assert!(!valid_executable(&vec![0; 600_000]));
        assert!(!valid_executable(b"MZ"));
        let mut bytes = vec![0; 512];
        bytes[..2].copy_from_slice(b"MZ");
        bytes[60..64].copy_from_slice(&64u32.to_le_bytes());
        bytes[64..68].copy_from_slice(b"PE\0\0");
        let machine: u16 = if cfg!(target_arch = "aarch64") {
            0xaa64
        } else {
            0x8664
        };
        bytes[68..70].copy_from_slice(&machine.to_le_bytes());
        bytes[156..158].copy_from_slice(&2u16.to_le_bytes());
        assert!(valid_executable(&bytes));
        bytes[60..64].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(!valid_executable(&bytes));
    }

    #[test]
    fn checksum_is_matched_by_exact_asset_name() {
        let sums = "aaa111  veluno.exe
bbb222  veluno-setup.exe
";
        assert_eq!(
            checksum_for(sums, "veluno-setup.exe").as_deref(),
            Some("bbb222")
        );
        assert_eq!(checksum_for(sums, "veluno.exe").as_deref(), Some("aaa111"));
        assert_eq!(checksum_for(sums, "other.exe"), None);
    }

    #[test]
    fn update_signature_policy_pins_to_a_valid_publisher() {
        let valid = |subject: &str| Signer::Valid(subject.into());
        assert!(signer_allows_update(&Signer::Unsigned, &Signer::Unsigned));
        assert!(signer_allows_update(&Signer::Unsigned, &valid("CN=Veluno")));
        assert!(!signer_allows_update(&Signer::Unsigned, &Signer::Broken));
        assert!(signer_allows_update(
            &valid("CN=Veluno"),
            &valid("CN=Veluno")
        ));
        assert!(!signer_allows_update(
            &valid("CN=Veluno"),
            &valid("CN=Someone Else")
        ));
        assert!(!signer_allows_update(
            &valid("CN=Veluno"),
            &Signer::Unsigned
        ));
        assert!(!signer_allows_update(&valid("CN=Veluno"), &Signer::Broken));
        assert!(!signer_allows_update(&Signer::Broken, &Signer::Unsigned));
        assert!(!signer_allows_update(&Signer::Broken, &valid("CN=Veluno")));
    }

    #[test]
    fn pe_certificate_table_precheck_detects_embedded_signatures() {
        let mut pe = vec![0u8; 512];
        pe[..2].copy_from_slice(b"MZ");
        pe[60..64].copy_from_slice(&0x80u32.to_le_bytes());
        pe[0x80..0x84].copy_from_slice(b"PE\0\0");
        let opt = 0x80 + 24;
        pe[opt..opt + 2].copy_from_slice(&0x20bu16.to_le_bytes());
        pe[opt + 108..opt + 112].copy_from_slice(&16u32.to_le_bytes());
        assert_eq!(pe_bytes_have_no_certificate_table(&pe), Some(true));

        let sec = opt + 112 + 32;
        pe[sec..sec + 8].copy_from_slice(&[1, 0, 0, 0, 64, 0, 0, 0]);
        assert_eq!(pe_bytes_have_no_certificate_table(&pe), Some(false));

        assert_eq!(pe_bytes_have_no_certificate_table(b"not a pe"), None);
    }

    #[test]
    #[cfg(windows)]
    fn signer_reads_unsigned_and_microsoft_signed_files() {
        let script = std::env::temp_dir().join(format!("veluno-signer-{}.ps1", std::process::id()));
        std::fs::write(&script, "Write-Output 'unsigned'").unwrap();
        let unsigned = signer(&script);
        std::fs::remove_file(&script).unwrap();
        assert_eq!(unsigned.unwrap(), Signer::Unsigned);
        match signer(&system_binary("explorer.exe")).unwrap() {
            Signer::Valid(subject) => assert!(subject.contains("Microsoft"), "{subject}"),
            other => panic!("expected a valid Microsoft signature, got {other:?}"),
        }
    }

    #[test]
    fn test_version_parsing() {
        assert_eq!(
            Version::parse("0.1.0"),
            Some(Version {
                major: 0,
                minor: 1,
                patch: 0,
                revision: 0,
                prerelease: false
            })
        );
        assert_eq!(
            Version::parse("v1.2.3"),
            Some(Version {
                major: 1,
                minor: 2,
                patch: 3,
                revision: 0,
                prerelease: false
            })
        );
        assert_eq!(
            Version::parse("V0.9.15-beta"),
            Some(Version {
                major: 0,
                minor: 9,
                patch: 15,
                revision: 0,
                prerelease: true
            })
        );
        assert_eq!(Version::parse("invalid"), None);
    }

    #[test]
    fn test_is_newer() {
        assert!(is_newer("0.2.0", "0.1.0"));
        assert!(is_newer("v0.1.1", "0.1.0"));
        assert!(is_newer("1.0.0", "0.9.9"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.1.0", "0.2.0"));
        assert!(!is_newer("v0.1.0", "0.1.0"));
        assert!(is_newer("0.3.5.1", "0.3.5"), "a fourth segment is compared");
        assert!(
            !is_newer("0.3.5.0", "0.3.5"),
            "a zero revision is the same release"
        );
        assert!(
            is_newer("0.3.5", "0.3.5-beta"),
            "a release beats its pre-release"
        );
        assert!(!is_newer("0.3.5-beta", "0.3.5"));
        assert!(
            !is_newer("0.3.5+build.7", "0.3.5"),
            "build metadata is ignored"
        );
        assert!(is_newer("0.3.6-beta", "0.3.5"));
        assert_eq!(
            Version::parse("1.2.3.4.5"),
            None,
            "five segments are rejected"
        );
        assert_eq!(
            Version::parse("1.2.3-beta"),
            Some(Version {
                major: 1,
                minor: 2,
                patch: 3,
                revision: 0,
                prerelease: true
            })
        );
    }

    #[test]
    fn test_parse_manifest_json() {
        let raw = r#"{
            "version": "0.2.0",
            "release_url": "https://github.com/khentmoba/Veluno/releases/tag/v0.2.0",
            "download_url": "https://github.com/khentmoba/Veluno/releases/download/v0.2.0/veluno.exe",
            "summary": "Zen UI update"
        }"#;

        let parsed: UpdateManifest = serde_json::from_str(raw).expect("valid manifest");
        assert_eq!(parsed.version, "0.2.0");
        assert_eq!(
            parsed.download_url.as_deref(),
            Some("https://github.com/khentmoba/Veluno/releases/download/v0.2.0/veluno.exe")
        );
        assert!(is_newer(&parsed.version, "0.1.0"));
    }
}
