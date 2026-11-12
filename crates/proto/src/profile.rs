//! Human-editable YAML profile format.
//!
//! ```yaml
//! layers:
//!   - slots:
//!       1: ctrl-c                 # keys pressed together
//!       2: { media: volup }
//!       3: { mouse: { buttons: [left] } }
//!       4: { mouse: { wheel: -1, modifier: ctrl } }
//!       5: { macro: [ { key: h, delay: 50 }, { key: i, delay: 50 } ] }
//!       6: none
//!     leds: { mode: 0, colors: ["#ff0000", "#00ff00"] }
//! ```

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::frame::{format_color, parse_color, Handshake, LedConfig};
use crate::keys;
use crate::record::{Action, MouseButtons, Step, RECORD_LEN};
use crate::{Error, Result, LED_COUNT};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    /// Informational: what the device reported when this profile was dumped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device: Option<Handshake>,
    #[serde(default)]
    pub layers: Vec<LayerSpec>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LayerSpec {
    /// Slot number (1-based; keys first, then knob actions) → action.
    /// Slots not listed are left untouched on upload.
    #[serde(default)]
    pub slots: BTreeMap<u8, ActionSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub leds: Option<LedSpec>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LedSpec {
    pub mode: u8,
    /// Up to 16 "#rrggbb" colours (missing entries are black).
    #[serde(default)]
    pub colors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ActionSpec {
    /// `ctrl-c`, `f13`, `!`, or `none`.
    Chord(String),
    Media { media: String },
    Mouse { mouse: MouseSpec },
    Macro {
        #[serde(rename = "macro")]
        steps: Vec<StepSpec>,
    },
    /// 64 bytes as hex, for records this tool does not understand.
    Raw { raw: String },
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MouseSpec {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub buttons: Vec<String>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub wheel: i8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modifier: Option<String>,
}

fn is_zero(v: &i8) -> bool {
    *v == 0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StepSpec {
    pub key: String,
    #[serde(default)]
    pub delay: u16,
}

const BUTTONS: &[(&str, u8)] =
    &[("left", MouseButtons::LEFT), ("right", MouseButtons::RIGHT), ("middle", MouseButtons::MIDDLE)];

impl ActionSpec {
    pub fn to_action(&self) -> Result<Action> {
        Ok(match self {
            ActionSpec::Chord(s) if s.trim().is_empty() || s.trim().eq_ignore_ascii_case("none") => Action::None,
            ActionSpec::Chord(s) => Action::Keys(keys::parse_chord(s)?),
            ActionSpec::Media { media } => Action::Media(keys::media_code(media)?),
            ActionSpec::Mouse { mouse } => {
                let mut buttons = 0u8;
                for b in &mouse.buttons {
                    let lower = b.to_ascii_lowercase();
                    buttons |= BUTTONS
                        .iter()
                        .find(|(n, _)| *n == lower)
                        .map(|(_, v)| *v)
                        .ok_or_else(|| Error::Invalid(format!("mouse button `{b}`")))?;
                }
                let modifier = match &mouse.modifier {
                    None => None,
                    Some(m) => Some(
                        keys::modifier_code(m).ok_or_else(|| Error::Invalid(format!("modifier `{m}`")))?,
                    ),
                };
                Action::Mouse { buttons, wheel: mouse.wheel, modifier }
            }
            ActionSpec::Macro { steps } => Action::Macro(
                steps
                    .iter()
                    .map(|s| {
                        let codes = keys::parse_chord(&s.key)?;
                        match codes.as_slice() {
                            [c] => Ok(Step { code: *c, delay_ms: s.delay }),
                            _ => Err(Error::Invalid(format!(
                                "macro step `{}` must be a single key or modifier",
                                s.key
                            ))),
                        }
                    })
                    .collect::<Result<_>>()?,
            ),
            ActionSpec::Raw { raw } => {
                let clean: String = raw.chars().filter(|c| c.is_ascii_hexdigit()).collect();
                if clean.len() != RECORD_LEN * 2 {
                    return Err(Error::Invalid(format!("raw record must be {RECORD_LEN} bytes of hex")));
                }
                let mut rec = [0u8; RECORD_LEN];
                for (i, b) in rec.iter_mut().enumerate() {
                    *b = u8::from_str_radix(&clean[i * 2..i * 2 + 2], 16).expect("hex checked");
                }
                Action::Raw(rec)
            }
        })
    }

    pub fn from_action(a: &Action) -> ActionSpec {
        match a {
            Action::None => ActionSpec::Chord("none".into()),
            Action::Keys(c) => ActionSpec::Chord(keys::format_chord(c)),
            Action::Media(m) => ActionSpec::Media { media: keys::media_name(*m) },
            Action::Mouse { buttons, wheel, modifier } => ActionSpec::Mouse {
                mouse: MouseSpec {
                    buttons: BUTTONS.iter().filter(|(_, v)| buttons & v != 0).map(|(n, _)| n.to_string()).collect(),
                    wheel: *wheel,
                    modifier: modifier.map(keys::code_name),
                },
            },
            Action::Macro(steps) => ActionSpec::Macro {
                steps: steps.iter().map(|s| StepSpec { key: keys::code_name(s.code), delay: s.delay_ms }).collect(),
            },
            Action::Raw(r) => ActionSpec::Raw { raw: r.iter().map(|b| format!("{b:02x}")).collect() },
        }
    }
}

impl LedSpec {
    pub fn to_config(&self) -> Result<LedConfig> {
        if self.colors.len() > LED_COUNT {
            return Err(Error::Invalid(format!("at most {LED_COUNT} LED colours per layer")));
        }
        if self.mode > 5 {
            return Err(Error::Invalid(format!("LED mode {} (expected 0..=5)", self.mode)));
        }
        let mut cfg = LedConfig { mode: self.mode, ..Default::default() };
        for (i, c) in self.colors.iter().enumerate() {
            cfg.colors[i] = parse_color(c)?;
        }
        Ok(cfg)
    }

    pub fn from_config(c: &LedConfig) -> LedSpec {
        LedSpec { mode: c.mode, colors: c.colors.iter().map(|&c| format_color(c)).collect() }
    }
}

impl Profile {
    pub fn from_yaml(s: &str) -> Result<Profile> {
        serde_yaml::from_str(s).map_err(|e| Error::Invalid(e.to_string()))
    }
    pub fn to_yaml(&self) -> String {
        serde_yaml::to_string(self).expect("profile serialises")
    }
    /// Validate every action and LED entry, reporting the first error with context.
    pub fn validate(&self) -> Result<()> {
        if self.layers.len() > crate::LAYERS {
            return Err(Error::Invalid(format!("at most {} layers", crate::LAYERS)));
        }
        for (li, l) in self.layers.iter().enumerate() {
            for (slot, a) in &l.slots {
                if *slot == 0 || *slot as usize >= crate::SLOTS_PER_LAYER {
                    return Err(Error::Invalid(format!("layer {}: slot {slot} out of range", li + 1)));
                }
                a.to_action()
                    .and_then(|a| a.encode(&[0; RECORD_LEN]))
                    .map_err(|e| Error::Invalid(format!("layer {} slot {slot}: {e}", li + 1)))?;
            }
            if let Some(leds) = &l.leds {
                leds.to_config().map_err(|e| Error::Invalid(format!("layer {} leds: {e}", li + 1)))?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE: &str = r##"
layers:
  - slots:
      1: ctrl-c
      2: { media: volup }
      3: { mouse: { buttons: [left, right] } }
      4: { mouse: { wheel: -1, modifier: ctrl } }
      5: { macro: [ { key: h, delay: 50 }, { key: shift, delay: 0 }, { key: i, delay: 50 } ] }
      6: none
    leds: { mode: 2, colors: ["#ff0000", "#00ff00"] }
"##;

    #[test]
    fn parse_example() {
        let p = Profile::from_yaml(EXAMPLE).unwrap();
        p.validate().unwrap();
        let s = &p.layers[0].slots;
        assert_eq!(s[&1].to_action().unwrap(), Action::Keys(vec![0xF1, 0x06]));
        assert_eq!(s[&2].to_action().unwrap(), Action::Media(0xE9));
        assert_eq!(s[&3].to_action().unwrap(), Action::Mouse { buttons: 3, wheel: 0, modifier: None });
        assert_eq!(s[&4].to_action().unwrap(), Action::Mouse { buttons: 0, wheel: -1, modifier: Some(0xF1) });
        assert_eq!(s[&6].to_action().unwrap(), Action::None);
        let leds = p.layers[0].leds.as_ref().unwrap().to_config().unwrap();
        assert_eq!(leds.mode, 2);
        assert_eq!(leds.colors[1], [0, 255, 0]);
    }

    #[test]
    fn yaml_roundtrip_via_actions() {
        let p = Profile::from_yaml(EXAMPLE).unwrap();
        for spec in p.layers[0].slots.values() {
            let a = spec.to_action().unwrap();
            assert_eq!(ActionSpec::from_action(&a).to_action().unwrap(), a);
        }
        let again = Profile::from_yaml(&p.to_yaml()).unwrap();
        assert_eq!(again, p);
    }

    #[test]
    fn raw_roundtrip() {
        let mut rec = [0u8; 64];
        rec[0] = 0xFD;
        rec[3] = 9;
        let spec = ActionSpec::from_action(&Action::Raw(rec));
        assert_eq!(spec.to_action().unwrap(), Action::Raw(rec));
    }

    #[test]
    fn validation_errors() {
        let p = Profile::from_yaml("layers: [ { slots: { 1: ctrl-bogus } } ]").unwrap();
        assert!(p.validate().unwrap_err().to_string().contains("slot 1"));
        let p = Profile::from_yaml("layers: [ { slots: { 0: a } } ]").unwrap();
        assert!(p.validate().is_err());
    }
}
