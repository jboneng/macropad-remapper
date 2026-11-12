//! Device access for MINI_KEYBOARD macropads: a small [`Transport`] abstraction,
//! a `hidapi` implementation, a scripted mock for tests, and the [`Keypad`] API.

#![forbid(unsafe_code)]

use std::collections::VecDeque;
use std::ffi::CString;

use macropad_remapper_proto::frame::{self, Frame, Handshake, LedConfig, Reply, REPLY_LEN};
use macropad_remapper_proto::profile::{ActionSpec, LayerSpec, LedSpec, Profile, ProfileDoc};
use macropad_remapper_proto::record::{Action, Record};
use macropad_remapper_proto::{LAYERS, PRODUCT_IDS, VENDOR_IDS};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no supported keypad found (looked for VID {vids} with PID {pids})")]
    NotFound { vids: String, pids: String },
    #[error("HID error: {0}")]
    Hid(#[from] hidapi::HidError),
    #[error("the keypad did not answer the handshake; it may use the older ch57x protocol")]
    NoHandshake,
    #[error("timed out waiting for a reply to {0}")]
    Timeout(&'static str),
    #[error("protocol generation {0} (legacy layout) is read-only in this version")]
    LegacyReadOnly(u8),
    #[error("layer {layer}: slot {slot} is not reported by the device (it has {available} slots)")]
    NoSuchSlot { layer: usize, slot: u8, available: usize },
    #[error(transparent)]
    Proto(#[from] macropad_remapper_proto::Error),
    #[error("{path}: {error}")]
    Io { path: String, error: std::io::Error },
    #[error("{path}: {error}")]
    File { path: String, error: macropad_remapper_proto::Error },
}

/// Read a YAML file: a full profile (with any `file:` includes resolved relative to
/// the profile's folder) or a single-layer file.
pub fn read_profile_file(path: &std::path::Path) -> Result<ProfileDoc> {
    let shown = path.display().to_string();
    let text = std::fs::read_to_string(path).map_err(|error| Error::Io { path: shown.clone(), error })?;
    let file_err = |error| Error::File { path: shown.clone(), error };
    match ProfileDoc::from_yaml(&text).map_err(file_err)? {
        ProfileDoc::Full(mut p) => {
            let dir = path.parent().map(std::path::Path::to_path_buf).unwrap_or_default();
            p.resolve_includes(|inc| std::fs::read_to_string(dir.join(inc))).map_err(file_err)?;
            p.validate().map_err(file_err)?;
            Ok(ProfileDoc::Full(p))
        }
        ProfileDoc::Layer(l) => {
            l.validate().map_err(file_err)?;
            Ok(ProfileDoc::Layer(l))
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// Byte-level link to a keypad.
pub trait Transport {
    fn write(&mut self, frame: &Frame) -> Result<()>;
    /// Read one 64-byte reply; `Ok(None)` on timeout.
    fn read(&mut self, timeout_ms: i32) -> Result<Option<Reply>>;
}

impl<T: Transport + ?Sized> Transport for Box<T> {
    fn write(&mut self, frame: &Frame) -> Result<()> {
        (**self).write(frame)
    }
    fn read(&mut self, timeout_ms: i32) -> Result<Option<Reply>> {
        (**self).read(timeout_ms)
    }
}

// ---------------------------------------------------------------- hidapi --

/// A candidate HID interface of a supported keypad.
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub vendor_id: u16,
    pub product_id: u16,
    pub interface: i32,
    pub usage_page: u16,
    pub usage: u16,
    pub product: Option<String>,
    pub path: CString,
}

impl std::fmt::Display for DeviceInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:04x}:{:04x} if={} usage={:04x}:{:04x} {}",
            self.vendor_id,
            self.product_id,
            self.interface,
            self.usage_page,
            self.usage,
            self.product.as_deref().unwrap_or("")
        )
    }
}

/// All HID interfaces of supported keypads, best candidate first
/// (vendor-defined usage page, then interface 0 like the vendor tool).
pub fn list_devices() -> Result<Vec<DeviceInfo>> {
    let api = hidapi::HidApi::new()?;
    let mut out: Vec<DeviceInfo> = api
        .device_list()
        .filter(|d| VENDOR_IDS.contains(&d.vendor_id()) && PRODUCT_IDS.contains(&d.product_id()))
        .map(|d| DeviceInfo {
            vendor_id: d.vendor_id(),
            product_id: d.product_id(),
            interface: d.interface_number(),
            usage_page: d.usage_page(),
            usage: d.usage(),
            product: d.product_string().map(str::to_owned),
            path: d.path().to_owned(),
        })
        .collect();
    let rank = |d: &DeviceInfo| {
        let vid = VENDOR_IDS.iter().position(|v| *v == d.vendor_id).unwrap_or(9);
        let pid = PRODUCT_IDS.iter().position(|p| *p == d.product_id).unwrap_or(99);
        let iface = if d.usage_page >= 0xFF00 { 0 } else if d.interface == 0 { 1 } else { 2 };
        (vid, pid, iface)
    };
    out.sort_by_key(rank);
    Ok(out)
}

pub struct HidTransport {
    dev: hidapi::HidDevice,
}

impl HidTransport {
    pub fn open(info: &DeviceInfo) -> Result<Self> {
        let api = hidapi::HidApi::new()?;
        Ok(HidTransport { dev: api.open_path(&info.path)? })
    }
}

impl Transport for HidTransport {
    fn write(&mut self, frame: &Frame) -> Result<()> {
        self.dev.write(frame)?;
        Ok(())
    }
    fn read(&mut self, timeout_ms: i32) -> Result<Option<Reply>> {
        let mut buf = [0u8; REPLY_LEN];
        let n = self.dev.read_timeout(&mut buf, timeout_ms)?;
        Ok((n > 0).then_some(buf))
    }
}

/// Try each candidate interface until one answers the handshake.
pub fn open_first(dry_run: bool) -> Result<Keypad<Box<dyn Transport>>> {
    let devices = list_devices()?;
    if devices.is_empty() {
        let hex = |v: &[u16]| v.iter().map(|x| format!("{x:04x}")).collect::<Vec<_>>().join("/");
        return Err(Error::NotFound { vids: hex(&VENDOR_IDS), pids: hex(&PRODUCT_IDS) });
    }
    // Only the vendor-defined interface takes commands; keyboard/mouse collections
    // reject writes. Fall back to all interfaces only if none is vendor-defined.
    let vendor: Vec<&DeviceInfo> = devices.iter().filter(|d| d.usage_page >= 0xFF00).collect();
    let candidates: Vec<&DeviceInfo> = if vendor.is_empty() { devices.iter().collect() } else { vendor };
    // Report the error of the best candidate, not of the last fallback tried.
    let mut first_err = None;
    for d in candidates {
        let res = HidTransport::open(d)
            .map(|t| Box::new(t) as Box<dyn Transport>)
            .and_then(|t| Keypad::connect(t, dry_run));
        match res {
            Ok(k) => return Ok(k),
            Err(e) => {
                first_err.get_or_insert(e);
            }
        }
    }
    Err(first_err.unwrap_or(Error::NoHandshake))
}

// ------------------------------------------------------------- simulator --

/// In-memory keypad that answers like the firmware is believed to (see
/// docs/protocol.md). Used by tests and by the `--simulate` mode of the apps.
/// It assumes the record header byte 1 is the slot number.
pub struct SimulatedKeypad {
    pub handshake: Handshake,
    /// `records[layer][slot-1]`
    pub records: Vec<Vec<Record>>,
    pub leds: Vec<LedConfig>,
    pub written: Vec<Frame>,
    pub commits: usize,
    pending: VecDeque<Reply>,
}

impl SimulatedKeypad {
    pub fn new(keys: u8, knobs: u8, generation: u8) -> Self {
        let handshake = Handshake { keys, knobs, generation };
        let n = handshake.slot_count();
        let records = (0..LAYERS)
            .map(|layer| {
                (1..=n)
                    .map(|slot| {
                        let mut r = [0u8; 64];
                        r[0] = 0xFD;
                        r[1] = slot;
                        r[2] = layer as u8 + 1;
                        r[3] = 1;
                        r
                    })
                    .collect()
            })
            .collect();
        SimulatedKeypad {
            handshake,
            records,
            leds: vec![LedConfig::default(); LAYERS],
            written: Vec::new(),
            commits: 0,
            pending: VecDeque::new(),
        }
    }
}

impl Transport for SimulatedKeypad {
    fn write(&mut self, f: &Frame) -> Result<()> {
        self.written.push(*f);
        let p = &f[1..];
        match (p[0], p[1], p[2]) {
            (0xFB, 0xFB, 0xFB) => self.pending.push_back(self.handshake.to_reply()),
            (0xFA, 0xB0, layer) => {
                let mut r = [0u8; 64];
                let l = &self.leds[layer as usize % LAYERS];
                r[0] = 0x03;
                r[2] = l.mode;
                for (i, c) in l.colors.iter().enumerate() {
                    r[3 + i * 3..6 + i * 3].copy_from_slice(c);
                }
                self.pending.push_back(r);
            }
            (0xFA, n, _) => {
                let layer = (p[3] as usize).saturating_sub(1) % LAYERS;
                for rec in self.records[layer].iter().take(n as usize) {
                    let mut r = [0u8; 64];
                    r[0] = 0x03;
                    r[2..61].copy_from_slice(&rec[1..60]);
                    self.pending.push_back(r);
                }
            }
            (0xFD, 0xFE, 0xFF) => self.commits += 1,
            (0xFE, 0xB0, layer) => {
                let l = &mut self.leds[layer as usize % LAYERS];
                l.mode = p[3];
                for (i, c) in l.colors.iter_mut().enumerate() {
                    c.copy_from_slice(&p[4 + i * 3..7 + i * 3]);
                }
            }
            (0xFD, slot, layer) if slot >= 1 => {
                let layer = (layer as usize).saturating_sub(1) % LAYERS;
                if let Some(rec) = self.records[layer].get_mut(slot as usize - 1) {
                    rec.copy_from_slice(&p[..64]);
                }
            }
            _ => {}
        }
        Ok(())
    }
    fn read(&mut self, _timeout_ms: i32) -> Result<Option<Reply>> {
        Ok(self.pending.pop_front())
    }
}

// ---------------------------------------------------------------- keypad --

/// Everything readable from the device.
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceImage {
    pub handshake: Handshake,
    /// Per layer: records for slots 1..=n (index 0 = slot 1).
    pub records: Vec<Vec<Record>>,
    pub leds: Vec<LedConfig>,
}

impl DeviceImage {
    pub fn to_profile(&self) -> Profile {
        Profile {
            device: Some(self.handshake),
            layers: self
                .records
                .iter()
                .zip(&self.leds)
                .map(|(recs, leds)| LayerSpec {
                    slots: recs
                        .iter()
                        .enumerate()
                        .map(|(i, r)| ((i + 1) as u8, ActionSpec::from_action(&Action::decode(r))))
                        .collect(),
                    leds: Some(LedSpec::from_config(leds)),
                    ..Default::default()
                })
                .collect(),
        }
    }

    /// One layer (0-based) as a standalone layer file.
    pub fn layer_spec(&self, layer: usize) -> Option<LayerSpec> {
        self.to_profile().layers.into_iter().nth(layer)
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ApplyReport {
    pub records_written: usize,
    pub led_layers_written: usize,
    /// Frames that would have been sent (dry run) or were sent.
    pub frames: Vec<Frame>,
}

impl ApplyReport {
    /// After a commit (any key change), the 514C:8850 stops updating its live LEDs,
    /// even for later LED-only writes, until it is unplugged. The settings are stored;
    /// they just show after a replug.
    pub fn needs_replug_for_leds(&self) -> bool {
        self.records_written > 0
    }
}

/// Message for users after a write that changed keys.
pub const REPLUG_HINT: &str =
    "Unplug and replug the keypad to see the lighting: after saving keys it stops updating its LEDs until it is reconnected.";

pub struct Keypad<T: Transport> {
    t: T,
    pub handshake: Handshake,
    dry_run: bool,
    /// Every mutating frame sent (or suppressed in dry-run mode).
    pub mutations: Vec<Frame>,
}

const HANDSHAKE_TIMEOUT_MS: i32 = 500;
const READ_TIMEOUT_MS: i32 = 1000;

impl<T: Transport> Keypad<T> {
    /// Perform the handshake. In `dry_run` mode, mutating frames are logged, not sent.
    pub fn connect(mut t: T, dry_run: bool) -> Result<Self> {
        drain(&mut t)?;
        t.write(&frame::handshake())?;
        let r = t.read(HANDSHAKE_TIMEOUT_MS)?.ok_or(Error::NoHandshake)?;
        let handshake = Handshake::parse(&r).ok_or(Error::NoHandshake)?;
        if handshake.keys == 0 || handshake.slot_count() as usize >= macropad_remapper_proto::SLOTS_PER_LAYER {
            return Err(Error::NoHandshake);
        }
        Ok(Keypad { t, handshake, dry_run, mutations: Vec::new() })
    }

    pub fn into_transport(self) -> T {
        self.t
    }

    fn send(&mut self, f: Frame) -> Result<()> {
        if frame::is_mutating(&f) {
            self.mutations.push(f);
            if self.dry_run {
                return Ok(());
            }
        }
        self.t.write(&f)
    }

    /// Records of one layer (0-based), slots 1..=slot_count.
    pub fn read_layer(&mut self, layer: u8) -> Result<Vec<Record>> {
        let legacy = self.handshake.is_legacy();
        let n = self.handshake.slot_count();
        drain(&mut self.t)?;
        let knobs = legacy.then_some(self.handshake.knobs);
        self.send(frame::read_layer(n, layer, knobs))?;
        let mut out = Vec::with_capacity(n as usize);
        for _ in 0..n {
            match self.t.read(READ_TIMEOUT_MS)? {
                Some(r) => out.push(frame::record_from_reply(&r, legacy)),
                None if out.is_empty() => return Err(Error::Timeout("layer read")),
                None => break,
            }
        }
        Ok(out)
    }

    pub fn read_leds(&mut self, layer: u8) -> Result<LedConfig> {
        drain(&mut self.t)?;
        self.send(frame::read_leds(layer))?;
        let r = self.t.read(READ_TIMEOUT_MS)?.ok_or(Error::Timeout("LED read"))?;
        Ok(LedConfig::parse(&r))
    }

    pub fn read_image(&mut self) -> Result<DeviceImage> {
        let mut records = Vec::new();
        let mut leds = Vec::new();
        for layer in 0..LAYERS as u8 {
            records.push(self.read_layer(layer)?);
            leds.push(self.read_leds(layer)?);
        }
        Ok(DeviceImage { handshake: self.handshake, records, leds })
    }

    /// Write only what differs between `profile` and the freshly read `image`, then commit.
    pub fn apply(&mut self, profile: &Profile, image: &DeviceImage) -> Result<ApplyReport> {
        profile.validate()?;
        if self.handshake.is_legacy() {
            return Err(Error::LegacyReadOnly(self.handshake.generation));
        }
        let start = self.mutations.len();
        // Plan everything first so an invalid entry never leaves a half-written keypad.
        let mut layer_records: Vec<Vec<Record>> = Vec::new();
        for (li, layer) in profile.layers.iter().enumerate() {
            let recs = &image.records[li];
            let mut changed = Vec::new();
            for (&slot, spec) in &layer.slots {
                let base = recs.get(slot as usize - 1).ok_or(Error::NoSuchSlot {
                    layer: li + 1,
                    slot,
                    available: recs.len(),
                })?;
                let new = spec.to_action()?.encode(base)?;
                if &new != base {
                    changed.push(new);
                }
            }
            layer_records.push(changed);
        }
        // LEDs: the new configuration of every layer (profile value, else what the keypad has).
        let mut new_leds = image.leds.clone();
        for (li, layer) in profile.layers.iter().enumerate() {
            if let Some(spec) = &layer.leds {
                new_leds[li] = spec.to_config()?;
            }
        }
        let leds_changed = new_leds != image.leds;

        // Then send: all changed key records → one commit (only if records were written)
        // → (if any LED changed) the LED block of *all three* layers in order 0,1,2,
        // with no commit after it.
        // Hardware-confirmed on a 514C:8850:
        // - the firmware only persists LED settings when it receives all three layers;
        // - a commit *after* the LED frames makes it restore its stored LED settings, so
        //   the new LEDs are lost. The vendor tool sends records → LEDs → commit and
        //   therefore loses LED changes made together with key changes; we commit first.
        let mut report = ApplyReport::default();
        for changed in &layer_records {
            for rec in changed {
                self.send(frame::write_record(rec))?;
            }
            report.records_written += changed.len();
        }
        if report.records_written > 0 {
            self.commit()?;
        }
        if leds_changed {
            for (layer, cfg) in new_leds.iter().enumerate() {
                self.send(frame::write_leds(layer as u8, cfg))?;
            }
            report.led_layers_written = new_leds.len();
        }
        report.frames = self.mutations[start..].to_vec();
        Ok(report)
    }

    fn commit(&mut self) -> Result<()> {
        self.send(frame::commit())?;
        if !self.dry_run {
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        Ok(())
    }

    /// Change the physical layout preset. Destructive; callers must confirm with the user.
    pub fn select_layout(&mut self, keys: u8, knobs: u8, flag: u8) -> Result<()> {
        self.send(frame::select_layout(keys, knobs, flag))?;
        if !self.dry_run {
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        Ok(())
    }
}

/// Discard stale input reports.
fn drain<T: Transport + ?Sized>(t: &mut T) -> Result<()> {
    for _ in 0..64 {
        if t.read(0)?.is_none() {
            break;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use macropad_remapper_proto::profile::Profile;

    fn sim() -> SimulatedKeypad {
        SimulatedKeypad::new(3, 1, 10)
    }

    #[test]
    fn handshake_is_first_frame() {
        let k = Keypad::connect(sim(), false).unwrap();
        assert_eq!(k.handshake.keys, 3);
        assert_eq!(k.handshake.slot_count(), 6);
        let t = k.into_transport();
        assert_eq!(&t.written[0][..4], &[0x03, 0xFB, 0xFB, 0xFB]);
    }

    #[test]
    fn no_handshake_reply_is_error() {
        let mut s = sim();
        s.handshake.keys = 0; // device answers with garbage
        assert!(matches!(Keypad::connect(s, false), Err(Error::NoHandshake)));
    }

    #[test]
    fn read_image_reads_all_layers() {
        let mut k = Keypad::connect(sim(), false).unwrap();
        let img = k.read_image().unwrap();
        assert_eq!(img.records.len(), 3);
        assert_eq!(img.records[2].len(), 6);
        assert_eq!(img.records[2][4][1], 5, "slot id preserved from reply");
        let t = k.into_transport();
        assert_eq!(&t.written[1][..5], &[0x03, 0xFA, 6, 0, 1]);
        assert_eq!(&t.written[2][..4], &[0x03, 0xFA, 0xB0, 0]);
    }

    #[test]
    fn apply_writes_only_diffs_then_commits() {
        let mut k = Keypad::connect(sim(), false).unwrap();
        let image = k.read_image().unwrap();
        let profile =
            Profile::from_yaml("layers:\n  - slots:\n      1: ctrl-c\n      2: none\n    leds: { mode: 0 }\n").unwrap();
        let rep = k.apply(&profile, &image).unwrap();
        assert_eq!(rep.records_written, 1, "slot 2 already empty, LEDs unchanged");
        assert_eq!(rep.led_layers_written, 0);
        assert_eq!(rep.frames.len(), 2);
        // header (slot 1, layer byte 1) copied from the device
        assert_eq!(&rep.frames[0][..13], &[0x03, 0xFD, 1, 1, 1, 0, 2, 0, 0, 0xF1, 0, 0, 0x06]);
        assert_eq!(&rep.frames[1][..4], &[0x03, 0xFD, 0xFE, 0xFF]);
        // and the simulated device now reads back the new mapping
        let after = k.read_image().unwrap();
        assert_eq!(Action::decode(&after.records[0][0]), Action::Keys(vec![0xF1, 0x06]));
        assert_eq!(k.into_transport().commits, 1);
    }

    #[test]
    fn leds_written_and_read_back() {
        let mut k = Keypad::connect(sim(), false).unwrap();
        let image = k.read_image().unwrap();
        let profile = Profile::from_yaml("layers:\n  - {}\n  - leds: { mode: 3, colors: ['#102030'] }\n").unwrap();
        let rep = k.apply(&profile, &image).unwrap();
        // All three LED layers in order and no commit: the firmware only persists LEDs
        // when it gets the full set.
        assert_eq!(rep.led_layers_written, 3);
        let heads: Vec<[u8; 4]> = rep.frames.iter().map(|f| [f[1], f[2], f[3], f[4]]).collect();
        assert_eq!(
            heads,
            vec![[0xFE, 0xB0, 0, 0], [0xFE, 0xB0, 1, 3], [0xFE, 0xB0, 2, 0]]
        );
        let leds = k.read_leds(1).unwrap();
        assert_eq!(leds.mode, 3);
        assert_eq!(leds.colors[0], [0x10, 0x20, 0x30]);
        // unchanged layers are re-sent with their current values
        assert_eq!(k.read_leds(0).unwrap(), image.leds[0]);
    }

    #[test]
    fn records_then_commit_then_leds() {
        let mut k = Keypad::connect(sim(), false).unwrap();
        let image = k.read_image().unwrap();
        let profile =
            Profile::from_yaml("layers:\n  - slots: { 1: a }\n  - slots: { 2: b }\n    leds: { mode: 1 }\n").unwrap();
        let rep = k.apply(&profile, &image).unwrap();
        let kinds: Vec<&str> = rep
            .frames
            .iter()
            .map(|f| match (f[1], f[2]) {
                (0xFD, 0xFE) => "commit",
                (0xFE, 0xB0) => "led",
                _ => "record",
            })
            .collect();
        // Commit before the LEDs: a commit after them makes the keypad drop the new LEDs.
        assert_eq!(kinds, ["record", "record", "commit", "led", "led", "led"]);
        assert_eq!(k.into_transport().commits, 1);
    }

    #[test]
    fn dry_run_sends_nothing_mutating() {
        let mut k = Keypad::connect(sim(), true).unwrap();
        let image = k.read_image().unwrap();
        let profile = Profile::from_yaml("layers:\n  - slots: { 1: a }\n    leds: { mode: 3 }\n").unwrap();
        let rep = k.apply(&profile, &image).unwrap();
        assert_eq!((rep.records_written, rep.led_layers_written), (1, 3));
        assert_eq!(rep.frames.len(), 5, "record + 3 LED layers + commit (records changed)");
        let t = k.into_transport();
        assert!(t.written.iter().all(|f| !frame::is_mutating(f)), "only reads reached the device");
        assert_eq!(t.commits, 0);
    }

    #[test]
    fn dump_then_apply_is_noop() {
        let mut k = Keypad::connect(sim(), false).unwrap();
        let image = k.read_image().unwrap();
        let yaml = image.to_profile().to_yaml();
        let profile = Profile::from_yaml(&yaml).unwrap();
        let rep = k.apply(&profile, &image).unwrap();
        assert_eq!(rep, ApplyReport::default());
    }

    #[test]
    fn unknown_slot_is_error() {
        let mut k = Keypad::connect(sim(), false).unwrap();
        let image = k.read_image().unwrap();
        let profile = Profile::from_yaml("layers:\n  - slots: { 20: a }\n").unwrap();
        assert!(matches!(k.apply(&profile, &image), Err(Error::NoSuchSlot { slot: 20, .. })));
    }

    #[test]
    fn invalid_slot_sends_nothing() {
        let mut k = Keypad::connect(sim(), false).unwrap();
        let image = k.read_image().unwrap();
        // slot 1 is fine, slot 20 doesn't exist: nothing at all may be written.
        let profile = Profile::from_yaml("layers:\n  - slots: { 1: a, 20: b }\n").unwrap();
        assert!(k.apply(&profile, &image).is_err());
        assert!(k.mutations.is_empty());
    }

    #[test]
    fn commit_only_for_changed_layers() {
        let mut k = Keypad::connect(sim(), false).unwrap();
        let image = k.read_image().unwrap();
        let profile = Profile::from_yaml("layers:\n  - slots: { 1: a }\n  - slots: { 1: none }\n").unwrap();
        let rep = k.apply(&profile, &image).unwrap();
        assert_eq!(rep.frames.len(), 2, "one record + one commit");
    }

    #[test]
    fn layer_files_from_disk_in_any_order() {
        let dir = std::env::temp_dir().join(format!("macropad-remapper-test-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("layers")).unwrap();
        std::fs::write(dir.join("layers/lr.yaml"), "name: Lightroom\nslots: { 1: p, 2: x }\n").unwrap();
        std::fs::write(dir.join("layers/np.yaml"), "name: NegPy\nslots: { 1: ctrl-z }\n").unwrap();
        std::fs::write(dir.join("a.yaml"), "layers:\n  - file: layers/np.yaml\n  - file: layers/lr.yaml\n").unwrap();
        std::fs::write(dir.join("b.yaml"), "layers:\n  - file: layers/lr.yaml\n  - file: layers/np.yaml\n").unwrap();

        let ProfileDoc::Full(a) = read_profile_file(&dir.join("a.yaml")).unwrap() else { panic!() };
        let ProfileDoc::Full(b) = read_profile_file(&dir.join("b.yaml")).unwrap() else { panic!() };
        assert_eq!(a.layers[0], b.layers[1]);
        assert_eq!(a.layers[1].name.as_deref(), Some("Lightroom"));
        assert!(matches!(read_profile_file(&dir.join("layers/lr.yaml")).unwrap(), ProfileDoc::Layer(_)));

        // Uploading both setups to the simulator puts the same keys on swapped layers.
        let mut k = Keypad::connect(sim(), false).unwrap();
        let image = k.read_image().unwrap();
        k.apply(&a, &image).unwrap();
        let img = k.read_image().unwrap();
        assert_eq!(Action::decode(&img.records[0][0]), Action::Keys(vec![0xF1, 0x1D])); // NegPy: ctrl-z
        assert_eq!(Action::decode(&img.records[1][0]), Action::Keys(vec![0x13])); // Lightroom: p
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn legacy_is_read_only() {
        let mut k = Keypad::connect(SimulatedKeypad::new(3, 1, 3), false).unwrap();
        let image = k.read_image().unwrap();
        let profile = Profile::from_yaml("layers:\n  - slots: { 1: a }\n").unwrap();
        assert!(matches!(k.apply(&profile, &image), Err(Error::LegacyReadOnly(3))));
    }
}
