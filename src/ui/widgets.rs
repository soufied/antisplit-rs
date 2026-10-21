use std::path::{Path, PathBuf};
use std::process::Command;

use eframe::egui::{self, Color32, RichText};

use super::theme::{caution, danger, good, muted};

pub(crate) fn card(ui: &mut egui::Ui, title: &str, add_contents: impl FnOnce(&mut egui::Ui)) {
    ui.group(|ui| {
        ui.label(RichText::new(title).size(15.0).strong());
        ui.add_space(6.0);
        add_contents(ui);
    });
}

pub(crate) fn hint(ui: &mut egui::Ui, text: impl Into<String>) {
    ui.label(RichText::new(text).weak());
}

pub(crate) fn notice(ui: &mut egui::Ui, color: Color32, text: impl Into<String>) {
    ui.label(RichText::new(text).color(color));
}

pub(crate) fn step_chip(ui: &mut egui::Ui, label: &str, done: bool) {
    let color = if done { good() } else { muted() };
    ui.label(RichText::new(label).color(color).strong());
}

pub(crate) fn dot(ui: &mut egui::Ui, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 14.0), egui::Sense::hover());
    ui.painter().circle_filled(rect.center(), 4.0, color);
}

pub(crate) fn status_dot(ui: &mut egui::Ui, color: Color32, text: String) {
    ui.horizontal(|ui| {
        dot(ui, color);
        ui.label(RichText::new(text).weak());
    });
}

pub(crate) fn file_status(ui: &mut egui::Ui, path: &str) {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        status_dot(ui, caution(), "required".to_owned());
    } else if Path::new(trimmed).is_file() {
        status_dot(ui, good(), "found".to_owned());
    } else {
        status_dot(ui, danger(), "file not found".to_owned());
    }
}

pub(crate) fn pick_file(title: &str, filter: Option<(&str, &[&str])>) -> Option<PathBuf> {
    let mut dialog = rfd::FileDialog::new().set_title(title);
    if let Some((name, extensions)) = filter {
        dialog = dialog.add_filter(name, extensions);
    }
    dialog.pick_file()
}

pub(crate) fn tool_row(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut String,
    detected: Option<&Path>,
    hint_text: &str,
    browse: impl FnOnce() -> Option<PathBuf>,
) {
    ui.label(label).on_hover_text(hint_text);
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut *value)
                .hint_text("auto-detect")
                .desired_width(320.0),
        )
        .on_hover_text(hint_text);
        if ui
            .button("Browse...")
            .on_hover_text("Pick the file from disk")
            .clicked()
        {
            if let Some(path) = browse() {
                *value = path.display().to_string();
            }
        }
        if ui
            .small_button("Auto")
            .on_hover_text("Forget the custom path and detect it automatically")
            .clicked()
        {
            value.clear();
        }
    });
    let trimmed = value.trim().to_owned();
    if !trimmed.is_empty() {
        if Path::new(&trimmed).is_file() {
            status_dot(ui, good(), format!("custom: {trimmed}"));
        } else {
            status_dot(ui, danger(), "custom path not found".to_owned());
        }
    } else if let Some(path) = detected {
        status_dot(ui, good(), format!("auto: {}", path.display()));
    } else {
        status_dot(ui, danger(), "not found: set a path or install it".to_owned());
    }
    ui.end_row();
}

pub(crate) fn open_folder(path: &Path) {
    if let Some(directory) = path.parent() {
        Command::new("xdg-open").arg(directory).spawn().ok();
    }
}
