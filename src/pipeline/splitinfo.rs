use std::path::Path;

use crate::format::archive::container::{Container, open_file};
use crate::format::axml::model::{Node, attribute_text, string_at};
use crate::format::axml::{self, Document};
use crate::pipeline::classify::SplitKind;
use crate::error::Result;

#[derive(Clone, Debug, Default)]
pub struct ManifestSummary {
    pub package: Option<String>,
    pub version_code: Option<String>,
    pub split_name: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct Check {
    pub conflicts: Vec<String>,
    pub warnings: Vec<String>,
}

pub fn read_manifest(apk: &Path) -> Result<Document> {
    let mut container = open_file(apk)?;
    let bytes = container.read("AndroidManifest.xml")?;
    axml::parse(&bytes)
}

pub fn summarize(document: &Document) -> ManifestSummary {
    let names = document.strings.as_slice();
    let manifest = document.nodes.iter().find_map(|node| match node {
        Node::Element(element) if string_at(names, element.name) == Some("manifest") => {
            Some(element)
        }
        _ => None,
    });
    let Some(manifest) = manifest else {
        return ManifestSummary::default();
    };
    ManifestSummary {
        package: attribute_text(names, manifest, "package"),
        version_code: attribute_text(names, manifest, "android:versionCode"),
        split_name: attribute_text(names, manifest, "split"),
    }
}

pub fn check_split(
    base: &ManifestSummary,
    entry: &str,
    kind: &SplitKind,
    summary: &ManifestSummary,
) -> Check {
    let mut check = Check::default();
    if summary.package != base.package {
        check.conflicts.push(format!(
            "{entry} declares package {} but the base declares {}",
            shown(&summary.package),
            shown(&base.package)
        ));
    }
    if summary.version_code != base.version_code {
        check.conflicts.push(format!(
            "{entry} declares versionCode {} but the base declares {}",
            shown(&summary.version_code),
            shown(&base.version_code)
        ));
    }
    match (kind, summary.split_name.as_deref()) {
        (SplitKind::Base, Some(name)) => check
            .warnings
            .push(format!("{entry} is the base module but declares split {name}")),
        (SplitKind::Base, None) => {}
        (SplitKind::Unrecognized(_), _) => {}
        (_, None) => check
            .warnings
            .push(format!("{entry} has no split attribute in its manifest")),
        (_, Some(declared)) => {
            if let Some(expected) = expected_split_name(kind) {
                if normalize(declared) != normalize(&expected) {
                    check.warnings.push(format!(
                        "{entry} declares split {declared} but its file name suggests {expected}"
                    ));
                }
            }
        }
    }
    check
}

fn shown(value: &Option<String>) -> &str {
    value.as_deref().unwrap_or("nothing")
}

fn expected_split_name(kind: &SplitKind) -> Option<String> {
    match kind {
        SplitKind::Abi(value) | SplitKind::Density(value) | SplitKind::Language(value) => {
            Some(format!("config.{value}"))
        }
        SplitKind::Feature(name) => Some(name.clone()),
        SplitKind::Base | SplitKind::Unrecognized(_) => None,
    }
}

fn normalize(value: &str) -> String {
    value.to_ascii_lowercase().replace('-', "_")
}
