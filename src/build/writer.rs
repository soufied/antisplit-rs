use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::format::archive::container::{Container, ZipContainer, open_file};
use crate::format::archive::extract::sanitize_entry;
use crate::format::arsc::format;
use crate::format::axml;
use crate::pipeline::consolidate::{Consolidation, dex_index, is_stripped};
use crate::error::{Error, Result};

const MANIFEST_ENTRY: &str = "AndroidManifest.xml";
const RESOURCE_TABLE: &str = "resources.arsc";
const DEX_FIRST: &str = "classes.dex";

#[derive(Clone, Debug)]
pub struct WriteSummary {
    pub entries: usize,
    pub bytes: u64,
}

#[derive(Clone, Debug)]
pub struct Integrity {
    pub entries: usize,
    pub dex: usize,
}

pub fn write_apk(
    output: &Path,
    manifest: &Path,
    resources: &[u8],
    consolidation: &Consolidation,
    input: &Path,
    mut on_file: impl FnMut(usize, usize),
) -> Result<WriteSummary> {
    let file = File::create(output)?;
    let mut writer = ZipWriter::new(BufWriter::new(file));
    let manifest_bytes = fs::read(manifest)?;
    writer.start_file(MANIFEST_ENTRY, deflated())?;
    writer.write_all(&manifest_bytes)?;
    writer.start_file(RESOURCE_TABLE, stored())?;
    writer.write_all(resources)?;

    let total = consolidation.files.len();
    let mut sources: HashMap<String, ZipContainer<BufReader<File>>> = HashMap::new();
    for (index, planned) in consolidation.files.iter().enumerate() {
        if planned.inner == RESOURCE_TABLE {
            on_file(index + 1, total);
            continue;
        }
        if !sources.contains_key(&planned.split) {
            let path = source_path(input, &planned.split)?;
            sources.insert(planned.split.clone(), open_file(&path)?);
        }
        let source = sources
            .get_mut(&planned.split)
            .ok_or_else(|| Error::Incompatible(format!("missing source {}", planned.split)))?;
        let options = if planned.inner == RESOURCE_TABLE {
            stored()
        } else {
            deflated()
        };
        writer.start_file(planned.name.as_str(), options)?;
        source.stream_to(&planned.inner, &mut writer)?;
        on_file(index + 1, total);
    }

    let mut buffered = writer.finish()?;
    buffered.flush()?;
    let bytes = buffered.get_ref().metadata()?.len();
    Ok(WriteSummary {
        entries: consolidation
            .files
            .iter()
            .filter(|file| file.inner != RESOURCE_TABLE)
            .count()
            + 2,
        bytes,
    })
}

pub fn verify(path: &Path) -> Result<Integrity> {
    let mut container = open_file(path)?;
    let names: Vec<String> = container
        .entries()
        .iter()
        .map(|entry| entry.name.clone())
        .collect();

    let mut seen: HashSet<&str> = HashSet::with_capacity(names.len());
    for name in &names {
        if !seen.insert(name.as_str()) {
            return Err(verification(format!("duplicate entry {name}")));
        }
        if is_stripped(name) {
            return Err(verification(format!(
                "split metadata leaked into the output: {name}"
            )));
        }
    }
    for required in [MANIFEST_ENTRY, DEX_FIRST, RESOURCE_TABLE] {
        if !seen.contains(required) {
            return Err(verification(format!("required entry {required} is missing")));
        }
    }

    let mut dex_indices: Vec<u32> = names.iter().filter_map(|name| dex_index(name)).collect();
    dex_indices.sort_unstable();
    for (position, index) in dex_indices.iter().enumerate() {
        if *index as usize != position + 1 {
            return Err(verification("dex slices are not numbered contiguously"));
        }
    }

    for name in &names {
        container
            .stream_to(name, &mut io::sink())
            .map_err(|error| verification(format!("{name} is unreadable: {error}")))?;
    }

    let manifest_bytes = container.read(MANIFEST_ENTRY)?;
    let mut document = axml::parse(&manifest_bytes)?;
    let patch = axml::make_standalone(&mut document)?;
    if !patch.actions.is_empty() {
        return Err(verification(format!(
            "{} split marker(s) remain in the manifest",
            patch.actions.len()
        )));
    }

    let table_bytes = container.read(RESOURCE_TABLE)?;
    format::parse(&table_bytes)
        .map_err(|error| verification(format!("resources.arsc is invalid: {error}")))?;

    Ok(Integrity {
        entries: names.len(),
        dex: dex_indices.len(),
    })
}

fn source_path(input: &Path, split: &str) -> Result<PathBuf> {
    sanitize_entry(split)
        .map(|relative| input.join(relative))
        .ok_or_else(|| Error::Incompatible(format!("unsafe split name {split}")))
}

fn verification(message: impl Into<String>) -> Error {
    Error::Verification(message.into())
}

fn deflated() -> SimpleFileOptions {
    SimpleFileOptions::default().compression_method(CompressionMethod::Deflated)
}

fn stored() -> SimpleFileOptions {
    SimpleFileOptions::default().compression_method(CompressionMethod::Stored)
}
