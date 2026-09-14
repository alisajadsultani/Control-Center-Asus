//! Control Center GUI. Top-level window; each panel below is a section
//! module (see `system_control/`) mirroring `config/project-description.txt`.

mod system_control;

use eframe::egui;

#[derive(Default)]
struct ControlCenterApp {
    system_control: system_control::State,
}

impl eframe::App for ControlCenterApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("System Control");
            ui.add_space(8.0);
            system_control::show(ui, &mut self.system_control);
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
