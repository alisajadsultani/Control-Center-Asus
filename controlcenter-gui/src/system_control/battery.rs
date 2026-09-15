//! Battery charge-limit control: proxy to `controlcenterd` over D-Bus.
//! The slider only edits a pending value -- nothing is sent to hardware
//! until Apply is clicked.

use eframe::egui;
use log::debug;

/// Mirrors `org.controlcenter.Daemon1.Battery` in
/// `rust-daemon/src/system_control/battery_iface.rs` -- names here must
/// stay in sync with that file.
///
/// Generates `BatteryProxy` (async) and `BatteryProxyBlocking`; we use the
/// blocking one since egui's `update()` is a plain synchronous function.
#[zbus::proxy(
    interface = "org.controlcenter.Daemon1.Battery",
    default_service = "org.controlcenter.Daemon1",
    default_path = "/org/controlcenter/Daemon1"
)]
trait Battery {
    #[zbus(property)]
    fn charge_limit(&self) -> zbus::Result<u8>;

    fn set_charge_limit(&self, limit: u8) -> zbus::Result<u8>;
}

/// A fresh connection per call is simplest-correct: one extra local socket
/// connect, cheap compared to keeping connection-lifecycle state around.
fn apply_charge_limit(limit: u8) -> zbus::Result<u8> {
    let connection = zbus::blocking::Connection::system()?;
    let battery = BatteryProxyBlocking::new(&connection)?;
    battery.set_charge_limit(limit)
}

fn fetch_charge_limit() -> zbus::Result<u8> {
    let connection = zbus::blocking::Connection::system()?;
    let battery: BatteryProxyBlocking<'_> = BatteryProxyBlocking::new(&connection)?;
    battery.charge_limit()
}

enum ApplyStatus {
    Idle,
    Success(u8),
    Error(String),
}

pub struct State {
    charge_limit: u8,
    apply_status: ApplyStatus,
}

impl Default for State {
    fn default() -> Self {

        Self {
            charge_limit: fetch_charge_limit().unwrap_or(100),
            apply_status: ApplyStatus::Idle,
        }
    }
}

pub(super) fn show(ui: &mut egui::Ui, state: &mut State) {
    ui.group(|ui| {
        ui.label(egui::RichText::new("Battery").strong());
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label("Charge limit:");
            ui.add(egui::Slider::new(&mut state.charge_limit, 20..=100).suffix("%"));
        });
        ui.label(format!(
            "Charging will stop at {}% to reduce long-term battery wear.",
            state.charge_limit
        ));

        ui.add_space(6.0);
        if ui.button("Apply").clicked() {
            state.apply_status = match apply_charge_limit(state.charge_limit) {
                Ok(applied) => {
                    // Reflect back what hardware actually accepted, in case
                    // it differs from the request.
                    state.charge_limit = applied;
                    ApplyStatus::Success(applied)
                }
                Err(err) => ApplyStatus::Error(err.to_string()),
            };
        }

        match &state.apply_status {
            ApplyStatus::Idle => {}
            ApplyStatus::Success(applied) => {
                ui.colored_label(
                    egui::Color32::from_rgb(80, 200, 120),
                    format!("Applied -- charge limit is now {applied}%."),
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
