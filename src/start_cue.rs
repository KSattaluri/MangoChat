use std::path::PathBuf;
use windows::core::PCWSTR;
use windows::Win32::Media::Audio::{
    PlaySoundW, SND_ASYNC, SND_FILENAME, SND_NODEFAULT,
};

pub const START_CUES: &[(&str, &str)] = &[
    ("audio1.wav", "Audio 1"),
    ("audio2.wav", "Audio 2"),
];
const STOP_CUE_FILE: &str = "audio_close.wav";

fn resolve_asset_path(file_name: &str) -> Option<PathBuf> {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));
    let cwd = std::env::current_dir().ok();

    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(dir) = exe_dir {
        candidates.push(dir.join("assets").join(file_name));
    }
    if let Some(dir) = cwd {
        candidates.push(dir.join("assets").join(file_name));
    }
    candidates.push(PathBuf::from("assets").join(file_name));

    candidates.into_iter().find(|p| p.exists())
}

pub fn play_start_cue(file_name: &str) -> Result<(), String> {
    let is_supported = START_CUES.iter().any(|(id, _)| *id == file_name);
    if !is_supported {
        return Err(format!("unsupported start cue: {}", file_name));
    }

    let path = resolve_asset_path(file_name)
        .ok_or_else(|| format!("start cue not found in assets/: {}", file_name))?;
    play_wave_path(&path)
}

pub fn play_stop_cue() -> Result<(), String> {
    let path = resolve_asset_path(STOP_CUE_FILE)
        .ok_or_else(|| format!("stop cue not found in assets/: {}", STOP_CUE_FILE))?;
    play_wave_path(&path)
}

fn play_wave_path(path: &PathBuf) -> Result<(), String> {
    let wide: Vec<u16> = path
        .as_os_str()
        .to_string_lossy()
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    let ok = unsafe {
        PlaySoundW(
            PCWSTR(wide.as_ptr()),
            None,
            SND_FILENAME | SND_ASYNC | SND_NODEFAULT,
        )
    };
    if ok.as_bool() {
        Ok(())
    } else {
        Err(format!("failed to play cue: {}", path.display()))
    }
}
