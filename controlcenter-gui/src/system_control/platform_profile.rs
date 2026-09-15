//! Platform-profile picker. Still local-only -- selecting a profile does
//! not talk to the daemon yet.

use eframe::egui;
use zbus::blocking::connection;

#[derive(Clone, Copy, PartialEq)]
enum Profile {
    Quiet,
    Balanced,
    Performance,
}

impl Profile {
    fn label(self) -> &'static str {
        match self {
            Profile::Quiet => "Quiet",
            Profile::Balanced => "Balanced",
            Profile::Performance => "Performance",
        }
    }

    fn from_damon_str(value: &str) -> Option<Self> {
        match value {
            "quiet" => Some(Profile::Quiet),
            "balanced" => Some(Profile::Balanced),
            "performance" => Some(Profile::Performance),
            _ => None,
        }
    }

    /// The reverse of `from_damon_str` -- what to send *to* the daemon.
    /// Sysfs (and so the daemon) speaks lowercase, `label()` is for
    /// display only.
    fn to_daemon_str(self) -> &'static str {
        match self {
            Profile::Quiet => "quiet",
            Profile::Balanced => "balanced",
            Profile::Performance => "performance",
        }
    }
}

#[zbus::proxy (
    interface = "org.controlcenter.Daemon1.PlatformProfile",
    default_service = "org.controlcenter.Daemon1",
    default_path = "/org/controlcenter/Daemon1"
)]
trait PlatformProfile {
    #[zbus(property)]
    fn read_profile(&self) -> zbus::Result<String>;
    fn set_profile(&self, current_profile: String) -> zbus::Result<String>;
}

// So 
// Why the type of the function is different compare to n platform_profile.rs
fn apply_profile(current_profile: String) -> zbus::Result<String> {
    let connection = zbus::blocking::Connection::system()?;
    let profile = PlatformProfileProxyBlocking::new(&connection)?;
    profile.set_profile(current_profile)
}

fn fetch_current_profile() -> zbus::Result<String> {
    let connection = zbus::blocking::Connection::system()?;
    let profile = PlatformProfileProxyBlocking::new(&connection)?;
    profile.read_profile()
}
enum ApplyStatus {
    Idle,
    Success(Profile),
    Error(String),
}

pub struct State {
    selected: Profile,
    apply_status: ApplyStatus,
}

impl Default for State {
    fn default() -> Self {
        let selected = fetch_current_profile().ok().and_then(|value| Profile::from_damon_str(&value)).unwrap_or(Profile::Balanced);
        Self {
            selected: selected,
            apply_status: ApplyStatus::Idle,
        }
    }
}

pub(super) fn show(ui: &mut egui::Ui, state: &mut State) {
    ui.group(|ui| {
        ui.label(egui::RichText::new("Platform Profile").strong());
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            for profile in [Profile::Quiet, Profile::Balanced, Profile::Performance] {
                ui.selectable_value(&mut state.selected, profile, profile.label());
            }
        });

        ui.add_space(6.0);
        if ui.button("Apply").clicked() {
            state.apply_status = match apply_profile(state.selected.to_daemon_str().to_string()) {
                Ok(applied) => match Profile::from_damon_str(&applied) {
                    // Reflect back what hardware actually accepted, in case
                    // it differs from the request.
                    Some(profile) => {
                        state.selected = profile;
                        ApplyStatus::Success(profile)
                    }
                    None => ApplyStatus::Error(format!("daemon returned unrecognized profile {applied:?}")),
                },
                Err(err) => ApplyStatus::Error(err.to_string()),
            };
        }

        match &state.apply_status {
            ApplyStatus::Idle => {}
            ApplyStatus::Success(applied) => {
                ui.colored_label(
                    egui::Color32::from_rgb(80, 200, 120),
                    format!("Applied -- profile is now {}.", applied.label()),
                );
            }
            ApplyStatus::Error(message) => {
                ui.colored_label(
                    egui::Color32::from_rgb(220, 90, 90),
                    format!("Failed to apply: {message}"),
                );
            }
        }
    });
}
