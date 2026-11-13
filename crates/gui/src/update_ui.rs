//! Update chip, dialog and settings menu (see `update.rs` for the mechanics).

use eframe::egui::{self, RichText};

use crate::theme::*;
use crate::update::{self, InstallKind, Msg, Release, Settings, Updater};
use crate::widgets::{ghost, primary};

pub struct UpdateUi {
    updater: Updater,
    settings: Settings,
    kind: InstallKind,
    available: Option<Release>,
    dialog_open: bool,
    checking: bool,
    /// A check the user asked for: report "up to date" / errors in the status bar.
    manual: bool,
    /// `Some` while downloading/installing.
    progress: Option<(u64, u64)>,
    error: Option<String>,
}

impl UpdateUi {
    pub fn new() -> Self {
        UpdateUi {
            updater: Updater::new(),
            settings: Settings::load(),
            kind: update::detect_install(),
            available: None,
            dialog_open: false,
            checking: false,
            manual: false,
            progress: None,
            error: None,
        }
    }

    /// Called once when the window opens.
    pub fn on_startup(&mut self, ctx: &egui::Context) {
        if self.settings.check_updates {
            self.check(ctx, false);
        }
    }

    fn check(&mut self, ctx: &egui::Context, manual: bool) {
        if !self.checking {
            self.checking = true;
            self.manual = manual;
            self.updater.check(ctx.clone());
        }
    }

    /// Handle background messages; returns a status-bar message, if any.
    pub fn poll(&mut self, ctx: &egui::Context) -> Option<(String, bool)> {
        let mut status = None;
        while let Some(msg) = self.updater.poll() {
            match msg {
                Msg::Available(r) => {
                    self.checking = false;
                    let skipped = self.settings.skipped_version.as_deref() == Some(&r.version.to_string());
                    if self.manual || !skipped {
                        self.available = Some(*r);
                        self.dialog_open = self.manual;
                    }
                }
                Msg::UpToDate => {
                    self.checking = false;
                    if self.manual {
                        status = Some((format!("You have the latest version (v{})", update::current_version()), false));
                    }
                }
                Msg::CheckFailed(e) => {
                    self.checking = false;
                    if self.manual {
                        status = Some((e, true));
                    }
                }
                Msg::Progress { done, total } => self.progress = Some((done, total)),
                Msg::Done(m) => {
                    status = Some((m, false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                Msg::Failed(e) => {
                    self.progress = None;
                    self.error = Some(e.clone());
                    status = Some((format!("Update failed: {e}"), true));
                }
            }
        }
        status
    }

    /// Toolbar chip, shown when a newer version is available.
    pub fn chip(&mut self, ui: &mut egui::Ui) {
        if let Some(r) = &self.available {
            let text = RichText::new(format!("⬆  v{} available", r.version)).color(TEXT).strong();
            let btn = egui::Button::new(text).fill(ACCENT_DIM).stroke(egui::Stroke::new(1.0, ACCENT)).corner_radius(14);
            if ui.add(btn).on_hover_text("Click to see what's new and install it").clicked() {
                self.dialog_open = true;
                self.error = None;
            }
        }
    }

    /// ⚙ menu with the update settings.
    pub fn settings_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button(RichText::new("⚙").size(16.0).color(TEXT), |ui| {
            if ui.checkbox(&mut self.settings.check_updates, "Check for updates on startup").changed() {
                self.settings.save();
            }
            let label = if self.checking { "Checking…" } else { "Check for updates now" };
            if ui.add_enabled(!self.checking, egui::Button::new(label)).clicked() {
                let ctx = ui.ctx().clone();
                self.check(&ctx, true);
                ui.close();
            }
        });
    }

    /// Progress line for the status bar while an update downloads.
    pub fn status_bar(&self, ui: &mut egui::Ui) {
        if let Some((done, total)) = self.progress {
            let frac = if total > 0 { done as f32 / total as f32 } else { 0.0 };
            ui.add(egui::ProgressBar::new(frac).desired_width(160.0).text(format!("Updating… {:.0}%", frac * 100.0)));
        }
    }

    /// The update dialog. `busy` is true while the keypad is being written.
    pub fn dialog(&mut self, ctx: &egui::Context, busy: bool) {
        if !self.dialog_open {
            return;
        }
        let Some(r) = self.available.clone() else { return };
        let installing = self.progress.is_some();
        let asset = update::pick_asset(&self.kind, &r.assets).is_some();
        let modal = egui::Modal::new(egui::Id::unique("update_dialog")).show(ctx, |ui| {
            ui.set_width(460.0);
            ui.label(RichText::new(format!("Update available: v{}", r.version)).size(18.0).strong());
            ui.add_space(2.0);
            ui.label(
                RichText::new(format!("You have v{} ({}).", update::current_version(), self.kind.describe()))
                    .small()
                    .color(MUTED),
            );
            ui.add_space(8.0);
            egui::Frame::new().fill(CARD).corner_radius(8).inner_margin(10).show(ui, |ui| {
                ui.set_width(ui.available_width());
                egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                    let notes = if r.notes.trim().is_empty() { "No release notes." } else { r.notes.trim() };
                    ui.label(RichText::new(notes).color(TEXT));
                });
            });
            ui.add_space(8.0);
            if let Some(e) = &self.error {
                ui.label(RichText::new(e).small().color(DANGER));
                ui.add_space(4.0);
            }
            if busy {
                ui.label(RichText::new("Wait until the keypad write has finished.").small().color(WARN));
                ui.add_space(4.0);
            }
            if installing {
                self.status_bar(ui);
                ui.label(RichText::new("The app will close and reopen when the update is installed.").small().color(MUTED));
                return;
            }
            if !asset {
                ui.label(
                    RichText::new("This copy can't update itself. Download the new version from the release page.")
                        .small()
                        .color(MUTED),
                );
                ui.add_space(4.0);
            }
            ui.horizontal(|ui| {
                if asset {
                    if primary(ui, !busy, "Download & install").clicked() {
                        self.error = None;
                        self.progress = Some((0, 0));
                        self.updater.install(r.clone(), self.kind.clone(), ctx.clone());
                    }
                } else if primary(ui, true, "Open release page").clicked() {
                    ctx.open_url(egui::OpenUrl::new_tab(&r.page));
                }
                if ghost(ui, true, "Skip this version").clicked() {
                    self.settings.skipped_version = Some(r.version.to_string());
                    self.settings.save();
                    self.available = None;
                    self.dialog_open = false;
                }
                if ghost(ui, true, "Later").clicked() {
                    self.dialog_open = false;
                }
            });
        });
        if modal.should_close() && !installing {
            self.dialog_open = false;
        }
    }
}
