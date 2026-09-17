//! OS updates: proxy to `controlcenterd`'s `Updates` interface. Apply runs
//! on a background thread (`std::thread::spawn`, `state.apply` is an
//! `Arc<Mutex<ApplyStatus>>`) instead of directly in `show()` like
//! battery/platform_profile, since `apt upgrade` can take minutes and that
//! would freeze the window.

use std::sync::{Arc, Mutex};

use eframe::egui;

#[zbus::proxy(
    interface = "org.controlcenter.Daemon1.Updates",
    default_service = "org.controlcenter.Daemon1",
    default_path = "/org/controlcenter/Daemon1"
)]
trait Updates {
    fn preview_os(&self) -> zbus::Result<String>;
    fn apply_os(&self) -> zbus::Result<()>;
}

fn preview_os() -> zbus::Result<String> {
    let connection = zbus::blocking::Connection::system()?;
    let updates = UpdatesProxyBlocking::new(&connection)?;
    updates.preview_os()
}

fn apply_os() -> zbus::Result<()> {
    let connection = zbus::blocking::Connection::system()?;
    let updates = UpdatesProxyBlocking::new(&connection)?;
    updates.apply_os()
}

/// Strips `apt`'s `Listing... Done` header, which prints even when nothing
/// is upgradable, before deciding `UpToDate` vs `Available`.
fn parse_preview(output: &str) -> PreviewStatus {
    let packages: Vec<&str> = output
        .lines()
        .filter(|line| !line.starts_with("Listing..."))
        .collect();

    if packages.is_empty() {
        PreviewStatus::UpToDate
    } else {
        PreviewStatus::Available(packages.join("\n"))
    }
}

enum PreviewStatus {
    Idle,
    UpToDate,
    Available(String),
    Error(String),
}

enum ApplyStatus {
    Idle,
    Running,
    Success,
    Error(String),
}

pub struct State {
    preview: PreviewStatus,
    show_confirm: bool,
    apply: Arc<Mutex<ApplyStatus>>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            preview: PreviewStatus::Idle,
            show_confirm: false,
            apply: Arc::new(Mutex::new(ApplyStatus::Idle)),
        }
    }
}

pub(super) fn show(ui: &mut egui::Ui, state: &mut State) {
    let applying = matches!(*state.apply.lock().unwrap(), ApplyStatus::Running);

    ui.group(|ui| {
        ui.label(egui::RichText::new("Operating System").strong());
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            if ui
                .add_enabled(!applying, egui::Button::new("Check for updates"))
                .clicked()
            {
                state.preview = match preview_os() {
                    Ok(output) => parse_preview(&output),
                    Err(err) => PreviewStatus::Error(err.to_string()),
                };
            }

            if matches!(state.preview, PreviewStatus::Available(_))
                && ui
                    .add_enabled(!applying, egui::Button::new("Update now"))
                    .clicked()
            {
                state.show_confirm = true;
            }
        });

        match &state.preview {
            PreviewStatus::Idle => {}
            PreviewStatus::UpToDate => {
                ui.label("Everything is up to date.");
            }
            PreviewStatus::Available(output) => {
                ui.label("Updates available:");
                egui::ScrollArea::vertical()
                    .max_height(120.0)
                    .show(ui, |ui| ui.monospace(output));
            }
            PreviewStatus::Error(message) => {
                ui.colored_label(
                    egui::Color32::from_rgb(220, 90, 90),
                    format!("Couldn't check for updates: {message}"),
                );
            }
        }

        ui.add_space(6.0);
        match &*state.apply.lock().unwrap() {
            ApplyStatus::Idle => {}
            ApplyStatus::Running => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Updating -- this can take a few minutes...");
                });
            }
            ApplyStatus::Success => {
                ui.colored_label(
                    egui::Color32::from_rgb(80, 200, 120),
                    "Update applied successfully.",
                );
            }
            ApplyStatus::Error(message) => {
                ui.colored_label(
                    egui::Color32::from_rgb(220, 90, 90),
                    format!("Update failed: {message}"),
                );
            }
        }
    });

    if state.show_confirm {
        confirm_dialog(ui.ctx(), state);
    }
}

fn confirm_dialog(ctx: &egui::Context, state: &mut State) {
    egui::Window::new("Apply OS updates?")
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.label(
                "Installs the updates listed above. Takes a few minutes; \
                 a restart may be needed after if the kernel was updated.",
            );
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Cancel").clicked() {
                    state.show_confirm = false;
                }
                if ui.button("Yes, update").clicked() {
                    state.show_confirm = false;
                    start_apply(ctx, state);
                }
            });
        });
}

/// Clears the preview immediately (hides "Update now" for the whole run,
/// not just after) and runs `apply_os` on a background thread; see the
/// module doc for why. `request_repaint` guarantees the final result gets
/// drawn even without other UI activity.
fn start_apply(ctx: &egui::Context, state: &mut State) {
    state.preview = PreviewStatus::Idle;
    *state.apply.lock().unwrap() = ApplyStatus::Running;
    let apply = Arc::clone(&state.apply);
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        let result = match apply_os() {
            Ok(()) => ApplyStatus::Success,
            Err(err) => ApplyStatus::Error(err.to_string()),
        };
        *apply.lock().unwrap() = result;
        ctx.request_repaint();
    });
}
