use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError};

use eframe::egui::{self, Color32};

use crate::build::MergeSummary;
use crate::build::sign;
use crate::logging::{LogLevel, LogLine};
use crate::pipeline::PreparedPlan;
use crate::pipeline::bundle::Bundle;
use crate::settings::Settings;
use crate::settings::store;
use crate::util::human_size;
use crate::worker::{self, WorkerEvent};

use super::theme::accent;
use super::toast::Toasts;

pub(crate) const MAX_LOG_LINES: usize = 5000;
pub(crate) const BUNDLE_EXTENSIONS: [&str; 5] = ["apkm", "apks", "xapk", "apkx", "zip"];
pub(crate) const KEYSTORE_EXTENSIONS: [&str; 4] = ["jks", "p12", "pfx", "keystore"];

pub(crate) fn has_extension(path: &Path, extensions: &[&str]) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .is_some_and(|extension| extensions.contains(&extension.as_str()))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Phase {
    Idle,
    Analyzing,
    Preparing,
    Merging,
    Ready,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Stage {
    Analysis,
    Preparation,
    Merge,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Detected {
    pub(crate) java: Option<PathBuf>,
    pub(crate) apksigner: Option<PathBuf>,
    pub(crate) zipalign: Option<PathBuf>,
    pub(crate) uber_jar: Option<PathBuf>,
}

pub(crate) struct App {
    pub(crate) selected: Option<PathBuf>,
    pub(crate) bundle: Option<Bundle>,
    pub(crate) prepared: Option<Arc<PreparedPlan>>,
    pub(crate) settings: Settings,
    pub(crate) phase: Phase,
    pub(crate) status: String,
    pub(crate) progress: f32,
    pub(crate) step: String,
    pub(crate) log: Vec<LogLine>,
    pub(crate) show_log: bool,
    pub(crate) receiver: Option<Receiver<WorkerEvent>>,

    pub(crate) selection_dirty: bool,
    pub(crate) merged: bool,
    pub(crate) last_output: Option<PathBuf>,

    pub(crate) last_error: Option<String>,
    pub(crate) detected: Detected,
    pub(crate) hovering: bool,
    pub(crate) last_saved: store::Values,
    pub(crate) toasts: Toasts,
    pub(crate) failed_stage: Option<Stage>,
    pub(crate) pending_replace: Option<PathBuf>,
}

impl App {
    pub(crate) fn new(creation: &eframe::CreationContext<'_>) -> Self {
        let mut visuals = egui::Visuals::dark();
        visuals.selection.bg_fill = accent();
        visuals.hyperlink_color = accent();
        visuals.panel_fill = Color32::from_rgb(20, 22, 28);
        visuals.window_fill = Color32::from_rgb(26, 29, 36);
        creation.egui_ctx.set_visuals(visuals);
        creation.egui_ctx.style_mut(|style| {
            style.spacing.item_spacing = egui::vec2(10.0, 8.0);
            style.spacing.button_padding = egui::vec2(12.0, 6.0);
        });
        let saved = store::load();
        let mut settings = Settings::default();
        settings.restore(&saved);
        let detected = Self::detect();
        if !saved.contains_key("signer")
            && detected.apksigner.is_none()
            && detected.uber_jar.is_some()
        {
            settings.signer = sign::SignerKind::Uber;
        }
        let last_saved = settings.persisted();
        let mut app = Self {
            selected: None,
            bundle: None,
            prepared: None,
            settings,
            phase: Phase::Idle,
            status: "Idle".to_owned(),
            progress: 0.0,
            step: String::new(),
            log: Vec::new(),
            show_log: true,
            receiver: None,
            selection_dirty: false,
            merged: false,
            last_output: None,
            last_error: None,
            detected,
            hovering: false,
            last_saved,
            toasts: Toasts::default(),
            failed_stage: None,
            pending_replace: None,
        };
        app.note(
            LogLevel::Info,
            "antisplit-rs ready. Choose a split bundle to begin.",
        );
        app
    }

    fn detect() -> Detected {
        Detected {
            java: sign::detect_java(),
            apksigner: sign::detect_executable("apksigner"),
            zipalign: sign::detect_executable("zipalign"),
            uber_jar: sign::detect_uber_jar(),
        }
    }

    pub(crate) fn recheck_tools(&mut self) {
        self.detected = Self::detect();
        self.note(LogLevel::Info, "Tool detection refreshed");
    }

    pub(crate) fn zipalign_available(&self) -> bool {
        match non_empty_path(&self.settings.zipalign_path) {
            Some(path) => path.is_file(),
            None => self.detected.zipalign.is_some(),
        }
    }

    pub(crate) fn busy(&self) -> bool {
        matches!(
            self.phase,
            Phase::Analyzing | Phase::Preparing | Phase::Merging
        )
    }

    fn push_line(&mut self, line: LogLine) {
        self.log.push(line);
        if self.log.len() > MAX_LOG_LINES {
            let excess = self.log.len() - MAX_LOG_LINES;
            self.log.drain(..excess);
        }
    }

    pub(crate) fn note(&mut self, level: LogLevel, text: impl Into<String>) {
        self.push_line(LogLine::new(level, text));
    }

    pub(crate) fn path_text(&self) -> String {
        self.selected.as_ref().map_or_else(
            || "No bundle selected".to_owned(),
            |path| path.display().to_string(),
        )
    }

    pub(crate) fn choose_file(&mut self) {
        if let Some(path) = super::widgets::pick_file(
            "Choose a split APK bundle",
            Some(("Split APK bundles", &BUNDLE_EXTENSIONS)),
        ) {
            self.request_bundle(path);
        }
    }

    pub(crate) fn pick_keystore(&mut self) {
        if let Some(path) = super::widgets::pick_file(
            "Choose a keystore",
            Some(("Keystores", &KEYSTORE_EXTENSIONS)),
        ) {
            self.settings.keystore = path.display().to_string();
        }
    }

    fn handle_drops(&mut self, ctx: &egui::Context) {
        let dropped: Vec<PathBuf> = ctx.input(|input| {
            input
                .raw
                .dropped_files
                .iter()
                .filter_map(|file| file.path.clone())
                .collect()
        });
        for path in dropped {
            if has_extension(&path, &BUNDLE_EXTENSIONS) {
                if self.busy() {
                    self.note(LogLevel::Warn, "Finish the current step before dropping a new bundle");
                } else {
                    self.request_bundle(path);
                }
            } else if has_extension(&path, &KEYSTORE_EXTENSIONS) {
                self.settings.sign = true;
                self.settings.keystore = path.display().to_string();
                self.note(LogLevel::Info, format!("Keystore set to {}", path.display()));
            } else if has_extension(&path, &["jar"]) {
                self.settings.uber_path = path.display().to_string();
                self.note(LogLevel::Info, format!("uber-apk-signer set to {}", path.display()));
            } else {
                self.note(LogLevel::Warn, format!("Ignored {}", path.display()));
            }
        }
    }

    pub(crate) fn begin(&mut self, path: PathBuf) {
        let name = path.file_name().map_or_else(
            || path.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        );
        self.note(LogLevel::Info, format!("Analysing {name}"));
        self.status = format!("Analysing {name}");
        self.phase = Phase::Analyzing;
        self.bundle = None;
        self.prepared = None;
        self.merged = false;
        self.selection_dirty = false;
        self.last_output = None;
        self.last_error = None;
        self.settings.split_choices.clear();
        self.progress = 0.0;
        self.step.clear();
        self.receiver = Some(worker::spawn_analysis(path.clone()));
        self.selected = Some(path);
    }

    fn request_bundle(&mut self, path: PathBuf) {
        if self.busy() {
            self.note(LogLevel::Warn, "Finish the current step before choosing a new bundle");
            return;
        }
        if self.prepared.is_some() || self.merged {
            self.pending_replace = Some(path);
        } else {
            self.begin(path);
        }
    }

    pub(crate) fn retry(&mut self) {
        match self.failed_stage.take() {
            Some(Stage::Analysis) => {
                if let Some(path) = self.selected.clone() {
                    self.begin(path);
                }
            }
            Some(Stage::Preparation) => self.prepare(),
            Some(Stage::Merge) => self.merge(),
            None => {}
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        if self.pending_replace.is_some() {
            return;
        }
        let open = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::O);
        let prepare = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::P);
        let merge = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::M);
        let console = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::L);
        let (open_hit, prepare_hit, merge_hit, console_hit) = ctx.input_mut(|input| {
            (
                input.consume_shortcut(&open),
                input.consume_shortcut(&prepare),
                input.consume_shortcut(&merge),
                input.consume_shortcut(&console),
            )
        });
        if open_hit && !self.busy() {
            self.choose_file();
        }
        let can_prepare = self.phase == Phase::Ready
            && self.bundle.as_ref().is_some_and(|bundle| bundle.has_base());
        if prepare_hit && can_prepare {
            self.prepare();
        }
        if merge_hit && self.phase == Phase::Ready && self.prepared.is_some() {
            self.merge();
        }
        if console_hit {
            self.show_log = !self.show_log;
        }
    }

    fn confirm_replace(&mut self, ctx: &egui::Context) {
        let Some(path) = self.pending_replace.clone() else {
            return;
        };
        let name = path.file_name().map_or_else(
            || path.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        );
        let mut decision = None;
        egui::Window::new("Replace the current bundle?")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.label("The prepared selection and any merge result for the current bundle will be discarded.");
                ui.add_space(4.0);
                ui.label(egui::RichText::new(name).monospace());
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        decision = Some(false);
                    }
                    if ui.button("Replace").clicked() {
                        decision = Some(true);
                    }
                });
            });
        if ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
            decision = Some(false);
        }
        match decision {
            Some(true) => {
                self.pending_replace = None;
                self.begin(path);
            }
            Some(false) => self.pending_replace = None,
            None => {}
        }
    }

    pub(crate) fn prepare(&mut self) {
        let Some(input) = self.selected.clone() else {
            return;
        };
        let options = self.settings.target_options();
        self.prepared = None;
        self.merged = false;
        self.selection_dirty = false;
        self.last_error = None;
        self.note(LogLevel::Info, "Preparing workspace and standalone manifest");
        self.status = "Preparing".to_owned();
        self.phase = Phase::Preparing;
        self.progress = 0.0;
        self.step = "Starting".to_owned();
        self.receiver = Some(worker::spawn_prepare(input, options));
    }

    pub(crate) fn merge(&mut self) {
        let Some(plan) = self.prepared.clone() else {
            return;
        };
        if self.selection_dirty {
            self.note(LogLevel::Warn, "Apply the split selection before merging");
            return;
        }
        let options = match self.settings.merge_options() {
            Ok(options) => options,
            Err(message) => {
                self.fail_merge(message);
                return;
            }
        };
        let package = plan
            .base
            .package
            .clone()
            .unwrap_or_else(|| "merged".to_owned());
        let file_name = format!("{package}.apk");
        let picked = rfd::FileDialog::new()
            .set_title("Save merged APK")
            .set_file_name(file_name.as_str())
            .add_filter("Android package", &["apk"])
            .save_file();
        let Some(output) = picked else {
            return;
        };
        self.note(LogLevel::Info, format!("Merging into {}", output.display()));
        self.status = "Merging".to_owned();
        self.phase = Phase::Merging;
        self.progress = 0.0;
        self.step = "Starting".to_owned();
        self.last_error = None;
        self.receiver = Some(worker::spawn_merge(plan, output, options));
    }

    fn poll(&mut self) {
        let Some(receiver) = self.receiver.take() else {
            return;
        };
        let mut finished = false;
        let mut disconnected = false;
        loop {
            match receiver.try_recv() {
                Ok(WorkerEvent::Log(line)) => self.push_line(line),
                Ok(WorkerEvent::Progress(update)) => {
                    self.progress = update.fraction;
                    self.step = update.step;
                }
                Ok(WorkerEvent::Analyzed(result)) => {
                    self.finish_analysis(result);
                    finished = true;
                    break;
                }
                Ok(WorkerEvent::Prepared(result)) => {
                    self.finish_prepare(result);
                    finished = true;
                    break;
                }
                Ok(WorkerEvent::Merged(result)) => {
                    self.finish_merge(result);
                    finished = true;
                    break;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    disconnected = true;
                    break;
                }
            }
        }
        if disconnected {
            if self.phase == Phase::Merging {
                self.fail_merge("worker stopped unexpectedly".to_owned());
            } else if self.phase == Phase::Preparing {
                self.fail_prepare("worker stopped unexpectedly".to_owned());
            } else {
                self.fail_analysis("worker stopped unexpectedly".to_owned());
            }
        } else if !finished {
            self.receiver = Some(receiver);
        }
    }

    fn finish_analysis(&mut self, outcome: std::result::Result<Bundle, String>) {
        match outcome {
            Ok(bundle) => {
                self.status = format!(
                    "{} - {} split(s) detected",
                    bundle.kind.label(),
                    bundle.splits.len()
                );
                self.phase = Phase::Ready;
                self.bundle = Some(bundle);
            }
            Err(message) => self.fail_analysis(message),
        }
    }

    fn finish_prepare(&mut self, outcome: std::result::Result<PreparedPlan, String>) {
        match outcome {
            Ok(plan) => {
                let selected = plan.selected();
                let total = plan.decisions.len();
                self.status = format!("Prepared - {selected} of {total} split(s) selected");
                self.note(
                    LogLevel::Success,
                    format!("Preparation complete: {selected} of {total} split(s) selected"),
                );
                self.progress = 1.0;
                self.step = "Ready".to_owned();
                self.toasts.push(
                    LogLevel::Success,
                    format!("Prepared: {selected} of {total} split(s) selected"),
                );
                self.prepared = Some(Arc::new(plan));
                self.phase = Phase::Ready;
            }
            Err(message) => self.fail_prepare(message),
        }
    }

    fn finish_merge(&mut self, outcome: std::result::Result<MergeSummary, String>) {
        match outcome {
            Ok(summary) => {
                self.status = format!("Merged - {}", summary.output.display());
                self.note(
                    LogLevel::Success,
                    format!(
                        "APK ready: {} ({}, {} entries, {} dex)",
                        summary.output.display(),
                        human_size(summary.size),
                        summary.entries,
                        summary.dex
                    ),
                );
                self.progress = 1.0;
                self.step = "Merged".to_owned();
                self.merged = true;
                self.toasts.push(LogLevel::Success, format!("Saved {}", summary.output.display()));
                self.last_output = Some(summary.output);
                self.phase = Phase::Ready;
            }
            Err(message) => self.fail_merge(message),
        }
    }

    fn fail_analysis(&mut self, message: String) {
        self.note(LogLevel::Error, format!("Analysis failed: {message}"));
        self.last_error = Some(format!("Analysis failed: {message}"));
        self.failed_stage = Some(Stage::Analysis);
        self.toasts.push(LogLevel::Error, format!("Analysis failed: {message}"));
        self.status = "Analysis failed".to_owned();
        self.phase = Phase::Failed;
    }

    fn fail_prepare(&mut self, message: String) {
        self.note(LogLevel::Error, format!("Preparation failed: {message}"));
        self.last_error = Some(format!("Preparation failed: {message}"));
        self.failed_stage = Some(Stage::Preparation);
        self.toasts.push(LogLevel::Error, format!("Preparation failed: {message}"));
        self.status = "Preparation failed".to_owned();
        self.progress = 0.0;
        self.step = "Failed".to_owned();
        self.phase = Phase::Ready;
    }

    fn fail_merge(&mut self, message: String) {
        self.note(LogLevel::Error, format!("Merge failed: {message}"));
        self.last_error = Some(format!("Merge failed: {message}"));
        self.failed_stage = Some(Stage::Merge);
        self.toasts.push(LogLevel::Error, format!("Merge failed: {message}"));
        self.status = "Merge failed".to_owned();
        self.progress = 0.0;
        self.step = "Failed".to_owned();
        self.phase = Phase::Ready;
    }
}

fn non_empty_path(text: &str) -> Option<PathBuf> {
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| PathBuf::from(trimmed))
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll();
        self.hovering = ctx.input(|input| !input.raw.hovered_files.is_empty());
        self.handle_drops(ctx);
        self.shortcuts(ctx);
        let current = self.settings.persisted();
        if current != self.last_saved {
            if let Err(error) = store::save(&current) {
                self.note(LogLevel::Warn, format!("Could not save settings: {error}"));
            }
            self.last_saved = current;
        }
        if self.busy() {
            ctx.request_repaint();
        }
        egui::TopBottomPanel::top("header").show(ctx, |ui| self.header(ui));
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| self.status_bar(ui));
        if self.show_log {
            egui::TopBottomPanel::bottom("console")
                .resizable(true)
                .default_height(200.0)
                .min_height(120.0)
                .show(ctx, |ui| self.console(ui));
        }
        egui::CentralPanel::default().show(ctx, |ui| self.body(ui));
        self.confirm_replace(ctx);
        self.toasts.show(ctx);
    }
}

impl App {
    fn body(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(4.0);
                self.error_banner(ui);
                self.input_card(ui);
                ui.add_space(8.0);
                self.advanced_card(ui);
                ui.add_space(8.0);
                self.splits_card(ui);
                ui.add_space(8.0);
                self.target_card(ui);
                ui.add_space(8.0);
                self.merge_card(ui);
                ui.add_space(8.0);
            });
    }
}
