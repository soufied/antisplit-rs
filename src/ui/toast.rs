use std::time::{Duration, Instant};

use eframe::egui::{self, Align2, Color32, Id, Order, RichText, Stroke};

use crate::logging::LogLevel;

use super::theme::level_color;

const LIFETIME: Duration = Duration::from_secs(4);
const FADE: f32 = 0.6;
const MAX_VISIBLE: usize = 4;

#[derive(Clone, Debug)]
struct Toast {
    level: LogLevel,
    text: String,
    created: Instant,
}

#[derive(Debug, Default)]
pub(crate) struct Toasts {
    items: Vec<Toast>,
}

impl Toasts {
    pub(crate) fn push(&mut self, level: LogLevel, text: impl Into<String>) {
        self.items.push(Toast {
            level,
            text: text.into(),
            created: Instant::now(),
        });
        if self.items.len() > MAX_VISIBLE {
            let excess = self.items.len() - MAX_VISIBLE;
            self.items.drain(..excess);
        }
    }

    pub(crate) fn show(&mut self, ctx: &egui::Context) {
        self.items.retain(|toast| toast.created.elapsed() < LIFETIME);
        if self.items.is_empty() {
            return;
        }
        ctx.request_repaint_after(Duration::from_millis(100));
        let items = &self.items;
        egui::Area::new(Id::new("toast_stack"))
            .anchor(Align2::RIGHT_BOTTOM, egui::vec2(-16.0, -16.0))
            .order(Order::Foreground)
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    for toast in items {
                        draw(ui, toast);
                        ui.add_space(6.0);
                    }
                });
            });
    }
}

fn alpha(toast: &Toast) -> f32 {
    let remaining = (LIFETIME.as_secs_f32() - toast.created.elapsed().as_secs_f32()).max(0.0);
    (remaining / FADE).clamp(0.0, 1.0)
}

fn draw(ui: &mut egui::Ui, toast: &Toast) {
    let factor = alpha(toast);
    let accent = level_color(toast.level).gamma_multiply(factor);
    egui::Frame::popup(ui.style())
        .fill(Color32::from_rgb(34, 38, 48).gamma_multiply(factor))
        .stroke(Stroke::new(1.0_f32, accent))
        .show(ui, |ui| {
            ui.label(RichText::new(toast.text.as_str()).color(accent));
        });
}
