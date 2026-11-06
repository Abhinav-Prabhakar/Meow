//! Optional user configuration. Meow works with zero configuration.

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub theme: Option<String>,
    #[serde(default)]
    pub editor: Option<String>,
    #[serde(default)]
    pub large_file_mb: Option<u64>,
    #[serde(default)]
    pub default_branch: Option<String>,
}

impl Config {
    pub fn load() -> Config {
        let Some(path) = Self::path() else {
            return Config::default();
        };
        fs::read_to_string(path)
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn path() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("meow").join("config.toml"))
    }

    pub fn editor(&self) -> String {
        if let Some(editor) = &self.editor {
            if !editor.trim().is_empty() {
                return editor.clone();
            }
        }
        std::env::var("VISUAL")
            .or_else(|_| std::env::var("EDITOR"))
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| {
                if cfg!(windows) {
                    "notepad".to_string()
                } else {
                    "vi".to_string()
                }
            })
    }

    pub fn large_file_mb(&self) -> u64 {
        self.large_file_mb.unwrap_or(10)
    }

    pub fn default_branch(&self) -> Option<String> {
        self.default_branch.clone()
    }
}
