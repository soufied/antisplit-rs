use std::collections::BTreeMap;

use crate::pipeline::bundle::Split;
use crate::pipeline::classify::SplitKind;

const DEFAULT_DENSITY: &str = "xxhdpi";
const DEFAULT_LANGUAGE: &str = "en";
const FALLBACK_DPI: u32 = 480;
const UNIVERSAL_DENSITIES: [&str; 2] = ["nodpi", "anydpi"];
const NODPI: &str = "nodpi";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TargetOptions {
    pub abi: Option<String>,
    pub density: Option<String>,
    pub language: Option<String>,
    pub include_features: bool,
    pub keep_intermediates: bool,
    pub overrides: BTreeMap<String, bool>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    pub abi: Option<String>,
    pub density: Option<String>,
    pub language: String,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decision {
    pub entry: String,
    pub kind: SplitKind,
    pub include: bool,
    pub reason: String,
}

impl Target {
    pub fn resolve(splits: &[Split], options: &TargetOptions) -> Self {
        let mut notes = Vec::new();

        let abi = options.abi.clone();
        let available_abis = abi_values(splits);
        match abi.as_deref() {
            _ if available_abis.is_empty() => {
                notes.push("bundle has no ABI-specific splits".to_owned());
            }
            Some(wanted) if available_abis.contains(&wanted) => {
                notes.push(format!("target ABI {wanted}"));
            }
            Some(wanted) => notes.push(format!(
                "ABI {wanted} is not in the bundle, all ABI splits will be included"
            )),
            None => notes.push("no ABI chosen, all ABI splits will be included".to_owned()),
        }

        let available_densities = density_values(splits);
        let wanted_density = options
            .density
            .clone()
            .unwrap_or_else(|| DEFAULT_DENSITY.to_owned());
        let density = if wanted_density == NODPI {
            notes.push("density nodpi, only density-independent resources are used".to_owned());
            Some(NODPI.to_owned())
        } else if available_densities.is_empty() {
            notes.push("bundle has no density-specific splits".to_owned());
            None
        } else if available_densities.contains(&wanted_density.as_str()) {
            notes.push(format!("target density {wanted_density}"));
            Some(wanted_density)
        } else {
            let nearest = nearest_density(&wanted_density, &available_densities);
            notes.push(format!(
                "density {wanted_density} is not in the bundle, using nearest {nearest}"
            ));
            Some(nearest)
        };

        let language = options.language.clone().unwrap_or_else(host_language);
        let available_languages = language_values(splits);
        if available_languages.is_empty() {
            notes.push("bundle has no language splits, base resources are used".to_owned());
        } else if available_languages
            .iter()
            .any(|value| primary_language(value) == language.as_str())
        {
            notes.push(format!("language {language} is present in the bundle"));
        } else {
            notes.push(format!(
                "no split for language {language}, base resources are used"
            ));
        }

        Self {
            abi,
            density,
            language,
            notes,
        }
    }
}

pub fn select(splits: &[Split], target: &Target, options: &TargetOptions) -> Vec<Decision> {
    let abi_present = target
        .abi
        .as_deref()
        .is_some_and(|wanted| abi_values(splits).contains(&wanted));
    splits
        .iter()
        .map(|split| {
            let (computed, computed_reason) = decide(split, target, options, abi_present);
            let (include, reason) = match options.overrides.get(&split.entry) {
                Some(choice) if !matches!(split.kind, SplitKind::Base) => (
                    *choice,
                    format!("set manually to {}", if *choice { "include" } else { "skip" }),
                ),
                _ => (computed, computed_reason),
            };
            Decision {
                entry: split.entry.clone(),
                kind: split.kind.clone(),
                include,
                reason,
            }
        })
        .collect()
}

fn decide(
    split: &Split,
    target: &Target,
    options: &TargetOptions,
    abi_present: bool,
) -> (bool, String) {
    match &split.kind {
        SplitKind::Base => (true, "base module".to_owned()),
        SplitKind::Abi(value) if target.abi.as_deref() == Some(value.as_str()) => {
            (true, format!("matches target ABI {value}"))
        }
        SplitKind::Abi(_) if !abi_present => (
            true,
            "no ABI matches this host, included as fallback".to_owned(),
        ),
        SplitKind::Abi(value) => (false, format!("ABI {value} is not the target")),
        SplitKind::Density(value) if is_universal(value) => {
            (true, format!("{value} applies to every density"))
        }
        SplitKind::Density(value) if target.density.as_deref() == Some(value.as_str()) => {
            (true, format!("matches target density {value}"))
        }
        SplitKind::Density(value) => (false, format!("density {value} is not the target")),
        SplitKind::Language(value) if primary_language(value) == target.language.as_str() => {
            (true, format!("matches language {}", target.language))
        }
        SplitKind::Language(value) => (false, format!("language {value} is not the target")),
        SplitKind::Feature(name) if options.include_features => {
            (true, format!("dynamic feature {name}, included by option"))
        }
        SplitKind::Feature(name) => (false, format!("dynamic feature {name}, excluded by default")),
        SplitKind::Unrecognized(file) => (false, format!("unrecognized split name {file}")),
    }
}

pub fn host_language() -> String {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .into_iter()
        .filter_map(|key| std::env::var(key).ok())
        .filter(|value| !value.is_empty())
        .find_map(|value| parse_language(&value))
        .unwrap_or_else(|| DEFAULT_LANGUAGE.to_owned())
}

pub fn parse_language(locale: &str) -> Option<String> {
    let base = locale.split(['.', '@']).next()?;
    let language = base.split('_').next()?.to_ascii_lowercase();
    let valid = (2..=3).contains(&language.len())
        && language.bytes().all(|byte| byte.is_ascii_lowercase());
    valid.then_some(language)
}

fn density_dpi(value: &str) -> Option<u32> {
    match value {
        "ldpi" => Some(120),
        "mdpi" => Some(160),
        "tvdpi" => Some(213),
        "hdpi" => Some(240),
        "xhdpi" => Some(320),
        "xxhdpi" => Some(480),
        "xxxhdpi" => Some(640),
        _ => None,
    }
}

fn nearest_density(wanted: &str, available: &[&str]) -> String {
    let target = density_dpi(wanted).unwrap_or(FALLBACK_DPI);
    available
        .iter()
        .filter_map(|candidate| density_dpi(candidate).map(|dpi| (*candidate, dpi)))
        .min_by_key(|(_, dpi)| (dpi.abs_diff(target), std::cmp::Reverse(*dpi)))
        .map(|(candidate, _)| candidate.to_owned())
        .unwrap_or_else(|| {
            available
                .first()
                .copied()
                .unwrap_or(DEFAULT_DENSITY)
                .to_owned()
        })
}

fn is_universal(value: &str) -> bool {
    UNIVERSAL_DENSITIES.contains(&value)
}

fn primary_language(value: &str) -> &str {
    value.split('_').next().unwrap_or(value)
}

fn abi_values(splits: &[Split]) -> Vec<&str> {
    splits
        .iter()
        .filter_map(|split| match &split.kind {
            SplitKind::Abi(value) => Some(value.as_str()),
            _ => None,
        })
        .collect()
}

fn density_values(splits: &[Split]) -> Vec<&str> {
    splits
        .iter()
        .filter_map(|split| match &split.kind {
            SplitKind::Density(value) if !is_universal(value) => Some(value.as_str()),
            _ => None,
        })
        .collect()
}

fn language_values(splits: &[Split]) -> Vec<&str> {
    splits
        .iter()
        .filter_map(|split| match &split.kind {
            SplitKind::Language(value) => Some(value.as_str()),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{Target, TargetOptions, nearest_density, parse_language, select};
    use crate::pipeline::bundle::Split;
    use crate::pipeline::classify::classify;

    fn split(entry: &str) -> Split {
        Split {
            entry: entry.to_owned(),
            kind: classify(entry),
            size: 1,
        }
    }

    #[test]
    fn language_parsing_handles_locales() {
        assert_eq!(parse_language("de_DE.UTF-8").as_deref(), Some("de"));
        assert_eq!(parse_language("pt_BR@euro").as_deref(), Some("pt"));
        assert_eq!(parse_language("C"), None);
        assert_eq!(parse_language("POSIX"), None);
    }

    #[test]
    fn nearest_density_prefers_closest_dpi() {
        assert_eq!(
            nearest_density("xxhdpi", &["mdpi", "hdpi", "xxxhdpi"]),
            "xxxhdpi"
        );
    }

    #[test]
    fn selection_keeps_only_device_matches() {
        let splits = vec![
            split("base.apk"),
            split("split_config.arm64_v8a.apk"),
            split("split_config.armeabi_v7a.apk"),
            split("split_config.xxhdpi.apk"),
            split("split_config.mdpi.apk"),
            split("split_config.en.apk"),
            split("split_config.fr.apk"),
        ];
        let target = Target {
            abi: Some("arm64_v8a".to_owned()),
            density: Some("xxhdpi".to_owned()),
            language: "en".to_owned(),
            notes: Vec::new(),
        };
        let decisions = select(&splits, &target, &TargetOptions::default());
        let included: Vec<&str> = decisions
            .iter()
            .filter(|decision| decision.include)
            .map(|decision| decision.entry.as_str())
            .collect();
        assert_eq!(
            included,
            vec![
                "base.apk",
                "split_config.arm64_v8a.apk",
                "split_config.xxhdpi.apk",
                "split_config.en.apk",
            ]
        );
    }

    #[test]
    fn nodpi_target_skips_density_splits() {
        let splits = vec![
            split("base.apk"),
            split("split_config.xxhdpi.apk"),
            split("split_config.nodpi.apk"),
        ];
        let target = Target {
            abi: None,
            density: Some("nodpi".to_owned()),
            language: "en".to_owned(),
            notes: Vec::new(),
        };
        let decisions = select(&splits, &target, &TargetOptions::default());
        let included: Vec<&str> = decisions
            .iter()
            .filter(|decision| decision.include)
            .map(|decision| decision.entry.as_str())
            .collect();
        assert_eq!(included, vec!["base.apk", "split_config.nodpi.apk"]);
    }
}
