//! Background thread that owns the keypad connection.

use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{SystemTime, UNIX_EPOCH};

use macropad_remapper_device::{open_first, DeviceImage, Keypad, SimulatedKeypad, Transport};
use macropad_remapper_proto::profile::Profile;

pub enum Job {
    Connect,
    Simulate(u8, u8),
    Read,
    /// Write the profile (the worker re-reads and backs up the keypad first).
    Apply(Box<Profile>),
}

pub enum Reply {
    Image(Box<DeviceImage>, String),
    Error(String),
}

pub struct Worker {
    tx: Sender<Job>,
    rx: Receiver<Reply>,
}

impl Worker {
    pub fn spawn() -> Self {
        let (tx, job_rx) = channel::<Job>();
        let (reply_tx, rx) = channel::<Reply>();
        std::thread::spawn(move || run(job_rx, reply_tx));
        Worker { tx, rx }
    }
    pub fn send(&self, job: Job) {
        let _ = self.tx.send(job);
    }
    pub fn poll(&self) -> Option<Reply> {
        self.rx.try_recv().ok()
    }
}

fn backup_path() -> PathBuf {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from);
    let ts = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    home.unwrap_or_default().join(format!("macropad-remapper-backup-{ts}.yaml"))
}

fn run(jobs: Receiver<Job>, replies: Sender<Reply>) {
    let mut kp: Option<Keypad<Box<dyn Transport>>> = None;
    for job in jobs {
        let reply = match handle(job, &mut kp) {
            Ok(r) => r,
            Err(e) => Reply::Error(e),
        };
        if replies.send(reply).is_err() {
            break;
        }
    }
}

fn handle(job: Job, kp: &mut Option<Keypad<Box<dyn Transport>>>) -> Result<Reply, String> {
    let s = |e: macropad_remapper_device::Error| e.to_string();
    match job {
        Job::Connect => {
            let mut k = open_first(false).map_err(s)?;
            let img = k.read_image().map_err(s)?;
            let h = k.handshake;
            *kp = Some(k);
            let note = if h.is_legacy() { " (legacy firmware: read-only)" } else { "" };
            Ok(Reply::Image(Box::new(img), format!("Connected: {} keys, {} knobs{note}", h.keys, h.knobs)))
        }
        Job::Simulate(keys, knobs) => {
            let sim = SimulatedKeypad::new(keys, knobs, 10);
            let mut k = Keypad::connect(Box::new(sim) as Box<dyn Transport>, false).map_err(s)?;
            let img = k.read_image().map_err(s)?;
            *kp = Some(k);
            Ok(Reply::Image(Box::new(img), "Simulated keypad (nothing is sent to real hardware)".into()))
        }
        Job::Read => {
            let k = kp.as_mut().ok_or("not connected")?;
            let img = k.read_image().map_err(s)?;
            Ok(Reply::Image(Box::new(img), "Read configuration from keypad".into()))
        }
        Job::Apply(profile) => {
            let k = kp.as_mut().ok_or("not connected")?;
            // Re-read so the diff is against what is really on the keypad, and back it up.
            let before = k.read_image().map_err(s)?;
            let path = backup_path();
            std::fs::write(&path, before.to_profile().to_yaml()).map_err(|e| format!("backup failed: {e}"))?;
            let rep = k.apply(&profile, &before).map_err(s)?;
            let after = k.read_image().map_err(s)?;
            Ok(Reply::Image(
                Box::new(after),
                format!(
                    "Wrote {} key(s) and {} LED layer(s). Backup: {}{}",
                    rep.records_written,
                    rep.led_layers_written,
                    path.display(),
                    if rep.needs_replug_for_leds() {
                        format!("\n{}", macropad_remapper_device::REPLUG_HINT)
                    } else {
                        String::new()
                    }
                ),
            ))
        }
    }
}
