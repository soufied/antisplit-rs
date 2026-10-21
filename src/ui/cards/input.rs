use eframe::egui::{self, RichText};

use crate::ui::app::App;
use crate::ui::theme::{accent, drop_zone, drop_zone_hover};
use crate::ui::widgets::{card, hint};

impl App {
    pub(crate) fn input_card(&mut self, ui: &mut egui::Ui) {
        let busy = self.busy();
        let hovering = self.hovering;
        let loaded = self.selected.is_some();
        let path_text = self.path_text();
        card(ui, "1 - Input bundle", |ui| {
            let fill = if hovering { drop_zone_hover() } else { drop_zone() };
            egui::Frame::group(ui.style())
                .fill(fill)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let button = egui::Button::new("Choose bundle...")
                            .min_size(egui::vec2(170.0, 32.0));
                        if ui
                            .add_enabled(!busy, button)
                            .on_hover_text("Open a .apkm, .apks, .xapk, .apkx or .zip bundle.\nYou can also drag one onto this window.")
                            .clicked()
                        {
                            self.choose_file();
                        }
                        ui.add(egui::Label::new(RichText::new(path_text).monospace()).truncate())
                            .on_hover_text("The bundle that is currently loaded");
                    });
                    ui.add_space(6.0);
                    if hovering {
                        ui.label(RichText::new("Release to load this bundle").color(accent()).strong());
                    } else if loaded {
                        hint(ui, "Choose another bundle, or drop one here, to replace it.");
                    } else {
                        hint(ui, "Drop a bundle here, or choose one.");
                    }
                });
            ui.add_space(4.0);
            hint(
                ui,
                "Supported: .apkm  .apks  .xapk  .apkx  .zip. Dropped keystores and uber-apk-signer.jar files are picked up as settings.",
            );
        });
    }
}
