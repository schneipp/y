use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EditorMode {
    Vim,
    Normie,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_theme")]
    pub theme: String,
    /// None means the user hasn't chosen yet (show mode selector on first launch).
    #[serde(default)]
    pub editor_mode: Option<EditorMode>,
    #[serde(default)]
    pub lsp: LspConfig,
    #[serde(default)]
    pub keybindings: KeybindingsConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct KeybindingsConfig {
    #[serde(default)]
    pub normal: HashMap<String, String>,
    #[serde(default)]
    pub insert: HashMap<String, String>,
    #[serde(default)]
    pub visual: HashMap<String, String>,
    #[serde(default)]
    pub visual_line: HashMap<String, String>,
    #[serde(default)]
    pub completion: HashMap<String, String>,
    #[serde(default)]
    pub normie: HashMap<String, String>,
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
            editor_mode: None,
            lsp: LspConfig::default(),
            keybindings: KeybindingsConfig::default(),
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

    /// Build a keybinding registry from defaults + user overrides.
    pub fn build_registry(&self) -> crate::keybindings::KeybindingRegistry {
        use crate::keybindings::defaults::build_default_registry;
        use crate::keybindings::key::parse_key_string;
        use crate::keybindings::registry::ModeKey;

        let mut reg = build_default_registry();

        let mode_sections: &[(ModeKey, &HashMap<String, String>)] = &[
            (ModeKey::Normal, &self.keybindings.normal),
            (ModeKey::Insert, &self.keybindings.insert),
            (ModeKey::Visual, &self.keybindings.visual),
            (ModeKey::VisualLine, &self.keybindings.visual_line),
            (ModeKey::Completion, &self.keybindings.completion),
            (ModeKey::Normie, &self.keybindings.normie),
        ];

        for (mode_key, bindings) in mode_sections {
            for (key_str, action_str) in *bindings {
                let keys = match parse_key_string(key_str) {
                    Ok(k) => k,
                    Err(_) => continue,
                };
                let action: crate::keybindings::Action = match serde_json::from_str(&format!("\"{}\"", action_str)) {
                    Ok(a) => a,
                    Err(_) => continue,
                };
                reg.bind(mode_key.clone(), keys, action);
            }
        }

        reg
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
