use semver::Version;
use serde::Deserialize;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc::Sender;

const REPO_OWNER: &str = "KSattaluri";
const REPO_NAME: &str = "MangoChat";
const APP_USER_AGENT: &str = "mangochat-updater";

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
    pub prerelease: bool,
    pub assets: Vec<ReleaseAsset>,
}

#[derive(Debug, Clone)]
pub enum WorkerMessage {
    CheckFinished(Result<CheckOutcome, String>),
    InstallFinished(Result<String, String>),
}

#[derive(Debug, Clone)]
pub enum CheckOutcome {
    UpToDate { current: Version },
    UpdateAvailable { current: Version, latest: ReleaseInfo },
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
    include_prerelease: bool,
    feed_url_override: Option<String>,
) {
    std::thread::spawn(move || {
        let result = check_for_updates(include_prerelease, feed_url_override.as_deref());
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

fn check_for_updates(
    include_prerelease: bool,
    feed_url_override: Option<&str>,
) -> Result<CheckOutcome, String> {
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
        if rel.prerelease && !include_prerelease {
            continue;
        }
        let Some(version) = parse_tag_version(&rel.tag_name) else {
            continue;
        };
        let info = ReleaseInfo {
            tag: rel.tag_name,
            version,
            html_url: rel.html_url,
            prerelease: rel.prerelease,
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
        return Ok(CheckOutcome::UpToDate { current });
    };

    if latest.version > current {
        Ok(CheckOutcome::UpdateAvailable { current, latest })
    } else {
        Ok(CheckOutcome::UpToDate { current })
    }
}

pub fn spawn_install(tx: Sender<WorkerMessage>, release: ReleaseInfo) {
    std::thread::spawn(move || {
        let result = download_and_launch_installer(&release);
        let _ = tx.send(WorkerMessage::InstallFinished(result));
    });
}

fn download_and_launch_installer(release: &ReleaseInfo) -> Result<String, String> {
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

    let bytes = client
        .get(&asset.download_url)
        .header("User-Agent", APP_USER_AGENT)
        .send()
        .map_err(|e| format!("download request failed: {e}"))?
        .error_for_status()
        .map_err(|e| format!("download failed: {e}"))?
        .bytes()
        .map_err(|e| format!("failed reading installer bytes: {e}"))?;

    let mut path: PathBuf = std::env::temp_dir();
    path.push(format!("MangoChat-Setup-{}.exe", release.version));
    let mut file = File::create(&path).map_err(|e| format!("cannot create installer file: {e}"))?;
    file.write_all(&bytes)
        .map_err(|e| format!("cannot write installer file: {e}"))?;

    Command::new(&path)
        .args(["/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART"])
        .spawn()
        .map_err(|e| format!("failed to launch installer: {e}"))?;

    Ok(path.display().to_string())
}

pub fn open_release_page(url: &str) -> Result<(), String> {
    Command::new("rundll32")
        .args(["url.dll,FileProtocolHandler", url])
        .spawn()
        .map_err(|e| format!("failed to open release url: {e}"))?;
    Ok(())
}
