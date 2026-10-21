pub(crate) mod presets;
pub(crate) mod store;

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::build::MergeOptions;
use crate::build::sign::{SignerKind, SigningConfig, ToolPaths};
use crate::pipeline::target::TargetOptions;

use store::Values;

#[derive(Clone, Debug)]
pub(crate) struct Settings {
    pub(crate) abi: Option<String>,
    pub(crate) density: Option<String>,
    pub(crate) language: String,
    pub(crate) include_features: bool,
    pub(crate) keep_intermediates: bool,
    pub(crate) verbose: bool,
    pub(crate) sign: bool,
    pub(crate) align: bool,
    pub(crate) signer: SignerKind,
    pub(crate) debug_key: bool,
    pub(crate) keystore: String,

    pub(crate) store_password: String,
    pub(crate) key_alias: String,

    pub(crate) key_password: String,
    pub(crate) show_passwords: bool,
    pub(crate) java_path: String,
    pub(crate) apksigner_path: String,
    pub(crate) zipalign_path: String,
    pub(crate) uber_path: String,
    pub(crate) split_choices: BTreeMap<String, bool>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            abi: None,
            density: None,
            language: String::new(),
            include_features: false,
            keep_intermediates: false,
            verbose: false,
            sign: false,
            align: true,
            signer: SignerKind::default(),
            debug_key: false,
            keystore: String::new(),
            store_password: String::new(),
            key_alias: String::new(),
            key_password: String::new(),
            show_passwords: false,
            java_path: String::new(),
            apksigner_path: String::new(),
            zipalign_path: String::new(),
            uber_path: String::new(),
            split_choices: BTreeMap::new(),
        }
    }
}

impl Settings {
    pub(crate) fn target_options(&self) -> TargetOptions {
        TargetOptions {
            abi: self.abi.clone(),
            density: self.density.clone(),
            language: non_empty(&self.language),
            include_features: self.include_features,
            keep_intermediates: self.keep_intermediates,
            overrides: self.split_choices.clone(),
        }
    }

    pub(crate) fn tool_paths(&self) -> ToolPaths {
        ToolPaths {
            java: non_empty(&self.java_path).map(PathBuf::from),
            apksigner: non_empty(&self.apksigner_path).map(PathBuf::from),
            zipalign: non_empty(&self.zipalign_path).map(PathBuf::from),
            uber_jar: non_empty(&self.uber_path).map(PathBuf::from),
        }
    }

    pub(crate) fn needs_keystore(&self) -> bool {
        self.signer == SignerKind::Apksigner || !self.debug_key
    }

    pub(crate) fn merge_options(&self) -> Result<MergeOptions, String> {
        let sign = if self.sign {
            let keystore = non_empty(&self.keystore).map(PathBuf::from);
            if self.needs_keystore() {
                if keystore.is_none() {
                    return Err("Choose a keystore under Signing before merging.".to_owned());
                }
                if self.store_password.is_empty() {
                    return Err("Enter the keystore password under Signing before merging.".to_owned());
                }
            }
            Some(SigningConfig {
                kind: self.signer,
                keystore: if self.needs_keystore() { keystore } else { None },
                store_password: self.store_password.clone(),
                key_alias: non_empty(&self.key_alias),
                key_password: (!self.key_password.is_empty()).then(|| self.key_password.clone()),
            })
        } else {
            None
        };
        Ok(MergeOptions {
            align: self.align,
            sign,
            tools: self.tool_paths(),
        })
    }

    pub(crate) fn reset_target_and_output(&mut self) {
        let defaults = Settings::default();
        self.abi = defaults.abi;
        self.density = defaults.density;
        self.language = defaults.language;
        self.include_features = defaults.include_features;
        self.keep_intermediates = defaults.keep_intermediates;
        self.verbose = defaults.verbose;
        self.align = defaults.align;
        self.split_choices.clear();
    }

    pub(crate) fn persisted(&self) -> Values {
        let mut values = Values::new();
        values.insert("align".to_owned(), self.align.to_string());
        values.insert("sign".to_owned(), self.sign.to_string());
        values.insert("debug_key".to_owned(), self.debug_key.to_string());
        values.insert(
            "signer".to_owned(),
            match self.signer {
                SignerKind::Apksigner => "apksigner",
                SignerKind::Uber => "uber",
            }
            .to_owned(),
        );
        values.insert("keystore".to_owned(), self.keystore.clone());
        values.insert("java".to_owned(), self.java_path.clone());
        values.insert("apksigner".to_owned(), self.apksigner_path.clone());
        values.insert("zipalign".to_owned(), self.zipalign_path.clone());
        values.insert("uber_jar".to_owned(), self.uber_path.clone());
        values
    }

    pub(crate) fn restore(&mut self, values: &Values) {
        if let Some(value) = values.get("align") {
            self.align = value == "true";
        }
        if let Some(value) = values.get("sign") {
            self.sign = value == "true";
        }
        if let Some(value) = values.get("debug_key") {
            self.debug_key = value == "true";
        }
        if let Some(value) = values.get("signer") {
            self.signer = if value == "uber" {
                SignerKind::Uber
            } else {
                SignerKind::Apksigner
            };
        }
        if let Some(value) = values.get("keystore") {
            self.keystore = value.clone();
        }
        if let Some(value) = values.get("java") {
            self.java_path = value.clone();
        }
        if let Some(value) = values.get("apksigner") {
            self.apksigner_path = value.clone();
        }
        if let Some(value) = values.get("zipalign") {
            self.zipalign_path = value.clone();
        }
        if let Some(value) = values.get("uber_jar") {
            self.uber_path = value.clone();
        }
    }
}

fn non_empty(text: &str) -> Option<String> {
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}
