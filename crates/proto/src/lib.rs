//! Protocol definitions for the MINI_KEYBOARD macropad family
//! (USB `1189:88xx` / `514C:88xx`). See `docs/protocol.md`.
//!
//! This crate performs no I/O: it only builds 65-byte output frames and
//! decodes 64-byte replies, so everything here is unit-testable.

#![forbid(unsafe_code)]

pub mod frame;
pub mod keys;
pub mod profile;
pub mod record;

pub use frame::*;
pub use record::{Action, MouseButtons, Record, Step};

/// Known vendor IDs, in the order the vendor tool tries them.
pub const VENDOR_IDS: [u16; 2] = [0x1189, 0x514C];
/// Known product IDs, in the order the vendor tool tries them.
pub const PRODUCT_IDS: [u16; 8] = [0x8842, 0x8840, 0x8830, 0x8831, 0x8832, 0x8833, 0x8850, 0x8851];

/// Number of layers supported by the firmware.
pub const LAYERS: usize = 3;
/// Slots per layer in the vendor tool's tables (slot 0 unused).
pub const SLOTS_PER_LAYER: usize = 60;
/// Number of LED colour entries per layer.
pub const LED_COUNT: usize = 16;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum Error {
    #[error("unknown key name `{0}`")]
    UnknownKey(String),
    #[error("unknown media key `{0}`")]
    UnknownMedia(String),
    #[error("too many steps ({0}); a record holds at most {max}", max = record::MAX_STEPS)]
    TooManySteps(usize),
    #[error("invalid value: {0}")]
    Invalid(String),
}

pub type Result<T> = std::result::Result<T, Error>;
