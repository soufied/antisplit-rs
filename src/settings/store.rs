use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub type Values = BTreeMap<String, String>;

const CONFIG_DIR: &str = "antisplit-rs";
const SETTINGS_FILE: &str = "settings.json";
const LEGACY_FILE: &str = "settings.conf";

fn config_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(base.join(CONFIG_DIR))
}

pub fn settings_path() -> Option<PathBuf> {
    config_dir().map(|dir| dir.join(SETTINGS_FILE))
}

pub fn load() -> Values {
    let Some(dir) = config_dir() else {
        return Values::new();
    };
    if let Some(values) = read_json(&dir.join(SETTINGS_FILE)) {
        return values;
    }
    read_legacy(&dir.join(LEGACY_FILE)).unwrap_or_default()
}

fn read_json(path: &Path) -> Option<Values> {
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

fn read_legacy(path: &Path) -> Option<Values> {
    let text = fs::read_to_string(path).ok()?;
    Some(
        text.lines()
            .filter_map(|line| {
                let (key, value) = line.split_once('=')?;
                Some((key.trim().to_owned(), value.trim().to_owned()))
            })
            .collect(),
    )
}

pub fn save(values: &Values) -> io::Result<()> {
    let Some(path) = settings_path() else {
        return Ok(());
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut text = serde_json::to_string_pretty(values).map_err(io::Error::other)?;
    text.push('\n');
    let temp = path.with_extension("json.tmp");
    fs::write(&temp, text)?;
    fs::rename(&temp, &path)
}
