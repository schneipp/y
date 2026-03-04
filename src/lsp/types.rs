use crate::config::LspServerConfig;
use std::process::Command;

/// Returns the hardcoded list of known LSP servers.
pub fn known_servers() -> Vec<LspServerConfig> {
    vec![
        LspServerConfig {
            name: "rust-analyzer".into(),
            language: "rust".into(),
            binary: "rust-analyzer".into(),
            args: vec![],
            extensions: vec!["rs".into()],
            enabled: false,
        },
        LspServerConfig {
            name: "typescript-language-server".into(),
            language: "typescript".into(),
            binary: "typescript-language-server".into(),
            args: vec!["--stdio".into()],
            extensions: vec!["ts".into(), "tsx".into(), "js".into(), "jsx".into()],
            enabled: false,
        },
        LspServerConfig {
            name: "pyright".into(),
            language: "python".into(),
            binary: "pyright-langserver".into(),
            args: vec!["--stdio".into()],
            extensions: vec!["py".into(), "pyi".into()],
            enabled: false,
        },
        LspServerConfig {
            name: "gopls".into(),
            language: "go".into(),
            binary: "gopls".into(),
            args: vec!["serve".into()],
            extensions: vec!["go".into()],
            enabled: false,
        },
        LspServerConfig {
            name: "clangd".into(),
            language: "c/c++".into(),
            binary: "clangd".into(),
            args: vec![],
            extensions: vec!["c".into(), "cpp".into(), "h".into(), "hpp".into(), "cc".into()],
            enabled: false,
        },
        LspServerConfig {
            name: "lua-language-server".into(),
            language: "lua".into(),
            binary: "lua-language-server".into(),
            args: vec![],
            extensions: vec!["lua".into()],
            enabled: false,
        },
        LspServerConfig {
            name: "zls".into(),
            language: "zig".into(),
            binary: "zls".into(),
            args: vec![],
            extensions: vec!["zig".into()],
            enabled: false,
        },
        LspServerConfig {
            name: "bash-language-server".into(),
            language: "bash".into(),
            binary: "bash-language-server".into(),
            args: vec!["start".into()],
            extensions: vec!["sh".into(), "bash".into()],
            enabled: false,
        },
    ]
}

/// Check if a binary is available on PATH.
pub fn is_binary_available(binary: &str) -> bool {
    Command::new("which")
        .arg(binary)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
