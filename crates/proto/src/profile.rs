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
//!
//! A **layer file** describes just one layer (same fields as a `layers:` entry, at the
//! top level) so it can be loaded into any layer:
//!
//! ```yaml
//! name: Lightroom
//! slots: { 1: p, 2: x }
//! ```
//!
//! A full profile can pull layer files in with `file:` (relative to the profile):
//!
//! ```yaml
//! layers:
//!   - file: negpy.yaml
//!   - file: lightroom.yaml
//!     slots: { 16: f13 }      # inline slots override the included ones
//! ```

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::frame::{format_color, parse_color, Handshake, LedConfig};
use crate::keys;
use crate::record::{Action, MouseButtons, Step, RECORD_LEN};
use crate::{Error, Result, LED_COUNT};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    /// Informational: what the device reported when this profile was dumped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device: Option<Handshake>,
    #[serde(default)]
    pub layers: Vec<LayerSpec>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LayerSpec {
    /// Optional human name, e.g. "Lightroom" (not stored on the keypad).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Include a layer file (path relative to the profile). Resolved by
    /// [`Profile::resolve_includes`]; inline `slots`/`leds`/`name` override it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    /// Slot number (1-based; keys first, then knob actions) → action.
    /// Slots not listed are left untouched on upload.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub slots: BTreeMap<u8, ActionSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub leds: Option<LedSpec>,
}

/// What a YAML file contains.
#[derive(Debug, Clone, PartialEq)]
pub enum ProfileDoc {
    /// `layers: [...]` — a whole keypad (possibly with `file:` includes).
    Full(Profile),
    /// A single layer that can be loaded into any layer.
    Layer(LayerSpec),
}

impl ProfileDoc {
    /// Parse either a full profile or a layer file (decided by a top-level `layers:` key).
    pub fn from_yaml(s: &str) -> Result<ProfileDoc> {
        let v: serde_yaml::Value = serde_yaml::from_str(s).map_err(|e| Error::Invalid(e.to_string()))?;
        let is_full = v.as_mapping().is_some_and(|m| m.contains_key("layers") || m.contains_key("device"));
        if is_full {
            Profile::from_yaml(s).map(ProfileDoc::Full)
        } else if v.is_null() {
            Err(Error::Invalid("the file is empty".into()))
        } else {
            let layer = LayerSpec::from_yaml(s)?;
            if layer.file.is_some() {
                return Err(Error::Invalid("a layer file cannot include another file".into()));
            }
            Ok(ProfileDoc::Layer(layer))
        }
    }
}

impl LayerSpec {
    pub fn from_yaml(s: &str) -> Result<LayerSpec> {
        serde_yaml::from_str(s).map_err(|e| Error::Invalid(e.to_string()))
    }
    pub fn to_yaml(&self) -> String {
        serde_yaml::to_string(self).expect("layer serialises")
    }
    /// `self` (inline entries) layered on top of `base` (an included file).
    fn merged_over(&self, base: LayerSpec) -> LayerSpec {
        let mut slots = base.slots;
        slots.extend(self.slots.clone());
        LayerSpec { name: self.name.clone().or(base.name), file: None, slots, leds: self.leds.clone().or(base.leds) }
    }
    /// Validate one layer on its own (used for layer files).
    pub fn validate(&self) -> Result<()> {
        Profile { device: None, layers: vec![self.clone()] }.validate()
    }
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
    /// A profile that only touches layer `layer` (0-based) with the given layer file.
    pub fn for_layer(layer: usize, spec: LayerSpec) -> Result<Profile> {
        if layer >= crate::LAYERS {
            return Err(Error::Invalid(format!("layer must be 1..={}", crate::LAYERS)));
        }
        let mut layers = vec![LayerSpec::default(); layer + 1];
        layers[layer] = spec;
        Ok(Profile { device: None, layers })
    }

    /// Replace every `file:` include with the referenced layer file's content.
    /// `load` receives the path exactly as written and returns the file's text
    /// (callers resolve it relative to the profile's directory).
    pub fn resolve_includes(&mut self, mut load: impl FnMut(&str) -> std::io::Result<String>) -> Result<()> {
        for (li, layer) in self.layers.iter_mut().enumerate() {
            let Some(path) = layer.file.clone() else { continue };
            let ctx = |e: String| Error::Invalid(format!("layer {} (file `{path}`): {e}", li + 1));
            let text = load(&path).map_err(|e| ctx(e.to_string()))?;
            let included = match ProfileDoc::from_yaml(&text).map_err(|e| ctx(e.to_string()))? {
                ProfileDoc::Layer(l) => l,
                ProfileDoc::Full(_) => return Err(ctx("is a full profile, not a layer file".into())),
            };
            *layer = layer.merged_over(included);
        }
        Ok(())
    }

    /// Validate every action and LED entry, reporting the first error with context.
    pub fn validate(&self) -> Result<()> {
        if self.layers.len() > crate::LAYERS {
            return Err(Error::Invalid(format!("at most {} layers", crate::LAYERS)));
        }
        for (li, l) in self.layers.iter().enumerate() {
            if let Some(f) = &l.file {
                return Err(Error::Invalid(format!("layer {}: include `{f}` was not resolved", li + 1)));
            }
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

    const LIGHTROOM: &str = "name: Lightroom\nslots:\n  1: p\n  2: x\n";
    const NEGPY: &str = "name: NegPy\nslots:\n  1: ctrl-z\n  3: f13\nleds: { mode: 2 }\n";

    fn loader(path: &str) -> std::io::Result<String> {
        match path {
            "lightroom.yaml" => Ok(LIGHTROOM.into()),
            "negpy.yaml" => Ok(NEGPY.into()),
            "full.yaml" => Ok("layers: []\n".into()),
            _ => Err(std::io::Error::new(std::io::ErrorKind::NotFound, "no such file")),
        }
    }

    #[test]
    fn detects_layer_files() {
        match ProfileDoc::from_yaml(LIGHTROOM).unwrap() {
            ProfileDoc::Layer(l) => {
                assert_eq!(l.name.as_deref(), Some("Lightroom"));
                assert_eq!(l.slots[&1].to_action().unwrap(), Action::Keys(vec![0x13]));
            }
            other => panic!("expected layer file, got {other:?}"),
        }
        assert!(matches!(ProfileDoc::from_yaml(EXAMPLE).unwrap(), ProfileDoc::Full(_)));
        assert!(ProfileDoc::from_yaml("").is_err());
        // typos are reported instead of silently ignored
        assert!(ProfileDoc::from_yaml("slot:\n  1: a\n").is_err());
        assert!(ProfileDoc::from_yaml("file: x.yaml\n").is_err(), "layer files can't include");
    }

    #[test]
    fn same_layer_files_in_different_orders() {
        let mut a = Profile::from_yaml("layers:\n  - file: negpy.yaml\n  - file: lightroom.yaml\n").unwrap();
        let mut b = Profile::from_yaml("layers:\n  - file: lightroom.yaml\n  - file: negpy.yaml\n").unwrap();
        a.resolve_includes(loader).unwrap();
        b.resolve_includes(loader).unwrap();
        a.validate().unwrap();
        assert_eq!(a.layers[0].name.as_deref(), Some("NegPy"));
        assert_eq!(a.layers[1].name.as_deref(), Some("Lightroom"));
        assert_eq!(a.layers[0], b.layers[1]);
        assert_eq!(a.layers[1], b.layers[0]);
        assert_eq!(a.layers[0].leds.as_ref().unwrap().mode, 2);
    }

    #[test]
    fn inline_overrides_include() {
        let mut p =
            Profile::from_yaml("layers:\n  - file: lightroom.yaml\n    name: LR (mine)\n    slots: { 2: u, 9: g }\n")
                .unwrap();
        p.resolve_includes(loader).unwrap();
        let l = &p.layers[0];
        assert_eq!(l.name.as_deref(), Some("LR (mine)"));
        assert_eq!(l.slots[&1].to_action().unwrap(), Action::Keys(vec![0x13]), "kept from file");
        assert_eq!(l.slots[&2].to_action().unwrap(), Action::Keys(vec![0x18]), "overridden");
        assert!(l.slots.contains_key(&9));
        assert!(l.file.is_none());
    }

    #[test]
    fn include_errors() {
        let mut p = Profile::from_yaml("layers:\n  - file: missing.yaml\n").unwrap();
        assert!(p.validate().unwrap_err().to_string().contains("not resolved"));
        assert!(p.resolve_includes(loader).unwrap_err().to_string().contains("missing.yaml"));
        let mut p = Profile::from_yaml("layers:\n  - file: full.yaml\n").unwrap();
        assert!(p.resolve_includes(loader).unwrap_err().to_string().contains("not a layer file"));
    }

    #[test]
    fn layer_file_into_any_layer() {
        let ProfileDoc::Layer(l) = ProfileDoc::from_yaml(LIGHTROOM).unwrap() else { panic!() };
        let p = Profile::for_layer(2, l.clone()).unwrap();
        assert_eq!(p.layers.len(), 3);
        assert!(p.layers[0].slots.is_empty() && p.layers[1].slots.is_empty());
        assert_eq!(p.layers[2], l);
        assert!(Profile::for_layer(3, l).is_err());
    }

    #[test]
    fn validation_errors() {
        let p = Profile::from_yaml("layers: [ { slots: { 1: ctrl-bogus } } ]").unwrap();
        assert!(p.validate().unwrap_err().to_string().contains("slot 1"));
        let p = Profile::from_yaml("layers: [ { slots: { 0: a } } ]").unwrap();
        assert!(p.validate().is_err());
    }
}
