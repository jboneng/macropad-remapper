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

/// LED effect names for modes 0..=5 (generation ≥ 10), observed on a 514C:8850.
/// The vendor tool only labels them "LED mode 0..5".
pub const LED_MODE_NAMES: [&str; 6] = ["Off", "Steady", "Light on press", "Ripple", "Wave", "Gradient"];

/// One-line description of each LED mode, same order as [`LED_MODE_NAMES`].
pub const LED_MODE_HELP: [&str; 6] = [
    "All LEDs off.",
    "Every key glows steadily in its own colour.",
    "A key lights up in its colour only while it is pressed.",
    "Pressing a key lights it, then the light spreads to the keys around it.",
    "Steady colours with a fading wave running from top-left to bottom-right.",
    "Built-in green-to-amber gradient across the columns (ignores the key colours).",
];

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
