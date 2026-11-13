//! Self-update: checks the latest GitHub release, downloads the file that matches how
//! this copy was installed, verifies it against `SHA256SUMS.txt` and installs it.
//!
//! The asset names produced by `.github/workflows/release.yml` are part of the contract
//! here (see [`InstallKind::asset_suffix`]).
//!
//! Testing: `MACROPAD_FAKE_VERSION=0.0.1` makes the app believe it is that version, so
//! the current release is offered as an update.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{channel, Receiver, Sender};

use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const REPO: &str = "jboneng/macropad-remapper";
const SUMS_NAME: &str = "SHA256SUMS.txt";
const GUI_BIN: &str = if cfg!(windows) { "macropad-remapper-gui.exe" } else { "macropad-remapper-gui" };
const CLI_BIN: &str = if cfg!(windows) { "macropad-remapper.exe" } else { "macropad-remapper" };

// ------------------------------------------------------------ settings --

/// Small settings file in the user's config folder.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Settings {
    pub check_updates: bool,
    pub skipped_version: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { check_updates: true, skipped_version: None }
    }
}

fn config_dir() -> Option<PathBuf> {
    let env = |k| std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    let base = if cfg!(windows) {
        env("APPDATA")
    } else if cfg!(target_os = "macos") {
        env("HOME").map(|h| h.join("Library/Application Support"))
    } else {
        env("XDG_CONFIG_HOME").or_else(|| env("HOME").map(|h| h.join(".config")))
    };
    base.map(|b| b.join("macropad-remapper"))
}

impl Settings {
    pub fn load() -> Self {
        config_dir()
            .and_then(|d| std::fs::read_to_string(d.join("settings.yaml")).ok())
            .and_then(|s| serde_yaml::from_str(&s).ok())
            .unwrap_or_default()
    }
    pub fn save(&self) {
        if let Some(d) = config_dir() {
            let _ = std::fs::create_dir_all(&d);
            if let Ok(s) = serde_yaml::to_string(self) {
                let _ = std::fs::write(d.join("settings.yaml"), s);
            }
        }
    }
}

// ------------------------------------------------------------ releases --

#[derive(Deserialize, Clone, Debug)]
pub struct Asset {
    pub name: String,
    pub browser_download_url: String,
    #[serde(default)]
    pub size: u64,
}

#[derive(Deserialize, Clone, Debug)]
struct ApiRelease {
    tag_name: String,
    #[serde(default)]
    body: Option<String>,
    html_url: String,
    #[serde(default)]
    assets: Vec<Asset>,
}

#[derive(Clone, Debug)]
pub struct Release {
    pub version: Version,
    pub notes: String,
    pub page: String,
    pub assets: Vec<Asset>,
}

pub fn current_version() -> Version {
    std::env::var("MACROPAD_FAKE_VERSION")
        .ok()
        .and_then(|v| Version::parse(v.trim_start_matches('v')).ok())
        .unwrap_or_else(|| Version::parse(env!("CARGO_PKG_VERSION")).expect("crate version is semver"))
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(600)))
        .timeout_connect(Some(std::time::Duration::from_secs(10)))
        .user_agent(concat!("macropad-remapper/", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

/// The newest published release, if it is newer than this build.
fn check() -> Result<Option<Release>, String> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let mut resp = agent()
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| format!("update check failed: {e}"))?;
    let r: ApiRelease = resp.body_mut().read_json().map_err(|e| format!("update check failed: {e}"))?;
    let version = Version::parse(r.tag_name.trim_start_matches('v')).map_err(|e| format!("bad release tag {}: {e}", r.tag_name))?;
    if version <= current_version() {
        return Ok(None);
    }
    Ok(Some(Release { version, notes: r.body.unwrap_or_default(), page: r.html_url, assets: r.assets }))
}

// ------------------------------------------------------------ install kind --

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InstallKind {
    /// Installed with the Inno Setup installer (has an uninstaller next to the exe).
    WindowsInstaller,
    /// A portable zip on Windows or Linux; `dir` holds the executables.
    Portable { dir: PathBuf },
    /// A `.app` bundle on macOS.
    MacApp { app: PathBuf },
    LinuxDeb,
    LinuxRpm,
    /// Development build or unsupported platform: only the release page is offered.
    Unknown,
}

impl InstallKind {
    /// End of the release asset name for this install kind.
    pub fn asset_suffix(&self) -> Option<&'static str> {
        let x64 = cfg!(target_arch = "x86_64");
        match self {
            InstallKind::WindowsInstaller if x64 => Some("-windows-x64-setup.exe"),
            InstallKind::Portable { .. } if x64 && cfg!(windows) => Some("-windows-x64-portable.zip"),
            InstallKind::Portable { .. } if x64 && cfg!(target_os = "linux") => Some("-linux-x86_64-portable.zip"),
            InstallKind::MacApp { .. } => Some("-macos-universal-portable.zip"),
            InstallKind::LinuxDeb if x64 => Some("-linux-amd64.deb"),
            InstallKind::LinuxRpm if x64 => Some("-linux-x86_64.rpm"),
            _ => None,
        }
    }

    pub fn describe(&self) -> &'static str {
        match self {
            InstallKind::WindowsInstaller => "Windows installer",
            InstallKind::Portable { .. } => "portable version",
            InstallKind::MacApp { .. } => "macOS app",
            InstallKind::LinuxDeb => "Debian/Ubuntu package",
            InstallKind::LinuxRpm => "RPM package",
            InstallKind::Unknown => "development build",
        }
    }
}

fn is_cargo_build(dir: &Path) -> bool {
    dir.ancestors().any(|a| a.file_name().is_some_and(|n| n == "target"))
}

fn succeeds(cmd: &str, args: &[&std::ffi::OsStr]) -> bool {
    Command::new(cmd).args(args).output().is_ok_and(|o| o.status.success())
}

pub fn detect_install() -> InstallKind {
    let Ok(exe) = std::env::current_exe().and_then(|p| p.canonicalize()) else { return InstallKind::Unknown };
    let Some(dir) = exe.parent().map(Path::to_path_buf) else { return InstallKind::Unknown };
    if is_cargo_build(&dir) {
        return InstallKind::Unknown;
    }
    if cfg!(windows) {
        if dir.join("unins000.exe").exists() {
            InstallKind::WindowsInstaller
        } else {
            InstallKind::Portable { dir }
        }
    } else if cfg!(target_os = "macos") {
        match exe.ancestors().find(|a| a.extension().is_some_and(|e| e == "app")) {
            Some(app) => InstallKind::MacApp { app: app.to_path_buf() },
            None => InstallKind::Unknown,
        }
    } else if cfg!(target_os = "linux") {
        if dir.starts_with("/usr") {
            if succeeds("dpkg-query", &["-S".as_ref(), exe.as_os_str()]) {
                InstallKind::LinuxDeb
            } else if succeeds("rpm", &["-qf".as_ref(), exe.as_os_str()]) {
                InstallKind::LinuxRpm
            } else {
                InstallKind::Unknown
            }
        } else {
            InstallKind::Portable { dir }
        }
    } else {
        InstallKind::Unknown
    }
}

pub fn pick_asset<'a>(kind: &InstallKind, assets: &'a [Asset]) -> Option<&'a Asset> {
    let suffix = kind.asset_suffix()?;
    assets.iter().find(|a| a.name.ends_with(suffix))
}

/// Parse `sha256sum` output: `<hex>  <name>` (binary-mode lines have `*name`).
pub fn parse_sums(text: &str) -> HashMap<String, String> {
    text.lines()
        .filter_map(|l| {
            let (hash, name) = l.trim().split_once(char::is_whitespace)?;
            let name = name.trim_start().trim_start_matches('*');
            (hash.len() == 64 && !name.is_empty()).then(|| (name.to_string(), hash.to_ascii_lowercase()))
        })
        .collect()
}

// ------------------------------------------------------------ background thread --

pub enum Msg {
    Available(Box<Release>),
    UpToDate,
    CheckFailed(String),
    Progress { done: u64, total: u64 },
    /// The update is installed (or the installer is running): the app should close.
    Done(String),
    Failed(String),
}

pub struct Updater {
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
}

impl Updater {
    pub fn new() -> Self {
        let (tx, rx) = channel();
        Updater { tx, rx }
    }

    pub fn poll(&self) -> Option<Msg> {
        self.rx.try_recv().ok()
    }

    pub fn check(&self, ctx: eframe::egui::Context) {
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let msg = match check() {
                Ok(Some(r)) => Msg::Available(Box::new(r)),
                Ok(None) => Msg::UpToDate,
                Err(e) => Msg::CheckFailed(e),
            };
            let _ = tx.send(msg);
            ctx.request_repaint();
        });
    }

    pub fn install(&self, release: Release, kind: InstallKind, ctx: eframe::egui::Context) {
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let progress = {
                let (tx, ctx) = (tx.clone(), ctx.clone());
                move |done, total| {
                    let _ = tx.send(Msg::Progress { done, total });
                    ctx.request_repaint();
                }
            };
            let msg = match download_and_install(&release, &kind, progress) {
                Ok(m) => Msg::Done(m),
                Err(e) => Msg::Failed(e),
            };
            let _ = tx.send(msg);
            ctx.request_repaint();
        });
    }
}

// ------------------------------------------------------------ download --

fn temp_dir(release: &Release) -> Result<PathBuf, String> {
    let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("macropad-remapper-update-{}-{ts}", release.version));
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    Ok(dir)
}

fn fetch_text(url: &str) -> Result<String, String> {
    agent().get(url).call().and_then(|mut r| r.body_mut().read_to_string()).map_err(|e| format!("download failed: {e}"))
}

/// Download `asset` into `dir`, returning its path and SHA-256 (lowercase hex).
fn fetch_file(asset: &Asset, dir: &Path, mut progress: impl FnMut(u64, u64)) -> Result<(PathBuf, String), String> {
    let resp = agent().get(&asset.browser_download_url).call().map_err(|e| format!("download failed: {e}"))?;
    let total = resp
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok()?.parse().ok())
        .unwrap_or(asset.size);
    let mut reader = resp.into_body().into_reader();
    let path = dir.join(&asset.name);
    let mut file = std::fs::File::create(&path).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    let mut done = 0u64;
    let mut last_report = 0u64;
    loop {
        let n = reader.read(&mut buf).map_err(|e| format!("download failed: {e}"))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        file.write_all(&buf[..n]).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        done += n as u64;
        if done - last_report >= 256 * 1024 {
            progress(done, total);
            last_report = done;
        }
    }
    file.flush().map_err(|e| e.to_string())?;
    progress(done, total);
    let hash = hasher.finalize().iter().map(|b| format!("{b:02x}")).collect();
    Ok((path, hash))
}

fn download_and_install(release: &Release, kind: &InstallKind, progress: impl FnMut(u64, u64)) -> Result<String, String> {
    let asset = pick_asset(kind, &release.assets)
        .ok_or_else(|| format!("release {} has no download for this {}", release.version, kind.describe()))?;
    let sums_asset = release
        .assets
        .iter()
        .find(|a| a.name == SUMS_NAME)
        .ok_or_else(|| format!("release {} has no {SUMS_NAME}; not installing an unverified file", release.version))?;
    let expected = parse_sums(&fetch_text(&sums_asset.browser_download_url)?)
        .remove(&asset.name)
        .ok_or_else(|| format!("{} is not listed in {SUMS_NAME}", asset.name))?;

    let dir = temp_dir(release)?;
    let (file, actual) = fetch_file(asset, &dir, progress)?;
    if actual != expected {
        let _ = std::fs::remove_file(&file);
        return Err(format!("checksum mismatch for {} — the download was corrupted, nothing was installed", asset.name));
    }
    install(kind, &file, &dir)
}

// ------------------------------------------------------------ install --

fn install(kind: &InstallKind, file: &Path, work: &Path) -> Result<String, String> {
    match kind {
        InstallKind::WindowsInstaller => {
            // Inno Setup elevates itself if the previous install was for all users. A
            // `[Run] … Check: WizardSilent` entry in installer.iss restarts the app afterwards.
            Command::new(file)
                .args(["/SILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/CLOSEAPPLICATIONS"])
                .spawn()
                .map_err(|e| format!("cannot start the installer: {e}"))?;
            Ok("Installer started; the app will reopen when it is done.".into())
        }
        InstallKind::Portable { dir } => {
            let unpacked = work.join("unpacked");
            let zip = std::fs::File::open(file).map_err(|e| e.to_string())?;
            zip::ZipArchive::new(zip)
                .and_then(|mut z| z.extract(&unpacked))
                .map_err(|e| format!("cannot unpack {}: {e}", file.display()))?;
            let src = find_dir_with(&unpacked, GUI_BIN).ok_or("the download does not contain the app")?;
            replace_portable(&src, dir)?;
            relaunch(&dir.join(GUI_BIN))?;
            Ok("Updated; restarting…".into())
        }
        InstallKind::MacApp { app } => {
            let unpacked = work.join("unpacked");
            run("ditto", &["-x".as_ref(), "-k".as_ref(), file.as_os_str(), unpacked.as_os_str()])?;
            let new_app = find_app(&unpacked).ok_or("the download does not contain the app")?;
            replace_mac_app(&new_app, app)?;
            run("open", &["-n".as_ref(), app.as_os_str()])?;
            Ok("Updated; restarting…".into())
        }
        InstallKind::LinuxDeb => {
            // apt's sandbox user must be able to read the file.
            set_mode(file, 0o644);
            set_mode(work, 0o755);
            pkexec(&["apt-get", "install", "-y", "--allow-downgrades"], file)?;
            relaunch(Path::new("/usr/bin/macropad-remapper-gui"))?;
            Ok("Updated; restarting…".into())
        }
        InstallKind::LinuxRpm => {
            if Command::new("dnf").arg("--version").output().is_ok() {
                pkexec(&["dnf", "install", "-y"], file)?;
            } else {
                pkexec(&["rpm", "-U", "--force"], file)?;
            }
            relaunch(Path::new("/usr/bin/macropad-remapper-gui"))?;
            Ok("Updated; restarting…".into())
        }
        InstallKind::Unknown => Err("this copy cannot update itself".into()),
    }
}

fn run(cmd: &str, args: &[&std::ffi::OsStr]) -> Result<(), String> {
    let out = Command::new(cmd).args(args).output().map_err(|e| format!("cannot run {cmd}: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!("{cmd} failed: {}", String::from_utf8_lossy(&out.stderr).trim()))
    }
}

/// Run a package manager as root through polkit. If that is not possible, open the
/// package with the desktop's software installer instead.
fn pkexec(cmd: &[&str], file: &Path) -> Result<(), String> {
    let status = Command::new("pkexec").args(cmd).arg(file).status();
    match status {
        Ok(s) if s.success() => Ok(()),
        // 126: the user dismissed the password prompt.
        Ok(s) if s.code() == Some(126) => Err("update cancelled".into()),
        _ => {
            run("xdg-open", &[file.as_os_str()])?;
            Err(format!("opened {} in your software installer — finish the update there", file.display()))
        }
    }
}

#[cfg(unix)]
fn set_mode(p: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(p, std::fs::Permissions::from_mode(mode));
}
#[cfg(not(unix))]
fn set_mode(_: &Path, _: u32) {}

fn find_dir_with(root: &Path, file: &str) -> Option<PathBuf> {
    if root.join(file).is_file() {
        return Some(root.to_path_buf());
    }
    std::fs::read_dir(root).ok()?.flatten().filter(|e| e.path().is_dir()).find_map(|e| find_dir_with(&e.path(), file))
}

fn find_app(root: &Path) -> Option<PathBuf> {
    for e in std::fs::read_dir(root).ok()?.flatten() {
        let p = e.path();
        if p.is_dir() {
            if p.extension().is_some_and(|x| x == "app") {
                return Some(p);
            }
            if let Some(found) = find_app(&p) {
                return Some(found);
            }
        }
    }
    None
}

fn replace_portable(src: &Path, dir: &Path) -> Result<(), String> {
    let err = |e: std::io::Error| format!("cannot update {}: {e}", dir.display());
    // The running GUI cannot be overwritten in place on Windows; self-replace handles it.
    let new_gui = src.join(GUI_BIN);
    set_mode(&new_gui, 0o755);
    if std::env::current_exe().ok().and_then(|p| p.canonicalize().ok()) == dir.join(GUI_BIN).canonicalize().ok() {
        self_replace::self_replace(&new_gui).map_err(err)?;
    } else {
        std::fs::copy(&new_gui, dir.join(GUI_BIN)).map_err(err)?;
    }
    if src.join(CLI_BIN).is_file() {
        std::fs::copy(src.join(CLI_BIN), dir.join(CLI_BIN)).map_err(err)?;
        set_mode(&dir.join(CLI_BIN), 0o755);
    }
    // Docs that describe the new version. Examples are left alone (users edit them).
    for doc in ["README.md", "CHANGELOG.md", "user-guide.md"] {
        if src.join(doc).is_file() {
            let _ = std::fs::copy(src.join(doc), dir.join(doc));
        }
    }
    Ok(())
}

fn replace_mac_app(new_app: &Path, app: &Path) -> Result<(), String> {
    let parent = app.parent().ok_or("app has no parent folder")?;
    let staged = parent.join(".macropad-remapper-update.app");
    let old = parent.join(".macropad-remapper-old.app");
    let _ = std::fs::remove_dir_all(&staged);
    let _ = std::fs::remove_dir_all(&old);
    // Copy next to the app first (the temp folder may be on another volume), then swap.
    run("ditto", &[new_app.as_os_str(), staged.as_os_str()])
        .map_err(|e| format!("cannot write to {} ({e}). Download the update from the release page instead.", parent.display()))?;
    std::fs::rename(app, &old).map_err(|e| format!("cannot replace {}: {e}", app.display()))?;
    if let Err(e) = std::fs::rename(&staged, app) {
        let _ = std::fs::rename(&old, app);
        return Err(format!("cannot replace {}: {e}", app.display()));
    }
    let _ = std::fs::remove_dir_all(&old);
    Ok(())
}

fn relaunch(exe: &Path) -> Result<(), String> {
    Command::new(exe).spawn().map(|_| ()).map_err(|e| format!("updated, but could not restart: {e}"))
}

// ------------------------------------------------------------ tests --

#[cfg(test)]
mod tests {
    use super::*;

    fn assets(names: &[&str]) -> Vec<Asset> {
        names
            .iter()
            .map(|n| Asset { name: n.to_string(), browser_download_url: format!("https://x/{n}"), size: 0 })
            .collect()
    }

    const NAMES: &[&str] = &[
        "SHA256SUMS.txt",
        "macropad-remapper-0.4.0-linux-amd64.deb",
        "macropad-remapper-0.4.0-linux-x86_64-portable.zip",
        "macropad-remapper-0.4.0-linux-x86_64.rpm",
        "macropad-remapper-0.4.0-macos-universal-portable.zip",
        "macropad-remapper-0.4.0-macos-universal.dmg",
        "macropad-remapper-0.4.0-windows-x64-portable.zip",
        "macropad-remapper-0.4.0-windows-x64-setup.exe",
    ];

    #[test]
    fn picks_matching_asset() {
        let a = assets(NAMES);
        let name = |k: InstallKind| pick_asset(&k, &a).map(|x| x.name.clone());
        assert_eq!(name(InstallKind::MacApp { app: "/Applications/X.app".into() }).unwrap(), NAMES[4]);
        assert_eq!(name(InstallKind::Unknown), None);
        if cfg!(all(windows, target_arch = "x86_64")) {
            assert_eq!(name(InstallKind::WindowsInstaller).unwrap(), NAMES[7]);
            assert_eq!(name(InstallKind::Portable { dir: "C:/x".into() }).unwrap(), NAMES[6]);
        }
        if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            assert_eq!(name(InstallKind::LinuxDeb).unwrap(), NAMES[1]);
            assert_eq!(name(InstallKind::LinuxRpm).unwrap(), NAMES[3]);
            assert_eq!(name(InstallKind::Portable { dir: "/opt/x".into() }).unwrap(), NAMES[2]);
        }
    }

    #[test]
    fn parses_sha256sum_output() {
        let h = "a".repeat(64);
        let text = format!("{h}  macropad-remapper-0.4.0-linux-amd64.deb\n{}  *bin.zip\n\ngarbage\n", "B".repeat(64));
        let m = parse_sums(&text);
        assert_eq!(m.len(), 2);
        assert_eq!(m["macropad-remapper-0.4.0-linux-amd64.deb"], h);
        assert_eq!(m["bin.zip"], "b".repeat(64));
    }

    #[test]
    fn parses_release_json() {
        let json = r#"{"tag_name":"v0.4.0","html_url":"https://github.com/r","body":"notes",
            "assets":[{"name":"SHA256SUMS.txt","browser_download_url":"https://x/s","size":10}],
            "draft":false}"#;
        let r: ApiRelease = serde_yaml::from_str(json).unwrap();
        assert_eq!(r.tag_name, "v0.4.0");
        assert_eq!(r.assets[0].size, 10);
        assert!(Version::parse(r.tag_name.trim_start_matches('v')).unwrap() > Version::parse("0.3.2").unwrap());
    }

    #[test]
    fn settings_roundtrip_and_defaults() {
        let s: Settings = serde_yaml::from_str("skipped_version: 0.4.0\n").unwrap();
        assert!(s.check_updates);
        assert_eq!(s.skipped_version.as_deref(), Some("0.4.0"));
        let s: Settings = serde_yaml::from_str(&serde_yaml::to_string(&Settings { check_updates: false, skipped_version: None }).unwrap()).unwrap();
        assert!(!s.check_updates);
    }
}

/// Network test against the real latest release:
/// `MACROPAD_FAKE_VERSION=0.0.1 MACROPAD_TEST_DIR=<empty dir> cargo test -p macropad-remapper-gui -- --ignored`
#[cfg(test)]
#[test]
#[ignore]
fn live_portable_update() {
    let dir = PathBuf::from(std::env::var("MACROPAD_TEST_DIR").expect("set MACROPAD_TEST_DIR"));
    let r = check().unwrap().expect("set MACROPAD_FAKE_VERSION=0.0.1 so the latest release counts as new");
    let kind = InstallKind::Portable { dir: dir.clone() };
    let mut last = (0, 0);
    // Install = unpack + copy (the test binary is not the GUI, so no self-replace),
    // then relaunch the freshly installed GUI.
    let msg = download_and_install(&r, &kind, |d, t| last = (d, t)).unwrap();
    println!("{msg}; downloaded {} of {} bytes", last.0, last.1);
    assert!(dir.join(GUI_BIN).is_file() && dir.join(CLI_BIN).is_file());
}
