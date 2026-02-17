use std::fs;
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

const START_CUE_1_BYTES: &[u8] = include_bytes!("../assets/audio1.wav");
const START_CUE_2_BYTES: &[u8] = include_bytes!("../assets/audio2.wav");
const STOP_CUE_BYTES: &[u8] = include_bytes!("../assets/audio_close.wav");

fn embedded_cue_bytes(file_name: &str) -> Option<&'static [u8]> {
    match file_name {
        "audio1.wav" => Some(START_CUE_1_BYTES),
        "audio2.wav" => Some(START_CUE_2_BYTES),
        STOP_CUE_FILE => Some(STOP_CUE_BYTES),
        _ => None,
    }
}

fn embedded_cue_path(file_name: &str) -> Result<PathBuf, String> {
    let bytes = embedded_cue_bytes(file_name)
        .ok_or_else(|| format!("unsupported cue: {}", file_name))?;

    let cue_dir = std::env::temp_dir().join("MangoChat").join("cues");
    fs::create_dir_all(&cue_dir)
        .map_err(|e| format!("failed to create cue temp dir '{}': {}", cue_dir.display(), e))?;
    let path = cue_dir.join(file_name);

    let should_write = match fs::metadata(&path) {
        Ok(meta) => meta.len() != bytes.len() as u64,
        Err(_) => true,
    };
    if should_write {
        fs::write(&path, bytes)
            .map_err(|e| format!("failed to write cue file '{}': {}", path.display(), e))?;
    }

    Ok(path)
}

pub fn play_start_cue(file_name: &str) -> Result<(), String> {
    let is_supported = START_CUES.iter().any(|(id, _)| *id == file_name);
    if !is_supported {
        return Err(format!("unsupported start cue: {}", file_name));
    }

    let path = embedded_cue_path(file_name)?;
    play_wave_path(&path)
}

pub fn play_stop_cue() -> Result<(), String> {
    let path = embedded_cue_path(STOP_CUE_FILE)?;
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
