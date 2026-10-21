use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::format::archive::container::{Container, open_file};
use crate::error::Result;

const MANIFEST_ENTRY: &str = "AndroidManifest.xml";
const RESOURCE_TABLE: &str = "resources.arsc";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    Native,
    Dex,
    Resource,
    Asset,
    Other,
}

#[derive(Clone, Debug)]
pub struct SourceEntry {
    pub name: String,
    pub size: u64,
    pub crc: u32,
}

#[derive(Clone, Debug)]
pub struct SplitEntries {
    pub split: String,
    pub is_base: bool,
    pub entries: Vec<SourceEntry>,
}

#[derive(Clone, Debug)]
pub struct IncludedSplit {
    pub entry: String,
    pub is_base: bool,
    pub path: PathBuf,
}

#[derive(Clone, Debug)]
pub struct PlannedFile {
    pub name: String,
    pub split: String,
    pub inner: String,
    pub size: u64,
    pub crc: u32,
    pub category: Category,
}

impl PlannedFile {
    fn from_entry(group: &SplitEntries, entry: &SourceEntry) -> Self {
        Self {
            name: entry.name.clone(),
            split: group.split.clone(),
            inner: entry.name.clone(),
            size: entry.size,
            crc: entry.crc,
            category: category_of(&entry.name),
        }
    }

    fn fingerprint(&self) -> (u64, u32) {
        (self.size, self.crc)
    }
}

#[derive(Clone, Debug, Default)]
pub struct Consolidation {
    pub files: Vec<PlannedFile>,
    pub conflicts: Vec<String>,
    pub deduplicated: usize,
    pub excluded: usize,
    pub deferred_resources: usize,
}

impl Consolidation {
    pub fn count(&self, category: Category) -> usize {
        self.files
            .iter()
            .filter(|file| file.category == category)
            .count()
    }

    pub fn total_size(&self) -> u64 {
        self.files.iter().map(|file| file.size).sum()
    }
}

pub fn gather(included: &[IncludedSplit]) -> Result<Vec<SplitEntries>> {
    included
        .iter()
        .map(|split| -> Result<SplitEntries> {
            let container = open_file(&split.path)?;
            let entries = container
                .entries()
                .iter()
                .map(|entry| SourceEntry {
                    name: entry.name.clone(),
                    size: entry.size,
                    crc: entry.crc,
                })
                .collect();
            Ok(SplitEntries {
                split: split.entry.clone(),
                is_base: split.is_base,
                entries,
            })
        })
        .collect()
}

pub fn plan(inputs: &[SplitEntries]) -> Consolidation {
    let mut consolidation = Consolidation::default();
    let mut regular: BTreeMap<String, usize> = BTreeMap::new();
    let mut dex: Vec<PlannedFile> = Vec::new();

    for group in inputs {
        let mut group_dex: Vec<(u32, PlannedFile)> = Vec::new();
        for entry in &group.entries {
            if entry.name == MANIFEST_ENTRY {
                continue;
            }
            if is_stripped(&entry.name) {
                consolidation.excluded += 1;
                continue;
            }
            let candidate = PlannedFile::from_entry(group, entry);
            if let Some(index) = dex_index(&entry.name) {
                group_dex.push((index, candidate));
                continue;
            }
            if entry.name == RESOURCE_TABLE && !group.is_base {
                consolidation.deferred_resources += 1;
                continue;
            }
            match regular.get(&entry.name).copied() {
                None => {
                    regular.insert(entry.name.clone(), consolidation.files.len());
                    consolidation.files.push(candidate);
                }
                Some(index) => {
                    let existing = &consolidation.files[index];
                    if existing.fingerprint() == candidate.fingerprint() {
                        consolidation.deduplicated += 1;
                    } else {
                        consolidation.conflicts.push(format!(
                            "{} in {} differs from the copy in {} and was skipped",
                            entry.name, group.split, existing.split
                        ));
                    }
                }
            }
        }
        group_dex.sort_by_key(|(index, _)| *index);
        for (_, candidate) in group_dex {
            if dex.iter().any(|existing| existing.fingerprint() == candidate.fingerprint()) {
                consolidation.deduplicated += 1;
            } else {
                dex.push(candidate);
            }
        }
    }

    for (position, mut file) in dex.into_iter().enumerate() {
        file.name = dex_name(position + 1);
        consolidation.files.push(file);
    }
    consolidation
}

pub fn is_stripped(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    if lower.starts_with("meta-inf/") {
        return lower == "meta-inf/manifest.mf"
            || [".sf", ".rsa", ".ec", ".dsa"]
                .iter()
                .any(|suffix| lower.ends_with(*suffix));
    }
    if name.contains('/') {
        return false;
    }
    lower == "split_info"
        || lower.ends_with(".split")
        || (lower.starts_with("split_") && lower.ends_with(".apk"))
}

pub fn dex_index(name: &str) -> Option<u32> {
    if name.contains('/') {
        return None;
    }
    let middle = name.strip_prefix("classes")?.strip_suffix(".dex")?;
    if middle.is_empty() {
        return Some(1);
    }
    if !middle.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    middle.parse::<u32>().ok().filter(|index| *index >= 2)
}

fn dex_name(index: usize) -> String {
    if index == 1 {
        "classes.dex".to_owned()
    } else {
        format!("classes{index}.dex")
    }
}

fn category_of(name: &str) -> Category {
    if name.starts_with("lib/") {
        Category::Native
    } else if dex_index(name).is_some() {
        Category::Dex
    } else if name == RESOURCE_TABLE || name.starts_with("res/") {
        Category::Resource
    } else if name.starts_with("assets/") {
        Category::Asset
    } else {
        Category::Other
    }
}

#[cfg(test)]
mod tests {
    use super::{Category, SourceEntry, SplitEntries, dex_index, is_stripped, plan};

    fn entry(name: &str, size: u64, crc: u32) -> SourceEntry {
        SourceEntry {
            name: name.to_owned(),
            size,
            crc,
        }
    }

    fn group(split: &str, is_base: bool, entries: Vec<SourceEntry>) -> SplitEntries {
        SplitEntries {
            split: split.to_owned(),
            is_base,
            entries,
        }
    }

    #[test]
    fn dex_slices_are_renumbered_after_base() {
        let base = group(
            "base.apk",
            true,
            vec![
                entry("classes.dex", 1, 10),
                entry("classes3.dex", 3, 30),
                entry("AndroidManifest.xml", 1, 1),
                entry("META-INF/CERT.RSA", 1, 1),
            ],
        );
        let config = group(
            "split_config.arm64_v8a.apk",
            false,
            vec![entry("classes.dex", 2, 20)],
        );
        let result = plan(&[base, config]);
        let dex: Vec<(&str, &str, &str)> = result
            .files
            .iter()
            .filter(|file| file.category == Category::Dex)
            .map(|file| (file.name.as_str(), file.inner.as_str(), file.split.as_str()))
            .collect();
        assert_eq!(
            dex,
            vec![
                ("classes.dex", "classes.dex", "base.apk"),
                ("classes2.dex", "classes3.dex", "base.apk"),
                ("classes3.dex", "classes.dex", "split_config.arm64_v8a.apk"),
            ]
        );
        assert_eq!(result.excluded, 1);
    }

    #[test]
    fn identical_dex_is_deduplicated() {
        let base = group("base.apk", true, vec![entry("classes.dex", 1, 10)]);
        let config = group(
            "split_config.en.apk",
            false,
            vec![entry("classes.dex", 1, 10)],
        );
        let result = plan(&[base, config]);
        assert_eq!(result.count(Category::Dex), 1);
        assert_eq!(result.deduplicated, 1);
        assert!(result.conflicts.is_empty());
    }

    #[test]
    fn differing_copies_are_reported_and_base_wins() {
        let base = group(
            "base.apk",
            true,
            vec![entry("lib/arm64-v8a/libx.so", 5, 5)],
        );
        let config = group(
            "split_config.arm64_v8a.apk",
            false,
            vec![
                entry("lib/arm64-v8a/libx.so", 6, 6),
                entry("assets/a.txt", 7, 7),
            ],
        );
        let result = plan(&[base, config]);
        assert_eq!(result.conflicts.len(), 1);
        let native = result
            .files
            .iter()
            .find(|file| file.name == "lib/arm64-v8a/libx.so")
            .expect("native file");
        assert_eq!(native.split, "base.apk");
        assert_eq!(result.count(Category::Asset), 1);
    }

    #[test]
    fn split_resource_tables_are_deferred() {
        let base = group("base.apk", true, vec![entry("resources.arsc", 1, 1)]);
        let config = group(
            "split_config.xxhdpi.apk",
            false,
            vec![entry("resources.arsc", 2, 2)],
        );
        let result = plan(&[base, config]);
        assert_eq!(result.deferred_resources, 1);
        assert_eq!(result.files.len(), 1);
        assert_eq!(result.files[0].split, "base.apk");
    }

    #[test]
    fn stripping_rules_match_split_and_signature_files() {
        assert!(is_stripped("META-INF/CERT.RSA"));
        assert!(is_stripped("META-INF/MANIFEST.MF"));
        assert!(is_stripped("split_config.en.apk"));
        assert!(is_stripped("split_info"));
        assert!(!is_stripped("assets/split_x.apk"));
        assert!(!is_stripped("lib/arm64-v8a/libx.so"));
    }

    #[test]
    fn dex_names_are_recognised() {
        assert_eq!(dex_index("classes.dex"), Some(1));
        assert_eq!(dex_index("classes12.dex"), Some(12));
        assert_eq!(dex_index("classes1.dex"), None);
        assert_eq!(dex_index("x/classes2.dex"), None);
    }
}
