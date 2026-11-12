//! Key name tables. Key codes are standard USB HID keyboard usages; modifiers
//! are encoded by the firmware as pseudo-codes 0xF1..0xF8; media keys are
//! 16-bit HID consumer-page usages.

use crate::{Error, Result};

/// Modifier pseudo-codes used inside records (one step per modifier).
pub const MODIFIERS: &[(&str, u8)] = &[
    ("ctrl", 0xF1),
    ("shift", 0xF2),
    ("alt", 0xF3),
    ("win", 0xF4),
    ("rctrl", 0xF5),
    ("rshift", 0xF6),
    ("ralt", 0xF7),
    ("rwin", 0xF8),
];

const MODIFIER_ALIASES: &[(&str, &str)] = &[
    ("control", "ctrl"),
    ("lctrl", "ctrl"),
    ("lshift", "shift"),
    ("lalt", "alt"),
    ("option", "alt"),
    ("opt", "alt"),
    ("gui", "win"),
    ("super", "win"),
    ("meta", "win"),
    ("cmd", "win"),
    ("command", "win"),
    ("lwin", "win"),
    ("altgr", "ralt"),
    ("rcmd", "rwin"),
];

/// Standard HID keyboard usages (page 0x07).
pub const KEYS: &[(&str, u8)] = &[
    ("a", 0x04), ("b", 0x05), ("c", 0x06), ("d", 0x07), ("e", 0x08), ("f", 0x09),
    ("g", 0x0A), ("h", 0x0B), ("i", 0x0C), ("j", 0x0D), ("k", 0x0E), ("l", 0x0F),
    ("m", 0x10), ("n", 0x11), ("o", 0x12), ("p", 0x13), ("q", 0x14), ("r", 0x15),
    ("s", 0x16), ("t", 0x17), ("u", 0x18), ("v", 0x19), ("w", 0x1A), ("x", 0x1B),
    ("y", 0x1C), ("z", 0x1D),
    ("1", 0x1E), ("2", 0x1F), ("3", 0x20), ("4", 0x21), ("5", 0x22),
    ("6", 0x23), ("7", 0x24), ("8", 0x25), ("9", 0x26), ("0", 0x27),
    ("enter", 0x28), ("esc", 0x29), ("backspace", 0x2A), ("tab", 0x2B), ("space", 0x2C),
    ("minus", 0x2D), ("equal", 0x2E), ("lbracket", 0x2F), ("rbracket", 0x30),
    ("backslash", 0x31), ("nonushash", 0x32), ("semicolon", 0x33), ("quote", 0x34),
    ("grave", 0x35), ("comma", 0x36), ("dot", 0x37), ("slash", 0x38), ("capslock", 0x39),
    ("f1", 0x3A), ("f2", 0x3B), ("f3", 0x3C), ("f4", 0x3D), ("f5", 0x3E), ("f6", 0x3F),
    ("f7", 0x40), ("f8", 0x41), ("f9", 0x42), ("f10", 0x43), ("f11", 0x44), ("f12", 0x45),
    ("printscreen", 0x46), ("scrolllock", 0x47), ("pause", 0x48), ("insert", 0x49),
    ("home", 0x4A), ("pageup", 0x4B), ("delete", 0x4C), ("end", 0x4D), ("pagedown", 0x4E),
    ("right", 0x4F), ("left", 0x50), ("down", 0x51), ("up", 0x52), ("numlock", 0x53),
    ("kpslash", 0x54), ("kpasterisk", 0x55), ("kpminus", 0x56), ("kpplus", 0x57),
    ("kpenter", 0x58), ("kp1", 0x59), ("kp2", 0x5A), ("kp3", 0x5B), ("kp4", 0x5C),
    ("kp5", 0x5D), ("kp6", 0x5E), ("kp7", 0x5F), ("kp8", 0x60), ("kp9", 0x61),
    ("kp0", 0x62), ("kpdot", 0x63), ("nonusbackslash", 0x64), ("menu", 0x65),
    ("f13", 0x68), ("f14", 0x69), ("f15", 0x6A), ("f16", 0x6B), ("f17", 0x6C),
    ("f18", 0x6D), ("f19", 0x6E), ("f20", 0x6F), ("f21", 0x70), ("f22", 0x71),
    ("f23", 0x72), ("f24", 0x73),
];

const KEY_ALIASES: &[(&str, &str)] = &[
    ("return", "enter"), ("escape", "esc"), ("bksp", "backspace"), ("spc", "space"),
    ("-", "minus"), ("=", "equal"), ("[", "lbracket"), ("]", "rbracket"),
    ("\\", "backslash"), (";", "semicolon"), ("'", "quote"), ("`", "grave"),
    (",", "comma"), (".", "dot"), ("/", "slash"), ("del", "delete"), ("ins", "insert"),
    ("pgup", "pageup"), ("pgdn", "pagedown"), ("prtsc", "printscreen"),
    ("app", "menu"), ("application", "menu"),
];

/// Shifted symbols the vendor UI offers as single buttons (encoded as shift + base key).
pub const SHIFTED: &[(&str, &str)] = &[
    ("!", "1"), ("@", "2"), ("#", "3"), ("$", "4"), ("%", "5"), ("^", "6"), ("&", "7"),
    ("*", "8"), ("(", "9"), (")", "0"), ("_", "minus"), ("+", "equal"), ("{", "lbracket"),
    ("}", "rbracket"), ("|", "backslash"), (":", "semicolon"), ("\"", "quote"),
    ("~", "grave"), ("<", "comma"), (">", "dot"), ("?", "slash"),
];

/// HID consumer-page usages offered by the vendor tool.
pub const MEDIA: &[(&str, u16)] = &[
    ("play", 0x00CD),
    ("stop", 0x00B7),
    ("prev", 0x00B6),
    ("next", 0x00B5),
    ("mute", 0x00E2),
    ("volup", 0x00E9),
    ("voldown", 0x00EA),
    ("brightnessup", 0x006F),
    ("brightnessdown", 0x0070),
    ("mediaselect", 0x0183),
    ("mail", 0x018A),
    ("calculator", 0x0192),
    ("mycomputer", 0x0194),
    ("wwwsearch", 0x0221),
    ("wwwhome", 0x0223),
    ("wwwback", 0x0224),
    ("wwwforward", 0x0225),
    ("wwwstop", 0x0226),
    ("wwwrefresh", 0x0227),
    ("wwwfavorites", 0x022A),
];

fn resolve<'a>(aliases: &[(&'a str, &'a str)], name: &'a str) -> &'a str {
    aliases.iter().find(|(a, _)| *a == name).map_or(name, |(_, t)| t)
}

/// Code of a modifier name, if it is one.
pub fn modifier_code(name: &str) -> Option<u8> {
    let n = name.to_ascii_lowercase();
    let n = resolve(MODIFIER_ALIASES, &n);
    MODIFIERS.iter().find(|(k, _)| *k == n).map(|(_, c)| *c)
}

/// Code of a plain (non-modifier) key name.
pub fn key_code(name: &str) -> Option<u8> {
    let lower = name.to_ascii_lowercase();
    let n = resolve(KEY_ALIASES, &lower);
    if let Some(hex) = n.strip_prefix("0x") {
        return u8::from_str_radix(hex, 16).ok();
    }
    KEYS.iter().find(|(k, _)| *k == n).map(|(_, c)| *c)
}

/// Name for any single-byte code (key or modifier); falls back to `0xNN`.
pub fn code_name(code: u8) -> String {
    MODIFIERS
        .iter()
        .chain(KEYS.iter())
        .find(|(_, c)| *c == code)
        .map_or_else(|| format!("0x{code:02x}"), |(n, _)| n.to_string())
}

pub fn is_modifier(code: u8) -> bool {
    (0xF1..=0xF8).contains(&code)
}

/// Parse a chord like `ctrl-shift-a`, `ctrl+c`, `!` or `f13` into step codes
/// (modifiers first, in the order given).
pub fn parse_chord(s: &str) -> Result<Vec<u8>> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(Vec::new());
    }
    // Single-character tokens like "-" or "+" must not be split.
    let parts: Vec<&str> = if s.len() == 1 { vec![s] } else { s.split(['-', '+']).collect() };
    let mut out = Vec::new();
    for p in parts {
        if p.is_empty() {
            return Err(Error::UnknownKey(s.to_string()));
        }
        if let Some(c) = modifier_code(p) {
            out.push(c);
        } else if let Some((_, base)) = SHIFTED.iter().find(|(sym, _)| *sym == p) {
            out.push(0xF2);
            out.push(key_code(base).expect("table is consistent"));
        } else if let Some(c) = key_code(p) {
            out.push(c);
        } else {
            return Err(Error::UnknownKey(p.to_string()));
        }
    }
    Ok(out)
}

/// Inverse of [`parse_chord`].
pub fn format_chord(codes: &[u8]) -> String {
    codes.iter().map(|&c| code_name(c)).collect::<Vec<_>>().join("-")
}

pub fn media_code(name: &str) -> Result<u16> {
    let n = name.trim().to_ascii_lowercase();
    if let Some(hex) = n.strip_prefix("0x") {
        return u16::from_str_radix(hex, 16).map_err(|_| Error::UnknownMedia(name.into()));
    }
    MEDIA
        .iter()
        .find(|(k, _)| *k == n)
        .map(|(_, c)| *c)
        .ok_or_else(|| Error::UnknownMedia(name.into()))
}

pub fn media_name(code: u16) -> String {
    MEDIA.iter().find(|(_, c)| *c == code).map_or_else(|| format!("0x{code:04x}"), |(n, _)| n.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chords() {
        assert_eq!(parse_chord("ctrl-c").unwrap(), vec![0xF1, 0x06]);
        assert_eq!(parse_chord("Ctrl+Shift+Esc").unwrap(), vec![0xF1, 0xF2, 0x29]);
        assert_eq!(parse_chord("!").unwrap(), vec![0xF2, 0x1E]);
        assert_eq!(parse_chord("-").unwrap(), vec![0x2D]);
        assert_eq!(parse_chord("cmd-space").unwrap(), vec![0xF4, 0x2C]);
        assert_eq!(parse_chord("0x68").unwrap(), vec![0x68]);
        assert!(parse_chord("ctrl-nope").is_err());
        assert_eq!(format_chord(&[0xF1, 0x06]), "ctrl-c");
        assert_eq!(parse_chord(&format_chord(&[0xF3, 0xF5, 0x3A])).unwrap(), vec![0xF3, 0xF5, 0x3A]);
    }

    #[test]
    fn media() {
        assert_eq!(media_code("VolUp").unwrap(), 0xE9);
        assert_eq!(media_code("0x1234").unwrap(), 0x1234);
        assert_eq!(media_name(0xCD), "play");
        assert_eq!(media_name(0x1234), "0x1234");
    }

    #[test]
    fn tables_have_unique_codes() {
        let mut seen = std::collections::HashSet::new();
        for (_, c) in KEYS.iter().chain(MODIFIERS.iter()) {
            assert!(seen.insert(*c), "duplicate code {c:#x}");
        }
    }
}
