use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default)]
    pub lsp: LspConfig,
}

fn default_theme() -> String {
    "monokai".to_string()
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LspConfig {
    #[serde(default)]
    pub servers: Vec<LspServerConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspServerConfig {
    pub name: String,
    pub language: String,
    pub binary: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub extensions: Vec<String>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    false
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            lsp: LspConfig::default(),
        }
    }
}

impl Config {
    fn config_path() -> Option<PathBuf> {
        let home = std::env::var("HOME").ok()?;
        Some(PathBuf::from(home).join(".config").join("y").join("config.toml"))
    }

    pub fn load() -> Self {
        let path = match Self::config_path() {
            Some(p) => p,
            None => return Self::default(),
        };

        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => return Self::default(),
        };

        match toml::from_str(&content) {
            Ok(config) => config,
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) {
        let path = match Self::config_path() {
            Some(p) => p,
            None => return,
        };

        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        if let Ok(content) = toml::to_string_pretty(self) {
            let _ = fs::write(&path, content);
        }
    }

    /// Merge hardcoded known servers into config, adding any new ones not already present.
    pub fn ensure_known_servers(&mut self) {
        let known = crate::lsp::types::known_servers();
        for server in known {
            if !self.lsp.servers.iter().any(|s| s.name == server.name) {
                self.lsp.servers.push(server);
            }
        }
    }

    /// Find the enabled server config for a given file extension.
    pub fn server_for_extension(&self, ext: &str) -> Option<&LspServerConfig> {
        self.lsp.servers.iter().find(|s| {
            s.enabled && s.extensions.iter().any(|e| e == ext)
        })
    }
}
