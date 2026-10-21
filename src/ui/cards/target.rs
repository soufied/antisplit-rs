use eframe::egui::{self, RichText};

use crate::pipeline::consolidate::Category;
use crate::ui::app::App;
use crate::ui::theme::{caution, good};
use crate::ui::widgets::card;

impl App {
    pub(crate) fn target_card(&self, ui: &mut egui::Ui) {
        let Some(plan) = &self.prepared else {
            return;
        };
        card(ui, "Target", |ui| {
            egui::Grid::new("target_grid")
                .num_columns(2)
                .spacing([18.0, 6.0])
                .show(ui, |ui| {
                    ui.label(RichText::new("Architecture").weak());
                    ui.label(plan.target.abi.as_deref().unwrap_or("all"));
                    ui.end_row();
                    ui.label(RichText::new("Density").weak());
                    ui.label(plan.target.density.as_deref().unwrap_or("none"));
                    ui.end_row();
                    ui.label(RichText::new("Language").weak());
                    ui.label(plan.target.language.as_str());
                    ui.end_row();
                    ui.label(RichText::new("Package").weak());
                    ui.label(plan.base.package.as_deref().unwrap_or("unknown"));
                    ui.end_row();
                    ui.label(RichText::new("Version code").weak());
                    ui.label(plan.base.version_code.as_deref().unwrap_or("unknown"));
                    ui.end_row();
                    ui.label(RichText::new("Merged files").weak());
                    ui.label(format!(
                        "{} ({} dex, {} native, {} asset, {} resource)",
                        plan.consolidation.files.len(),
                        plan.consolidation.count(Category::Dex),
                        plan.consolidation.count(Category::Native),
                        plan.consolidation.count(Category::Asset),
                        plan.consolidation.count(Category::Resource)
                    ));
                    ui.end_row();
                    ui.label(RichText::new("Conflicts").weak());
                    let conflicts = plan.consolidation.conflicts.len();
                    let color = if conflicts == 0 { good() } else { caution() };
                    ui.label(RichText::new(conflicts.to_string()).color(color));
                    ui.end_row();
                });
            ui.add_space(6.0);
            for note in &plan.target.notes {
                ui.label(RichText::new(note.as_str()).weak());
            }
        });
    }
}
