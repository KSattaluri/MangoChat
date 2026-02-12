use crate::state::AppEvent;
use std::sync::mpsc::Sender as EventSender;
use std::time::Duration;

#[cfg(windows)]
use windows::Win32::Foundation::BOOL;
#[cfg(windows)]
use windows::Win32::Media::Audio::{IMMDeviceEnumerator, MMDeviceEnumerator, eCapture, eConsole};
#[cfg(windows)]
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
#[cfg(windows)]
use windows::Win32::System::Com::{
    CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
};

/// Windows-only test watcher:
/// mute -> stop dictation, unmute -> start dictation.
pub fn start_mute_watcher(event_tx: EventSender<AppEvent>) {
    #[cfg(not(windows))]
    {
        let _ = event_tx;
        return;
    }

    #[cfg(windows)]
    std::thread::spawn(move || unsafe {
        if let Err(e) = CoInitializeEx(None, COINIT_MULTITHREADED).ok() {
            eprintln!("[headset] CoInitializeEx failed: {}", e);
            return;
        }

        let enumerator: IMMDeviceEnumerator =
            match CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("[headset] MMDeviceEnumerator init failed: {}", e);
                    CoUninitialize();
                    return;
                }
            };

        let mut last_mute: Option<bool> = None;

        loop {
            match read_default_capture_mute(&enumerator) {
                Ok(muted) => {
                    if let Some(prev) = last_mute {
                        if prev != muted {
                            if muted {
                                println!("[headset] capture muted -> stop");
                                let _ = event_tx.send(AppEvent::HotkeyRelease);
                            } else {
                                println!("[headset] capture unmuted -> start");
                                let _ = event_tx.send(AppEvent::HotkeyPush);
                            }
                        }
                    }
                    last_mute = Some(muted);
                }
                Err(e) => {
                    eprintln!("[headset] mute poll error: {}", e);
                }
            }
            std::thread::sleep(Duration::from_millis(250));
        }
    });
}

#[cfg(windows)]
unsafe fn read_default_capture_mute(
    enumerator: &IMMDeviceEnumerator,
) -> Result<bool, String> {
    let device = enumerator
        .GetDefaultAudioEndpoint(eCapture, eConsole)
        .map_err(|e| format!("GetDefaultAudioEndpoint failed: {}", e))?;

    let endpoint: IAudioEndpointVolume = device
        .Activate(CLSCTX_ALL, None)
        .map_err(|e| format!("Activate(IAudioEndpointVolume) failed: {}", e))?;

    let muted: BOOL = endpoint
        .GetMute()
        .map_err(|e| format!("GetMute failed: {}", e))?;
    Ok(muted.as_bool())
}
