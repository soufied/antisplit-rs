const ABIS: [&str; 7] = [
    "arm64_v8a",
    "armeabi_v7a",
    "armeabi",
    "x86_64",
    "x86",
    "mips64",
    "mips",
];

const DENSITIES: [&str; 9] = [
    "ldpi", "mdpi", "hdpi", "xhdpi", "xxhdpi", "xxxhdpi", "tvdpi", "nodpi", "anydpi",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SplitKind {
    Base,
    Abi(String),
    Density(String),
    Language(String),
    Feature(String),
    Unrecognized(String),
}

impl SplitKind {
    pub fn rank(&self) -> u8 {
        match self {
            Self::Base => 0,
            Self::Abi(_) => 1,
            Self::Density(_) => 2,
            Self::Language(_) => 3,
            Self::Feature(_) => 4,
            Self::Unrecognized(_) => 5,
        }
    }

    pub fn label(&self) -> String {
        match self {
            Self::Base => "base".to_owned(),
            Self::Abi(value) => format!("abi:{value}"),
            Self::Density(value) => format!("dpi:{value}"),
            Self::Language(value) => format!("lang:{value}"),
            Self::Feature(value) => format!("feature:{value}"),
            Self::Unrecognized(value) => format!("unknown:{value}"),
        }
    }

    pub fn is_base(&self) -> bool {
        matches!(self, Self::Base)
    }
}

pub fn classify(entry: &str) -> SplitKind {
    let file = entry.rsplit(['/', '\\']).next().unwrap_or(entry);
    let lower = file.to_ascii_lowercase();
    let stem = lower.strip_suffix(".apk").unwrap_or(lower.as_str());

    if stem == "base" || !(stem.starts_with("config") || stem.starts_with("split")) {
        return SplitKind::Base;
    }

    if let Some(rest) = stem
        .strip_prefix("split")
        .filter(|rest| rest.starts_with(['_', '.', '-']))
    {
        let rest = &rest[1..];
        return match rest.strip_prefix("config.") {
            Some(qualifier) => classify_qualifier(qualifier, file),
            None => SplitKind::Feature(rest.to_owned()),
        };
    }

    match stem.strip_prefix("config.") {
        Some(qualifier) => classify_qualifier(qualifier, file),
        None => SplitKind::Unrecognized(file.to_owned()),
    }
}

fn classify_qualifier(qualifier: &str, file: &str) -> SplitKind {
    let normalized = qualifier.replace('-', "_");
    if ABIS.contains(&normalized.as_str()) {
        return SplitKind::Abi(normalized);
    }
    if DENSITIES.contains(&normalized.as_str()) {
        return SplitKind::Density(normalized);
    }
    if is_language(&normalized) {
        return SplitKind::Language(normalized);
    }
    SplitKind::Unrecognized(file.to_owned())
}

fn is_language(value: &str) -> bool {
    let mut parts = value.split('_');
    let Some(language) = parts.next() else {
        return false;
    };
    let language_ok =
        (2..=3).contains(&language.len()) && language.bytes().all(|byte| byte.is_ascii_lowercase());
    language_ok
        && parts.all(|part| {
            (2..=4).contains(&part.len()) && part.bytes().all(|byte| byte.is_ascii_alphanumeric())
        })
}
