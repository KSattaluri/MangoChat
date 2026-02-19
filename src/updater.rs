use sha2::{Digest, Sha256};
use semver::Version;
use serde::Deserialize;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::Sender;
use std::time::{Duration, SystemTime};

const REPO_OWNER: &str = "KSattaluri";
const REPO_NAME: &str = "MangoChat";
const REPO_RELEASE_PAGE_NAME: &str = "MangoChat";
const APP_USER_AGENT: &str = "mangochat-updater";
const POWERSHELL_VERIFY_TIMEOUT_SECS: u64 = 15;

#[derive(Debug, Clone)]
pub struct ReleaseAsset {
    pub name: String,
    pub download_url: String,
}

#[derive(Debug, Clone)]
pub struct ReleaseInfo {
    pub tag: String,
    pub version: Version,
    pub html_url: String,
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
    html_url: String,
    prerelease: bool,
    draft: bool,
    assets: Vec<GitHubAsset>,
}

#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
}

fn current_version() -> Result<Version, String> {
    Version::parse(env!("CARGO_PKG_VERSION"))
        .map_err(|e| format!("invalid current version: {e}"))
}

fn parse_tag_version(tag: &str) -> Option<Version> {
    let raw = tag.trim().trim_start_matches('v');
    Version::parse(raw).ok()
}

pub fn spawn_check_with_override(
    tx: Sender<WorkerMessage>,
    feed_url_override: Option<String>,
) {
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

pub fn default_release_page_url() -> String {
    format!(
        "https://github.com/{}/{}/releases",
        REPO_OWNER, REPO_RELEASE_PAGE_NAME
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
            html_url: rel.html_url,
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

    let checksums_asset = release
        .assets
        .iter()
        .find(|a| a.name.eq_ignore_ascii_case("SHA256SUMS.txt"))
        .ok_or("missing SHA256SUMS.txt asset on release")?;
    let checksums_text = client
        .get(&checksums_asset.download_url)
        .header("User-Agent", APP_USER_AGENT)
        .send()
        .map_err(|e| format!("checksums request failed: {e}"))?
        .error_for_status()
        .map_err(|e| format!("checksums download failed: {e}"))?
        .text()
        .map_err(|e| format!("failed reading SHA256SUMS.txt: {e}"))?;

    verify_sha256_from_release(
        &checksums_text,
        &asset.name,
        installer_bytes.as_ref(),
    )?;

    let mut path: PathBuf = std::env::temp_dir();
    path.push(format!("MangoChat-Setup-{}.exe", release.version));
    let mut file = File::create(&path).map_err(|e| format!("cannot create installer file: {e}"))?;
    file.write_all(&installer_bytes)
        .map_err(|e| format!("cannot write installer file: {e}"))?;

    verify_authenticode_signature(&path)?;

    Ok(path.display().to_string())
}

pub fn schedule_silent_install_and_relaunch(installer_path: &str) -> Result<(), String> {
    let current_pid = std::process::id();
    let app_exe = std::env::current_exe()
        .map_err(|e| format!("failed to resolve current exe: {e}"))?;

    let installer = PathBuf::from(installer_path);
    if !installer.exists() {
        return Err(format!(
            "installer not found at expected path: {}",
            installer.display()
        ));
    }

    let temp = std::env::temp_dir();
    let script_path = temp.join(format!("mangochat-self-update-{}.cmd", current_pid));
    let installer_escaped = installer.display().to_string().replace('"', "\"\"");
    let app_exe_escaped = app_exe.display().to_string().replace('"', "\"\"");
    let script = format!(
        "@echo off\r\n\
setlocal\r\n\
set \"UPDATER_PID={pid}\"\r\n\
set \"INSTALLER={installer}\"\r\n\
set \"APP_EXE={app}\"\r\n\
:waitloop\r\n\
tasklist /FI \"PID eq %UPDATER_PID%\" 2>NUL | findstr /R /C:\" %UPDATER_PID% \" >NUL\r\n\
if %ERRORLEVEL%==0 (\r\n\
  timeout /T 1 /NOBREAK >NUL\r\n\
  goto waitloop\r\n\
)\r\n\
start \"\" /wait \"%INSTALLER%\" /VERYSILENT /SUPPRESSMSGBOXES /NORESTART\r\n\
start \"\" \"%APP_EXE%\"\r\n\
del \"%~f0\"\r\n",
        pid = current_pid,
        installer = installer_escaped,
        app = app_exe_escaped,
    );
    fs::write(&script_path, script)
        .map_err(|e| format!("failed to write updater script: {e}"))?;

    Command::new("cmd")
        .args(["/C", &script_path.display().to_string()])
        .spawn()
        .map_err(|e| format!("failed to launch updater helper script: {e}"))?;

    Ok(())
}

pub fn open_release_page(url: &str) -> Result<(), String> {
    Command::new("rundll32")
        .args(["url.dll,FileProtocolHandler", url])
        .spawn()
        .map_err(|e| format!("failed to open release url: {e}"))?;
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
    let expected = checksums
        .get(installer_name)
        .ok_or_else(|| format!("SHA256SUMS.txt missing entry for installer '{}'", installer_name))?;
    let actual = sha256_hex(installer_bytes);
    if actual != *expected {
        return Err(format!(
            "installer checksum mismatch: expected {}, got {}",
            expected, actual
        ));
    }
    Ok(())
}

fn verify_authenticode_signature(installer_path: &Path) -> Result<(), String> {
    let escaped_path = installer_path
        .to_string_lossy()
        .replace('\'', "''");
    let ps = format!(
        "$ErrorActionPreference='Stop'; \
         $sig = Get-AuthenticodeSignature -FilePath '{}'; \
         Write-Output $sig.Status.ToString()",
        escaped_path
    );
    let mut shells: Vec<String> = Vec::new();
    if let Ok(windir) = std::env::var("WINDIR") {
        shells.push(
            Path::new(&windir)
                .join(r"System32\WindowsPowerShell\v1.0\powershell.exe")
                .to_string_lossy()
                .to_string(),
        );
        shells.push(
            Path::new(&windir)
                .join(r"Sysnative\WindowsPowerShell\v1.0\powershell.exe")
                .to_string_lossy()
                .to_string(),
        );
    }
    shells.push("powershell".to_string());
    shells.push("pwsh".to_string());
    shells.dedup();

    let mut last_err = String::new();
    for shell in shells {
        let mut child = match Command::new(&shell)
            .args(["-NoProfile", "-Command", &ps])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                last_err = format!("{}: {}", shell, e);
                continue;
            }
        };

        let deadline =
            std::time::Instant::now() + Duration::from_secs(POWERSHELL_VERIFY_TIMEOUT_SECS);
        loop {
            match child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) => {
                    if std::time::Instant::now() >= deadline {
                        let _ = child.kill();
                        let _ = child.wait();
                        return Err(format!(
                            "signature verification timed out after {}s",
                            POWERSHELL_VERIFY_TIMEOUT_SECS
                        ));
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
                Err(e) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!("signature verification process error: {e}"));
                }
            }
        }

        let output = child
            .wait_with_output()
            .map_err(|e| format!("signature verification failed to collect output: {e}"))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stderr_lc = stderr.to_ascii_lowercase();
            let missing_cmd = stderr_lc.contains("get-authenticodesignature")
                && stderr_lc.contains("not recognized");
            if missing_cmd {
                last_err = format!("{}: {}", shell, stderr);
                continue;
            }
            return Err(format!(
                "signature verification command failed ({}): {}",
                shell, stderr
            ));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let status = stdout.lines().last().unwrap_or("").trim();
        if !status.eq_ignore_ascii_case("Valid") {
            return Err(format!("installer signature is not valid (status: {})", status));
        }
        return Ok(());
    }
    Err(format!(
        "signature verification failed to run with available shells: {}",
        last_err
    ))
}

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
        let Ok(modified) = meta.modified() else { continue };
        let Ok(age) = now.duration_since(modified) else { continue };
        if age < max_age {
            continue;
        }
        if fs::remove_file(&path).is_ok() {
            removed += 1;
        }
    }
    Ok(removed)
}
