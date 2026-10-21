use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{Error, Result};
use crate::logging::LogLevel;
use crate::worker::Reporter;

const STORE_PASS_VAR: &str = "ANTISPLIT_RS_STORE_PASS";
const KEY_PASS_VAR: &str = "ANTISPLIT_RS_KEY_PASS";
const JAR_PREFIX: &str = "uber-apk-signer";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SignerKind {
    #[default]
    Apksigner,
    Uber,
}

#[derive(Clone, Debug)]
pub struct SigningConfig {
    pub kind: SignerKind,
    pub keystore: Option<PathBuf>,
    pub store_password: String,
    pub key_alias: Option<String>,
    pub key_password: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ToolPaths {
    pub java: Option<PathBuf>,
    pub apksigner: Option<PathBuf>,
    pub zipalign: Option<PathBuf>,
    pub uber_jar: Option<PathBuf>,
}

pub fn resolve(configured: &Option<PathBuf>, detect: fn() -> Option<PathBuf>) -> Option<PathBuf> {
    match configured {
        Some(path) => path.is_file().then(|| path.clone()),
        None => detect(),
    }
}

pub fn detect_java() -> Option<PathBuf> {
    find_on_path("java")
        .or_else(|| find_in(&bin_dirs(), "java"))
        .or_else(find_java_home)
}

pub fn detect_executable(name: &str) -> Option<PathBuf> {
    find_on_path(name)
        .or_else(|| find_in(&bin_dirs(), name))
        .or_else(|| find_in_build_tools(name))
}

pub fn detect_uber_jar() -> Option<PathBuf> {
    let mut directories: Vec<PathBuf> = [
        "/usr/share/java",
        "/usr/share/java/uber-apk-signer",
        "/usr/share/uber-apk-signer",
        "/usr/lib/uber-apk-signer",
        "/usr/local/share/java",
        "/usr/local/lib/uber-apk-signer",
        "/opt",
        "/opt/uber-apk-signer",
        "/usr/local/bin",
        "/usr/bin",
    ]
    .into_iter()
    .map(PathBuf::from)
    .collect();
    if let Some(home) = home_dir() {
        for relative in [
            "Downloads",
            "Desktop",
            "Applications",
            ".local/share/java",
            ".local/share/uber-apk-signer",
            ".local/bin",
        ] {
            directories.push(home.join(relative));
        }
        directories.push(home);
    }
    directories.iter().find_map(|directory| jar_in(directory))
}

pub fn align(zipalign: &Path, input: &Path, output: &Path, reporter: &Reporter) -> Result<()> {
    let args = vec![
        "-f".to_owned(),
        "-p".to_owned(),
        "4".to_owned(),
        path_arg(input),
        path_arg(output),
    ];
    run(zipalign, &args, &[])?;
    reporter.log(
        LogLevel::Info,
        format!("Aligned entries with {}", zipalign.display()),
    );
    Ok(())
}

pub fn sign_apksigner(
    apksigner: &Path,
    input: &Path,
    output: &Path,
    config: &SigningConfig,
    reporter: &Reporter,
) -> Result<()> {
    let keystore = config
        .keystore
        .as_ref()
        .ok_or_else(|| Error::Tool("apksigner needs a keystore".to_owned()))?;
    let mut args = vec![
        "sign".to_owned(),
        "--ks".to_owned(),
        path_arg(keystore),
        "--ks-pass".to_owned(),
        format!("env:{STORE_PASS_VAR}"),
    ];
    let mut envs: Vec<(&str, &str)> = vec![(STORE_PASS_VAR, config.store_password.as_str())];
    if let Some(alias) = &config.key_alias {
        args.push("--ks-key-alias".to_owned());
        args.push(alias.clone());
    }
    if let Some(password) = &config.key_password {
        args.push("--key-pass".to_owned());
        args.push(format!("env:{KEY_PASS_VAR}"));
        envs.push((KEY_PASS_VAR, password.as_str()));
    }
    args.push("--out".to_owned());
    args.push(path_arg(output));
    args.push(path_arg(input));
    reporter.log(LogLevel::Info, "Signing with apksigner");
    run(apksigner, &args, &envs)?;
    reporter.log(LogLevel::Success, "APK signed with apksigner");
    Ok(())
}

pub fn verify_apksigner(apksigner: &Path, file: &Path, reporter: &Reporter) -> Result<()> {
    run(apksigner, &["verify".to_owned(), path_arg(file)], &[])?;
    reporter.log(LogLevel::Success, "Signature verification passed");
    Ok(())
}

pub fn sign_uber(
    java: &Path,
    jar: &Path,
    input: &Path,
    output: &Path,
    out_dir: &Path,
    zipalign: Option<&Path>,
    config: &SigningConfig,
    reporter: &Reporter,
) -> Result<()> {
    if out_dir.exists() {
        fs::remove_dir_all(out_dir)?;
    }
    fs::create_dir_all(out_dir)?;

    let alias = match (&config.keystore, &config.key_alias) {
        (Some(keystore), None) => {
            let alias = first_alias(java, keystore, &config.store_password)?;
            reporter.log(
                LogLevel::Info,
                format!("No key alias given, using the first key in the store: {alias}"),
            );
            Some(alias)
        }
        (_, alias) => alias.clone(),
    };
    let mut args = vec![
        "-jar".to_owned(),
        path_arg(jar),
        "--apks".to_owned(),
        path_arg(input),
        "--out".to_owned(),
        path_arg(out_dir),
    ];
    match &config.keystore {
        Some(keystore) => {
            args.push("--ks".to_owned());
            args.push(path_arg(keystore));
            args.push("--ksPass".to_owned());
            args.push(config.store_password.clone());
            if let Some(alias) = &alias {
                args.push("--ksAlias".to_owned());
                args.push(alias.clone());
            }
            if let Some(password) = &config.key_password {
                args.push("--ksKeyPass".to_owned());
                args.push(password.clone());
            }
        }
        None => reporter.log(
            LogLevel::Info,
            "Using uber-apk-signer's built-in debug key",
        ),
    }
    match zipalign {
        Some(path) => {
            args.push("--zipAlignPath".to_owned());
            args.push(path_arg(path));
        }
        None => args.push("--skipZipAlign".to_owned()),
    }
    reporter.log(LogLevel::Info, "Signing with uber-apk-signer");
    let (stdout, stderr) = run_output(java, &args, &[])?;
    for line in non_blank_lines(&stdout) {
        reporter.log(LogLevel::Debug, format!("uber-apk-signer: {line}"));
    }
    for line in non_blank_lines(&stderr) {
        reporter.log(LogLevel::Info, format!("uber-apk-signer: {line}"));
    }
    let Some(produced) = find_apk(out_dir)? else {

        let said = tail_lines(&[stdout.as_str(), stderr.as_str()], 12);
        return Err(Error::Tool(format!(
            "uber-apk-signer finished but produced no APK in {}. Its output ended with:\n{said}",
            out_dir.display()
        )));
    };
    fs::rename(&produced, output)?;
    reporter.log(LogLevel::Success, "APK signed with uber-apk-signer");
    Ok(())
}

fn find_apk(directory: &Path) -> Result<Option<PathBuf>> {
    let mut apks = Vec::new();
    collect_apks(directory, 0, &mut apks)?;
    apks.sort();
    Ok(apks.pop())
}

fn collect_apks(directory: &Path, depth: usize, found: &mut Vec<PathBuf>) -> Result<()> {
    if depth > 3 {
        return Ok(());
    }
    for entry in fs::read_dir(directory)?.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_apks(&path, depth + 1, found)?;
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("apk") {
            found.push(path);
        }
    }
    Ok(())
}

fn first_alias(java: &Path, keystore: &Path, store_password: &str) -> Result<String> {
    let sibling = java.with_file_name("keytool");
    let keytool = if sibling.is_file() {
        sibling
    } else {
        find_on_path("keytool").ok_or_else(|| {
            Error::Tool("keytool was not found. Set a key alias under Signing.".to_owned())
        })?
    };
    let args = vec![
        "-list".to_owned(),
        "-keystore".to_owned(),
        path_arg(keystore),
        "-storepass:env".to_owned(),
        STORE_PASS_VAR.to_owned(),
    ];
    let (stdout, _) = run_output(&keytool, &args, &[(STORE_PASS_VAR, store_password)])?;
    stdout
        .lines()
        .find_map(|line| {

            let mut parts = line.splitn(2, ',');
            let alias = parts.next()?.trim();
            let rest = parts.next()?;
            (!alias.is_empty() && rest.contains("PrivateKeyEntry")).then(|| alias.to_owned())
        })
        .ok_or_else(|| {
            Error::Tool(format!(
                "no private key found in {}. Check the keystore and set a key alias under Signing.",
                keystore.display()
            ))
        })
}

fn non_blank_lines(text: &str) -> impl Iterator<Item = &str> {
    text.lines().map(str::trim).filter(|line| !line.is_empty())
}

fn tail_lines(sources: &[&str], count: usize) -> String {
    let lines: Vec<&str> = sources.iter().flat_map(|text| non_blank_lines(text)).collect();
    if lines.is_empty() {
        return "(no output)".to_owned();
    }
    lines[lines.len().saturating_sub(count)..].join("\n")
}

fn find_on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|directory| directory.join(name))
        .find(|candidate| candidate.is_file())
}

fn find_in(directories: &[PathBuf], name: &str) -> Option<PathBuf> {
    directories
        .iter()
        .map(|directory| directory.join(name))
        .find(|candidate| candidate.is_file())
}

fn find_java_home() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    for root in ["/usr/lib/jvm", "/usr/java", "/opt/java"] {
        if let Ok(entries) = fs::read_dir(root) {
            for entry in entries.flatten() {
                candidates.push(entry.path().join("bin").join("java"));
            }
        }
    }
    candidates.into_iter().find(|candidate| candidate.is_file())
}

fn find_in_build_tools(name: &str) -> Option<PathBuf> {
    for root in build_tool_roots() {
        let Ok(entries) = fs::read_dir(&root) else {
            continue;
        };
        let mut versions: Vec<PathBuf> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.is_dir())
            .collect();
        versions.sort_by_key(|path| version_key(path));
        for version in versions.into_iter().rev() {
            let candidate = version.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn build_tool_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for key in ["ANDROID_HOME", "ANDROID_SDK_ROOT"] {
        if let Some(value) = std::env::var_os(key) {
            roots.push(PathBuf::from(value));
        }
    }
    if let Some(home) = home_dir() {
        roots.push(home.join("Android/Sdk"));
    }
    for root in [
        "/opt/android-sdk",
        "/usr/lib/android-sdk",
        "/usr/share/android-sdk",
        "/usr/local/android-sdk",
    ] {
        roots.push(PathBuf::from(root));
    }
    roots
        .into_iter()
        .map(|root| root.join("build-tools"))
        .collect()
}

fn bin_dirs() -> Vec<PathBuf> {
    let mut directories: Vec<PathBuf> = [
        "/usr/bin",
        "/usr/local/bin",
        "/bin",
        "/usr/sbin",
        "/usr/local/sbin",
        "/opt/bin",
    ]
    .into_iter()
    .map(PathBuf::from)
    .collect();
    if let Some(home) = home_dir() {
        directories.push(home.join(".local/bin"));
    }
    directories
}

fn jar_in(directory: &Path) -> Option<PathBuf> {
    let entries = fs::read_dir(directory).ok()?;
    let mut matches: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| is_uber_jar(path))
        .collect();
    matches.sort();
    matches.pop()
}

fn is_uber_jar(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with(JAR_PREFIX) && name.ends_with(".jar"))
        && path.is_file()
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

fn version_key(path: &Path) -> Vec<u64> {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("")
        .split('.')
        .map(|part| part.parse::<u64>().unwrap_or(0))
        .collect()
}

fn run(tool: &Path, args: &[String], envs: &[(&str, &str)]) -> Result<String> {
    run_output(tool, args, envs).map(|(stdout, _stderr)| stdout)
}

fn run_output(tool: &Path, args: &[String], envs: &[(&str, &str)]) -> Result<(String, String)> {
    let output = Command::new(tool)
        .args(args)
        .envs(envs.iter().copied())
        .output()?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    if output.status.success() {
        return Ok((stdout, stderr));
    }
    let detail = if stderr.trim().is_empty() {
        stdout.trim().to_owned()
    } else {
        stderr.trim().to_owned()
    };
    Err(Error::Tool(format!(
        "{} exited with {}: {}",
        tool.display(),
        output.status,
        detail
    )))
}

fn path_arg(path: &Path) -> String {
    path.display().to_string()
}
