use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use base64::Engine as _;

#[derive(Debug, Default, Serialize, Deserialize)]
struct SecretsFile {
    #[serde(default)]
    api_keys: HashMap<String, String>,
}

fn secrets_path() -> Result<PathBuf, String> {
    if let Some(dir) = dirs::data_local_dir() {
        return Ok(dir.join("MangoChat").join("secrets.json"));
    }
    if let Some(home) = dirs::home_dir() {
        return Ok(home.join(".mangochat").join("secrets.json"));
    }
    Err("Failed to resolve data directory".into())
}

fn legacy_secrets_path() -> Result<PathBuf, String> {
    if let Some(dir) = dirs::data_local_dir() {
        return Ok(dir.join("Jarvis").join("secrets.json"));
    }
    if let Some(home) = dirs::home_dir() {
        return Ok(home.join(".jarvis").join("secrets.json"));
    }
    Err("Failed to resolve data directory".into())
}

pub fn load_api_keys() -> Result<HashMap<String, String>, String> {
    let path = secrets_path()?;
    let read_path = if path.exists() {
        path
    } else {
        match legacy_secrets_path() {
            Ok(p) => p,
            Err(_) => return Ok(HashMap::new()),
        }
    };
    let text = match fs::read_to_string(&read_path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(HashMap::new()),
        Err(e) => return Err(format!("Failed to read secrets file: {}", e)),
    };
    let parsed: SecretsFile =
        serde_json::from_str(&text).map_err(|e| format!("Failed to parse secrets file: {}", e))?;

    let mut out = HashMap::new();
    for (provider, enc_b64) in parsed.api_keys {
        if enc_b64.trim().is_empty() {
            continue;
        }
        let encrypted = match base64::engine::general_purpose::STANDARD.decode(enc_b64.as_bytes()) {
            Ok(v) => v,
            Err(e) => {
                eprintln!(
                    "[secrets] invalid base64 blob for provider '{}': {}",
                    provider, e
                );
                continue;
            }
        };
        match decrypt_for_current_user(&encrypted) {
            Ok(bytes) => match String::from_utf8(bytes) {
                Ok(key) if !key.is_empty() => {
                    out.insert(provider, key);
                }
                Ok(_) => {}
                Err(e) => {
                    eprintln!(
                        "[secrets] decrypted invalid UTF-8 for provider '{}': {}",
                        provider, e
                    );
                }
            },
            Err(e) => {
                eprintln!(
                    "[secrets] failed to decrypt key for provider '{}': {}",
                    provider, e
                );
            }
        }
    }
    Ok(out)
}

pub fn save_api_keys(api_keys: &HashMap<String, String>) -> Result<(), String> {
    let path = secrets_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create secrets dir: {}", e))?;
    }

    let mut encrypted_map: HashMap<String, String> = HashMap::new();
    for (provider, key) in api_keys {
        if key.trim().is_empty() {
            continue;
        }
        let encrypted = encrypt_for_current_user(key.as_bytes())?;
        encrypted_map.insert(
            provider.clone(),
            base64::engine::general_purpose::STANDARD.encode(encrypted),
        );
    }

    if encrypted_map.is_empty() {
        if path.exists() {
            fs::remove_file(&path)
                .map_err(|e| format!("Failed to remove empty secrets file: {}", e))?;
        }
        return Ok(());
    }

    let json = serde_json::to_string_pretty(&SecretsFile {
        api_keys: encrypted_map,
    })
    .map_err(|e| format!("Failed to serialize secrets file: {}", e))?;
    fs::write(&path, json).map_err(|e| format!("Failed to write secrets file: {}", e))?;
    Ok(())
}

#[cfg(windows)]
fn encrypt_for_current_user(plain: &[u8]) -> Result<Vec<u8>, String> {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{HLOCAL, LocalFree};
    use windows::Win32::Security::Cryptography::{
        CryptProtectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    unsafe {
        let mut in_blob = CRYPT_INTEGER_BLOB {
            cbData: plain.len() as u32,
            pbData: plain.as_ptr() as *mut u8,
        };
        let mut out_blob = CRYPT_INTEGER_BLOB::default();
        CryptProtectData(
            &mut in_blob,
            PCWSTR::null(),
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut out_blob,
        )
        .map_err(|e| format!("CryptProtectData failed: {}", e))?;

        let out =
            std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize).to_vec();
        let _ = LocalFree(HLOCAL(out_blob.pbData as *mut core::ffi::c_void));
        Ok(out)
    }
}

#[cfg(windows)]
fn decrypt_for_current_user(encrypted: &[u8]) -> Result<Vec<u8>, String> {
    use windows::Win32::Security::Cryptography::{
        CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };
    use windows::Win32::Foundation::{HLOCAL, LocalFree};

    unsafe {
        let mut in_blob = CRYPT_INTEGER_BLOB {
            cbData: encrypted.len() as u32,
            pbData: encrypted.as_ptr() as *mut u8,
        };
        let mut out_blob = CRYPT_INTEGER_BLOB::default();

        CryptUnprotectData(
            &mut in_blob,
            None,
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut out_blob,
        )
        .map_err(|e| format!("CryptUnprotectData failed: {}", e))?;

        let out =
            std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize).to_vec();
        let _ = LocalFree(HLOCAL(out_blob.pbData as *mut core::ffi::c_void));
        Ok(out)
    }
}

#[cfg(not(windows))]
fn encrypt_for_current_user(plain: &[u8]) -> Result<Vec<u8>, String> {
    Ok(plain.to_vec())
}

#[cfg(not(windows))]
fn decrypt_for_current_user(encrypted: &[u8]) -> Result<Vec<u8>, String> {
    Ok(encrypted.to_vec())
}
