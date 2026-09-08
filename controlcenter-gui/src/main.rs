//! System Control GUI.
//!
//! The Battery section is wired to `controlcenterd` over D-Bus: the slider
//! only edits a local, pending value -- nothing is sent to hardware until
//! the Apply button is clicked. Platform Profile is still a local-only
//! mock, not wired up yet.

use eframe::egui;

/// Mirrors the `org.controlcenter.Daemon1.Battery` interface implemented
/// in `rust-daemon/src/dbus_iface.rs`. Method/property names here must
/// stay in sync with that file.
///
/// zbus generates *two* proxies from this trait: an async `BatteryProxy`
/// and (because zbus's default features include `blocking-api`) a
/// `BatteryProxyBlocking`. We use only the blocking one below -- egui's
/// `update()` is a plain synchronous function, so a blocking D-Bus call
/// (a local socket round-trip, normally a few milliseconds) is simpler
/// here than pulling in an async runtime just for this.
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

/// Connects to the daemon and asks it to set the charge limit. A fresh
/// connection per call is the simplest correct thing to do -- it costs
/// one extra local socket connect, which is cheap compared to keeping
/// connection-lifecycle/reconnect logic in the app's state right now.
fn apply_charge_limit(limit: u8) -> zbus::Result<u8> {
    let connection = zbus::blocking::Connection::system()?;
    let battery = BatteryProxyBlocking::new(&connection)?;
    battery.set_charge_limit(limit)
}

/// Reads the current charge limit from the daemon, if it's reachable.
fn fetch_charge_limit() -> zbus::Result<u8> {
    let connection = zbus::blocking::Connection::system()?;
    let battery = BatteryProxyBlocking::new(&connection)?;
    battery.charge_limit()
}

#[derive(Clone, Copy, PartialEq)]
enum PlatformProfile {
    Quiet,
    Balanced,
    Performance,
}

impl PlatformProfile {
    fn label(self) -> &'static str {
        match self {
            PlatformProfile::Quiet => "Quiet",
            PlatformProfile::Balanced => "Balanced",
            PlatformProfile::Performance => "Performance",
        }
    }
}

/// Result of the last Apply click, so the UI can show something useful
/// (success, or *why* it failed) instead of just going quiet.
enum ApplyStatus {
    Idle,
    Success(u8),
    Error(String),
}

struct ControlCenterApp {
    /// Value the slider is currently set to -- just a pending choice until
    /// Apply is clicked. Does not necessarily match what hardware has
    /// applied.
    charge_limit: u8,
    platform_profile: PlatformProfile,
    apply_status: ApplyStatus,
}

impl Default for ControlCenterApp {
    fn default() -> Self {
        // Best-effort: if the daemon is up, start the slider at the real
        // current value instead of an arbitrary default. If it's not
        // reachable, fall back quietly -- the user can still pick a value
        // and Apply once the daemon is running.
        let charge_limit = fetch_charge_limit().unwrap_or(100);

        Self {
            charge_limit,
            platform_profile: PlatformProfile::Balanced,
            apply_status: ApplyStatus::Idle,
        }
    }
}

impl eframe::App for ControlCenterApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("System Control");
            ui.add_space(8.0);

            ui.group(|ui| {
                ui.label(egui::RichText::new("Battery").strong());
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label("Charge limit:");
                    ui.add(egui::Slider::new(&mut self.charge_limit, 20..=100).suffix("%"));
                });
                ui.label(format!(
                    "Charging will stop at {}% to reduce long-term battery wear.",
                    self.charge_limit
                ));

                ui.add_space(6.0);
                if ui.button("Apply").clicked() {
                    self.apply_status = match apply_charge_limit(self.charge_limit) {
                        Ok(applied) => {
                            // The daemon returns what hardware actually
                            // accepted -- reflect that back into the
                            // slider in case it differs from the request.
                            self.charge_limit = applied;
                            ApplyStatus::Success(applied)
                        }
                        Err(err) => ApplyStatus::Error(err.to_string()),
                    };
                }

                match &self.apply_status {
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

            ui.add_space(12.0);

            ui.group(|ui| {
                ui.label(egui::RichText::new("Platform Profile").strong());
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    for profile in [
                        PlatformProfile::Quiet,
                        PlatformProfile::Balanced,
                        PlatformProfile::Performance,
                    ] {
                        ui.selectable_value(&mut self.platform_profile, profile, profile.label());
                    }
                });
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new("Not wired up yet -- selecting a profile does nothing.")
                        .weak()
                        .italics(),
                );
            });
        });
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1200.0, 800.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Control Center",
        options,
        Box::new(|_cc| Ok(Box::new(ControlCenterApp::default()))),
    )
}
