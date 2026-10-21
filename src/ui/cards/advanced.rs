use eframe::egui::{self, RichText};

use crate::settings::presets;
use crate::ui::app::App;
use crate::ui::widgets::{hint, pick_file, tool_row};

const ABI_CHOICES: [(Option<&str>, &str); 5] = [
    (None, "Auto"),
    (Some("arm64_v8a"), "arm64_v8a"),
    (Some("armeabi_v7a"), "armeabi_v7a"),
    (Some("x86_64"), "x86_64"),
    (Some("x86"), "x86"),
];

const DENSITY_CHOICES: [(Option<&str>, &str); 8] = [
    (None, "Auto"),
    (Some("ldpi"), "ldpi"),
    (Some("mdpi"), "mdpi"),
    (Some("hdpi"), "hdpi"),
    (Some("xhdpi"), "xhdpi"),
    (Some("xxhdpi"), "xxhdpi"),
    (Some("xxxhdpi"), "xxxhdpi"),
    (Some("nodpi"), "nodpi"),
];

impl App {
    pub(crate) fn advanced_card(&mut self, ui: &mut egui::Ui) {
        let busy = self.busy();
        egui::CollapsingHeader::new(RichText::new("Advanced settings").strong())
            .default_open(false)
            .show(ui, |ui| {
                ui.add_enabled_ui(!busy, |ui| {
                    hint(
                        ui,
                        "Target settings apply on the next Prepare. Tool settings apply on the next Merge.",
                    );
                    ui.add_space(8.0);

                    ui.label(RichText::new("Target").strong());
                    ui.horizontal(|ui| {
                        ui.label("Device");
                        egui::ComboBox::from_id_salt("device_preset")
                            .selected_text("Choose your phone")
                            .show_ui(ui, |ui| {
                                for brand in presets::brands() {
                                    ui.label(RichText::new(brand).strong());
                                    for device in presets::DEVICES.iter().filter(|d| d.brand == brand) {
                                        let hover = format!(
                                            "About {} dpi. Check with: adb shell wm density",
                                            device.dpi
                                        );
                                        if ui.selectable_label(false, device.model).on_hover_text(hover).clicked() {
                                            self.settings.abi = Some(device.abi.to_owned());
                                            self.settings.density = Some(device.density().to_owned());
                                        }
                                    }
                                    ui.separator();
                                }
                            });
                        hint(ui, "Sets architecture and density. You can still change them below.");
                    });
                    ui.add_space(4.0);
                    ui.label("Architecture");
                    ui.horizontal_wrapped(|ui| {
                        for (value, text) in ABI_CHOICES {
                            let hover = match value {
                                None => "Keep every ABI so Android picks the right native libraries".to_owned(),
                                Some(abi) => format!("Keep the {abi} native libraries"),
                            };
                            ui.selectable_value(
                                &mut self.settings.abi,
                                value.map(str::to_owned),
                                text,
                            )
                            .on_hover_text(hover);
                        }
                    });
                    ui.add_space(4.0);
                    ui.label("Screen density");
                    ui.horizontal_wrapped(|ui| {
                        for (value, text) in DENSITY_CHOICES {
                            let hover = match value {
                                None => "Use xxhdpi, the closest match to a desktop display".to_owned(),
                                Some("nodpi") => "Only density-independent resources, skip per-density splits".to_owned(),
                                Some(density) => format!("Keep the {density} resources"),
                            };
                            ui.selectable_value(
                                &mut self.settings.density,
                                value.map(str::to_owned),
                                text,
                            )
                            .on_hover_text(hover);
                        }
                    });
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label("Language");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.settings.language)
                                .hint_text("auto, from your locale")
                                .desired_width(180.0),
                        )
                        .on_hover_text("A language code such as en, de or pt_BR. Leave empty to follow your locale.");
                    });

                    ui.add_space(8.0);
                    ui.label(RichText::new("Output").strong());
                    ui.checkbox(&mut self.settings.include_features, "Include dynamic feature splits")
                        .on_hover_text("Dynamic feature modules are off by default. Turn this on to fold them into the APK.");
                    ui.checkbox(&mut self.settings.keep_intermediates, "Keep intermediate files")
                        .on_hover_text("Leave the temporary workspace on disk after the app closes. Useful for debugging.");
                    ui.checkbox(&mut self.settings.verbose, "Verbose logging")
                        .on_hover_text("Show debug messages in the console, including the full output of uber-apk-signer.");

                    ui.add_space(10.0);
                    ui.separator();
                    ui.add_space(4.0);
                    ui.label(RichText::new("Tools").strong());
                    hint(
                        ui,
                        "Leave a field empty to detect the tool automatically. Paths are remembered between sessions.",
                    );
                    ui.add_space(4.0);
                    egui::Grid::new("tools_grid")
                        .num_columns(3)
                        .spacing([14.0, 8.0])
                        .show(ui, |ui| {
                            tool_row(
                                ui,
                                "Java",
                                &mut self.settings.java_path,
                                self.detected.java.as_deref(),
                                "The java runtime that runs uber-apk-signer",
                                || pick_file("Choose the java executable", None),
                            );
                            tool_row(
                                ui,
                                "apksigner",
                                &mut self.settings.apksigner_path,
                                self.detected.apksigner.as_deref(),
                                "Google's signer from the Android build-tools",
                                || pick_file("Choose apksigner", None),
                            );
                            tool_row(
                                ui,
                                "zipalign",
                                &mut self.settings.zipalign_path,
                                self.detected.zipalign.as_deref(),
                                "Aligns stored entries. Optional but recommended.",
                                || pick_file("Choose zipalign", None),
                            );
                            tool_row(
                                ui,
                                "uber-apk-signer",
                                &mut self.settings.uber_path,
                                self.detected.uber_jar.as_deref(),
                                "The uber-apk-signer .jar file",
                                || {
                                    pick_file(
                                        "Choose uber-apk-signer.jar",
                                        Some(("Java archive", &["jar"][..])),
                                    )
                                },
                            );
                        });
                    ui.add_space(4.0);
                    if ui
                        .button("Re-detect tools")
                        .on_hover_text("Search the system again for every tool")
                        .clicked()
                    {
                        self.recheck_tools();
                    }

                    ui.add_space(10.0);
                    if ui
                        .button("Reset target and output")
                        .on_hover_text("Restore architecture, density, language and output options to their defaults. Tool paths and signing are kept.")
                        .clicked()
                    {
                        self.settings.reset_target_and_output();
                        self.selection_dirty = self.prepared.is_some();
                    }
                });
            });
    }
}
