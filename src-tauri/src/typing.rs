use enigo::{Enigo, Key, Keyboard, Settings};

/// Strip punctuation, lowercase, collapse whitespace.
/// "Jarvis: back, back." → "jarvis back back"
fn normalize(text: &str) -> String {
    let lower = text.trim().to_lowercase();
    // Replace any non-alphanumeric char with a space, then collapse.
    // "Jarvis-Enter." → "jarvis enter"
    // "Jarvis: back, back." → "jarvis back back"
    lower
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ")
}

/// Commands sorted longest-first so "back back" matches before "back".
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

const WAKE_WORDS: &[&str] = &["jarvis", "jarvi", "jarbi", "jarbis", "jarviss"];

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

fn match_command(phrase: &str) -> Option<(&'static str, fn())> {
    for (keyword, action) in COMMANDS {
        if phrase == *keyword {
            return Some((*keyword, *action));
        }
    }
    None
}

pub fn process_transcript(text: &str) {
    let norm = normalize(text);
    let mut parts = norm.split_whitespace();
    let first = parts.next().unwrap_or("");

    if WAKE_WORDS.contains(&first) {
        let after_prefix = parts.collect::<Vec<&str>>().join(" ");
        // Try each command pattern (longest first)
        for (keyword, action) in COMMANDS {
            // Exact match or keyword followed by more text
            if after_prefix == *keyword || after_prefix.starts_with(&format!("{} ", keyword)) {
                println!("[typing] command: \"{}\"", keyword);
                action();
                // Type any remaining text after the command keyword
                let remainder = after_prefix[keyword.len()..].trim();
                if !remainder.is_empty() {
                    println!("[typing] typing remainder: \"{}\"", remainder);
                    type_text(remainder);
                }
                return;
            }
        }
        // Wake word but no known keyword — type the whole original
        println!("[typing] unknown command in: \"{}\"", after_prefix);
        type_text(text);
        return;
    }

    // No wake word: only accept standalone command (no extra words)
    if let Some((keyword, action)) = match_command(&norm) {
        println!("[typing] command: \"{}\"", keyword);
        action();
        return;
    }

    type_text(text);
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
