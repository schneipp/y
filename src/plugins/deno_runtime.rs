use deno_core::{JsRuntime, RuntimeOptions, op2};
use serde::{Deserialize, Serialize};

/// Deno runtime wrapper for managing JavaScript plugins
pub struct DenoPluginRuntime {
    pub runtime: JsRuntime,
}

/// Data structure for key events passed to JS
#[derive(Serialize, Deserialize, Debug)]
pub struct JsKeyEvent {
    pub code: String,
    pub modifiers: Vec<String>,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub char: Option<String>,
}

/// Data structure for editor context passed to JS
#[derive(Serialize, Deserialize, Debug)]
pub struct JsContext {
    pub cursor_row: usize,
    pub cursor_col: usize,
    pub mode: String,
    pub filename: Option<String>,
    pub modified: bool,
    pub buffer_lines: Vec<String>,
}

/// Actions that JS plugins can request
#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
pub enum JsPluginAction {
    MoveCursor { row: usize, col: usize },
    InsertText { text: String },
    DeleteText { from_row: usize, from_col: usize, to_row: usize, to_col: usize },
    SetMode { mode: String },
    OpenFile { path: String, #[serde(skip_serializing_if = "Option::is_none")] line: Option<usize> },
    ShowPopup { title: String, content: Vec<String> },
    None,
}

/// Response from JS plugin
#[derive(Serialize, Deserialize, Debug)]
pub struct JsPluginResponse {
    pub consumed: bool,
    pub action: Option<JsPluginAction>,
}

// Deno ops for exposing Rust functions to JavaScript
#[op2(fast)]
fn op_log(#[string] msg: String) {
    eprintln!("[JS Plugin] {}", msg);
}

#[op2]
#[string]
fn op_read_file(#[string] path: String) -> Result<String, std::io::Error> {
    std::fs::read_to_string(&path)
}

#[op2]
#[string]
fn op_exec_command(#[string] cmd: String, #[serde] args: Vec<String>) -> Result<String, std::io::Error> {
    let output = std::process::Command::new(&cmd)
        .args(&args)
        .output()?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Command failed: {}", String::from_utf8_lossy(&output.stderr))
        ))
    }
}

impl DenoPluginRuntime {
    pub fn new() -> Self {
        // Create extensions with our ops
        deno_core::extension!(
            y_editor_ops,
            ops = [op_log, op_read_file, op_exec_command]
        );

        let mut runtime = JsRuntime::new(RuntimeOptions {
            extensions: vec![y_editor_ops::init_ops()],
            ..Default::default()
        });

        // Initialize the JavaScript environment with plugin API
        let init_code = r#"
            // Plugin API for JavaScript plugins
            globalThis.YEditor = {
                log: (msg) => Deno.core.ops.op_log(msg),
                readFile: (path) => Deno.core.ops.op_read_file(path),
                execCommand: (cmd, args) => Deno.core.ops.op_exec_command(cmd, args),
            };

            // Base plugin class
            globalThis.Plugin = class Plugin {
                constructor(name) {
                    this.name = name;
                    this.active = false;
                }

                // Override these methods
                handleKey(keyEvent, context) {
                    return { consumed: false, action: null };
                }

                render(area, context) {
                    return null;
                }

                activate() {
                    this.active = true;
                }

                deactivate() {
                    this.active = false;
                }

                isActive() {
                    return this.active;
                }
            };
        "#;

        runtime
            .execute_script("<init>", init_code)
            .expect("Failed to initialize plugin API");

        Self { runtime }
    }

    /// Load a JavaScript plugin from a file
    pub fn load_plugin(&mut self, plugin_path: &str) -> Result<(), String> {
        let code = std::fs::read_to_string(plugin_path)
            .map_err(|e| format!("Failed to read plugin file: {}", e))?;

        self.runtime
            .execute_script("<plugin>", code)
            .map_err(|e| format!("Failed to execute plugin: {}", e))?;

        Ok(())
    }

    /// Call a plugin method and get JSON result
    fn eval_json(&mut self, code: String) -> Result<serde_json::Value, String> {
        let result = self
            .runtime
            .execute_script("<eval>", code)
            .map_err(|e| format!("Failed to evaluate: {}", e))?;

        let scope = &mut self.runtime.handle_scope();
        let local = deno_core::v8::Local::new(scope, result);
        let result_str = local.to_rust_string_lossy(scope);

        serde_json::from_str(&result_str)
            .map_err(|e| format!("Failed to parse JSON: {}", e))
    }

    /// Handle a key event in a JS plugin
    pub fn handle_key(
        &mut self,
        plugin_name: &str,
        key_event: &JsKeyEvent,
        context: &JsContext,
    ) -> Result<JsPluginResponse, String> {
        let key_json = serde_json::to_string(key_event)
            .map_err(|e| format!("Failed to serialize key event: {}", e))?;
        let ctx_json = serde_json::to_string(context)
            .map_err(|e| format!("Failed to serialize context: {}", e))?;

        let code = format!(
            r#"
            (function() {{
                if (typeof {} !== 'undefined' && {} instanceof Plugin) {{
                    const result = {}.handleKey({}, {});
                    return JSON.stringify(result);
                }}
                return JSON.stringify({{ consumed: false, action: null }});
            }})()
            "#,
            plugin_name, plugin_name, plugin_name, key_json, ctx_json
        );

        let result = self.eval_json(code)?;
        serde_json::from_value(result)
            .map_err(|e| format!("Failed to parse handleKey response: {}", e))
    }

    /// Check if a plugin is active
    pub fn is_plugin_active(&mut self, plugin_name: &str) -> bool {
        let code = format!(
            r#"
            (function() {{
                if (typeof {} !== 'undefined' && {} instanceof Plugin) {{
                    return {}.isActive();
                }}
                return false;
            }})()
            "#,
            plugin_name, plugin_name, plugin_name
        );

        if let Ok(result) = self.runtime.execute_script("<check>", code) {
            let scope = &mut self.runtime.handle_scope();
            let local = deno_core::v8::Local::new(scope, result);
            return local.is_true();
        }

        false
    }

    /// Activate a JS plugin
    pub fn activate_plugin(&mut self, plugin_name: &str) -> Result<(), String> {
        let code = format!(
            r#"
            if (typeof {} !== 'undefined' && {} instanceof Plugin) {{
                {}.activate();
            }}
            "#,
            plugin_name, plugin_name, plugin_name
        );

        self.runtime
            .execute_script("<activate>", code)
            .map_err(|e| format!("Failed to activate plugin: {}", e))?;

        Ok(())
    }

    /// Deactivate a JS plugin
    pub fn deactivate_plugin(&mut self, plugin_name: &str) -> Result<(), String> {
        let code = format!(
            r#"
            if (typeof {} !== 'undefined' && {} instanceof Plugin) {{
                {}.deactivate();
            }}
            "#,
            plugin_name, plugin_name, plugin_name
        );

        self.runtime
            .execute_script("<deactivate>", code)
            .map_err(|e| format!("Failed to deactivate plugin: {}", e))?;

        Ok(())
    }
}
