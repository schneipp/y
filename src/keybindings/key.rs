use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KeyCombo {
    pub key: Key,
    pub ctrl: bool,
    pub alt: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Key {
    Char(char),
    Enter,
    Esc,
    Backspace,
    Tab,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    Delete,
    F(u8),
}

impl KeyCombo {
    pub fn from_event(event: &KeyEvent) -> Self {
        let key = match event.code {
            KeyCode::Char(c) => Key::Char(c),
            KeyCode::Enter => Key::Enter,
            KeyCode::Esc => Key::Esc,
            KeyCode::Backspace => Key::Backspace,
            KeyCode::Tab => Key::Tab,
            KeyCode::Left => Key::Left,
            KeyCode::Right => Key::Right,
            KeyCode::Up => Key::Up,
            KeyCode::Down => Key::Down,
            KeyCode::Home => Key::Home,
            KeyCode::End => Key::End,
            KeyCode::PageUp => Key::PageUp,
            KeyCode::PageDown => Key::PageDown,
            KeyCode::Delete => Key::Delete,
            KeyCode::F(n) => Key::F(n),
            _ => Key::Char('\0'),
        };
        Self {
            key,
            ctrl: event.modifiers.contains(KeyModifiers::CONTROL),
            alt: event.modifiers.contains(KeyModifiers::ALT),
        }
    }

    pub fn char(c: char) -> Self {
        Self { key: Key::Char(c), ctrl: false, alt: false }
    }

    pub fn ctrl(c: char) -> Self {
        Self { key: Key::Char(c), ctrl: true, alt: false }
    }

    pub fn special(key: Key) -> Self {
        Self { key, ctrl: false, alt: false }
    }
}

/// Parse a key string like "h", "Ctrl+r", "Esc", "Space f f" into a Vec<KeyCombo>.
/// Space-separated tokens form a sequence.
pub fn parse_key_string(s: &str) -> Result<Vec<KeyCombo>, String> {
    let tokens: Vec<&str> = s.split_whitespace().collect();
    if tokens.is_empty() {
        return Err("empty key string".into());
    }
    let mut result = Vec::new();
    for token in tokens {
        result.push(parse_single_token(token)?);
    }
    Ok(result)
}

fn parse_single_token(token: &str) -> Result<KeyCombo, String> {
    // Check for modifier prefixes
    let parts: Vec<&str> = token.split('+').collect();
    let mut ctrl = false;
    let mut alt = false;

    if parts.len() > 1 {
        // Last part is the key, everything before is modifiers
        for &modifier in &parts[..parts.len() - 1] {
            match modifier.to_lowercase().as_str() {
                "ctrl" => ctrl = true,
                "alt" => alt = true,
                "shift" => {} // shift is encoded in the character itself
                _ => return Err(format!("unknown modifier: {}", modifier)),
            }
        }
        let key_str = parts[parts.len() - 1];
        let key = parse_key_name(key_str)?;
        Ok(KeyCombo { key, ctrl, alt })
    } else {
        let key = parse_key_name(token)?;
        Ok(KeyCombo { key, ctrl: false, alt: false })
    }
}

fn parse_key_name(name: &str) -> Result<Key, String> {
    match name.to_lowercase().as_str() {
        "enter" | "return" | "cr" => Ok(Key::Enter),
        "esc" | "escape" => Ok(Key::Esc),
        "backspace" | "bs" => Ok(Key::Backspace),
        "tab" => Ok(Key::Tab),
        "left" => Ok(Key::Left),
        "right" => Ok(Key::Right),
        "up" => Ok(Key::Up),
        "down" => Ok(Key::Down),
        "home" => Ok(Key::Home),
        "end" => Ok(Key::End),
        "pageup" => Ok(Key::PageUp),
        "pagedown" => Ok(Key::PageDown),
        "delete" | "del" => Ok(Key::Delete),
        "space" => Ok(Key::Char(' ')),
        s if s.starts_with('f') && s.len() >= 2 => {
            if let Ok(n) = s[1..].parse::<u8>() {
                if n >= 1 && n <= 12 {
                    return Ok(Key::F(n));
                }
            }
            // Fall through to single-char check
            let chars: Vec<char> = name.chars().collect();
            if chars.len() == 1 {
                Ok(Key::Char(chars[0]))
            } else {
                Err(format!("unknown key: {}", name))
            }
        }
        _ => {
            // Single character — preserve case from original name (not lowercased)
            let chars: Vec<char> = name.chars().collect();
            if chars.len() == 1 {
                Ok(Key::Char(chars[0]))
            } else {
                Err(format!("unknown key: {}", name))
            }
        }
    }
}

/// Convert a KeyCombo back to its string representation for serialization.
pub fn key_combo_to_string(combo: &KeyCombo) -> String {
    let mut parts = Vec::new();
    if combo.ctrl {
        parts.push("Ctrl".to_string());
    }
    if combo.alt {
        parts.push("Alt".to_string());
    }
    let key_str = match &combo.key {
        Key::Char(' ') => "Space".to_string(),
        Key::Char(c) => c.to_string(),
        Key::Enter => "Enter".to_string(),
        Key::Esc => "Esc".to_string(),
        Key::Backspace => "Backspace".to_string(),
        Key::Tab => "Tab".to_string(),
        Key::Left => "Left".to_string(),
        Key::Right => "Right".to_string(),
        Key::Up => "Up".to_string(),
        Key::Down => "Down".to_string(),
        Key::Home => "Home".to_string(),
        Key::End => "End".to_string(),
        Key::PageUp => "PageUp".to_string(),
        Key::PageDown => "PageDown".to_string(),
        Key::Delete => "Delete".to_string(),
        Key::F(n) => format!("F{}", n),
    };
    parts.push(key_str);
    parts.join("+")
}

/// Convert a sequence of KeyCombos to a string representation.
pub fn sequence_to_string(seq: &[KeyCombo]) -> String {
    seq.iter()
        .map(key_combo_to_string)
        .collect::<Vec<_>>()
        .join(" ")
}
