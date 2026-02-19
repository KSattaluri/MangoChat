use chrono::Local;
use std::backtrace::Backtrace;
use std::fs::{self, File};
use std::io::{Seek, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use zip::write::FileOptions;

static LOG_FILE: OnceLock<Mutex<File>> = OnceLock::new();

const LOG_ROTATE_KEEP: usize = 5;
const CRASH_LOG_KEEP: usize = 5;
const SUPPORT_EMAIL: &str = "mangochathelp@gmail.com";

pub fn support_email() -> &'static str {
    SUPPORT_EMAIL
}

pub fn data_dir() -> Result<PathBuf, String> {
    if let Some(dir) = dirs::data_local_dir() {
        return Ok(dir.join("MangoChat"));
    }
    if let Some(home) = dirs::home_dir() {
        return Ok(home.join(".mangochat"));
    }
    Err("Failed to resolve MangoChat data directory".into())
}

pub fn logs_dir() -> Result<PathBuf, String> {
    Ok(data_dir()?.join("logs"))
}

pub fn init_session_logging() -> Result<PathBuf, String> {
    let dir = logs_dir()?;
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create logs dir: {}", e))?;
    rotate_logs(&dir)?;
    let active = dir.join("app.log");
    let file = File::options()
        .create(true)
        .append(true)
        .open(&active)
        .map_err(|e| format!("Failed to open app log: {}", e))?;
    let _ = LOG_FILE.set(Mutex::new(file));
    append_line(
        "INFO",
        &format!(
            "session_start version={} ts={}",
            env!("CARGO_PKG_VERSION"),
            Local::now().format("%Y-%m-%d %H:%M:%S")
        ),
    );
    Ok(active)
}

fn rotate_logs(dir: &Path) -> Result<(), String> {
    for i in (1..LOG_ROTATE_KEEP).rev() {
        let from = dir.join(format!("app.{}.log", i));
        let to = dir.join(format!("app.{}.log", i + 1));
        if from.exists() {
            let _ = fs::remove_file(&to);
            fs::rename(&from, &to).map_err(|e| format!("Failed to rotate log {}: {}", i, e))?;
        }
    }
    let active = dir.join("app.log");
    if active.exists() {
        let to = dir.join("app.1.log");
        let _ = fs::remove_file(&to);
        fs::rename(&active, &to).map_err(|e| format!("Failed to rotate active log: {}", e))?;
    }
    prune_crash_logs(dir, CRASH_LOG_KEEP)?;
    Ok(())
}

fn prune_crash_logs(dir: &Path, keep: usize) -> Result<(), String> {
    let mut crash_files: Vec<(std::time::SystemTime, PathBuf)> = vec![];
    for entry in fs::read_dir(dir).map_err(|e| format!("Failed to read logs dir: {}", e))? {
        let entry = match entry {
            Ok(v) => v,
            Err(_) => continue,
        };
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if !(name.starts_with("crash-") && name.ends_with(".log")) {
            continue;
        }
        let modified = entry
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        crash_files.push((modified, path));
    }
    crash_files.sort_by(|a, b| b.0.cmp(&a.0));
    for (_, path) in crash_files.into_iter().skip(keep) {
        let _ = fs::remove_file(path);
    }
    Ok(())
}

pub fn append_line(level: &str, msg: &str) {
    let ts = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
    let line = format!("[{}] [{}] {}\n", ts, level, msg);
    if let Some(lock) = LOG_FILE.get() {
        if let Ok(mut f) = lock.lock() {
            let _ = f.write_all(line.as_bytes());
            let _ = f.flush();
        }
    }
}

pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let panic_msg = format!("{}", info);
        append_line("PANIC", &panic_msg);
        let bt = Backtrace::force_capture();
        append_line("PANIC", &format!("backtrace:\n{}", bt));
        let _ = write_crash_file(&panic_msg, &bt.to_string());
        previous(info);
    }));
}

fn write_crash_file(message: &str, backtrace: &str) -> Result<PathBuf, String> {
    let dir = logs_dir()?;
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create logs dir: {}", e))?;
    let name = format!("crash-{}.log", Local::now().format("%Y%m%d-%H%M%S"));
    let path = dir.join(name);
    let body = format!(
        "Mango Chat crash report\nversion: {}\ntime: {}\n\nmessage:\n{}\n\nbacktrace:\n{}\n",
        env!("CARGO_PKG_VERSION"),
        Local::now().to_rfc3339(),
        message,
        backtrace
    );
    fs::write(&path, body).map_err(|e| format!("Failed to write crash log: {}", e))?;
    Ok(path)
}

pub fn open_logs_folder() -> Result<(), String> {
    let dir = logs_dir()?;
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create logs dir: {}", e))?;
    std::process::Command::new("explorer")
        .arg(dir)
        .spawn()
        .map_err(|e| format!("Failed to open logs folder: {}", e))?;
    Ok(())
}

pub fn default_export_zip_path() -> Result<PathBuf, String> {
    let base = data_dir()?;
    let exports_dir = base.join("diagnostics");
    fs::create_dir_all(&exports_dir)
        .map_err(|e| format!("Failed to create diagnostics dir: {}", e))?;

    let zip_name = "MangoChat-diagnostics.zip";
    Ok(exports_dir.join(zip_name))
}

pub fn export_diagnostics_zip_to(zip_path: &Path) -> Result<PathBuf, String> {
    if let Some(parent) = zip_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create export dir: {}", e))?;
    }
    if zip_path.exists() {
        let _ = fs::remove_file(zip_path);
    }

    let file = File::create(zip_path).map_err(|e| format!("Failed to create zip: {}", e))?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = FileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o644);

    add_text(
        &mut zip,
        "manifest.txt",
        &format!(
            "Mango Chat diagnostics\nversion={}\ncreated={}\nsupport_email={}\n",
            env!("CARGO_PKG_VERSION"),
            Local::now().to_rfc3339(),
            SUPPORT_EMAIL
        ),
        opts,
    )?;

    if let Ok(path) = crate::settings::settings_path() {
        add_file(&mut zip, &path, "settings.json", opts)?;
    }
    if let Ok(path) = crate::usage::usage_path() {
        add_file(&mut zip, &path, "usage.jsonl", opts)?;
    }
    if let Ok(path) = crate::usage::session_usage_path() {
        add_file(&mut zip, &path, "usage-session.jsonl", opts)?;
    }
    if let Ok(path) = crate::usage::provider_totals_path() {
        add_file(&mut zip, &path, "usage-provider.json", opts)?;
    }

    if let Ok(logs) = collect_recent_logs(5) {
        for path in logs {
            let filename = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown.log")
                .to_string();
            add_file(&mut zip, &path, &format!("logs/{}", filename), opts)?;
        }
    }

    zip.finish()
        .map_err(|e| format!("Failed to finalize diagnostics zip: {}", e))?;
    Ok(zip_path.to_path_buf())
}

fn collect_recent_logs(limit: usize) -> Result<Vec<PathBuf>, String> {
    let dir = logs_dir()?;
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut entries: Vec<(std::time::SystemTime, PathBuf)> = vec![];
    for entry in fs::read_dir(&dir).map_err(|e| format!("Failed to read logs dir: {}", e))? {
        let entry = match entry {
            Ok(v) => v,
            Err(_) => continue,
        };
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if !(name.starts_with("app") && name.ends_with(".log")
            || name.starts_with("crash-") && name.ends_with(".log"))
        {
            continue;
        }
        let modified = entry
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        entries.push((modified, path));
    }
    entries.sort_by(|a, b| b.0.cmp(&a.0));
    Ok(entries.into_iter().take(limit).map(|(_, p)| p).collect())
}

fn add_text<W: Write + Seek>(
    zip: &mut zip::ZipWriter<W>,
    name: &str,
    text: &str,
    opts: FileOptions,
) -> Result<(), String> {
    zip.start_file(name, opts)
        .map_err(|e| format!("Failed to add {}: {}", name, e))?;
    zip.write_all(text.as_bytes())
        .map_err(|e| format!("Failed to write {}: {}", name, e))
}

fn add_file<W: Write + Seek>(
    zip: &mut zip::ZipWriter<W>,
    path: &Path,
    name: &str,
    opts: FileOptions,
) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    let bytes = fs::read(path).map_err(|e| format!("Failed to read {}: {}", name, e))?;
    zip.start_file(name, opts)
        .map_err(|e| format!("Failed to add {}: {}", name, e))?;
    zip.write_all(&bytes)
        .map_err(|e| format!("Failed to write {}: {}", name, e))
}

#[macro_export]
macro_rules! app_log {
    ($($arg:tt)*) => {{
        ::std::println!($($arg)*);
        $crate::diagnostics::append_line("INFO", &format!($($arg)*));
    }};
}

#[macro_export]
macro_rules! app_err {
    ($($arg:tt)*) => {{
        ::std::eprintln!($($arg)*);
        $crate::diagnostics::append_line("ERROR", &format!($($arg)*));
    }};
}
