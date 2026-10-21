use std::path::Path;

use crate::format::archive::container::{Container, open_file};
use crate::format::archive::detect::{ArchiveKind, ManifestKind, detect_kind};
use crate::pipeline::classify::{SplitKind, classify};
use crate::error::{Error, Result};

#[derive(Clone, Debug)]
pub struct Split {
    pub entry: String,
    pub kind: SplitKind,
    pub size: u64,
}

#[derive(Clone, Debug)]
pub struct Bundle {
    pub kind: ArchiveKind,
    pub manifest: ManifestKind,
    pub splits: Vec<Split>,
}

impl Bundle {
    pub fn has_base(&self) -> bool {
        self.splits.iter().any(|split| split.kind.is_base())
    }

    pub fn total_size(&self) -> u64 {
        self.splits.iter().map(|split| split.size).sum()
    }
}

pub fn analyze(path: &Path) -> Result<Bundle> {
    let container = open_file(path)?;
    let entries = container.entries();

    let top_level: Vec<&str> = entries
        .iter()
        .map(|entry| entry.name.as_str())
        .filter(|name| !name.contains('/'))
        .collect();
    let manifest = ManifestKind::from_top_level(&top_level);

    let mut splits: Vec<Split> = entries
        .iter()
        .filter(|entry| entry.name.to_ascii_lowercase().ends_with(".apk"))
        .map(|entry| Split {
            entry: entry.name.clone(),
            kind: classify(&entry.name),
            size: entry.size,
        })
        .collect();

    if splits.is_empty() {
        return Err(if top_level.contains(&"AndroidManifest.xml") {
            Error::AlreadyStandalone
        } else {
            Error::NoSplitEntries
        });
    }

    splits.sort_by(|left, right| {
        left.kind
            .rank()
            .cmp(&right.kind.rank())
            .then_with(|| left.entry.cmp(&right.entry))
    });

    let extension = path.extension().and_then(|extension| extension.to_str());
    let kind = detect_kind(extension, manifest);
    tracing::debug!(
        entries = entries.len(),
        ?kind,
        ?manifest,
        "classified bundle"
    );

    Ok(Bundle {
        kind,
        manifest,
        splits,
    })
}
