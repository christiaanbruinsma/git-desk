use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

const MAX_RECENT: usize = 10;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentProject {
    pub path: PathBuf,
    pub last_opened: u64,
}

#[derive(Debug, Clone)]
pub struct RecentProjects {
    file: PathBuf,
}

impl RecentProjects {
    pub fn new() -> Self {
        let config_home = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
            .unwrap_or_else(|| PathBuf::from("."));
        Self {
            file: config_home.join("git-desk").join("recent-projects.json"),
        }
    }

    pub fn load(&self) -> Vec<RecentProject> {
        let Ok(contents) = fs::read_to_string(&self.file) else {
            return Vec::new();
        };
        serde_json::from_str(&contents).unwrap_or_default()
    }

    pub fn add(&self, path: &Path) {
        let mut items = self.load();
        items.retain(|item| item.path != path);
        items.insert(
            0,
            RecentProject {
                path: path.to_path_buf(),
                last_opened: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|value| value.as_secs())
                    .unwrap_or(0),
            },
        );
        items.truncate(MAX_RECENT);
        self.save(&items);
    }

    pub fn remove(&self, path: &Path) {
        let mut items = self.load();
        items.retain(|item| item.path != path);
        self.save(&items);
    }

    pub fn replace_path(&self, old_path: &Path, new_path: &Path) {
        if old_path == new_path {
            return;
        }

        let mut items = self.load();
        let Some(index) = items.iter().position(|item| item.path == old_path) else {
            return;
        };

        if items.iter().any(|item| item.path == new_path) {
            items.remove(index);
        } else {
            items[index].path = new_path.to_path_buf();
        }

        self.save(&items);
    }

    fn save(&self, items: &[RecentProject]) {
        if let Some(parent) = self.file.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(contents) = serde_json::to_string_pretty(items) {
            let _ = fs::write(&self.file, contents);
        }
    }
}
