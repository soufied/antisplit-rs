use eframe::egui::{self, RichText};

use crate::build::sign::SignerKind;
use crate::ui::app::App;
use crate::ui::theme::{caution, good};
use crate::ui::widgets::{file_status, hint};

impl App {
    pub(crate) fn signing_section(&mut self, ui: &mut egui::Ui) {
        ui.checkbox(&mut self.settings.sign, "Sign the output APK")
            .on_hover_text("Signing makes the APK installable. Without it the APK is unsigned.");
        if !self.settings.sign {
            hint(ui, "The APK will be unsigned and will not install until you sign it.");
            return;
        }

        ui.horizontal(|ui| {
            ui.label("Signer");
            ui.selectable_value(&mut self.settings.signer, SignerKind::Uber, "uber-apk-signer")
                .on_hover_text("Java signer. Can use a built-in debug key for testing.");
            ui.selectable_value(&mut self.settings.signer, SignerKind::Apksigner, "apksigner")
                .on_hover_text("Google's signer from the Android build-tools. Needs your own keystore.");
        });
        if self.settings.signer == SignerKind::Uber {
            ui.checkbox(&mut self.settings.debug_key, "Use the built-in debug key (testing only)")
                .on_hover_text("Signs with uber-apk-signer's debug certificate. Good for trying the APK, not for release.");
        }
        ui.add_space(4.0);

        if !self.settings.needs_keystore() {
            hint(ui, "No keystore needed: uber-apk-signer will sign with its built-in debug certificate.");
            return;
        }
        egui::Grid::new("signing_grid")
            .num_columns(3)
            .spacing([14.0, 10.0])
            .show(ui, |ui| {
                ui.label("Keystore");
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.settings.keystore)
                            .hint_text("path to a .jks, .p12 or .pfx file")
                            .desired_width(320.0),
                    )
                    .on_hover_text("Path to the keystore that signs the APK. You can also drag a keystore onto this window.");
                    if ui
                        .button("Browse...")
                        .on_hover_text("Pick the keystore file")
                        .clicked()
                    {
                        self.pick_keystore();
                    }
                });
                file_status(ui, &self.settings.keystore);
                ui.end_row();

                ui.label("Store password");
                ui.add(
                    egui::TextEdit::singleline(&mut self.settings.store_password)
                        .password(!self.settings.show_passwords)
                        .desired_width(220.0),
                )
                .on_hover_text("The password that opens the keystore. It is never saved to disk.");
                password_status(ui, &self.settings.store_password);
                ui.end_row();

                ui.label("Key alias");
                ui.add(
                    egui::TextEdit::singleline(&mut self.settings.key_alias)
                        .hint_text("first key in the store")
                        .desired_width(220.0),
                )
                .on_hover_text("Which key signs the APK. Leave empty to use the first key in the store.");
                ui.label("");
                ui.end_row();

                ui.label("Key password");
                ui.add(
                    egui::TextEdit::singleline(&mut self.settings.key_password)
                        .password(!self.settings.show_passwords)
                        .hint_text("same as store password if empty")
                        .desired_width(220.0),
                )
                .on_hover_text("The password for the key itself, if it differs from the store password");
                ui.label("");
                ui.end_row();
            });
        ui.add_space(4.0);
        ui.checkbox(&mut self.settings.show_passwords, "Show passwords")
            .on_hover_text("Reveal the password fields while you type");
    }
}

fn password_status(ui: &mut egui::Ui, password: &str) {
    if password.is_empty() {
        ui.label(RichText::new("required").color(caution()));
    } else {
        ui.label(RichText::new("entered").color(good()));
    }
}
