use std::fs;
use std::path::{Path, PathBuf};

use crate::format::archive::container::Container;
use crate::error::Result;

#[derive(Clone, Debug, Default)]
pub struct ExtractReport {
    pub files: usize,
    pub bytes: u64,
    pub skipped: Vec<String>,
}

pub fn sanitize_entry(name: &str) -> Option<PathBuf> {
    let mut path = PathBuf::new();
    for part in name.split(['/', '\\']) {
        match part {
            "" | "." => {}
            ".." => return None,
            other if other.len() == 2 && other.as_bytes()[1] == b':' => return None,
            other => path.push(other),
        }
    }
    (!path.as_os_str().is_empty()).then_some(path)
}

pub fn extract_all<C: Container>(
    container: &mut C,
    destination: &Path,
    mut on_entry: impl FnMut(usize, usize),
) -> Result<ExtractReport> {
    let names: Vec<String> = container
        .entries()
        .iter()
        .map(|entry| entry.name.clone())
        .collect();
    let total = names.len();
    let mut report = ExtractReport::default();
    fs::create_dir_all(destination)?;
    for (index, name) in names.iter().enumerate() {
        match sanitize_entry(name) {
            Some(relative) => {
                let target = destination.join(relative);
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent)?;
                }
                report.bytes += container.extract_to(name, &target)?;
                report.files += 1;
            }
            None => report.skipped.push(name.clone()),
        }
        on_entry(index + 1, total);
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::sanitize_entry;

    #[test]
    fn sanitize_keeps_safe_paths() {
        assert_eq!(
            sanitize_entry("splits/base.apk"),
            Some(PathBuf::from("splits").join("base.apk"))
        );
        assert_eq!(
            sanitize_entry("./x\\y.apk"),
            Some(PathBuf::from("x").join("y.apk"))
        );
    }

    #[test]
    fn sanitize_rejects_escapes() {
        assert_eq!(sanitize_entry("../evil.apk"), None);
        assert_eq!(sanitize_entry("a/../../b"), None);
        assert_eq!(sanitize_entry("C:/boot.ini"), None);
        assert_eq!(sanitize_entry("/"), None);
    }
}
