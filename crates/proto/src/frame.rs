//! Raw command frames (65 bytes: report ID 0x03 + 64 payload bytes) and
//! reply parsers (64 bytes as returned by `hid_read`).

use crate::{Error, Result, LED_COUNT};

pub const REPORT_ID: u8 = 0x03;
pub const FRAME_LEN: usize = 65;
pub const REPLY_LEN: usize = 64;

pub type Frame = [u8; FRAME_LEN];
pub type Reply = [u8; REPLY_LEN];

fn frame(payload: &[u8]) -> Frame {
    let mut f = [0u8; FRAME_LEN];
    f[0] = REPORT_ID;
    f[1..1 + payload.len()].copy_from_slice(payload);
    f
}

/// `03 FB FB FB` — asks the keypad for its key/knob counts and protocol generation.
pub fn handshake() -> Frame {
    frame(&[0xFB, 0xFB, 0xFB])
}

/// `03 FA n 00 L` — request `n` key records of `layer` (0-based here, sent 1-based).
pub fn read_layer(count: u8, layer: u8, legacy_knobs: Option<u8>) -> Frame {
    frame(&[0xFA, count, legacy_knobs.unwrap_or(0), layer + 1])
}

/// `03 FD FE FF` — persist the preceding writes.
pub fn commit() -> Frame {
    frame(&[0xFD, 0xFE, 0xFF])
}

/// `03 FA B0 L` — read LED mode/colours of `layer` (0-based).
pub fn read_leds(layer: u8) -> Frame {
    frame(&[0xFA, 0xB0, layer])
}

/// `03 FE B0 L mode RGB×16` — write LED mode/colours of `layer` (0-based).
pub fn write_leds(layer: u8, leds: &LedConfig) -> Frame {
    let mut f = frame(&[0xFE, 0xB0, layer, leds.mode]);
    for (i, c) in leds.colors.iter().enumerate() {
        f[5 + i * 3..8 + i * 3].copy_from_slice(c);
    }
    f
}

/// `03 FC FC keys knobs flag` — switch the physical layout preset. Dangerous:
/// the vendor tool rewrites default mappings afterwards.
pub fn select_layout(keys: u8, knobs: u8, flag: u8) -> Frame {
    frame(&[0xFC, 0xFC, keys, knobs, flag])
}

/// `03 <record>` — write one 64-byte key record.
pub fn write_record(rec: &[u8; 64]) -> Frame {
    frame(rec)
}

/// Returns true if sending `f` changes device state (anything but reads/handshake).
pub fn is_mutating(f: &Frame) -> bool {
    !matches!(f[1], 0xFA | 0xFB)
}

/// Parsed handshake reply.
///
/// Observed on a real 514C:8850 (16 keys, 3 knobs): `03 FB 10 03 0B 00 …`
/// i.e. `[report id, FB echo, keys, knobs, generation]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Handshake {
    pub keys: u8,
    pub knobs: u8,
    pub generation: u8,
}

impl Handshake {
    /// Parse a handshake reply; `None` if it doesn't echo the `FB` command.
    pub fn parse(r: &Reply) -> Option<Self> {
        (r[1] == 0xFB).then(|| Handshake { keys: r[2], knobs: r[3], generation: r[4] })
    }
    /// Build the reply a device would send (used by the simulator).
    pub fn to_reply(&self) -> Reply {
        let mut r = [0u8; REPLY_LEN];
        r[..5].copy_from_slice(&[REPORT_ID, 0xFB, self.keys, self.knobs, self.generation]);
        r
    }
    /// Generations below 10 use the legacy record layout.
    pub fn is_legacy(&self) -> bool {
        self.generation < 10
    }
    /// Number of key-record slots used (keys + 3 actions per knob). Inferred.
    pub fn slot_count(&self) -> u8 {
        self.keys.saturating_add(self.knobs.saturating_mul(3)).min(59)
    }
}

/// LED settings of one layer.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LedConfig {
    pub mode: u8,
    pub colors: [[u8; 3]; LED_COUNT],
}

impl LedConfig {
    pub fn parse(r: &Reply) -> Self {
        let mut colors = [[0u8; 3]; LED_COUNT];
        for (i, c) in colors.iter_mut().enumerate() {
            c.copy_from_slice(&r[3 + i * 3..6 + i * 3]);
        }
        LedConfig { mode: r[2], colors }
    }
}

/// Extract a key record from a `read_layer` reply: `rec[1..60] = r[2..61]`, `rec[0] = 0xFD`.
pub fn record_from_reply(r: &Reply, legacy: bool) -> [u8; 64] {
    let mut rec = [0u8; 64];
    let n = if legacy { 0x31 } else { 0x3B };
    rec[0] = if legacy { 0xFE } else { 0xFD };
    rec[1..1 + n].copy_from_slice(&r[2..2 + n]);
    rec
}

/// Validate the "#rrggbb" colour syntax.
pub fn parse_color(s: &str) -> Result<[u8; 3]> {
    let h = s.trim().trim_start_matches('#');
    if h.len() != 6 {
        return Err(Error::Invalid(format!("colour `{s}` must be #rrggbb")));
    }
    let v = u32::from_str_radix(h, 16).map_err(|_| Error::Invalid(format!("colour `{s}`")))?;
    Ok([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

pub fn format_color(c: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(f: &[u8]) -> String {
        let end = f.iter().rposition(|&b| b != 0).map_or(0, |p| p + 1);
        f[..end].iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ")
    }

    #[test]
    fn golden_frames() {
        assert_eq!(hex(&handshake()), "03 fb fb fb");
        assert_eq!(hex(&commit()), "03 fd fe ff");
        assert_eq!(hex(&read_layer(15, 0, None)), "03 fa 0f 00 01");
        assert_eq!(hex(&read_leds(2)), "03 fa b0 02");
        assert_eq!(hex(&select_layout(12, 2, 0)), "03 fc fc 0c 02");
        let mut leds = LedConfig { mode: 3, ..Default::default() };
        leds.colors[0] = [0xff, 0x00, 0x80];
        assert_eq!(hex(&write_leds(1, &leds)), "03 fe b0 01 03 ff 00 80");
        assert_eq!(write_leds(1, &leds).len(), 65);
    }

    #[test]
    fn mutating_classification() {
        assert!(!is_mutating(&handshake()));
        assert!(!is_mutating(&read_leds(0)));
        assert!(is_mutating(&commit()));
        assert!(is_mutating(&select_layout(1, 0, 0)));
    }

    #[test]
    fn handshake_parse_real_8850_reply() {
        // Captured from a real 514C:8850 on 2026-11-12.
        let mut r = [0u8; 64];
        r[..5].copy_from_slice(&[0x03, 0xFB, 0x10, 0x03, 0x0B]);
        let h = Handshake::parse(&r).unwrap();
        assert_eq!(h, Handshake { keys: 16, knobs: 3, generation: 11 });
        assert!(!h.is_legacy());
        assert_eq!(h.slot_count(), 25);
        assert_eq!(h.to_reply(), r);
        r[1] = 0;
        assert_eq!(Handshake::parse(&r), None);
    }

    #[test]
    fn led_parse_roundtrip() {
        let mut leds = LedConfig { mode: 4, ..Default::default() };
        leds.colors[15] = [1, 2, 3];
        let f = write_leds(0, &leds);
        // A reply mirrors the payload layout shifted by one (report-id/byte0 then FE?): build one.
        let mut r = [0u8; 64];
        r[2] = 4;
        r[3..51].copy_from_slice(&f[5..53]);
        assert_eq!(LedConfig::parse(&r), leds);
    }

    #[test]
    fn colors() {
        assert_eq!(parse_color("#FF0080").unwrap(), [255, 0, 128]);
        assert_eq!(format_color([255, 0, 128]), "#ff0080");
        assert!(parse_color("red").is_err());
    }
}
