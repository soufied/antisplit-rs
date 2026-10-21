
mod build;
mod error;
mod format;
mod logging;
mod pipeline;
mod settings;
mod ui;
mod util;
mod worker;

use eframe::egui;

fn main() -> eframe::Result<()> {
    logging::init();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("antisplit-rs")
            .with_inner_size([940.0, 680.0])
            .with_min_inner_size([680.0, 480.0]),
        ..Default::default()
    };
    eframe::run_native(
        "antisplit-rs",
        options,
        Box::new(|cc| Ok(Box::new(ui::App::new(cc)))),
    )
}
