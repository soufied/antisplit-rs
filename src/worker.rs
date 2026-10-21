use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use crate::pipeline::bundle::{self, Bundle};
use crate::pipeline::classify::SplitKind;
use crate::logging::{LogLevel, LogLine};
use crate::build::{self, MergeOptions, MergeSummary};
use crate::pipeline::{self, PreparedPlan};
use crate::pipeline::target::TargetOptions;
use crate::util::human_size;

pub enum WorkerEvent {
    Log(LogLine),
    Progress(Progress),
    Analyzed(std::result::Result<Bundle, String>),
    Prepared(std::result::Result<PreparedPlan, String>),
    Merged(std::result::Result<MergeSummary, String>),
}

#[derive(Clone, Debug)]
pub struct Progress {
    pub fraction: f32,
    pub step: String,
}

#[derive(Clone)]
pub struct Reporter {
    sender: Sender<WorkerEvent>,
}

impl Reporter {
    pub fn log(&self, level: LogLevel, text: impl Into<String>) {
        self.sender
            .send(WorkerEvent::Log(LogLine::new(level, text)))
            .ok();
    }

    pub fn progress(&self, fraction: f32, step: impl Into<String>) {
        let fraction = fraction.clamp(0.0, 1.0);
        self.sender
            .send(WorkerEvent::Progress(Progress {
                fraction,
                step: step.into(),
            }))
            .ok();
    }
}

pub fn spawn_analysis(path: PathBuf) -> Receiver<WorkerEvent> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let reporter = Reporter {
            sender: sender.clone(),
        };
        let outcome = match bundle::analyze(&path) {
            Ok(bundle) => {
                report(&reporter, &bundle);
                Ok(bundle)
            }
            Err(error) => Err(error.to_string()),
        };
        sender.send(WorkerEvent::Analyzed(outcome)).ok();
    });
    receiver
}

pub fn spawn_prepare(bundle_path: PathBuf, options: TargetOptions) -> Receiver<WorkerEvent> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let reporter = Reporter {
            sender: sender.clone(),
        };
        let outcome = pipeline::run(&bundle_path, &options, &reporter)
            .map_err(|error| error.to_string());
        sender.send(WorkerEvent::Prepared(outcome)).ok();
    });
    receiver
}

pub fn spawn_merge(
    plan: Arc<PreparedPlan>,
    output: PathBuf,
    options: MergeOptions,
) -> Receiver<WorkerEvent> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let reporter = Reporter {
            sender: sender.clone(),
        };
        let outcome = build::run(&plan, &output, &options, &reporter)
            .map_err(|error| error.to_string());
        sender.send(WorkerEvent::Merged(outcome)).ok();
    });
    receiver
}

fn report(reporter: &Reporter, bundle: &Bundle) {
    reporter.log(
        LogLevel::Info,
        format!("Format: {}", bundle.kind.label()),
    );
    reporter.log(
        LogLevel::Info,
        format!("Metadata: {}", bundle.manifest.label()),
    );
    reporter.log(
        LogLevel::Info,
        format!(
            "Found {} split APK(s), {} in total",
            bundle.splits.len(),
            human_size(bundle.total_size())
        ),
    );
    for split in &bundle.splits {
        reporter.log(
            LogLevel::Info,
            format!(
                "  {:<22} {:>10}  {}",
                split.kind.label(),
                human_size(split.size),
                split.entry
            ),
        );
    }
    for split in &bundle.splits {
        if matches!(split.kind, SplitKind::Unrecognized(_)) {
            reporter.log(
                LogLevel::Warn,
                format!(
                    "Unrecognized split name, it is excluded by default: {}",
                    split.entry
                ),
            );
        }
    }
    if !bundle.has_base() {
        reporter.log(
            LogLevel::Warn,
            "No base module detected; preparation cannot continue without one".to_owned(),
        );
    }
    reporter.log(LogLevel::Success, "Analysis complete".to_owned());
}
