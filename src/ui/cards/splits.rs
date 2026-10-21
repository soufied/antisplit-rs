use eframe::egui::{self, RichText};

use crate::pipeline::classify::SplitKind;
use crate::ui::app::{App, Phase};
use crate::ui::theme::{accent, caution, kind_color};
use crate::ui::widgets::{card, hint};
use crate::util::human_size;

pub(crate) struct Row {
    pub(crate) entry: String,
    pub(crate) kind: SplitKind,
    pub(crate) size: u64,
    pub(crate) include: bool,
    pub(crate) reason: String,
    pub(crate) base: bool,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Bulk {
    All,
    Device,
    None,
}

impl App {
    pub(crate) fn apply_bulk(&mut self, action: Bulk, rows: &[Row]) {
        match action {
            Bulk::Device => self.settings.split_choices.clear(),
            Bulk::All => {
                for row in rows.iter().filter(|row| !row.base) {
                    self.settings.split_choices.insert(row.entry.clone(), true);
                }
            }
            Bulk::None => {
                for row in rows.iter().filter(|row| !row.base) {
                    self.settings.split_choices.insert(row.entry.clone(), false);
                }
            }
        }
        self.selection_dirty = true;
    }

    pub(crate) fn split_rows(&self) -> Vec<Row> {
        let Some(bundle) = &self.bundle else {
            return Vec::new();
        };
        let prepared = self.prepared.as_deref();
        bundle
            .splits
            .iter()
            .map(|split| {
                let decision = prepared.and_then(|plan| plan.decision_for(&split.entry));
                let base = split.kind.is_base();
                let automatic = decision.is_some_and(|decision| decision.include);
                let include = if base {
                    true
                } else {
                    self.settings
                        .split_choices
                        .get(&split.entry)
                        .copied()
                        .unwrap_or(automatic)
                };
                let reason = match decision {
                    Some(decision) => decision.reason.clone(),
                    None => "run Prepare to see the automatic choice".to_owned(),
                };
                Row {
                    entry: split.entry.clone(),
                    kind: split.kind.clone(),
                    size: split.size,
                    include,
                    reason,
                    base,
                }
            })
            .collect()
    }

    pub(crate) fn splits_card(&mut self, ui: &mut egui::Ui) {
        let busy = self.busy();
        let ready = self.phase == Phase::Ready && self.bundle.is_some();
        let preparing = self.phase == Phase::Preparing;
        let dirty = self.selection_dirty;
        let progress = self.progress;
        let step = self.step.clone();
        let rows = self.split_rows();
        let total = rows.len();
        let selected = rows.iter().filter(|row| row.include).count();
        let selected_bytes: u64 = rows.iter().filter(|row| row.include).map(|row| row.size).sum();
        let (format_label, metadata_label) = match &self.bundle {
            Some(bundle) => (bundle.kind.label(), bundle.manifest.label()),
            None => ("", ""),
        };
        let mut toggles: Vec<(String, bool)> = Vec::new();
        let mut bulk: Option<Bulk> = None;
        let mut prepare_clicked = false;

        card(ui, "2 - Splits", |ui| {
            if self.bundle.is_none() {
                hint(ui, "Choose a bundle in step 1 to see its splits here.");
                return;
            }
            ui.horizontal(|ui| {
                hint(ui, "Format");
                ui.label(RichText::new(format_label).strong());
                ui.add_space(14.0);
                hint(ui, "Metadata");
                ui.label(metadata_label);
            });
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(!busy, egui::Button::new("Select all"))
                    .on_hover_text("Include every split, including dynamic features")
                    .clicked()
                {
                    bulk = Some(Bulk::All);
                }
                if ui
                    .add_enabled(!busy, egui::Button::new("Device match"))
                    .on_hover_text("Go back to the automatic choice for this computer")
                    .clicked()
                {
                    bulk = Some(Bulk::Device);
                }
                if ui
                    .add_enabled(!busy, egui::Button::new("Deselect all"))
                    .on_hover_text("Keep only the base module")
                    .clicked()
                {
                    bulk = Some(Bulk::None);
                }
                ui.add_space(10.0);
                ui.label(
                    RichText::new(format!(
                        "{selected} of {total} selected, {}",
                        human_size(selected_bytes)
                    ))
                    .strong(),
                );
                if dirty {
                    ui.label(RichText::new("- unapplied changes").color(caution()));
                }
            });
            ui.add_space(8.0);
            egui::Grid::new("split_grid")
                .num_columns(4)
                .striped(true)
                .spacing([18.0, 6.0])
                .show(ui, |ui| {
                    ui.label(RichText::new("Use").strong());
                    ui.label(RichText::new("Kind").strong());
                    ui.label(RichText::new("Size").strong());
                    ui.label(RichText::new("Entry").strong());
                    ui.end_row();
                    for row in &rows {
                        let mut value = row.include;
                        let response = ui
                            .add_enabled(!row.base, egui::Checkbox::new(&mut value, ""))
                            .on_hover_text(row.reason.as_str());
                        if response.changed() {
                            toggles.push((row.entry.clone(), value));
                        }
                        ui.colored_label(kind_color(&row.kind), row.kind.label());
                        ui.label(human_size(row.size));
                        ui.label(RichText::new(row.entry.as_str()).monospace());
                        ui.end_row();
                    }
                });
            ui.add_space(10.0);
            ui.separator();
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let label = if dirty {
                    "Apply selection and prepare"
                } else {
                    "Prepare"
                };
                let button = egui::Button::new(RichText::new(label).strong())
                    .min_size(egui::vec2(230.0, 34.0))
                    .fill(accent());
                let hover = if dirty {
                    "Re-run the analysis with your selection"
                } else {
                    "Unpack, select, validate and consolidate the splits"
                };
                if ui
                    .add_enabled(ready && !busy, button)
                    .on_hover_text(hover)
                    .clicked()
                {
                    prepare_clicked = true;
                }
            });
            if preparing {
                ui.add_space(6.0);
                ui.add(
                    egui::ProgressBar::new(progress)
                        .text(format!("{step} - {:.0}%", progress * 100.0)),
                );
            }
            ui.add_space(4.0);
            hint(
                ui,
                "Toggle splits here, then prepare. Preparing unpacks the bundle into a temporary workspace and validates every split manifest. No APK is written yet.",
            );
        });

        if let Some(action) = bulk {
            self.apply_bulk(action, &rows);
        }
        for (entry, value) in toggles {
            self.settings.split_choices.insert(entry, value);
            self.selection_dirty = true;
        }
        if prepare_clicked {
            self.prepare();
        }
    }
}
