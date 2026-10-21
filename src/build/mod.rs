pub(crate) mod sign;
pub(crate) mod writer;

use std::fs;
use std::path::{Path, PathBuf};

use crate::format::archive::container::{Container, open_file};
use crate::format::archive::extract::sanitize_entry;
use crate::format::arsc::format::{self, StringPool, Table};
use crate::format::arsc::merge::{self as table_merge, MergeStats};
use crate::error::{Error, Result};
use crate::logging::LogLevel;
use crate::pipeline::PreparedPlan;
use self::sign::{SignerKind, SigningConfig, ToolPaths, resolve};
use crate::util::human_size;
use crate::worker::Reporter;

const RESOURCE_TABLE: &str = "resources.arsc";

#[derive(Clone, Debug, Default)]
pub struct MergeOptions {
    pub align: bool,
    pub sign: Option<SigningConfig>,
    pub tools: ToolPaths,
}

#[derive(Clone, Debug)]
pub struct MergeSummary {
    pub output: PathBuf,
    pub size: u64,
    pub entries: usize,
    pub dex: usize,
}

pub fn run(
    plan: &PreparedPlan,
    output: &Path,
    options: &MergeOptions,
    reporter: &Reporter,
) -> Result<MergeSummary> {
    match produce(plan, output, options, reporter) {
        Ok(summary) => Ok(summary),
        Err(error) => {
            for suffix in ["part", "aligned", "signed", "unsigned.apk"] {
                fs::remove_file(suffixed(output, suffix)).ok();
            }
            fs::remove_dir_all(suffixed(output, "uber")).ok();
            Err(error)
        }
    }
}

fn produce(
    plan: &PreparedPlan,
    output: &Path,
    options: &MergeOptions,
    reporter: &Reporter,
) -> Result<MergeSummary> {
    let partial = suffixed(output, "part");
    let mut summary = build(plan, &partial, reporter)?;
    let finished = match &options.sign {
        Some(config) if config.kind == SignerKind::Uber => {
            sign_uber_step(&partial, output, options, config, reporter)?
        }
        Some(config) => {
            align_step(&partial, output, options, reporter)?;
            sign_apksigner_step(&partial, output, options, config, reporter)?
        }
        None => {
            align_step(&partial, output, options, reporter)?;
            reporter.log(
                LogLevel::Warn,
                "The APK is unsigned and will not install until it is signed; enable signing in Advanced settings",
            );
            partial.clone()
        }
    };

    fs::rename(&finished, output)?;
    summary.output = output.to_path_buf();
    summary.size = fs::metadata(output)?.len();
    reporter.log(
        LogLevel::Success,
        format!("Merged APK saved to {}", output.display()),
    );
    reporter.progress(1.0, "Merge complete");
    Ok(summary)
}

fn align_step(current: &Path, output: &Path, options: &MergeOptions, reporter: &Reporter) -> Result<()> {
    if !options.align {
        return Ok(());
    }
    let Some(zipalign) = resolve(&options.tools.zipalign, || sign::detect_executable("zipalign"))
    else {
        reporter.log(
            LogLevel::Warn,
            "zipalign was not found; the APK is written without alignment",
        );
        return Ok(());
    };
    let aligned = suffixed(output, "aligned");
    sign::align(&zipalign, current, &aligned, reporter)?;
    fs::rename(&aligned, current)?;
    writer::verify(current)?;
    Ok(())
}

fn sign_apksigner_step(
    current: &Path,
    output: &Path,
    options: &MergeOptions,
    config: &SigningConfig,
    reporter: &Reporter,
) -> Result<PathBuf> {
    let apksigner = resolve(&options.tools.apksigner, || {
        sign::detect_executable("apksigner")
    })
    .ok_or_else(|| {
        Error::Tool(
            "apksigner was not found; set its path in Advanced settings or switch the signer to uber-apk-signer"
                .to_owned(),
        )
    })?;
    let signed = suffixed(output, "signed");
    sign::sign_apksigner(&apksigner, current, &signed, config, reporter)?;
    sign::verify_apksigner(&apksigner, &signed, reporter)?;
    Ok(signed)
}

fn sign_uber_step(
    current: &Path,
    output: &Path,
    options: &MergeOptions,
    config: &SigningConfig,
    reporter: &Reporter,
) -> Result<PathBuf> {
    let java = resolve(&options.tools.java, sign::detect_java).ok_or_else(|| {
        Error::Tool("java was not found; install a JDK or set the java path in Advanced settings".to_owned())
    })?;
    let jar = resolve(&options.tools.uber_jar, sign::detect_uber_jar).ok_or_else(|| {
        Error::Tool("uber-apk-signer.jar was not found; set its path in Advanced settings".to_owned())
    })?;
    let zipalign = if options.align {
        resolve(&options.tools.zipalign, || sign::detect_executable("zipalign"))
    } else {
        None
    };
    if options.align && zipalign.is_none() {
        reporter.log(
            LogLevel::Warn,
            "zipalign was not found, so uber-apk-signer will skip alignment",
        );
    }
    let signed = suffixed(output, "signed");

    let work = suffixed(output, "uber");
    let staged_dir = work.join("input");
    let out_dir = work.join("output");
    fs::create_dir_all(&staged_dir)?;
    let staged = staged_dir.join("unsigned.apk");
    fs::rename(current, &staged)?;
    sign::sign_uber(
        &java,
        &jar,
        &staged,
        &signed,
        &out_dir,
        zipalign.as_deref(),
        config,
        reporter,
    )?;
    fs::remove_dir_all(&work).ok();
    Ok(signed)
}

fn build(plan: &PreparedPlan, partial: &Path, reporter: &Reporter) -> Result<MergeSummary> {
    reporter.progress(0.05, "Merging resource tables");
    let (resources, stats) = build_resources(plan, reporter)?;
    reporter.log(
        LogLevel::Info,
        format!(
            "Merged {} resource table(s) into {}: {} entries added, {} config block(s) in total, {} collision(s) resolved in favour of the base",
            stats.tables,
            human_size(resources.len() as u64),
            stats.entries_added,
            stats.blocks_added,
            stats.collisions
        ),
    );

    reporter.progress(0.12, "Writing entries");
    let input = plan.workspace.input_dir();
    let written = writer::write_apk(
        partial,
        &plan.manifest_path,
        &resources,
        &plan.consolidation,
        &input,
        |done, count| {
            let fraction = 0.12 + 0.68 * ((done as f32) / (count.max(1) as f32));
            reporter.progress(fraction, format!("Writing entry {done} of {count}"));
        },
    )?;
    reporter.log(
        LogLevel::Info,
        format!(
            "Wrote {} entries, {}",
            written.entries,
            human_size(written.bytes)
        ),
    );

    reporter.progress(0.85, "Verifying APK");
    let integrity = writer::verify(partial)?;
    reporter.log(
        LogLevel::Success,
        format!(
            "Integrity check passed: {} entries, {} dex file(s)",
            integrity.entries, integrity.dex
        ),
    );

    Ok(MergeSummary {
        output: partial.to_path_buf(),
        size: written.bytes,
        entries: integrity.entries,
        dex: integrity.dex,
    })
}

fn build_resources(plan: &PreparedPlan, reporter: &Reporter) -> Result<(Vec<u8>, MergeStats)> {
    let input = plan.workspace.input_dir();
    let base_entry = plan
        .bundle
        .splits
        .iter()
        .find(|split| split.kind.is_base())
        .map(|split| split.entry.clone())
        .ok_or(Error::NoBaseModule)?;
    let base = match read_table(&input, &base_entry)? {
        Some(table) => table,
        None => {
            reporter.log(
                LogLevel::Warn,
                "base module has no resources.arsc; merging split tables into an empty table",
            );
            empty_table()
        }
    };
    let mut others = Vec::new();
    for decision in plan
        .decisions
        .iter()
        .filter(|decision| decision.include && decision.entry != base_entry)
    {
        if let Some(table) = read_table(&input, &decision.entry)? {
            reporter.log(
                LogLevel::Debug,
                format!("Resource table from {}", decision.entry),
            );
            others.push(table);
        }
    }
    let (merged, stats) = table_merge::merge(&base, &others)?;
    table_merge::verify_type_names(&base, &merged)?;
    Ok((format::serialize(&merged), stats))
}

fn read_table(input: &Path, entry: &str) -> Result<Option<Table>> {
    let relative = sanitize_entry(entry)
        .ok_or_else(|| Error::Incompatible(format!("unsafe split name {entry}")))?;
    let mut container = open_file(&input.join(relative))?;
    let present = container
        .entries()
        .iter()
        .any(|item| item.name == RESOURCE_TABLE);
    if !present {
        return Ok(None);
    }
    let bytes = container.read(RESOURCE_TABLE)?;
    Ok(Some(format::parse(&bytes)?))
}

fn empty_table() -> Table {
    Table {
        global: StringPool::from_strings(Vec::new(), true),
        packages: Vec::new(),
    }
}

fn suffixed(output: &Path, suffix: &str) -> PathBuf {
    let mut name = output.as_os_str().to_owned();
    name.push(".");
    name.push(suffix);
    PathBuf::from(name)
}
