use crate::config::LspServerConfig;
use std::path::PathBuf;
use std::process::Command;

/// LSP install directory: ~/.local/share/y/lsp
fn lsp_install_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".local").join("share").join("y").join("lsp")
}

/// The bin directory where locally installed LSP binaries live.
pub fn lsp_bin_dir() -> PathBuf {
    lsp_install_dir().join("node_modules").join(".bin")
}

struct LspInstallInfo {
    name: &'static str,
    strategies: &'static [InstallStrategyRef],
}

// Workaround: can't have enum with &'static str in const context easily,
// so use a simpler representation for the const table.
struct InstallStrategyRef {
    kind: &'static str, // "npm", "pip", "rustup", "go", "cargo", "github"
    packages: &'static [&'static str],
}

const INSTALL_INFO: &[LspInstallInfo] = &[
    LspInstallInfo {
        name: "rust-analyzer",
        strategies: &[
            InstallStrategyRef { kind: "rustup", packages: &["component", "add", "rust-analyzer"] },
        ],
    },
    LspInstallInfo {
        name: "typescript-language-server",
        strategies: &[
            InstallStrategyRef { kind: "npm", packages: &["typescript-language-server", "typescript"] },
        ],
    },
    LspInstallInfo {
        name: "pyright",
        strategies: &[
            InstallStrategyRef { kind: "npm", packages: &["pyright"] },
            InstallStrategyRef { kind: "pip", packages: &["pyright"] },
        ],
    },
    LspInstallInfo {
        name: "gopls",
        strategies: &[
            InstallStrategyRef { kind: "go", packages: &["golang.org/x/tools/gopls@latest"] },
        ],
    },
    LspInstallInfo {
        name: "clangd",
        strategies: &[
            // clangd is a system binary — no user-local install available
            // but we can try npm's @anthropic-ai/clangd wrapper if it exists, or just guide the user
        ],
    },
    LspInstallInfo {
        name: "lua-language-server",
        strategies: &[
            InstallStrategyRef { kind: "npm", packages: &["lua-language-server"] },
        ],
    },
    LspInstallInfo {
        name: "zls",
        strategies: &[
            // No npm package; needs system install or manual download
        ],
    },
    LspInstallInfo {
        name: "bash-language-server",
        strategies: &[
            InstallStrategyRef { kind: "npm", packages: &["bash-language-server"] },
        ],
    },
    LspInstallInfo {
        name: "intelephense",
        strategies: &[
            InstallStrategyRef { kind: "npm", packages: &["intelephense"] },
        ],
    },
];

fn run_npm_install(packages: &[&str]) -> Result<String, String> {
    let dir = lsp_install_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("Failed to create dir: {}", e))?;

    let npm = if is_binary_available("npm") { "npm" } else {
        return Err("npm not found — install Node.js first".into());
    };

    // Ensure package.json exists so npm does a local install
    let pkg_json = dir.join("package.json");
    if !pkg_json.exists() {
        std::fs::write(&pkg_json, "{\"private\":true}")
            .map_err(|e| format!("Failed to create package.json: {}", e))?;
    }

    let mut full_args: Vec<String> = vec!["install".into()];
    for pkg in packages {
        full_args.push(pkg.to_string());
    }

    let result = Command::new(npm)
        .args(&full_args)
        .current_dir(&dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output();

    match result {
        Ok(out) if out.status.success() => {
            Ok(format!("Installed via npm to {}", lsp_bin_dir().display()))
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            Err(format!("npm failed: {}", stderr.lines().find(|l| !l.is_empty()).unwrap_or("unknown error")))
        }
        Err(e) => Err(format!("Failed to run npm: {}", e)),
    }
}

fn run_pip_install(packages: &[&str]) -> Result<String, String> {
    let pip = if is_binary_available("pip3") { "pip3" }
        else if is_binary_available("pip") { "pip" }
        else { return Err("pip not found — install Python first".into()); };

    let mut args: Vec<String> = vec!["install".into(), "--user".into()];
    for pkg in packages {
        args.push(pkg.to_string());
    }

    let result = Command::new(pip)
        .args(&args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output();

    match result {
        Ok(out) if out.status.success() => Ok("Installed via pip --user".into()),
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            Err(format!("pip failed: {}", stderr.lines().next().unwrap_or("unknown")))
        }
        Err(e) => Err(format!("Failed to run pip: {}", e)),
    }
}

fn run_rustup(args: &[&str]) -> Result<String, String> {
    if !is_binary_available("rustup") {
        return Err("rustup not found — install Rust first".into());
    }
    let result = Command::new("rustup")
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output();

    match result {
        Ok(out) if out.status.success() => Ok("Installed via rustup".into()),
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            Err(format!("rustup failed: {}", stderr.lines().next().unwrap_or("unknown")))
        }
        Err(e) => Err(format!("Failed to run rustup: {}", e)),
    }
}

fn run_go_install(args: &[&str]) -> Result<String, String> {
    if !is_binary_available("go") {
        return Err("go not found — install Go first".into());
    }
    let mut full_args = vec!["install".to_string()];
    for a in args {
        full_args.push(a.to_string());
    }
    let result = Command::new("go")
        .args(&full_args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output();

    match result {
        Ok(out) if out.status.success() => Ok("Installed via go install".into()),
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            Err(format!("go install failed: {}", stderr.lines().next().unwrap_or("unknown")))
        }
        Err(e) => Err(format!("Failed to run go: {}", e)),
    }
}

/// Attempt to install an LSP server into user-local directories.
pub fn install_server(name: &str) -> Result<String, String> {
    let info = INSTALL_INFO.iter().find(|i| i.name == name)
        .ok_or_else(|| format!("No install info for '{}'", name))?;

    if info.strategies.is_empty() {
        return Err(format!("{} requires system installation — use your package manager", name));
    }

    for strategy in info.strategies {
        let result = match strategy.kind {
            "npm" => run_npm_install(strategy.packages),
            "pip" => run_pip_install(strategy.packages),
            "rustup" => run_rustup(strategy.packages),
            "go" => run_go_install(strategy.packages),
            _ => continue,
        };

        match result {
            Ok(msg) => return Ok(msg),
            Err(_) => continue, // try next strategy
        }
    }

    // All strategies failed — return the last one's error
    let last = info.strategies.last().unwrap();
    match last.kind {
        "npm" => run_npm_install(last.packages),
        "pip" => run_pip_install(last.packages),
        "rustup" => run_rustup(last.packages),
        "go" => run_go_install(last.packages),
        _ => Err(format!("No install method available for {}", name)),
    }
}

/// Get the install command as a display string.
pub fn install_command_str(name: &str) -> Option<String> {
    let info = INSTALL_INFO.iter().find(|i| i.name == name)?;
    let strategy = info.strategies.first()?;
    match strategy.kind {
        "npm" => Some(format!("npm install --prefix ~/.local/share/y/lsp {}", strategy.packages.join(" "))),
        "pip" => Some(format!("pip install --user {}", strategy.packages.join(" "))),
        "rustup" => Some(format!("rustup {}", strategy.packages.join(" "))),
        "go" => Some(format!("go install {}", strategy.packages.join(" "))),
        _ => None,
    }
}

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
        LspServerConfig {
            name: "intelephense".into(),
            language: "php".into(),
            binary: "intelephense".into(),
            args: vec!["--stdio".into()],
            extensions: vec!["php".into()],
            enabled: false,
        },
    ]
}

/// Check if a binary is available on PATH or in our local LSP bin dir.
pub fn is_binary_available(binary: &str) -> bool {
    // Check local LSP bin dir first
    let local_bin = lsp_bin_dir().join(binary);
    if local_bin.exists() {
        return true;
    }
    // Check system PATH
    Command::new("which")
        .arg(binary)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Resolve a binary name to its full path, checking our local dir first.
pub fn resolve_binary(binary: &str) -> String {
    let local_bin = lsp_bin_dir().join(binary);
    if local_bin.exists() {
        return local_bin.to_string_lossy().to_string();
    }
    binary.to_string()
}
