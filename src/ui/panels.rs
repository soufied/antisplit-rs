use eframe::egui::{self, RichText};

use crate::logging::LogLevel;

use super::app::App;
use super::theme::{accent, danger, error_background, level_color, phase_color};
use super::widgets::{dot, step_chip};

impl App {
    pub(crate) fn header(&self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("antisplit-rs")
                    .size(24.0)
                    .strong()
                    .color(accent()),
            );
            ui.label(RichText::new("Merge split APK bundles into one installable APK").weak());
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            step_chip(ui, "1  Bundle", self.bundle.is_some());
            ui.label(RichText::new(">").weak());
            step_chip(ui, "2  Prepare", self.prepared.is_some());
            ui.label(RichText::new(">").weak());
            step_chip(ui, "3  Merge", self.merged);
        });
        ui.add_space(6.0);
    }

    pub(crate) fn error_banner(&mut self, ui: &mut egui::Ui) {
        let Some(message) = self.last_error.clone() else {
            return;
        };
        let mut dismiss = false;
        let mut reveal_console = false;
        let mut retry = false;
        let can_retry = self.failed_stage.is_some() && !self.busy();
        egui::Frame::group(ui.style())
            .fill(error_background())
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Something went wrong").color(danger()).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if can_retry
                            && ui
                                .small_button("Retry")
                                .on_hover_text("Run the failed step again")
                                .clicked()
                        {
                            retry = true;
                        }
                        if ui
                            .small_button("Dismiss")
                            .on_hover_text("Hide this message")
                            .clicked()
                        {
                            dismiss = true;
                        }
                        if ui
                            .small_button("Copy error")
                            .on_hover_text("Copy the message to the clipboard, for a bug report")
                            .clicked()
                        {
                            ui.ctx().copy_text(message.clone());
                        }
                        if !self.show_log
                            && ui
                                .small_button("Show console")
                                .on_hover_text("Show the full log")
                                .clicked()
                        {
                            reveal_console = true;
                        }
                    });
                });
                ui.label(RichText::new(message.as_str()).monospace());
            });
        ui.add_space(8.0);
        if dismiss {
            self.last_error = None;
        }
        if reveal_console {
            self.show_log = true;
        }
        if retry {
            self.retry();
        }
    }

    pub(crate) fn status_bar(&mut self, ui: &mut egui::Ui) {
        let color = phase_color(self.phase);
        ui.horizontal(|ui| {
            dot(ui, color);
            ui.label(RichText::new(self.status.as_str()).color(color));
            if self.busy() {
                ui.spinner();
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let label = if self.show_log {
                    "Hide console"
                } else {
                    "Show console"
                };
                if ui
                    .button(label)
                    .on_hover_text("Show or hide the log console at the bottom")
                    .clicked()
                {
                    self.show_log = !self.show_log;
                }
            });
        });
    }

    pub(crate) fn console(&mut self, ui: &mut egui::Ui) {
        let verbose = self.settings.verbose;
        ui.horizontal(|ui| {
            ui.label(RichText::new("Console").strong());
            ui.label(RichText::new(format!("{} lines", self.log.len())).weak());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .small_button("Clear")
                    .on_hover_text("Erase the log")
                    .clicked()
                {
                    self.log.clear();
                }
                if ui
                    .small_button("Copy log")
                    .on_hover_text("Copy the whole log, including debug lines, to the clipboard")
                    .clicked()
                {
                    let text = self
                        .log
                        .iter()
                        .map(|line| format!("[{}] {}", level_tag(line.level), line.text))
                        .collect::<Vec<_>>()
                        .join("\n");
                    ui.ctx().copy_text(text);
                }
            });
        });
        ui.separator();
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for line in self
                    .log
                    .iter()
                    .filter(|line| line.level != LogLevel::Debug || verbose)
                {
                    ui.label(
                        RichText::new(format!("[{}] {}", level_tag(line.level), line.text))
                            .monospace()
                            .color(level_color(line.level)),
                    );
                }
            });
    }
}

pub(crate) fn level_tag(level: LogLevel) -> &'static str {
    match level {
        LogLevel::Debug => "DBUG",
        LogLevel::Info => "INFO",
        LogLevel::Success => " OK ",
        LogLevel::Warn => "WARN",
        LogLevel::Error => "ERR ",
    }
}
