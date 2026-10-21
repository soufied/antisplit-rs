use eframe::egui::{self, RichText};

use crate::build::sign::SignerKind;
use crate::ui::app::{App, Phase};
use crate::ui::theme::{accent, caution};
use crate::ui::widgets::{card, hint, notice, open_folder};

impl App {
    pub(crate) fn merge_card(&mut self, ui: &mut egui::Ui) {
        let ready = self.phase == Phase::Ready && self.prepared.is_some() && !self.selection_dirty;
        let merging = self.phase == Phase::Merging;
        let progress = self.progress;
        let step = self.step.clone();
        let dirty = self.selection_dirty;
        let signing = self.settings.sign;
        let last_output = self.last_output.clone();
        let problem = self.settings.merge_options().err();
        let zipalign_missing = self.settings.align && !self.zipalign_available();
        let output_line = self.output_summary();
        let signing_title = self.signing_title();

        let hover = if dirty {
            "Apply the split selection first".to_owned()
        } else if let Some(message) = &problem {
            message.clone()
        } else if signing {
            "Write, align, sign, verify and save one APK".to_owned()
        } else {
            "Write and verify one APK. It will not be signed.".to_owned()
        };

        let mut merge_clicked = false;
        let mut open_clicked = false;
        card(ui, "3 - Merge", |ui| {
            ui.checkbox(&mut self.settings.align, "Align entries with zipalign")
                .on_hover_text("Pads stored entries so Android can map them directly. Needs zipalign.");
            if zipalign_missing {
                notice(
                    ui,
                    caution(),
                    "zipalign was not found, so the APK will not be aligned. Set its path under Advanced settings.",
                );
            }
            ui.add_space(4.0);
            egui::CollapsingHeader::new(RichText::new(format!("Signing: {signing_title}")).strong())
                .id_salt("signing_section")
                .default_open(signing)
                .show(ui, |ui| self.signing_section(ui));

            ui.add_space(8.0);
            hint(ui, output_line);
            ui.add_space(6.0);

            let button = egui::Button::new(RichText::new("Merge into APK...").strong())
                .min_size(egui::vec2(220.0, 36.0))
                .fill(accent());
            if ui
                .add_enabled(ready && problem.is_none(), button)
                .on_hover_text(hover)
                .clicked()
            {
                merge_clicked = true;
            }
            if let Some(message) = &problem {
                ui.add_space(4.0);
                notice(ui, caution(), message.clone());
            }

            if merging {
                ui.add_space(6.0);
                ui.add(
                    egui::ProgressBar::new(progress)
                        .text(format!("{step} - {:.0}%", progress * 100.0)),
                );
            }
            ui.add_space(6.0);
            let note = if signing {
                "Writes one APK, aligns it when enabled, signs it and verifies the result."
            } else {
                "Writes one APK and verifies it. The APK is unsigned, so turn on signing above to install it."
            };
            hint(ui, note);

            if let Some(path) = &last_output {
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(6.0);
                hint(ui, "Last output");
                ui.label(RichText::new(path.display().to_string()).monospace());
                if ui
                    .button("Open folder")
                    .on_hover_text("Show the APK in your file manager")
                    .clicked()
                {
                    open_clicked = true;
                }
            }
        });

        if merge_clicked {
            self.merge();
        }
        if open_clicked {
            if let Some(path) = &self.last_output {
                open_folder(path);
            }
        }
    }

    fn signing_title(&self) -> String {
        if !self.settings.sign {
            return "off".to_owned();
        }
        signer_label(self.settings.signer, self.settings.debug_key).to_owned()
    }

    fn output_summary(&self) -> String {
        let signed = if self.settings.sign {
            format!(
                "signed with {}",
                signer_label(self.settings.signer, self.settings.debug_key)
            )
        } else {
            "unsigned".to_owned()
        };
        let aligned = match (self.settings.align, self.zipalign_available()) {
            (false, _) => "not aligned",
            (true, true) => "aligned",
            (true, false) => "not aligned (no zipalign)",
        };
        format!("Output: {signed}, {aligned}.")
    }
}

fn signer_label(kind: SignerKind, debug_key: bool) -> &'static str {
    match (kind, debug_key) {
        (SignerKind::Uber, true) => "uber-apk-signer (debug key)",
        (SignerKind::Uber, false) => "uber-apk-signer",
        (SignerKind::Apksigner, _) => "apksigner",
    }
}
