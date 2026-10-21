#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArchiveKind {
    Apkm,
    Apks,
    Xapk,
    Apkx,
    PlainZip,
}

impl ArchiveKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Apkm => "APKM bundle",
            Self::Apks => "APKS bundle",
            Self::Xapk => "XAPK bundle",
            Self::Apkx => "APKX bundle",
            Self::PlainZip => "ZIP of split APKs",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ManifestKind {
    ApkmInfo,
    XapkManifest,
    ApksTableOfContents,
    Absent,
}

impl ManifestKind {
    pub fn from_top_level(names: &[&str]) -> Self {
        let has = |target: &str| names.iter().any(|name| name.eq_ignore_ascii_case(target));
        if has("info.json") {
            Self::ApkmInfo
        } else if has("manifest.json") {
            Self::XapkManifest
        } else if has("toc.pb") {
            Self::ApksTableOfContents
        } else {
            Self::Absent
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::ApkmInfo => "info.json (APKM metadata)",
            Self::XapkManifest => "manifest.json (XAPK metadata)",
            Self::ApksTableOfContents => "toc.pb (APKS table of contents)",
            Self::Absent => "none",
        }
    }
}

pub fn detect_kind(extension: Option<&str>, manifest: ManifestKind) -> ArchiveKind {
    match manifest {
        ManifestKind::ApkmInfo => ArchiveKind::Apkm,
        ManifestKind::XapkManifest => ArchiveKind::Xapk,
        ManifestKind::ApksTableOfContents => ArchiveKind::Apks,
        ManifestKind::Absent => match extension.map(str::to_ascii_lowercase).as_deref() {
            Some("apkm") => ArchiveKind::Apkm,
            Some("apks") => ArchiveKind::Apks,
            Some("xapk") => ArchiveKind::Xapk,
            Some("apkx") => ArchiveKind::Apkx,
            _ => ArchiveKind::PlainZip,
        },
    }
}
