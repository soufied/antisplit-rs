use eframe::egui::Color32;

use crate::logging::LogLevel;
use crate::pipeline::classify::SplitKind;

use super::app::Phase;

pub(crate) fn accent() -> Color32 {
    Color32::from_rgb(92, 140, 255)
}

pub(crate) fn good() -> Color32 {
    Color32::from_rgb(110, 220, 140)
}

pub(crate) fn caution() -> Color32 {
    Color32::from_rgb(240, 190, 80)
}

pub(crate) fn danger() -> Color32 {
    Color32::from_rgb(240, 110, 110)
}

pub(crate) fn muted() -> Color32 {
    Color32::from_rgb(140, 145, 160)
}

pub(crate) fn info() -> Color32 {
    Color32::from_rgb(90, 160, 255)
}

pub(crate) fn drop_zone_hover() -> Color32 {
    Color32::from_rgb(40, 58, 96)
}

pub(crate) fn drop_zone() -> Color32 {
    Color32::from_rgb(28, 31, 40)
}

pub(crate) fn error_background() -> Color32 {
    Color32::from_rgb(72, 30, 36)
}

pub(crate) fn phase_color(phase: Phase) -> Color32 {
    match phase {
        Phase::Idle => muted(),
        Phase::Analyzing => info(),
        Phase::Preparing => caution(),
        Phase::Merging => Color32::from_rgb(180, 140, 255),
        Phase::Ready => good(),
        Phase::Failed => danger(),
    }
}

pub(crate) fn level_color(level: LogLevel) -> Color32 {
    match level {
        LogLevel::Debug => Color32::from_rgb(120, 120, 135),
        LogLevel::Info => Color32::from_rgb(200, 200, 210),
        LogLevel::Success => good(),
        LogLevel::Warn => caution(),
        LogLevel::Error => danger(),
    }
}

pub(crate) fn kind_color(kind: &SplitKind) -> Color32 {
    match kind {
        SplitKind::Base => good(),
        SplitKind::Abi(_) => info(),
        SplitKind::Density(_) => Color32::from_rgb(190, 140, 255),
        SplitKind::Language(_) => caution(),
        SplitKind::Feature(_) => Color32::from_rgb(80, 210, 220),
        SplitKind::Unrecognized(_) => danger(),
    }
}
