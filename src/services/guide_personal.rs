use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::PathBuf,
};

use log::warn;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct GuidePersonalData {
    pub favorites: BTreeSet<String>,
    pub notes: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct GuidePersonalStore {
    file: PathBuf,
}

impl GuidePersonalStore {
    pub fn new() -> Self {
        let data_home = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(|home| PathBuf::from(home).join(".local").join("share"))
            })
            .unwrap_or_else(|| PathBuf::from("."));

        Self {
            file: data_home.join("git-desk").join("guide-personal.json"),
        }
    }

    pub fn load(&self) -> GuidePersonalData {
        let Ok(contents) = fs::read_to_string(&self.file) else {
            warn!("Failed to read guide personal data file: {}", self.file.display());
            return GuidePersonalData::default();
        };
        serde_json::from_str(&contents).unwrap_or_else(|e| {
            warn!("Failed to parse guide personal data file: {}", e);
            GuidePersonalData::default()
        })
    }

    pub fn save(&self, data: &GuidePersonalData) {
        if let Some(parent) = self.file.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(contents) = serde_json::to_string_pretty(data) {
            let _ = fs::write(&self.file, contents);
        }
    }
}
