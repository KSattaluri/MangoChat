use semver::Version;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc::Sender;
use std::time::{Duration, SystemTime};

#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
use windows::Win32::Foundation::{CloseHandle, HANDLE};
#[cfg(windows)]
use windows::Win32::System::Threading::{OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE};

const REPO_OWNER: &str = "KSattaluri";
const REPO_NAME: &str = "MangoChat";
const APP_USER_AGENT: &str = "mangochat-updater";
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
#[cfg(windows)]
const UPDATE_HELPER_WAIT_TIMEOUT_MS: u32 = 120_000;

#[derive(Debug, Clone)]
pub struct ReleaseAsset {
    pub name: String,
    pub download_url: String,
}

#[derive(Debug, Clone)]
pub struct ReleaseInfo {
    pub tag: String,
    pub version: Version,
    pub assets: Vec<ReleaseAsset>,
}

#[derive(Debug, Clone)]
pub enum WorkerMessage {
    CheckFinished(Result<CheckOutcome, String>),
    InstallFinished(Result<String, String>),
}

#[derive(Debug, Clone)]
pub enum CheckOutcome {
    UpToDate,
    UpdateAvailable { latest: ReleaseInfo },
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    prerelease: bool,
    draft: bool,
    #[serde(default)]
    assets: Vec<GitHubAsset>,
}

#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
}

fn current_version() -> Result<Version, String> {
    Version::parse(env!("CARGO_PKG_VERSION")).map_err(|e| format!("invalid current version: {e}"))
}

fn parse_tag_version(tag: &str) -> Option<Version> {
    let raw = tag.trim().trim_start_matches('v');
    Version::parse(raw).ok()
}

pub fn spawn_check_with_override(tx: Sender<WorkerMessage>, feed_url_override: Option<String>) {
    std::thread::spawn(move || {
        let result = check_for_updates(feed_url_override.as_deref());
        let _ = tx.send(WorkerMessage::CheckFinished(result));
    });
}

fn to_github_releases_api_url(feed_url: &str) -> Option<String> {
    let trimmed = feed_url.trim().trim_end_matches('/');
    let marker = "github.com/";
    let idx = trimmed.find(marker)?;
    let tail = &trimmed[idx + marker.len()..];
    let mut parts = tail.split('/');
    let owner = parts.next()?;
    let repo = parts.next()?;
    if owner.is_empty() || repo.is_empty() {
        return None;
    }
    if !trimmed.contains("/releases") {
        return None;
    }
    Some(format!(
        "https://api.github.com/repos/{}/{}/releases?per_page=20",
        owner, repo
    ))
}

fn release_feed_url(feed_url_override: Option<&str>) -> String {
    if let Some(override_url) = feed_url_override {
        let trimmed = override_url.trim();
        if !trimmed.is_empty() {
            if trimmed.contains("github.com/") && trimmed.contains("/releases") {
                if let Some(api_url) = to_github_releases_api_url(trimmed) {
                    return api_url;
                }
            }
            return trimmed.to_string();
        }
    }
    format!(
        "https://api.github.com/repos/{}/{}/releases?per_page=20",
        REPO_OWNER, REPO_NAME
    )
}

fn check_for_updates(feed_url_override: Option<&str>) -> Result<CheckOutcome, String> {
    let current = current_version()?;
    let url = release_feed_url(feed_url_override);

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("http client error: {e}"))?;

    let releases = client
        .get(url)
        .header("User-Agent", APP_USER_AGENT)
        .send()
        .map_err(|e| format!("request failed: {e}"))?
        .error_for_status()
        .map_err(|e| format!("github api error: {e}"))?
        .json::<Vec<GitHubRelease>>()
        .map_err(|e| format!("invalid response json: {e}"))?;

    let mut best: Option<ReleaseInfo> = None;
    for rel in releases {
        if rel.draft {
            continue;
        }
        if rel.prerelease {
            continue;
        }
        let Some(version) = parse_tag_version(&rel.tag_name) else {
            continue;
        };
        let info = ReleaseInfo {
            tag: rel.tag_name,
            version,
            assets: rel
                .assets
                .into_iter()
                .map(|a| ReleaseAsset {
                    name: a.name,
                    download_url: a.browser_download_url,
                })
                .collect(),
        };
        let replace = best
            .as_ref()
            .map(|b| info.version > b.version)
            .unwrap_or(true);
        if replace {
            best = Some(info);
        }
    }

    let Some(latest) = best else {
        return Ok(CheckOutcome::UpToDate);
    };

    if latest.version > current {
        Ok(CheckOutcome::UpdateAvailable { latest })
    } else {
        Ok(CheckOutcome::UpToDate)
    }
}

pub fn spawn_install(tx: Sender<WorkerMessage>, release: ReleaseInfo) {
    std::thread::spawn(move || {
        let result = download_installer_for_update(&release);
        let _ = tx.send(WorkerMessage::InstallFinished(result));
    });
}

fn download_installer_for_update(release: &ReleaseInfo) -> Result<String, String> {
    let asset = release
        .assets
        .iter()
        .find(|a| {
            let n = a.name.to_ascii_lowercase();
            n.ends_with(".exe") && n.contains("setup")
        })
        .or_else(|| {
            release
                .assets
                .iter()
                .find(|a| a.name.to_ascii_lowercase().ends_with(".exe"))
        })
        .ok_or("no .exe installer asset found on release")?;

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(90))
        .build()
        .map_err(|e| format!("http client error: {e}"))?;

    let installer_bytes = client
        .get(&asset.download_url)
        .header("User-Agent", APP_USER_AGENT)
        .send()
        .map_err(|e| format!("download request failed: {e}"))?
        .error_for_status()
        .map_err(|e| format!("download failed: {e}"))?
        .bytes()
        .map_err(|e| format!("failed reading installer bytes: {e}"))?;

    if let Some(checksums_asset) = release
        .assets
        .iter()
        .find(|a| a.name.eq_ignore_ascii_case("SHA256SUMS.txt"))
    {
        let checksums_text = client
            .get(&checksums_asset.download_url)
            .header("User-Agent", APP_USER_AGENT)
            .send()
            .map_err(|e| format!("checksums request failed: {e}"))?
            .error_for_status()
            .map_err(|e| format!("checksums download failed: {e}"))?
            .text()
            .map_err(|e| format!("failed reading SHA256SUMS.txt: {e}"))?;
        verify_sha256_from_release(&checksums_text, &asset.name, installer_bytes.as_ref())?;
    } else {
        app_log!(
            "[updater] SHA256SUMS.txt not present for release {}; skipping checksum verification",
            release.tag
        );
    }

    let mut path: PathBuf = std::env::temp_dir();
    path.push(format!("MangoChat-Setup-{}.exe", release.version));
    let mut file = File::create(&path).map_err(|e| format!("cannot create installer file: {e}"))?;
    file.write_all(&installer_bytes)
        .map_err(|e| format!("cannot write installer file: {e}"))?;
    Ok(path.display().to_string())
}

pub fn schedule_silent_install_and_relaunch(installer_path: &str) -> Result<(), String> {
    let current_pid = std::process::id();
    let app_exe =
        std::env::current_exe().map_err(|e| format!("failed to resolve current exe: {e}"))?;
    let mut cmd = Command::new(&app_exe);
    cmd.arg("--apply-update")
        .arg("--wait-pid")
        .arg(current_pid.to_string())
        .arg("--installer")
        .arg(installer_path)
        .arg("--relaunch")
        .arg(app_exe.to_string_lossy().to_string());
    #[cfg(windows)]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd.spawn()
        .map_err(|e| format!("failed to launch updater helper: {e}"))?;
    Ok(())
}

pub fn run_update_helper_from_args(args: &[String]) -> Result<(), String> {
    let mut wait_pid: Option<u32> = None;
    let mut installer: Option<String> = None;
    let mut relaunch: Option<String> = None;
    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--wait-pid" => {
                i += 1;
                let v = args.get(i).ok_or("missing value for --wait-pid")?;
                wait_pid = v.parse::<u32>().ok();
            }
            "--installer" => {
                i += 1;
                installer = args.get(i).cloned();
            }
            "--relaunch" => {
                i += 1;
                relaunch = args.get(i).cloned();
            }
            _ => {}
        }
        i += 1;
    }
    let installer_path = installer.ok_or("missing --installer")?;
    let relaunch_path = relaunch.ok_or("missing --relaunch")?;

    if let Some(pid) = wait_pid {
        wait_for_pid_exit(pid);
    }

    let status = Command::new(&installer_path)
        .args(["/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART"])
        .status()
        .map_err(|e| format!("failed to run installer: {e}"))?;
    if !status.success() {
        return Err(format!("installer exited with status: {}", status));
    }

    Command::new(&relaunch_path)
        .spawn()
        .map_err(|e| format!("failed to relaunch app: {e}"))?;
    Ok(())
}

fn parse_sha256sums(text: &str) -> std::collections::HashMap<String, String> {
    let mut out = std::collections::HashMap::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let mut parts = trimmed.split_whitespace();
        let Some(hash) = parts.next() else {
            continue;
        };
        let Some(name) = parts.next() else {
            continue;
        };
        let clean_name = name.trim_start_matches('*').trim_start_matches("./");
        out.insert(clean_name.to_string(), hash.to_ascii_lowercase());
    }
    out
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    digest.iter().map(|b| format!("{:02x}", b)).collect()
}

fn verify_sha256_from_release(
    checksums_text: &str,
    installer_name: &str,
    installer_bytes: &[u8],
) -> Result<(), String> {
    let checksums = parse_sha256sums(checksums_text);
    let expected = checksums.get(installer_name).ok_or_else(|| {
        format!(
            "SHA256SUMS.txt missing entry for installer '{}'",
            installer_name
        )
    })?;
    let actual = sha256_hex(installer_bytes);
    if actual != *expected {
        return Err(format!(
            "installer checksum mismatch: expected {}, got {}",
            expected, actual
        ));
    }
    Ok(())
}

#[cfg(windows)]
fn wait_for_pid_exit(pid: u32) {
    unsafe {
        let Ok(handle): Result<HANDLE, _> = OpenProcess(PROCESS_SYNCHRONIZE, false, pid) else {
            return;
        };
        if handle.is_invalid() {
            return;
        }
        let _ = WaitForSingleObject(handle, UPDATE_HELPER_WAIT_TIMEOUT_MS);
        let _ = CloseHandle(handle);
    }
}

#[cfg(not(windows))]
fn wait_for_pid_exit(_pid: u32) {}

pub fn cleanup_stale_temp_installers(max_age_days: u64) -> Result<usize, String> {
    let dir = std::env::temp_dir();
    let now = SystemTime::now();
    let max_age = Duration::from_secs(max_age_days.saturating_mul(24 * 60 * 60));
    let mut removed = 0usize;

    let entries = fs::read_dir(&dir).map_err(|e| format!("cannot read temp dir: {e}"))?;
    for entry in entries {
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !(name.starts_with("MangoChat-Setup-") && name.ends_with(".exe")) {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }
        let Ok(modified) = meta.modified() else {
            continue;
        };
        let Ok(age) = now.duration_since(modified) else {
            continue;
        };
        if age < max_age {
            continue;
        }
        if fs::remove_file(&path).is_ok() {
            removed += 1;
        }
    }
    Ok(removed)
}
