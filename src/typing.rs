use enigo::{Enigo, Key, Keyboard, Settings};
#[cfg(windows)]
use windows::Win32::Foundation::{BOOL, HWND, LPARAM};
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, IsWindowVisible, SetForegroundWindow, ShowWindow, SW_RESTORE,
};

/// Strip punctuation, lowercase, collapse whitespace.
/// "Mango Chat: back, back." -> "mango chat back back"
fn normalize(text: &str) -> String {
    let lower = text.trim().to_lowercase();
    // Replace any non-alphanumeric char with a space, then collapse.
    // "Mango-Chat Enter." -> "mango chat enter"
    // "Mango Chat: back, back." -> "mango chat back back"
    lower
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ")
}

/// Commands sorted longest-first so "back back" matches before "back".
/// NOTE: chrome/github/youtube URL commands are handled dynamically via settings.
const COMMANDS: &[(&str, fn())] = &[
    ("back back",      cmd_delete_line as fn()),
    ("new paragraph",  cmd_new_paragraph as fn()),
    ("new line",       cmd_new_line as fn()),
    ("select all",     cmd_select_all as fn()),
    ("line break",     cmd_new_line as fn()),
    ("new para",       cmd_new_paragraph as fn()),
    ("enter",          cmd_new_line as fn()),
    ("center",         cmd_new_line as fn()),
    ("centre",         cmd_new_line as fn()),
    ("yes",            cmd_new_line as fn()),
    ("paragraph",      cmd_new_paragraph as fn()),
    ("newline",        cmd_new_line as fn()),
    ("back",           cmd_delete_word as fn()),
    ("bak",            cmd_delete_word as fn()),
    ("bac",            cmd_delete_word as fn()),
    ("bag",            cmd_delete_word as fn()),
    ("bog",            cmd_delete_word as fn()),
    ("bug",            cmd_delete_word as fn()),
    ("buck",           cmd_delete_word as fn()),
    ("undo",           cmd_undo as fn()),
    ("redo",           cmd_redo as fn()),
    ("copy",           cmd_copy as fn()),
    ("paste",          cmd_paste as fn()),
    ("cut",            cmd_cut as fn()),
];

const WAKE_WORDS: &[&str] = &["mangochat", "mango", "jarvis", "jarvi", "jarbi"];

fn cmd_new_line()       { press_enter(); }
fn cmd_new_paragraph()  { press_enter(); press_enter(); }
fn cmd_delete_word()    { delete_word(); }
fn cmd_delete_line()    { press_key_combo(&[Key::Home], true); press_key_single(Key::Backspace); }
fn cmd_undo()           { press_ctrl_key(Key::Unicode('z')); }
fn cmd_redo()           { press_ctrl_key(Key::Unicode('y')); }
fn cmd_copy()           { press_ctrl_key(Key::Unicode('c')); }
fn cmd_paste()          { press_ctrl_key(Key::Unicode('v')); }
fn cmd_cut()            { press_ctrl_key(Key::Unicode('x')); }
fn cmd_select_all()     { press_ctrl_key(Key::Unicode('a')); }
/// Open a URL in the user's chosen browser.
/// Tries the explicit path first, then a bare command name derived from the
/// path (so Firefox falls back to "firefox", Edge to "msedge", Chrome to
/// "chrome"), and finally the OS default URL handler.
pub fn open_url_in_chrome(browser_path: &str, url: &str) {
    #[cfg(windows)]
    {
        if launch_chrome_with_url(browser_path, url) {
            return;
        }
        let lower = browser_path.to_lowercase();
        let fallback = if lower.contains("firefox") {
            "firefox"
        } else if lower.contains("msedge") || lower.contains("\\edge\\") {
            "msedge"
        } else {
            "chrome"
        };
        if launch_chrome_with_url(fallback, url) {
            return;
        }
        let _ = std::process::Command::new("rundll32")
            .args(["url.dll,FileProtocolHandler", url])
            .spawn();
    }
    #[cfg(not(windows))]
    {
        let _ = (browser_path, url);
        println!("[typing] open_url_in_browser not supported on this OS");
    }
}

/// Launch an application by path.
pub fn launch_app(path: &str) {
    if path.is_empty() {
        return;
    }
    let _ = std::process::Command::new(path).spawn();
}

/// Open a path in Windows File Explorer.
pub fn open_in_explorer(path: &str) {
    #[cfg(windows)]
    {
        let target = path.trim().trim_matches('"');
        let arg = if target.is_empty() { r"C:\" } else { target };
        let _ = std::process::Command::new("explorer.exe")
            .arg(arg)
            .spawn();
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        println!("[typing] explorer command not supported on this OS");
    }
}

fn focus_or_launch_chrome(chrome_path: &str) {
    #[cfg(windows)]
    {
        if focus_existing_chrome_window() {
            return;
        }
        if launch_chrome(chrome_path) {
            return;
        }
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "", "chrome"])
            .spawn();
    }
    #[cfg(not(windows))]
    {
        let _ = chrome_path;
        println!("[typing] chrome command not supported on this OS");
    }
}

#[cfg(windows)]
fn launch_chrome_with_url(chrome_path: &str, url: &str) -> bool {
    let exe = chrome_path.trim().trim_matches('"');
    if exe.is_empty() {
        return false;
    }
    std::process::Command::new(exe)
        .arg(url)
        .spawn()
        .is_ok()
}

#[cfg(windows)]
fn launch_chrome(chrome_path: &str) -> bool {
    let exe = chrome_path.trim().trim_matches('"');
    if exe.is_empty() {
        return false;
    }
    std::process::Command::new(exe).spawn().is_ok()
}

#[cfg(windows)]
fn focus_existing_chrome_window() -> bool {
    unsafe extern "system" fn enum_windows_cb(hwnd: HWND, lparam: LPARAM) -> BOOL {
        if !IsWindowVisible(hwnd).as_bool() {
            return BOOL(1);
        }

        let mut class_buf = [0u16; 256];
        let len = unsafe { GetClassNameW(hwnd, &mut class_buf) };
        if len <= 0 {
            return BOOL(1);
        }

        let class_name = String::from_utf16_lossy(&class_buf[..len as usize]);
        if class_name.starts_with("Chrome_WidgetWin_") {
            let found = lparam.0 as *mut Option<HWND>;
            if !found.is_null() {
                unsafe {
                    *found = Some(hwnd);
                }
            }
            return BOOL(0);
        }
        BOOL(1)
    }

    let mut found: Option<HWND> = None;
    unsafe {
        let _ = EnumWindows(Some(enum_windows_cb), LPARAM(&mut found as *mut _ as isize));
    }
    if let Some(hwnd) = found {
        unsafe {
            let _ = ShowWindow(hwnd, SW_RESTORE);
            return SetForegroundWindow(hwnd).as_bool();
        }
    }
    false
}

fn match_command(phrase: &str) -> Option<(&'static str, fn())> {
    for (keyword, action) in COMMANDS {
        if phrase == *keyword {
            return Some((*keyword, *action));
        }
    }
    None
}

pub fn process_transcript(
    text: &str,
    chrome_path: &str,
    paint_path: &str,
    url_commands: &[(String, String)],
    alias_commands: &[(String, String)],
) {
    let norm = normalize(text);
    let mut parts = norm.split_whitespace();
    let first = parts.next().unwrap_or("");

    // Determine command phrase (strip wake word if present).
    let (has_wake, phrase) = if WAKE_WORDS.contains(&first) {
        (true, parts.collect::<Vec<&str>>().join(" "))
    } else {
        (false, norm.clone())
    };

    // 1. URL commands (dynamic, from settings).
    for (trigger, url) in url_commands {
        let t = normalize(trigger);
        if phrase == t
            || phrase == format!("open {}", t)
            || phrase == format!("{} com", t)
            || phrase == format!("open {} com", t)
        {
            if t == "explorer" {
                println!("[typing] explorer command: \"{}\" -> {}", trigger, url);
                open_in_explorer(url);
            } else {
                println!("[typing] url command: \"{}\" -> {}", trigger, url);
                open_url_in_chrome(chrome_path, url);
            }
            return;
        }
    }

    // 2. App-launch commands.
    if phrase == "chrome" || phrase == "open chrome" {
        println!("[typing] command: focus chrome");
        focus_or_launch_chrome(chrome_path);
        return;
    }
    if phrase == "paint" || phrase == "open paint" {
        println!("[typing] command: launch paint");
        launch_app(paint_path);
        return;
    }

    // 3. Alias commands (dynamic, from settings): exact match trigger -> type replacement.
    for (trigger, replacement) in alias_commands {
        let t = normalize(trigger);
        if !t.is_empty() && phrase == t {
            println!("[typing] alias command: \"{}\" -> \"{}\"", trigger, replacement);
            type_text(replacement);
            return;
        }
    }

    // 4. Static commands.
    if has_wake {
        for (keyword, action) in COMMANDS {
            if phrase == *keyword || phrase.starts_with(&format!("{} ", keyword)) {
                println!("[typing] command: \"{}\"", keyword);
                action();
                let remainder = phrase[keyword.len()..].trim();
                if !remainder.is_empty() {
                    println!("[typing] typing remainder: \"{}\"", remainder);
                    type_text(remainder);
                }
                return;
            }
        }
        // Wake word but no known command — type original.
        println!("[typing] unknown command in: \"{}\"", phrase);
        type_text(text);
    } else {
        // Standalone: exact match only.
        if let Some((keyword, action)) = match_command(&phrase) {
            println!("[typing] command: \"{}\"", keyword);
            action();
        } else {
            type_text(text);
        }
    }
}

// --- Input helpers ---

fn make_enigo() -> Option<Enigo> {
    match Enigo::new(&Settings::default()) {
        Ok(e) => Some(e),
        Err(e) => {
            log::error!("Failed to create enigo instance: {}", e);
            None
        }
    }
}

fn release_modifiers(enigo: &mut Enigo) {
    let _ = enigo.key(Key::Control, enigo::Direction::Release);
    let _ = enigo.key(Key::Shift, enigo::Direction::Release);
    let _ = enigo.key(Key::Alt, enigo::Direction::Release);
    let _ = enigo.key(Key::Meta, enigo::Direction::Release);
}

pub fn type_text(text: &str) {
    let Some(mut enigo) = make_enigo() else { return };
    release_modifiers(&mut enigo);

    let with_space = format!("{} ", text);
    if let Err(e) = enigo.text(&with_space) {
        log::error!("Failed to type text: {}", e);
    }
}

pub fn press_enter() {
    let Some(mut enigo) = make_enigo() else { return };
    release_modifiers(&mut enigo);

    if let Err(e) = enigo.key(Key::Return, enigo::Direction::Click) {
        log::error!("Failed to press enter: {}", e);
    }
}

/// Ctrl+Backspace — delete previous word
fn delete_word() {
    let Some(mut enigo) = make_enigo() else { return };
    release_modifiers(&mut enigo);
    // Select previous word, then delete selection for consistent behavior.
    let _ = enigo.key(Key::Control, enigo::Direction::Press);
    let _ = enigo.key(Key::Shift, enigo::Direction::Press);
    let _ = enigo.key(Key::LeftArrow, enigo::Direction::Click);
    let _ = enigo.key(Key::Shift, enigo::Direction::Release);
    let _ = enigo.key(Key::Control, enigo::Direction::Release);
    let _ = enigo.key(Key::Backspace, enigo::Direction::Click);
    // Remove trailing space that type_text appends.
    let _ = enigo.key(Key::Backspace, enigo::Direction::Click);
}

/// Press Ctrl+<key>
fn press_ctrl_key(key: Key) {
    let Some(mut enigo) = make_enigo() else { return };
    release_modifiers(&mut enigo);
    press_ctrl_key_with(&mut enigo, key);
}

fn press_ctrl_key_with(enigo: &mut Enigo, key: Key) {
    let _ = enigo.key(Key::Control, enigo::Direction::Press);
    let _ = enigo.key(key, enigo::Direction::Click);
    let _ = enigo.key(Key::Control, enigo::Direction::Release);
}

/// Press a single key
fn press_key_single(key: Key) {
    let Some(mut enigo) = make_enigo() else { return };
    release_modifiers(&mut enigo);
    let _ = enigo.key(key, enigo::Direction::Click);
}

/// Press keys with Shift held (e.g. Shift+Home to select to line start)
fn press_key_combo(keys: &[Key], with_shift: bool) {
    let Some(mut enigo) = make_enigo() else { return };
    release_modifiers(&mut enigo);

    if with_shift {
        let _ = enigo.key(Key::Shift, enigo::Direction::Press);
    }
    for key in keys {
        let _ = enigo.key(*key, enigo::Direction::Click);
    }
    if with_shift {
        let _ = enigo.key(Key::Shift, enigo::Direction::Release);
    }
}

#[allow(dead_code)]
pub fn copy_to_clipboard(text: &str) {
    match arboard::Clipboard::new() {
        Ok(mut clipboard) => {
            if let Err(e) = clipboard.set_text(text) {
                log::error!("Failed to copy to clipboard: {}", e);
            }
        }
        Err(e) => {
            log::error!("Failed to create clipboard: {}", e);
        }
    }
}


