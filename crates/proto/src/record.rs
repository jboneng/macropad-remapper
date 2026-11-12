//! 64-byte key records (protocol generation >= 10).
//!
//! Layout (see docs/protocol.md):
//! `[0]=0xFD  [1..3)=header (kept from device)  [3]=type  [4]=sub-mode
//!  [5]=step count  step n: [6+3n]=delay_hi [7+3n]=delay_lo [8+3n]=code`

use crate::{Error, Result};

pub const RECORD_LEN: usize = 64;
pub const MAX_STEPS: usize = 18;

pub const TYPE_KEYS: u8 = 1;
pub const TYPE_MEDIA: u8 = 2;
pub const TYPE_MOUSE: u8 = 3;
/// Listed as a type in the vendor tool, but the firmware ignores type-5 writes (tested
/// on a 514C:8850). Timed macros are type-1 records with per-step delays instead.
pub const TYPE_MACRO_REJECTED: u8 = 5;
pub const TYPE_LED: u8 = 8;

pub type Record = [u8; RECORD_LEN];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Step {
    pub code: u8,
    pub delay_ms: u16,
}

/// Mouse button bitmask as used by the firmware.
pub struct MouseButtons;
impl MouseButtons {
    pub const LEFT: u8 = 0x01;
    pub const RIGHT: u8 = 0x02;
    pub const MIDDLE: u8 = 0x04;
}

/// High-level meaning of one key slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Slot does nothing.
    None,
    /// Keys pressed together (modifier pseudo-codes 0xF1..0xF8 + HID usages).
    Keys(Vec<u8>),
    /// HID consumer usage (volume, play/pause, ...).
    Media(u16),
    /// Mouse click / wheel, optionally with a modifier pseudo-code held.
    Mouse { buttons: u8, wheel: i8, modifier: Option<u8> },
    /// Timed sequence: each step waits its delay, then sends its key code. Encoded
    /// as a type-1 (keys) record with delays, like the vendor tool's delay page
    /// (hardware-confirmed: `h`, wait 1000 ms, `i`).
    Macro(Vec<Step>),
    /// Anything this tool does not model yet, kept byte-for-byte.
    Raw(Record),
}

fn put_step(rec: &mut Record, n: usize, code: u8, delay: u16) {
    rec[6 + 3 * n] = (delay >> 8) as u8;
    rec[7 + 3 * n] = delay as u8;
    rec[8 + 3 * n] = code;
}

fn step_code(rec: &Record, n: usize) -> u8 {
    rec[8 + 3 * n]
}

fn step_delay(rec: &Record, n: usize) -> u16 {
    u16::from_be_bytes([rec[6 + 3 * n], rec[7 + 3 * n]])
}

impl Action {
    /// Encode onto `base`, keeping its header bytes 0..3 (as read from the device).
    pub fn encode(&self, base: &Record) -> Result<Record> {
        if let Action::Raw(r) = self {
            return Ok(*r);
        }
        let mut rec = [0u8; RECORD_LEN];
        rec[..3].copy_from_slice(&base[..3]);
        match self {
            Action::None => {
                rec[3] = TYPE_KEYS;
            }
            Action::Keys(codes) => {
                if codes.len() > MAX_STEPS {
                    return Err(Error::TooManySteps(codes.len()));
                }
                rec[3] = TYPE_KEYS;
                rec[5] = codes.len() as u8;
                for (i, &c) in codes.iter().enumerate() {
                    put_step(&mut rec, i, c, 0);
                }
            }
            Action::Media(code) => {
                rec[3] = TYPE_MEDIA;
                rec[5] = 2;
                put_step(&mut rec, 0, *code as u8, 0);
                put_step(&mut rec, 1, (*code >> 8) as u8, 0);
            }
            Action::Mouse { buttons, wheel, modifier } => {
                rec[3] = TYPE_MOUSE;
                rec[4] = 1;
                rec[5] = 4;
                rec[8] = modifier.unwrap_or(0);
                rec[11] = *buttons;
                rec[20] = *wheel as u8;
            }
            Action::Macro(steps) => {
                if steps.len() > MAX_STEPS {
                    return Err(Error::TooManySteps(steps.len()));
                }
                rec[3] = TYPE_KEYS;
                rec[5] = steps.len() as u8;
                for (i, s) in steps.iter().enumerate() {
                    put_step(&mut rec, i, s.code, s.delay_ms);
                }
            }
            Action::Raw(_) => unreachable!(),
        }
        // Byte 4 is 0x01 on some factory-programmed slots; its meaning is unknown.
        // Keep it while the action type is unchanged (the vendor tool zeroes it on a type change).
        if !matches!(self, Action::Mouse { .. }) && rec[3] == base[3] {
            rec[4] = base[4];
        }
        Ok(rec)
    }

    /// Decode a record. Anything that would not re-encode byte-identically is
    /// returned as [`Action::Raw`], so dump → upload is always lossless.
    pub fn decode(rec: &Record) -> Action {
        let ty = rec[3];
        let count = (rec[5] as usize).min(MAX_STEPS);
        let candidate = if rec[4..].iter().all(|&b| b == 0) || (count == 0 && ty == TYPE_KEYS) {
            Action::None
        } else {
            match ty {
                TYPE_KEYS if (0..count).any(|i| step_delay(rec, i) != 0) => Action::Macro(
                    (0..count).map(|i| Step { code: step_code(rec, i), delay_ms: step_delay(rec, i) }).collect(),
                ),
                TYPE_KEYS => Action::Keys((0..count).map(|i| step_code(rec, i)).collect()),
                TYPE_MEDIA if count == 2 => {
                    Action::Media(u16::from_le_bytes([step_code(rec, 0), step_code(rec, 1)]))
                }
                TYPE_MOUSE if rec[4] == 1 => Action::Mouse {
                    buttons: rec[11],
                    wheel: rec[20] as i8,
                    modifier: (rec[8] != 0).then_some(rec[8]),
                },
                _ => Action::Raw(*rec),
            }
        };
        match candidate.encode(rec) {
            Ok(re) if re == *rec => candidate,
            _ => Action::Raw(*rec),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Record {
        let mut r = [0u8; 64];
        r[0] = 0xFD;
        r[1] = 0x07;
        r[2] = 0x01;
        r
    }

    #[test]
    fn keys_golden() {
        let rec = Action::Keys(vec![0xF1, 0x06]).encode(&base()).unwrap();
        assert_eq!(&rec[..12], &[0xFD, 0x07, 0x01, 1, 0, 2, 0, 0, 0xF1, 0, 0, 0x06]);
        assert!(rec[12..].iter().all(|&b| b == 0));
    }

    #[test]
    fn media_golden() {
        let rec = Action::Media(0x0192).encode(&base()).unwrap();
        assert_eq!(&rec[3..12], &[2, 0, 2, 0, 0, 0x92, 0, 0, 0x01]);
    }

    #[test]
    fn mouse_golden() {
        let rec = Action::Mouse { buttons: 0, wheel: -1, modifier: Some(0xF1) }.encode(&base()).unwrap();
        assert_eq!(rec[3], 3);
        assert_eq!(rec[4], 1);
        assert_eq!(rec[5], 4);
        assert_eq!(rec[8], 0xF1);
        assert_eq!(rec[20], 0xFF);
    }

    #[test]
    fn macro_golden() {
        let rec = Action::Macro(vec![
            Step { code: 0x0B, delay_ms: 50 },
            Step { code: 0x0C, delay_ms: 1000 },
        ])
        .encode(&base())
        .unwrap();
        assert_eq!(&rec[3..15], &[1, 0, 2, 0x00, 0x32, 0x0B, 0x03, 0xE8, 0x0C, 0, 0, 0]);
    }

    #[test]
    fn hardware_macro_record_decodes() {
        // Written to a real 514C:8850 and confirmed to type "h", pause 1 s, "i".
        let mut r = base();
        r[1] = 0x03;
        r[3..12].copy_from_slice(&[1, 0, 2, 0, 0, 0x0B, 0x03, 0xE8, 0x0C]);
        let a = Action::decode(&r);
        assert_eq!(
            a,
            Action::Macro(vec![Step { code: 0x0B, delay_ms: 0 }, Step { code: 0x0C, delay_ms: 1000 }])
        );
        assert_eq!(a.encode(&r).unwrap(), r);
    }

    #[test]
    fn roundtrips() {
        let actions = [
            Action::None,
            Action::Keys(vec![0xF1, 0xF2, 0x29]),
            Action::Media(0xE9),
            Action::Mouse { buttons: MouseButtons::LEFT, wheel: 0, modifier: None },
            Action::Macro(vec![Step { code: 4, delay_ms: 60000 }]),
        ];
        for a in actions {
            let rec = a.encode(&base()).unwrap();
            assert_eq!(Action::decode(&rec), a, "roundtrip of {a:?}");
        }
    }

    #[test]
    fn unknown_is_raw_and_lossless() {
        let mut rec = base();
        rec[3] = 3;
        rec[4] = 4; // mouse "special" sub-mode we don't model
        rec[5] = 4;
        rec[8] = 2;
        let a = Action::decode(&rec);
        assert!(matches!(a, Action::Raw(_)));
        assert_eq!(a.encode(&base()).unwrap(), rec);
        // Junk in an unused step must not be silently dropped either.
        let mut rec2 = Action::Keys(vec![4]).encode(&base()).unwrap();
        rec2[40] = 9;
        assert!(matches!(Action::decode(&rec2), Action::Raw(_)));
    }

    /// Records exactly as read from a real 514C:8850 (rec[1..] = reply[2..]).
    fn real(reply_tail: &[u8]) -> Record {
        let mut r = [0u8; 64];
        r[0] = 0xFD;
        r[1..1 + reply_tail.len()].copy_from_slice(reply_tail);
        r
    }

    #[test]
    fn real_device_records() {
        // slot 1 layer 1: "t"
        let r = real(&[0x01, 0x01, 0x01, 0x00, 0x01, 0x00, 0x00, 0x17]);
        assert_eq!(Action::decode(&r), Action::Keys(vec![0x17]));
        // slot 12 layer 1: "l" with the unknown byte 4 = 1, must round-trip
        let r = real(&[0x0c, 0x01, 0x01, 0x01, 0x01, 0x00, 0x00, 0x0f]);
        assert_eq!(Action::decode(&r), Action::Keys(vec![0x0f]));
        // changing the key keeps byte 4 and the header
        let n = Action::Keys(vec![0xF1, 0x06]).encode(&r).unwrap();
        assert_eq!(&n[..12], &[0xFD, 0x0c, 0x01, 1, 1, 2, 0, 0, 0xF1, 0, 0, 0x06]);
        // switching type to media clears it, like the vendor tool
        assert_eq!(Action::Media(0xE9).encode(&r).unwrap()[4], 0);
        // slot 10 layer 1: empty
        assert_eq!(Action::decode(&real(&[0x0a, 0x01, 0x01])), Action::None);
        // slot 17: key with leftover 50 ms delays in unused steps -> kept as raw
        let mut tail = vec![0x11, 0x01, 0x01, 0x01, 0x01, 0x00, 0x00, 0x14];
        for _ in 0..17 {
            tail.extend_from_slice(&[0x00, 0x32, 0x00]);
        }
        let r = real(&tail);
        let a = Action::decode(&r);
        assert!(matches!(a, Action::Raw(_)));
        assert_eq!(a.encode(&r).unwrap(), r);
    }

    #[test]
    fn too_many_steps() {
        assert_eq!(Action::Keys(vec![4; 19]).encode(&base()), Err(Error::TooManySteps(19)));
    }
}
