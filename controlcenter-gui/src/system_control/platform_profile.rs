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
}

#[zbus::proxy (
    interface = "org.controlcenter.Daemon1.set-platform-profile",
    default_service = "org.controlcenter.Daemon1",
    default_path = "/org/controlcenter/Daemon1"
)]
trait Battery {
    #[zbus(property)]
    fn read_profile(&self) -> zbus::Result<String>;
    fn set_profile(&self, profile: String) -> zbus::Result<String>;
}
// Why the type of the function is different compare to n platform_profile.rs
fn apply_profile(profile: String) -> zbus::Result<String> {
    let connection = zbus::blocking::Connection::new(&connection)?;
    let battery = 
}
pub struct State {
    selected: Profile,
}

impl Default for State {
    fn default() -> Self {
        Self {
            selected: Profile::Balanced,
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
        ui.label(
            egui::RichText::new("Not wired up yet -- selecting a profile does nothing.")
                .weak()
                .italics(),
        );
    });
}
