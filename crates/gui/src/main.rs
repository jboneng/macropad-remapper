//! macropad-remapper-gui: graphical configurator. All USB I/O runs on a worker thread.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod layout;
mod theme;
mod widgets;
mod worker;

use eframe::egui::{self, Color32, RichText};
use macropad_remapper_device::{read_profile_file, DeviceImage};
use macropad_remapper_proto::frame::LedConfig;
use macropad_remapper_proto::keys;
use macropad_remapper_proto::profile::{ActionSpec, LayerSpec, LedSpec, Profile, ProfileDoc};
use macropad_remapper_proto::record::{Action, MouseButtons, Step, MAX_STEPS};
use macropad_remapper_proto::{LAYERS, LED_COUNT};
use theme::*;
use widgets::{card, ghost, keycap, knob, pill, primary, status_chip, KnobPart};
use worker::{Job, Reply, Worker};

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1180.0, 780.0])
            .with_min_inner_size([900.0, 640.0])
            .with_icon(
                eframe::icon_data::from_png_bytes(include_bytes!("../../../packaging/icons/icon-256.png"))
                    .expect("embedded icon is a valid PNG"),
            )
            .with_title("Macropad Remapper"),
        ..Default::default()
    };
    // `--simulate KEYS,KNOBS` starts with a simulated keypad (handy for demos/screenshots).
    let simulate = std::env::args().skip_while(|a| a != "--simulate").nth(1).and_then(|s| {
        let (k, n) = s.split_once(',')?;
        Some((k.trim().parse().ok()?, n.trim().parse().ok()?))
    });
    eframe::run_native(
        "macropad-remapper",
        options,
        Box::new(move |cc| {
            theme::apply(&cc.egui_ctx);
            let mut app = App::default();
            if let Some((k, n)) = simulate {
                app.start_simulator(k, n);
            } else if std::env::args().any(|a| a == "--connect") {
                app.send(Job::Connect, "Connecting…");
            }
            Ok(Box::new(app))
        }),
    )
}

// ------------------------------------------------------------ labels --

fn key_label(code: u8) -> String {
    let name = keys::code_name(code);
    match name.as_str() {
        "ctrl" => "Ctrl".into(),
        "shift" => "Shift".into(),
        "alt" => "Alt".into(),
        "win" => "Win".into(),
        "rctrl" => "RCtrl".into(),
        "rshift" => "RShift".into(),
        "ralt" => "AltGr".into(),
        "rwin" => "RWin".into(),
        "space" => "Space".into(),
        "enter" => "Enter".into(),
        "esc" => "Esc".into(),
        "backspace" => "⌫".into(),
        "tab" => "Tab".into(),
        "up" => "↑".into(),
        "down" => "↓".into(),
        "left" => "←".into(),
        "right" => "→".into(),
        "minus" => "-".into(),
        "equal" => "=".into(),
        "lbracket" => "[".into(),
        "rbracket" => "]".into(),
        "backslash" => "\\".into(),
        "semicolon" => ";".into(),
        "quote" => "'".into(),
        "grave" => "`".into(),
        "comma" => ",".into(),
        "dot" => ".".into(),
        "slash" => "/".into(),
        "pageup" => "PgUp".into(),
        "pagedown" => "PgDn".into(),
        "printscreen" => "PrtSc".into(),
        n if n.starts_with("kp") => format!("Num {}", &n[2..]),
        n if n.len() == 1 => n.to_uppercase(),
        n if n.starts_with('f') && n[1..].parse::<u8>().is_ok() => n.to_uppercase(),
        n => {
            let mut c = n.chars();
            c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
        }
    }
}

const MEDIA_LABELS: &[(&str, &str)] = &[
    ("play", "Play / Pause"),
    ("stop", "Stop"),
    ("prev", "Previous"),
    ("next", "Next"),
    ("mute", "Mute"),
    ("volup", "Volume +"),
    ("voldown", "Volume −"),
    ("brightnessup", "Brightness +"),
    ("brightnessdown", "Brightness −"),
    ("mediaselect", "Media player"),
    ("mail", "Mail"),
    ("calculator", "Calculator"),
    ("mycomputer", "My computer"),
    ("wwwsearch", "Web search"),
    ("wwwhome", "Browser home"),
    ("wwwback", "Browser back"),
    ("wwwforward", "Browser forward"),
    ("wwwstop", "Browser stop"),
    ("wwwrefresh", "Refresh"),
    ("wwwfavorites", "Favourites"),
];

fn media_label(code: u16) -> String {
    let n = keys::media_name(code);
    MEDIA_LABELS.iter().find(|(k, _)| *k == n).map_or(n, |(_, l)| l.to_string())
}

fn action_label(a: &Action) -> String {
    match a {
        Action::None => String::new(),
        Action::Keys(c) => c.iter().map(|&c| key_label(c)).collect::<Vec<_>>().join("+"),
        Action::Media(m) => media_label(*m),
        Action::Mouse { buttons, wheel, modifier } => {
            let mut parts = Vec::new();
            if let Some(m) = modifier {
                parts.push(key_label(*m));
            }
            for (bit, n) in [(MouseButtons::LEFT, "L-click"), (MouseButtons::RIGHT, "R-click"), (MouseButtons::MIDDLE, "M-click")] {
                if buttons & bit != 0 {
                    parts.push(n.into());
                }
            }
            match wheel.signum() {
                1 => parts.push("Wheel ↑".into()),
                -1 => parts.push("Wheel ↓".into()),
                _ => {}
            }
            if parts.is_empty() { "Mouse".into() } else { parts.join("+") }
        }
        Action::Macro(s) => format!("Macro · {}", s.len()),
        Action::Raw(_) => "Custom".into(),
    }
}

/// Map an egui key to our key name (for "record shortcut").
fn egui_key_name(k: egui::Key) -> Option<&'static str> {
    use egui::Key::*;
    Some(match k {
        A => "a", B => "b", C => "c", D => "d", E => "e", F => "f", G => "g", H => "h", I => "i",
        J => "j", K => "k", L => "l", M => "m", N => "n", O => "o", P => "p", Q => "q", R => "r",
        S => "s", T => "t", U => "u", V => "v", W => "w", X => "x", Y => "y", Z => "z",
        Num0 => "0", Num1 => "1", Num2 => "2", Num3 => "3", Num4 => "4", Num5 => "5", Num6 => "6",
        Num7 => "7", Num8 => "8", Num9 => "9",
        F1 => "f1", F2 => "f2", F3 => "f3", F4 => "f4", F5 => "f5", F6 => "f6", F7 => "f7",
        F8 => "f8", F9 => "f9", F10 => "f10", F11 => "f11", F12 => "f12", F13 => "f13", F14 => "f14",
        F15 => "f15", F16 => "f16", F17 => "f17", F18 => "f18", F19 => "f19", F20 => "f20",
        Enter => "enter", Escape => "esc", Backspace => "backspace", Tab => "tab", Space => "space",
        Insert => "insert", Delete => "delete", Home => "home", End => "end", PageUp => "pageup",
        PageDown => "pagedown", ArrowUp => "up", ArrowDown => "down", ArrowLeft => "left",
        ArrowRight => "right", Minus => "minus", Equals => "equal", Comma => "comma", Period => "dot",
        Slash => "slash", Backslash => "backslash", Semicolon => "semicolon", Quote => "quote",
        Backtick => "grave", OpenBracket => "lbracket", CloseBracket => "rbracket",
        _ => return None,
    })
}

// --------------------------------------------------------------- app --

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    None,
    Keys,
    Media,
    Mouse,
    Macro,
    Raw,
}

impl Kind {
    fn of(a: &Action) -> Kind {
        match a {
            Action::None => Kind::None,
            Action::Keys(_) => Kind::Keys,
            Action::Media(_) => Kind::Media,
            Action::Mouse { .. } => Kind::Mouse,
            Action::Macro(_) => Kind::Macro,
            Action::Raw(_) => Kind::Raw,
        }
    }
}

struct App {
    worker: Worker,
    busy: bool,
    status: String,
    status_is_error: bool,
    simulated: bool,
    image: Option<DeviceImage>,
    /// Editable copy: `actions[layer][slot-1]`.
    actions: Vec<Vec<Action>>,
    leds: Vec<LedConfig>,
    layer: usize,
    slot: usize,
    chord_buf: String,
    chord_err: Option<String>,
    recording: bool,
    confirm_write: bool,
    /// Optional names per layer ("Lightroom"), from layer files; not stored on the keypad.
    layer_names: Vec<String>,
}

impl Default for App {
    fn default() -> Self {
        App {
            worker: Worker::spawn(),
            busy: false,
            status: "Not connected".into(),
            status_is_error: false,
            simulated: false,
            image: None,
            actions: Vec::new(),
            leds: Vec::new(),
            layer: 0,
            slot: 0,
            chord_buf: String::new(),
            chord_err: None,
            recording: false,
            confirm_write: false,
            layer_names: vec![String::new(); LAYERS],
        }
    }
}

impl App {
    fn send(&mut self, job: Job, status: &str) {
        self.busy = true;
        self.status = status.into();
        self.status_is_error = false;
        self.worker.send(job);
    }

    fn start_simulator(&mut self, keys: u8, knobs: u8) {
        self.simulated = true;
        self.send(Job::Simulate(keys, knobs), "Starting simulator…");
    }

    fn load_image(&mut self, img: DeviceImage) {
        self.actions = img.records.iter().map(|l| l.iter().map(Action::decode).collect()).collect();
        self.leds = img.leds.clone();
        self.image = Some(img);
        self.slot = self.slot.min(self.slot_count().saturating_sub(1));
        self.select(self.slot);
    }

    fn keys(&self) -> usize {
        self.image.as_ref().map_or(0, |i| i.handshake.keys as usize).min(self.slot_count())
    }

    fn slot_count(&self) -> usize {
        self.actions.first().map_or(0, Vec::len)
    }

    fn select(&mut self, slot: usize) {
        self.slot = slot;
        self.recording = false;
        self.chord_err = None;
        self.chord_buf = match self.actions.get(self.layer).and_then(|l| l.get(slot)) {
            Some(Action::Keys(c)) => keys::format_chord(c),
            _ => String::new(),
        };
    }

    /// One layer (0-based) as a standalone layer file.
    fn layer_spec(&self, li: usize) -> LayerSpec {
        let name = self.layer_names[li].trim();
        LayerSpec {
            name: (!name.is_empty()).then(|| name.to_string()),
            slots: self.actions[li].iter().enumerate().map(|(i, a)| ((i + 1) as u8, ActionSpec::from_action(a))).collect(),
            leds: Some(LedSpec::from_config(&self.leds[li])),
            ..Default::default()
        }
    }

    fn to_profile(&self) -> Profile {
        Profile {
            device: self.image.as_ref().map(|i| i.handshake),
            layers: (0..self.actions.len()).map(|li| self.layer_spec(li)).collect(),
        }
    }

    /// Apply a profile to the editor (all-or-nothing). Nothing is written to the keypad.
    fn merge_profile(&mut self, p: &Profile) -> Result<(), String> {
        p.validate().map_err(|e| e.to_string())?;
        if self.actions.is_empty() {
            return Err("Connect (or start the simulator) first so the key layout is known.".into());
        }
        let (mut actions, mut leds, mut names) = (self.actions.clone(), self.leds.clone(), self.layer_names.clone());
        for (li, layer) in p.layers.iter().enumerate() {
            for (&slot, spec) in &layer.slots {
                let a = spec.to_action().map_err(|e| e.to_string())?;
                *actions[li]
                    .get_mut(slot as usize - 1)
                    .ok_or_else(|| format!("layer {} slot {slot} does not exist on this keypad", li + 1))? = a;
            }
            if let Some(l) = &layer.leds {
                leds[li] = l.to_config().map_err(|e| e.to_string())?;
            }
            if let Some(n) = &layer.name {
                names[li] = n.clone();
            }
        }
        (self.actions, self.leds, self.layer_names) = (actions, leds, names);
        self.select(self.slot);
        Ok(())
    }

    /// Load a layer file into the currently selected layer.
    fn load_layer_file(&mut self) {
        let Some(path) = rfd::FileDialog::new().add_filter("YAML", &["yaml", "yml"]).pick_file() else { return };
        let target = self.layer;
        let res = match read_profile_file(&path).map_err(|e| e.to_string()) {
            Ok(ProfileDoc::Layer(mut l)) => {
                if l.name.is_none() {
                    l.name = path.file_stem().map(|s| s.to_string_lossy().into_owned());
                }
                Profile::for_layer(target, l).map_err(|e| e.to_string()).and_then(|p| self.merge_profile(&p))
            }
            Ok(ProfileDoc::Full(_)) => Err(format!(
                "{} is a full profile (all layers). Use Open in the toolbar for it, or save a single layer with \"Save layer…\".",
                path.display()
            )),
            Err(e) => Err(e),
        };
        (self.status, self.status_is_error) = match res {
            Ok(()) => (
                format!("Loaded {} into layer {} — press Write to apply it", path.display(), target + 1),
                false,
            ),
            Err(e) => (e, true),
        };
    }

    /// Save the selected layer as a layer file that can be loaded into any layer.
    fn save_layer_file(&mut self) {
        let li = self.layer;
        let stem = match self.layer_names[li].trim() {
            "" => format!("layer-{}", li + 1),
            n => n.to_lowercase().replace(char::is_whitespace, "-"),
        };
        let Some(path) =
            rfd::FileDialog::new().add_filter("YAML", &["yaml", "yml"]).set_file_name(format!("{stem}.yaml")).save_file()
        else {
            return;
        };
        if self.layer_names[li].trim().is_empty() {
            self.layer_names[li] = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        }
        let header = "# Layer file — load it into any layer with \"Load into layer…\" in Macropad Remapper,\n# or: macropad-remapper upload <this file> --layer N\n";
        (self.status, self.status_is_error) = match std::fs::write(&path, format!("{header}{}", self.layer_spec(li).to_yaml())) {
            Ok(()) => (format!("Saved layer {} to {}", li + 1, path.display()), false),
            Err(e) => (e.to_string(), true),
        };
    }

    /// Number of changed key slots + changed LED layers.
    fn pending_changes(&self) -> usize {
        let Some(img) = &self.image else { return 0 };
        let keys: usize = img
            .records
            .iter()
            .zip(&self.actions)
            .map(|(recs, acts)| recs.iter().zip(acts).filter(|(r, a)| a.encode(r).map_or(true, |n| &n != *r)).count())
            .sum();
        keys + img.leds.iter().zip(&self.leds).filter(|(a, b)| a != b).count()
    }

    fn slot_name(&self, i: usize) -> String {
        let keys = self.keys();
        if i < keys {
            format!("Key {}", i + 1)
        } else {
            let k = i - keys;
            format!("Knob {} · {}", k / 3 + 1, ["turn left", "press", "turn right"][k % 3])
        }
    }

    fn led_color(&self, key: usize) -> Option<Color32> {
        (key < LED_COUNT).then(|| {
            let [r, g, b] = self.leds[self.layer].colors[key];
            Color32::from_rgb(r, g, b)
        })
    }
}

impl eframe::App for App {
    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = root.ctx().clone();
        while let Some(reply) = self.worker.poll() {
            self.busy = false;
            match reply {
                Reply::Image(img, msg) => {
                    self.load_image(*img);
                    self.status = msg;
                    self.status_is_error = false;
                }
                Reply::Error(e) => {
                    self.status = e;
                    self.status_is_error = true;
                }
            }
        }
        if self.busy {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
        self.handle_recording(&ctx);

        egui::Panel::top("toolbar")
            .frame(egui::Frame::new().fill(BG).inner_margin(egui::Margin::symmetric(16, 10)))
            .show(root, |ui| self.toolbar(ui));
        egui::Panel::bottom("status")
            .frame(egui::Frame::new().fill(BG).inner_margin(egui::Margin::symmetric(16, 6)))
            .show(root, |ui| {
                ui.horizontal(|ui| {
                    if self.busy {
                        ui.spinner();
                    }
                    let color = if self.status_is_error { DANGER } else { MUTED };
                    ui.label(RichText::new(&self.status).small().color(color));
                });
            });

        if self.actions.is_empty() {
            egui::CentralPanel::default()
                .frame(egui::Frame::new().fill(BG))
                .show(root, |ui| self.empty_state(ui));
            return;
        }

        egui::Panel::right("inspector")
            .resizable(false)
            .exact_size(380.0)
            .frame(egui::Frame::new().fill(BG).inner_margin(egui::Margin { left: 0, right: 16, top: 8, bottom: 16 }))
            .show(root, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| self.inspector(ui));
            });
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BG).inner_margin(egui::Margin { left: 16, right: 16, top: 8, bottom: 16 }))
            .show(root, |ui| {
                egui::ScrollArea::both().show(ui, |ui| self.device_view(ui));
            });

        if self.confirm_write {
            self.confirm_dialog(&ctx);
        }
    }
}

impl App {
    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("⌨").size(22.0).color(ACCENT));
            ui.label(RichText::new("Macropad Remapper").size(18.0).strong().color(TEXT));
            ui.label(RichText::new(concat!("v", env!("CARGO_PKG_VERSION"))).small().color(MUTED));
            ui.add_space(16.0);
            let idle = !self.busy;
            let connected = self.image.is_some();
            if ghost(ui, idle, "🔌  Connect").clicked() {
                self.simulated = false;
                self.send(Job::Connect, "Connecting…");
            }
            if ghost(ui, idle && connected, "⟳  Reload").clicked() {
                self.send(Job::Read, "Reading from keypad…");
            }
            if ghost(ui, idle && connected, "📂  Open").clicked() {
                self.open_profile();
            }
            if ghost(ui, idle && connected, "💾  Save").clicked() {
                self.save_profile();
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let changes = self.pending_changes();
                let label = if changes == 0 { "Write to keypad".to_string() } else { format!("Write {changes} change(s)") };
                if primary(ui, idle && changes > 0, &label).clicked() {
                    self.confirm_write = true;
                }
                if let Some(img) = &self.image {
                    let h = img.handshake;
                    let (color, what) = if self.simulated {
                        (WARN, "Simulator".to_string())
                    } else if h.is_legacy() {
                        (WARN, "Keypad (read-only firmware)".to_string())
                    } else {
                        (OK, "Keypad connected".to_string())
                    };
                    status_chip(ui, color, &format!("{what} · {} keys · {} knobs", h.keys, h.knobs));
                }
            });
        });
    }

    fn empty_state(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() * 0.22);
            ui.label(RichText::new("⌨").size(64.0).color(ACCENT));
            ui.add_space(8.0);
            ui.label(RichText::new("Configure your macropad").size(26.0).strong().color(TEXT));
            ui.label(
                RichText::new("Plug in the keypad and connect. Nothing is changed until you press Write.").color(MUTED),
            );
            ui.add_space(20.0);
            ui.horizontal(|ui| {
                let w = 340.0;
                ui.add_space((ui.available_width() - w).max(0.0) / 2.0);
                if primary(ui, !self.busy, "🔌  Connect keypad").clicked() {
                    self.simulated = false;
                    self.send(Job::Connect, "Connecting…");
                }
                if ghost(ui, !self.busy, "Try the simulator").clicked() {
                    self.start_simulator(16, 3);
                }
            });
            ui.add_space(24.0);
            ui.label(
                RichText::new("Supports USB 1189 / 514C : 8830–8833, 8840, 8842, 8850, 8851").small().color(MUTED),
            );
        });
    }

    fn device_view(&mut self, ui: &mut egui::Ui) {
        // Layer selector
        ui.horizontal(|ui| {
            ui.label(RichText::new("LAYER").small().strong().color(MUTED));
            for l in 0..LAYERS {
                let label = match self.layer_names[l].trim() {
                    "" => format!("  {}  ", l + 1),
                    n => format!("{} · {n}", l + 1),
                };
                if pill(ui, self.layer == l, &label).clicked() {
                    self.layer = l;
                    self.select(self.slot);
                }
            }
        });
        ui.horizontal(|ui| {
            let li = self.layer;
            ui.label(RichText::new("Name").small().color(MUTED));
            ui.add(
                egui::TextEdit::singleline(&mut self.layer_names[li])
                    .hint_text("e.g. Lightroom")
                    .desired_width(150.0),
            );
            if ghost(ui, !self.busy, &format!("📂  Load into layer {}…", li + 1))
                .on_hover_text("Load a layer file (e.g. lightroom.yaml) into this layer. Press Write to apply.")
                .clicked()
            {
                self.load_layer_file();
            }
            if ghost(ui, !self.busy, &format!("💾  Save layer {}…", li + 1))
                .on_hover_text("Save just this layer as a file you can load into any layer, or share.")
                .clicked()
            {
                self.save_layer_file();
            }
        });
        ui.add_space(10.0);

        let keys = self.keys();
        let knobs = (self.slot_count() - keys) / 3;
        let lay = layout::pad_layout(keys, knobs);
        // Centre the plate horizontally in the available width (keys shrink to fit).
        let avail = ui.available_width();
        let geo = PlateGeometry::new(keys, knobs, lay, avail);
        let (row, _) = ui.allocate_exact_size(egui::vec2(avail.max(geo.size.x), geo.size.y), egui::Sense::hover());
        let plate = egui::Rect::from_min_size(
            egui::pos2(row.left() + ((row.width() - geo.size.x) / 2.0).max(0.0), row.top()),
            geo.size,
        );
        ui.painter().rect(plate, 22, DEVICE, egui::Stroke::new(1.0, BORDER), egui::StrokeKind::Inside);

        for k in 0..knobs {
            let r = geo.knob_rect(k).translate(plate.min.to_vec2());
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(r));
            self.draw_knob(&mut child, keys, k);
        }
        for i in 0..keys {
            let r = geo.key_rect(i).translate(plate.min.to_vec2());
            let text = action_label(&self.actions[self.layer][i]);
            let led = self.led_color(i);
            let selected = self.slot == i;
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(r));
            let clicked = keycap(&mut child, r.size(), &(i + 1).to_string(), &text, led, selected).clicked();
            if clicked {
                self.select(i);
            }
        }

        ui.add_space(8.0);
        ui.vertical_centered(|ui| {
            let note = if lay.known { "" } else { " Layout guessed from the key count; your pad may be arranged differently." };
            ui.label(
                RichText::new(format!(
                    "Click a key, or a knob's ↺ ● ↻ to edit what turning/pressing it does. Knob order not yet verified on hardware.{note}"
                ))
                .small()
                .color(MUTED),
            );
        });
    }

    /// One knob widget; clicking a part selects its slot.
    fn draw_knob(&mut self, ui: &mut egui::Ui, keys: usize, k: usize) {
        let base = keys + k * 3;
        let selected = (self.slot >= base && self.slot < base + 3)
            .then(|| [KnobPart::Left, KnobPart::Press, KnobPart::Right][self.slot - base]);
        let short = |a: &Action| {
            let s = action_label(a);
            if s.chars().count() > 9 { s.chars().take(8).collect::<String>() + "…" } else { s }
        };
        let acts = &self.actions[self.layer];
        let texts = [short(&acts[base]), short(&acts[base + 1]), short(&acts[base + 2])];
        let texts = [texts[0].as_str(), texts[1].as_str(), texts[2].as_str()];
        if let Some(part) = knob(ui, &format!("KNOB {}", k + 1), selected, texts) {
            let off = match part {
                KnobPart::Left => 0,
                KnobPart::Press => 1,
                KnobPart::Right => 2,
            };
            self.select(base + off);
        }
    }
}

/// Exact positions (relative to the plate's top-left) of every key and knob.
struct PlateGeometry {
    size: egui::Vec2,
    cap: egui::Vec2,
    grid_origin: egui::Pos2,
    knob_origin: egui::Pos2,
    cols: usize,
    knobs: usize,
    knob_side: layout::KnobSide,
}

const PLATE_MARGIN: f32 = 28.0;
const KEY_GAP: f32 = 14.0;
const KNOB_GAP: f32 = 12.0;
const KEY_HEIGHT: f32 = 84.0;

impl PlateGeometry {
    fn new(keys: usize, knobs: usize, lay: layout::PadLayout, max_width: f32) -> Self {
        use layout::KnobSide;
        let cols = lay.cols.max(1);
        let rows = keys.div_ceil(cols).max(1);
        let (kw, kh) = (widgets::KNOB_WIDTH, widgets::KNOB_HEIGHT);
        let span = |n: usize, w: f32, gap: f32| n as f32 * w + n.saturating_sub(1) as f32 * gap;

        let (cap_w, knob_w, knob_h) = match (lay.knob_side, knobs) {
            (_, 0) => (120.0, 0.0, 0.0),
            (KnobSide::Top, n) => {
                let row_w = span(n, kw, KNOB_GAP);
                // Keys stretch so the grid roughly matches the knob row.
                (((row_w - (cols as f32 - 1.0) * KEY_GAP) / cols as f32).clamp(104.0, 150.0), row_w, kh)
            }
            (KnobSide::Right, n) => {
                let kc = layout::right_cluster_cols(n);
                (112.0, span(kc, kw, KNOB_GAP), span(n.div_ceil(kc), kh, 8.0))
            }
        };
        // Shrink keys (not below 72 px) when the plate would be wider than the panel.
        let beside = if lay.knob_side == KnobSide::Right && knobs > 0 { 24.0 + knob_w } else { 0.0 };
        let fit_w = (max_width - 2.0 * PLATE_MARGIN - beside - (cols as f32 - 1.0) * KEY_GAP) / cols as f32;
        let cap_w = cap_w.min(fit_w).max(72.0);
        let cap = egui::vec2(cap_w, KEY_HEIGHT);
        let grid = egui::vec2(span(cols, cap.x, KEY_GAP), span(rows, cap.y, KEY_GAP));
        let m = PLATE_MARGIN;

        let (inner, grid_origin, knob_origin) = match (lay.knob_side, knobs) {
            (_, 0) => (grid, egui::pos2(m, m), egui::pos2(m, m)),
            (KnobSide::Top, _) => {
                let w = grid.x.max(knob_w);
                let inner = egui::vec2(w, knob_h + 18.0 + grid.y);
                (
                    inner,
                    egui::pos2(m + (w - grid.x) / 2.0, m + knob_h + 18.0),
                    egui::pos2(m + (w - knob_w) / 2.0, m),
                )
            }
            (KnobSide::Right, _) => {
                let h = grid.y.max(knob_h);
                let inner = egui::vec2(grid.x + 24.0 + knob_w, h);
                (
                    inner,
                    egui::pos2(m, m + (h - grid.y) / 2.0),
                    egui::pos2(m + grid.x + 24.0, m + (h - knob_h) / 2.0),
                )
            }
        };
        PlateGeometry {
            size: inner + egui::vec2(2.0 * m, 2.0 * m),
            cap,
            grid_origin,
            knob_origin,
            cols,
            knobs,
            knob_side: lay.knob_side,
        }
    }

    fn key_rect(&self, i: usize) -> egui::Rect {
        let (r, c) = (i / self.cols, i % self.cols);
        let min = self.grid_origin + egui::vec2(c as f32 * (self.cap.x + KEY_GAP), r as f32 * (self.cap.y + KEY_GAP));
        egui::Rect::from_min_size(min, self.cap)
    }

    fn knob_rect(&self, k: usize) -> egui::Rect {
        let (kw, kh) = (widgets::KNOB_WIDTH, widgets::KNOB_HEIGHT);
        let min = match self.knob_side {
            layout::KnobSide::Top => self.knob_origin + egui::vec2(k as f32 * (kw + KNOB_GAP), 0.0),
            layout::KnobSide::Right => {
                let kc = layout::right_cluster_cols(self.knobs);
                let (r, c) = (k / kc, k % kc);
                // A last row with fewer knobs is centred under the full rows.
                let in_row = (self.knobs - r * kc).min(kc);
                let row_offset = (kc - in_row) as f32 * (kw + KNOB_GAP) / 2.0;
                self.knob_origin + egui::vec2(row_offset + c as f32 * (kw + KNOB_GAP), r as f32 * (kh + 8.0))
            }
        };
        egui::Rect::from_min_size(min, egui::vec2(kw, kh))
    }
}

impl App {
    fn inspector(&mut self, ui: &mut egui::Ui) {
        let (layer, slot) = (self.layer, self.slot);
        if slot >= self.slot_count() {
            return;
        }
        let title = self.slot_name(slot);
        card(ui, "", |ui| {
            ui.label(RichText::new(&title).size(20.0).strong().color(TEXT));
            ui.label(RichText::new(format!("Layer {}", layer + 1)).color(MUTED));
        });
        ui.add_space(10.0);

        card(ui, "Action", |ui| {
            let current = Kind::of(&self.actions[layer][slot]);
            ui.horizontal_wrapped(|ui| {
                for (k, label) in [
                    (Kind::None, "None"),
                    (Kind::Keys, "Shortcut"),
                    (Kind::Media, "Media"),
                    (Kind::Mouse, "Mouse"),
                    (Kind::Macro, "Macro"),
                ] {
                    if pill(ui, current == k, label).clicked() && current != k {
                        self.actions[layer][slot] = match k {
                            Kind::None | Kind::Raw => Action::None,
                            Kind::Keys => Action::Keys(Vec::new()),
                            Kind::Media => Action::Media(0xE9),
                            Kind::Mouse => Action::Mouse { buttons: MouseButtons::LEFT, wheel: 0, modifier: None },
                            Kind::Macro => Action::Macro(vec![Step { code: 0x04, delay_ms: 50 }]),
                        };
                        self.select(slot);
                        if k == Kind::Keys {
                            self.recording = true;
                        }
                    }
                }
            });
            ui.add_space(8.0);
            self.action_editor(ui);
        });
        ui.add_space(10.0);

        card(ui, "Lighting", |ui| self.lighting_editor(ui));
    }

    fn action_editor(&mut self, ui: &mut egui::Ui) {
        let (layer, slot) = (self.layer, self.slot);
        let recording = self.recording;
        match &mut self.actions[layer][slot] {
            Action::None => {
                ui.label(RichText::new("This key does nothing.").color(MUTED));
            }
            Action::Keys(_) => {
                let rec_text = if recording { "●  Press the shortcut now…" } else { "●  Record shortcut" };
                let btn = egui::Button::new(RichText::new(rec_text).color(if recording { Color32::WHITE } else { TEXT }))
                    .fill(if recording { DANGER.gamma_multiply(0.7) } else { CARD_HI })
                    .corner_radius(8)
                    .min_size(egui::vec2(ui.available_width(), 36.0));
                if ui.add(btn).clicked() {
                    self.recording = !recording;
                }
                ui.add_space(4.0);
                ui.label(RichText::new("or type it").small().color(MUTED));
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut self.chord_buf)
                        .hint_text("e.g. ctrl-shift-esc")
                        .desired_width(f32::INFINITY),
                );
                if resp.changed() {
                    self.apply_chord_buf();
                }
                if let Some(e) = &self.chord_err {
                    ui.label(RichText::new(e).small().color(DANGER));
                }
                ui.collapsing(RichText::new("Key names").small().color(MUTED), |ui| {
                    ui.label(RichText::new("Modifiers: ctrl shift alt win rctrl rshift ralt rwin").small());
                    ui.label(RichText::new(keys::KEYS.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(" ")).small());
                    ui.label(RichText::new(keys::SHIFTED.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(" ")).small());
                });
            }
            Action::Media(code) => {
                ui.horizontal_wrapped(|ui| {
                    for (name, label) in MEDIA_LABELS {
                        let c = keys::media_code(name).expect("table");
                        if pill(ui, *code == c, label).clicked() {
                            *code = c;
                        }
                    }
                });
            }
            Action::Mouse { buttons, wheel, modifier } => {
                ui.label(RichText::new("Buttons").small().color(MUTED));
                ui.horizontal(|ui| {
                    for (name, bit) in [("Left", MouseButtons::LEFT), ("Middle", MouseButtons::MIDDLE), ("Right", MouseButtons::RIGHT)] {
                        if pill(ui, *buttons & bit != 0, name).clicked() {
                            *buttons ^= bit;
                        }
                    }
                });
                ui.label(RichText::new("Scroll wheel").small().color(MUTED));
                ui.horizontal(|ui| {
                    for (v, name) in [(0i8, "None"), (1, "Up"), (-1, "Down")] {
                        if pill(ui, *wheel == v, name).clicked() {
                            *wheel = v;
                        }
                    }
                });
                ui.label(RichText::new("Hold modifier").small().color(MUTED));
                ui.horizontal(|ui| {
                    if pill(ui, modifier.is_none(), "None").clicked() {
                        *modifier = None;
                    }
                    for (code, name) in [(0xF1u8, "Ctrl"), (0xF2, "Shift"), (0xF3, "Alt")] {
                        if pill(ui, *modifier == Some(code), name).clicked() {
                            *modifier = Some(code);
                        }
                    }
                });
                ui.label(RichText::new("Tip: Ctrl + wheel = zoom in most apps.").small().color(MUTED));
            }
            Action::Macro(steps) => {
                ui.label(RichText::new("Keys are tapped in order, waiting the delay before each.").small().color(MUTED));
                let mut remove = None;
                for (i, s) in steps.iter_mut().enumerate() {
                    egui::Frame::new().fill(CARD_HI).corner_radius(8).inner_margin(6).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(format!("{:>2}", i + 1)).color(MUTED).monospace());
                            ui.add(egui::DragValue::new(&mut s.delay_ms).range(0..=60000).suffix(" ms").speed(5));
                            egui::ComboBox::from_id_salt(("step", i))
                                .width(110.0)
                                .selected_text(key_label(s.code))
                                .show_ui(ui, |ui| {
                                    for (_, c) in keys::MODIFIERS.iter().chain(keys::KEYS.iter()) {
                                        ui.selectable_value(&mut s.code, *c, key_label(*c));
                                    }
                                });
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.add(egui::Button::new(RichText::new("✕").color(MUTED)).frame(false)).clicked() {
                                    remove = Some(i);
                                }
                            });
                        });
                    });
                }
                if let Some(i) = remove {
                    steps.remove(i);
                }
                ui.add_space(4.0);
                ui.add_enabled_ui(steps.len() < MAX_STEPS, |ui| {
                    if pill(ui, false, &format!("+ Add step  ({}/{MAX_STEPS})", steps.len())).clicked() {
                        steps.push(Step { code: 0x04, delay_ms: 50 });
                    }
                });
            }
            Action::Raw(rec) => {
                ui.label(RichText::new("This key uses a feature macropad-remapper doesn't understand yet. It is kept exactly as is.").color(MUTED));
                let hex = rec.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ");
                ui.label(RichText::new(hex).monospace().small().color(MUTED));
            }
        }
    }

    fn apply_chord_buf(&mut self) {
        let (layer, slot) = (self.layer, self.slot);
        match keys::parse_chord(&self.chord_buf) {
            Ok(c) if c.len() <= MAX_STEPS => {
                self.actions[layer][slot] = Action::Keys(c);
                self.chord_err = None;
            }
            Ok(_) => self.chord_err = Some(format!("At most {MAX_STEPS} keys")),
            Err(e) => self.chord_err = Some(e.to_string()),
        }
    }

    /// While recording, capture the next key press (with modifiers) as the shortcut.
    fn handle_recording(&mut self, ctx: &egui::Context) {
        if !self.recording || !matches!(self.actions.get(self.layer).and_then(|l| l.get(self.slot)), Some(Action::Keys(_))) {
            return;
        }
        let events = ctx.input(|i| i.events.clone());
        for e in events {
            if let egui::Event::Key { key, pressed: true, modifiers, .. } = e {
                if let Some(name) = egui_key_name(key) {
                    let mut parts = Vec::new();
                    if modifiers.ctrl {
                        parts.push("ctrl");
                    }
                    if modifiers.shift {
                        parts.push("shift");
                    }
                    if modifiers.alt {
                        parts.push("alt");
                    }
                    if modifiers.mac_cmd {
                        parts.push("win");
                    }
                    parts.push(name);
                    self.chord_buf = parts.join("-");
                    self.apply_chord_buf();
                    self.recording = false;
                    ctx.input_mut(|i| i.events.clear());
                    return;
                }
            }
        }
    }

    fn lighting_editor(&mut self, ui: &mut egui::Ui) {
        let layer = self.layer;
        ui.label(RichText::new("Effect (whole layer)").small().color(MUTED));
        ui.horizontal_wrapped(|ui| {
            for m in 0..=5u8 {
                if pill(ui, self.leds[layer].mode == m, &format!("Mode {m}")).clicked() {
                    self.leds[layer].mode = m;
                }
            }
        });
        ui.add_space(6.0);
        let keys = self.keys().min(LED_COUNT);
        if self.slot < keys {
            let i = self.slot;
            ui.label(RichText::new(format!("Key {} colour", i + 1)).small().color(MUTED));
            ui.horizontal(|ui| {
                ui.spacing_mut().interact_size = egui::vec2(56.0, 30.0);
                ui.color_edit_button_srgb(&mut self.leds[layer].colors[i]);
                for c in [
                    [0xff, 0x00, 0x00],
                    [0xff, 0x80, 0x00],
                    [0xff, 0xff, 0x00],
                    [0x00, 0xff, 0x00],
                    [0x00, 0xff, 0xff],
                    [0x00, 0x00, 0xff],
                    [0x80, 0x00, 0xff],
                    [0xff, 0xff, 0xff],
                ] {
                    let (rect, resp) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), egui::Sense::click());
                    ui.painter().circle_filled(rect.center(), 9.0, Color32::from_rgb(c[0], c[1], c[2]));
                    if resp.hovered() {
                        ui.painter().circle_stroke(rect.center(), 10.5, egui::Stroke::new(1.5, TEXT));
                    }
                    if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                        self.leds[layer].colors[i] = c;
                    }
                }
            });
            if pill(ui, false, "Use this colour for every key").clicked() {
                let c = self.leds[layer].colors[i];
                for k in 0..keys {
                    self.leds[layer].colors[k] = c;
                }
            }
        } else {
            ui.label(RichText::new("Select a key to change its colour.").small().color(MUTED));
        }
        ui.label(RichText::new("Mode names are not decoded yet — try them and see.").small().color(MUTED));
    }

    fn confirm_dialog(&mut self, ctx: &egui::Context) {
        let changes = self.pending_changes();
        egui::Modal::new(egui::Id::unique("confirm_write")).show(ctx, |ui| {
            ui.set_width(380.0);
            ui.label(RichText::new("Write to keypad?").size(18.0).strong());
            ui.add_space(4.0);
            ui.label(RichText::new(format!(
                "{changes} change(s) will be written. The current keypad configuration is backed up to your home folder first."
            ))
            .color(MUTED));
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if primary(ui, true, "Write").clicked() {
                    self.confirm_write = false;
                    let p = self.to_profile();
                    self.send(Job::Apply(Box::new(p)), "Backing up and writing…");
                }
                if ghost(ui, true, "Cancel").clicked() {
                    self.confirm_write = false;
                }
            });
        });
    }

    fn open_profile(&mut self) {
        if let Some(path) = rfd::FileDialog::new().add_filter("YAML", &["yaml", "yml"]).pick_file() {
            // A full profile (incl. `file:` includes) fills all layers; a layer file goes
            // into the currently selected layer.
            let target = self.layer;
            let res = match read_profile_file(&path).map_err(|e| e.to_string()) {
                Ok(ProfileDoc::Full(p)) => self.merge_profile(&p).map(|()| String::new()),
                Ok(ProfileDoc::Layer(mut l)) => {
                    if l.name.is_none() {
                        l.name = path.file_stem().map(|s| s.to_string_lossy().into_owned());
                    }
                    Profile::for_layer(target, l)
                        .map_err(|e| e.to_string())
                        .and_then(|p| self.merge_profile(&p))
                        .map(|()| format!(" into layer {}", target + 1))
                }
                Err(e) => Err(e),
            };
            (self.status, self.status_is_error) = match res {
                Ok(into) => (format!("Loaded {}{into} — press Write to apply it", path.display()), false),
                Err(e) => (e, true),
            };
        }
    }

    fn save_profile(&mut self) {
        if let Some(path) =
            rfd::FileDialog::new().add_filter("YAML", &["yaml", "yml"]).set_file_name("macropad-remapper-profile.yaml").save_file()
        {
            (self.status, self.status_is_error) = match std::fs::write(&path, self.to_profile().to_yaml()) {
                Ok(()) => (format!("Saved {}", path.display()), false),
                Err(e) => (e.to_string(), true),
            };
        }
    }
}
