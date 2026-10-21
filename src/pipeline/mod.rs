pub(crate) mod bundle;
pub(crate) mod classify;
pub(crate) mod consolidate;
pub(crate) mod splitinfo;
pub(crate) mod target;
pub(crate) mod workspace;

use std::fs;
use std::path::{Path, PathBuf};

use crate::format::archive::container::open_file;
use crate::format::archive::extract::{extract_all, sanitize_entry};
use crate::format::axml;
use bundle::{Bundle};
use consolidate::{Category, IncludedSplit};
use crate::error::{Error, Result};
use crate::logging::LogLevel;
use splitinfo::{ManifestSummary};
use target::{Decision, Target, TargetOptions};
use crate::util::human_size;
use crate::worker::Reporter;
use workspace::Workspace;

#[derive(Debug)]
pub struct PreparedPlan {
    pub bundle: Bundle,
    pub workspace: Workspace,
    pub target: Target,
    pub decisions: Vec<Decision>,
    pub base: ManifestSummary,
    pub manifest_path: PathBuf,
    pub consolidation: consolidate::Consolidation,
}

impl PreparedPlan {
    pub fn selected(&self) -> usize {
        self.decisions
            .iter()
            .filter(|decision| decision.include)
            .count()
    }

    pub fn decision_for(&self, entry: &str) -> Option<&Decision> {
        self.decisions
            .iter()
            .find(|decision| decision.entry == entry)
    }
}

pub fn run(
    bundle_path: &Path,
    options: &TargetOptions,
    reporter: &Reporter,
) -> Result<PreparedPlan> {
    reporter.progress(0.02, "Creating workspace");
    let mut workspace = Workspace::create()?;
    workspace.set_keep(options.keep_intermediates);
    reporter.log(
        LogLevel::Info,
        format!("Workspace {}", workspace.root().display()),
    );

    reporter.progress(0.04, "Analysing bundle");
    let bundle = bundle::analyze(bundle_path)?;
    if !bundle.has_base() {
        return Err(Error::NoBaseModule);
    }

    reporter.progress(0.05, "Unpacking archive");
    let input = workspace.input_dir();
    let mut container = open_file(bundle_path)?;
    let report = extract_all(&mut container, &input, |done, total| {
        if done == total || done % 16 == 0 {
            let fraction = 0.05 + 0.55 * ((done as f32) / (total.max(1) as f32));
            reporter.progress(fraction, format!("Unpacking entry {done} of {total}"));
        }
    })?;
    for name in &report.skipped {
        reporter.log(
            LogLevel::Warn,
            format!("Skipped unsafe archive path {name}"),
        );
    }
    reporter.log(
        LogLevel::Info,
        format!(
            "Unpacked {} file(s), {}",
            report.files,
            human_size(report.bytes)
        ),
    );

    reporter.progress(0.62, "Selecting target configuration");
    let target = Target::resolve(&bundle.splits, options);
    for note in &target.notes {
        reporter.log(LogLevel::Info, note.clone());
    }
    let decisions = target::select(&bundle.splits, &target, options);

    let base_split = bundle
        .splits
        .iter()
        .find(|split| split.kind.is_base())
        .ok_or(Error::NoBaseModule)?;
    let base_path = locate(&input, &base_split.entry)?;
    reporter.progress(0.68, "Reading base manifest");
    let mut base_document = splitinfo::read_manifest(&base_path)?;
    let base = splitinfo::summarize(&base_document);

    let non_base: Vec<_> = bundle
        .splits
        .iter()
        .filter(|split| !split.kind.is_base())
        .collect();
    let total = non_base.len().max(1);
    for (index, split) in non_base.iter().enumerate() {
        let fraction = 0.72 + 0.18 * ((index as f32) / (total as f32));
        reporter.progress(fraction, format!("Validating {}", split.entry));
        let path = locate(&input, &split.entry)?;
        let summary = splitinfo::summarize(&splitinfo::read_manifest(&path)?);
        let check = splitinfo::check_split(&base, &split.entry, &split.kind, &summary);
        let selected = decisions
            .iter()
            .any(|decision| decision.entry == split.entry && decision.include);
        if selected && !check.conflicts.is_empty() {
            return Err(Error::Incompatible(check.conflicts.join("; ")));
        }
        for conflict in &check.conflicts {
            reporter.log(
                LogLevel::Warn,
                format!("Not selected, ignoring conflict: {conflict}"),
            );
        }
        for warning in &check.warnings {
            reporter.log(LogLevel::Warn, warning.clone());
        }
        let reason = decisions
            .iter()
            .find(|decision| decision.entry == split.entry)
            .map_or("", |decision| decision.reason.as_str());
        reporter.log(
            LogLevel::Info,
            format!(
                "{} · split {} · {}",
                split.kind.label(),
                summary.split_name.as_deref().unwrap_or("no split name"),
                reason
            ),
        );
    }

    reporter.progress(0.90, "Consolidating splits");
    let mut included = Vec::new();
    for split in &bundle.splits {
        let selected = decisions
            .iter()
            .any(|decision| decision.entry == split.entry && decision.include);
        if selected {
            included.push(IncludedSplit {
                entry: split.entry.clone(),
                is_base: split.kind.is_base(),
                path: locate(&input, &split.entry)?,
            });
        }
    }
    let consolidation = consolidate::plan(&consolidate::gather(&included)?);
    if consolidation
        .files
        .iter()
        .any(|file| file.name.ends_with("libpairipcore.so"))
    {
        reporter.log(
            LogLevel::Warn,
            "This bundle is protected by Pairip (libpairipcore.so); the merged APK may crash at launch".to_owned(),
        );
    }
    for conflict in &consolidation.conflicts {
        reporter.log(LogLevel::Warn, conflict.clone());
    }
    if consolidation.deferred_resources > 0 {
        reporter.log(
            LogLevel::Info,
            format!(
                "{} split resource table(s) will be merged when the APK is written",
                consolidation.deferred_resources
            ),
        );
    }
    reporter.log(
        LogLevel::Info,
        format!(
            "Consolidated {} file(s), {}: {} native, {} dex, {} resource, {} asset, {} other",
            consolidation.files.len(),
            human_size(consolidation.total_size()),
            consolidation.count(Category::Native),
            consolidation.count(Category::Dex),
            consolidation.count(Category::Resource),
            consolidation.count(Category::Asset),
            consolidation.count(Category::Other)
        ),
    );
    reporter.log(
        LogLevel::Info,
        format!(
            "Dropped {} signature or split metadata entries, skipped {} identical duplicate(s), {} conflict(s)",
            consolidation.excluded,
            consolidation.deduplicated,
            consolidation.conflicts.len()
        ),
    );

    reporter.progress(0.92, "Patching base manifest");
    let patch = axml::make_standalone(&mut base_document)?;
    for action in &patch.actions {
        reporter.log(LogLevel::Debug, action.clone());
    }
    if patch.actions.is_empty() {
        reporter.log(LogLevel::Info, "Base manifest has no split markers");
    }
    let bytes = axml::write(&base_document);
    let mut verify = axml::parse(&bytes)?;
    let recheck = axml::make_standalone(&mut verify)?;
    if !recheck.actions.is_empty() {
        return Err(Error::Verification(format!(
            "{} split marker(s) survived patching",
            recheck.actions.len()
        )));
    }
    let manifest_path = workspace.output_dir().join("AndroidManifest.xml");
    fs::write(&manifest_path, &bytes)?;
    reporter.log(
        LogLevel::Success,
        format!(
            "Standalone manifest for {} written ({})",
            base.package.as_deref().unwrap_or("unknown package"),
            human_size(bytes.len() as u64)
        ),
    );
    reporter.progress(1.0, "Preparation complete");

    Ok(PreparedPlan {
        bundle,
        workspace,
        target,
        decisions,
        base,
        manifest_path,
        consolidation,
    })
}

fn locate(input: &Path, entry: &str) -> Result<PathBuf> {
    sanitize_entry(entry)
        .map(|relative| input.join(relative))
        .ok_or_else(|| Error::Incompatible(format!("unsafe entry name {entry}")))
}
