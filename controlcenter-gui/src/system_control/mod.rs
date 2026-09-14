//! System Control panel: battery, platform profile, and other laptop
//! settings (per `config/project-description.txt`).

mod battery;
mod platform_profile;

use eframe::egui;

#[derive(Default)]
pub struct State {
    battery: battery::State,
    platform_profile: platform_profile::State,
}

pub fn show(ui: &mut egui::Ui, state: &mut State) {
    battery::show(ui, &mut state.battery);
    ui.add_space(12.0);
    platform_profile::show(ui, &mut state.platform_profile);
}
