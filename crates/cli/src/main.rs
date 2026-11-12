use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use macropad_remapper_device::{list_devices, open_first, read_profile_file, Keypad, SimulatedKeypad, Transport};
use macropad_remapper_proto::frame::{format_color, parse_color, Frame, LedConfig};
use macropad_remapper_proto::keys;
use macropad_remapper_proto::profile::{LayerSpec, LedSpec, Profile, ProfileDoc};

#[derive(Parser)]
#[command(name = "macropad-remapper", version, about = "Open-source configurator for MINI_KEYBOARD macropads (USB 1189/514C:88xx)")]
struct Cli {
    /// Use an in-memory simulated keypad instead of USB, e.g. `--simulate 12,2`.
    #[arg(long, global = true, value_name = "KEYS,KNOBS")]
    simulate: Option<String>,
    /// Print every frame sent to the device.
    #[arg(long, short, global = true)]
    verbose: bool,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// List connected keypad HID interfaces.
    List,
    /// Handshake and show the key/knob count and protocol generation.
    Info,
    /// Read the keypad's configuration into a YAML profile (or one layer into a layer file).
    Dump {
        /// Output file (stdout if omitted).
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Export only this layer (1..3) as a layer file that can be loaded into any layer.
        #[arg(long)]
        layer: Option<u8>,
        /// Name to store in an exported layer file, e.g. "Lightroom".
        #[arg(long, requires = "layer")]
        name: Option<String>,
    },
    /// Write a YAML profile or layer file to the keypad (only changed slots are written).
    Upload {
        /// A full profile, or a layer file (then --layer chooses where it goes).
        profile: PathBuf,
        /// Target layer (1..3) for a layer file.
        #[arg(long)]
        layer: Option<u8>,
        /// Show the frames that would be sent without changing the keypad.
        #[arg(long)]
        dry_run: bool,
        /// Skip the automatic backup dump taken before writing.
        #[arg(long)]
        no_backup: bool,
    },
    /// Set the LED mode and colours of one layer.
    Led {
        /// Layer 1..3.
        #[arg(long, default_value_t = 1)]
        layer: u8,
        /// Mode 0..5: 0 off, 1 steady, 2 light on press, 3 ripple, 4 wave,
        /// 5 gradient (built-in colours).
        #[arg(long)]
        mode: u8,
        /// Colours "#rrggbb", one per key (repeat the flag).
        #[arg(long = "color")]
        colors: Vec<String>,
        #[arg(long)]
        dry_run: bool,
    },
    /// Change the physical layout preset (keys/knobs). DANGEROUS: resets mappings.
    Layout {
        #[arg(long)]
        keys: u8,
        #[arg(long)]
        knobs: u8,
        #[arg(long, default_value_t = 0)]
        flag: u8,
        /// Required acknowledgement.
        #[arg(long)]
        i_know_what_im_doing: bool,
    },
    /// List every key, modifier and media name usable in profiles.
    Keys,
}

fn main() {
    if let Err(e) = run(Cli::parse()) {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

fn connect(cli: &Cli, dry_run: bool) -> Result<Keypad<Box<dyn Transport>>> {
    if let Some(spec) = &cli.simulate {
        let (k, n) = spec.split_once(',').unwrap_or((spec, "0"));
        let sim = SimulatedKeypad::new(k.trim().parse()?, n.trim().parse()?, 10);
        return Ok(Keypad::connect(Box::new(sim) as Box<dyn Transport>, dry_run)?);
    }
    open_first(dry_run).context("could not open keypad (on Linux, install the udev rule from packaging/)")
}

fn hex(f: &Frame) -> String {
    let end = f.iter().rposition(|&b| b != 0).map_or(1, |p| p + 1);
    f[..end].iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ")
}

fn print_frames(frames: &[Frame], dry_run: bool) {
    let verb = if dry_run { "would send" } else { "sent" };
    for f in frames {
        println!("  {verb}: {}", hex(f));
    }
}

fn backup(kp: &mut Keypad<Box<dyn Transport>>) -> Result<(PathBuf, macropad_remapper_device::DeviceImage)> {
    let image = kp.read_image()?;
    let ts = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let path = PathBuf::from(format!("macropad-remapper-backup-{ts}.yaml"));
    std::fs::write(&path, image.to_profile().to_yaml()).with_context(|| format!("writing {}", path.display()))?;
    Ok((path, image))
}

fn run(cli: Cli) -> Result<()> {
    match &cli.cmd {
        Cmd::List => {
            let devs = list_devices()?;
            if devs.is_empty() {
                println!("no supported keypad connected");
            }
            for d in devs {
                println!("{d}");
            }
        }
        Cmd::Info => {
            let kp = connect(&cli, false)?;
            let h = kp.handshake;
            println!("keys:       {}", h.keys);
            println!("knobs:      {}", h.knobs);
            println!("generation: {}{}", h.generation, if h.is_legacy() { " (legacy, read-only)" } else { "" });
            println!("slots:      1..={} (keys first, then 3 per knob)", h.slot_count());
        }
        Cmd::Dump { output, layer, name } => {
            let mut kp = connect(&cli, false)?;
            let image = kp.read_image()?;
            let yaml = match layer {
                None => image.to_profile().to_yaml(),
                Some(l) => {
                    let idx = layer_index(*l)?;
                    let mut spec = image.layer_spec(idx).context("layer not reported by the keypad")?;
                    spec.name = name.clone();
                    spec.to_yaml()
                }
            };
            match output {
                Some(p) => {
                    std::fs::write(p, yaml).with_context(|| format!("writing {}", p.display()))?;
                    println!("saved {}", p.display());
                }
                None => print!("{yaml}"),
            }
        }
        Cmd::Upload { profile, layer, dry_run, no_backup } => {
            let profile = load_profile(profile, *layer)?;
            let mut kp = connect(&cli, *dry_run)?;
            let image = if *dry_run || *no_backup {
                kp.read_image()?
            } else {
                let (path, image) = backup(&mut kp)?;
                println!("backup of current configuration: {}", path.display());
                image
            };
            let rep = kp.apply(&profile, &image)?;
            if cli.verbose || *dry_run {
                print_frames(&rep.frames, *dry_run);
            }
            println!(
                "{} {} key record(s) and {} LED layer(s){}",
                if *dry_run { "would write" } else { "wrote" },
                rep.records_written,
                rep.led_layers_written,
                if rep.frames.is_empty() { " — keypad already matches" } else { "" }
            );
        }
        Cmd::Led { layer, mode, colors, dry_run } => {
            if !(1..=3).contains(layer) {
                bail!("layer must be 1..3");
            }
            let mut cfg = LedConfig { mode: *mode, ..Default::default() };
            for (i, c) in colors.iter().take(16).enumerate() {
                cfg.colors[i] = parse_color(c)?;
            }
            let mut layers = vec![LayerSpec::default(); *layer as usize];
            layers[*layer as usize - 1].leds = Some(LedSpec::from_config(&cfg));
            let mut kp = connect(&cli, *dry_run)?;
            let image = kp.read_image()?;
            let rep = kp.apply(&Profile { device: None, layers }, &image)?;
            if cli.verbose || *dry_run {
                print_frames(&rep.frames, *dry_run);
            }
            println!("LED layer {layer}: mode {mode}, colours {}", cfg.colors.iter().take(colors.len()).map(|c| format_color(*c)).collect::<Vec<_>>().join(" "));
        }
        Cmd::Layout { keys, knobs, flag, i_know_what_im_doing } => {
            if !i_know_what_im_doing {
                bail!("changing the layout resets the keypad's mappings; re-run with --i-know-what-im-doing");
            }
            let mut kp = connect(&cli, false)?;
            let (path, _) = backup(&mut kp)?;
            println!("backup of current configuration: {}", path.display());
            kp.select_layout(*keys, *knobs, *flag)?;
            println!("layout set to {keys} keys / {knobs} knobs; unplug and replug the keypad, then run `macropad-remapper info`");
        }
        Cmd::Keys => {
            println!("Modifiers: {}", keys::MODIFIERS.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(" "));
            println!("Keys:      {}", keys::KEYS.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(" "));
            println!("Shifted:   {}", keys::SHIFTED.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(" "));
            println!("Media:     {}", keys::MEDIA.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(" "));
            println!("Mouse:     buttons left/right/middle, wheel -1/1, optional modifier");
            println!("Raw codes: 0xNN (key) or media 0xNNNN");
        }
    }
    Ok(())
}

fn layer_index(layer: u8) -> Result<usize> {
    if !(1..=3).contains(&layer) {
        bail!("layer must be 1..3");
    }
    Ok(layer as usize - 1)
}

/// Load a full profile, or a layer file placed into `layer`.
fn load_profile(path: &Path, layer: Option<u8>) -> Result<Profile> {
    match (read_profile_file(path)?, layer) {
        (ProfileDoc::Full(p), None) => Ok(p),
        (ProfileDoc::Full(_), Some(_)) => {
            bail!("{} is a full profile (it has `layers:`); --layer is only for layer files", path.display())
        }
        (ProfileDoc::Layer(_), None) => bail!(
            "{} is a layer file; choose where it goes with --layer 1, 2 or 3",
            path.display()
        ),
        (ProfileDoc::Layer(l), Some(n)) => {
            let name = l.name.clone();
            let p = Profile::for_layer(layer_index(n)?, l)?;
            println!("loading {} into layer {n}", name.as_deref().unwrap_or("layer file"));
            Ok(p)
        }
    }
}
