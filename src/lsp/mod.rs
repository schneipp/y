pub mod client;
pub mod types;

use crate::config::Config;
use client::{LspClient, LspMessage};
use serde_json::{json, Value};
use std::collections::HashMap;

pub struct LspResponse {
    pub id: i64,
    pub result: Option<Value>,
    pub error: Option<Value>,
}

struct PendingDidOpen {
    server_name: String,
    uri: String,
    language_id: String,
    text: String,
}

pub struct LspManager {
    clients: HashMap<String, LspClient>,
    initialized: HashMap<String, bool>,
    init_request_ids: HashMap<String, i64>,
    open_documents: HashMap<String, Vec<String>>,
    pub pending_responses: Vec<LspResponse>,
    /// Per-server human-readable status (e.g. "indexing (45%)")
    pub status: HashMap<String, String>,
    /// didOpen notifications queued before server finished initializing
    pending_did_opens: Vec<PendingDidOpen>,
    /// Recent message log for diagnostics (capped at 30)
    message_log: Vec<String>,
}

impl LspManager {
    pub fn new() -> Self {
        Self {
            clients: HashMap::new(),
            initialized: HashMap::new(),
            init_request_ids: HashMap::new(),
            open_documents: HashMap::new(),
            pending_responses: Vec::new(),
            status: HashMap::new(),
            pending_did_opens: Vec::new(),
            message_log: Vec::new(),
        }
    }

    /// Start an LSP server for the given file extension if one is enabled and not already running.
    pub fn ensure_server_for_extension(&mut self, ext: &str, config: &Config, root_path: &str) {
        let server_config = match config.server_for_extension(ext) {
            Some(s) => s.clone(),
            None => return,
        };

        if self.clients.contains_key(&server_config.name) {
            return;
        }

        let client = match LspClient::start(&server_config.binary, &server_config.args) {
            Ok(c) => c,
            Err(_) => return,
        };

        let name = server_config.name.clone();
        self.clients.insert(name.clone(), client);
        self.initialized.insert(name.clone(), false);
        self.status.insert(name.clone(), "starting".to_string());

        // Send initialize request
        let root_uri = format!("file://{}", root_path);
        let id = self.clients.get_mut(&name).unwrap().send_request(
            "initialize",
            json!({
                "processId": std::process::id(),
                "rootUri": root_uri,
                "capabilities": {
                    "textDocument": {
                        "synchronization": {
                            "openClose": true,
                            "change": 1
                        },
                        "completion": {
                            "completionItem": {
                                "snippetSupport": true,
                                "resolveSupport": {
                                    "properties": ["documentation", "detail"]
                                }
                            }
                        },
                        "formatting": {
                            "dynamicRegistration": false
                        }
                    },
                    "window": {
                        "workDoneProgress": true
                    }
                }
            }),
        );
        self.init_request_ids.insert(name, id);
    }

    /// Poll all clients for messages. Handles initialize responses, server
    /// requests, and progress notifications automatically. Non-init responses
    /// are collected in `pending_responses` for the editor to process.
    pub fn poll(&mut self) {
        let names: Vec<String> = self.clients.keys().cloned().collect();
        for name in names {
            // Detect dead reader thread and update status
            if let Some(client) = self.clients.get(&name) {
                if !client.is_reader_alive()
                    && !self.initialized.get(&name).copied().unwrap_or(false)
                {
                    if self.status.get(&name).map(|s| s.as_str()) != Some("error") {
                        self.status.insert(name.clone(), "error".to_string());
                        self.log(format!("{}: reader thread died", name));
                    }
                }
            }

            let messages = if let Some(client) = self.clients.get(&name) {
                client.poll_messages()
            } else {
                continue;
            };

            for msg in messages {
                match msg {
                    LspMessage::Response { id, result, error } => {
                        let expected_init_id = self.init_request_ids.get(&name).copied();
                        self.log(format!(
                            "recv Response id={} (init_id={:?}, match={})",
                            id,
                            expected_init_id,
                            expected_init_id == Some(id)
                        ));
                        if expected_init_id == Some(id) {
                            self.init_request_ids.remove(&name);
                            self.initialized.insert(name.clone(), true);
                            self.status.insert(name.clone(), "ready".to_string());
                            if let Some(client) = self.clients.get_mut(&name) {
                                client.send_notification("initialized", json!({}));
                            }
                        } else {
                            self.pending_responses
                                .push(LspResponse { id, result, error });
                        }
                    }
                    LspMessage::ServerRequest { id, method, .. } => {
                        self.log(format!("recv ServerRequest id={} method={}", id, method));
                        // Respond to all server-initiated requests
                        if let Some(client) = self.clients.get_mut(&name) {
                            client.send_response(id, Value::Null);
                        }
                    }
                    LspMessage::Notification { method, params } => {
                        self.log(format!("recv Notification method={}", method));
                        self.handle_notification(&name, &method, params.as_ref());
                    }
                }
            }
        }

        // Flush any queued didOpen notifications for newly-initialized servers
        self.flush_pending_did_opens();
    }

    fn handle_notification(&mut self, server: &str, method: &str, params: Option<&Value>) {
        match method {
            "$/progress" => {
                if let Some(params) = params {
                    self.handle_progress(server, params);
                }
            }
            "window/logMessage" | "window/showMessage" => {
                // Could log these; for now just update status with the message
                if let Some(msg) = params
                    .and_then(|p| p.get("message"))
                    .and_then(|v| v.as_str())
                {
                    // Only show short messages as status
                    if msg.len() <= 60 {
                        self.status.insert(server.to_string(), msg.to_string());
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_progress(&mut self, server: &str, params: &Value) {
        let value = match params.get("value") {
            Some(v) => v,
            None => return,
        };

        let kind = value
            .get("kind")
            .and_then(|k| k.as_str())
            .unwrap_or("");
        let title = value
            .get("title")
            .and_then(|t| t.as_str())
            .unwrap_or("");
        let message = value
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("");
        let percentage = value.get("percentage").and_then(|p| p.as_u64());

        match kind {
            "begin" => {
                let status = if let Some(pct) = percentage {
                    format!("{} ({}%)", title, pct)
                } else {
                    title.to_string()
                };
                self.status.insert(server.to_string(), status);
            }
            "report" => {
                let status = if let Some(pct) = percentage {
                    if !message.is_empty() {
                        format!("{} ({}%)", message, pct)
                    } else if !title.is_empty() {
                        format!("{} ({}%)", title, pct)
                    } else {
                        format!("working ({}%)", pct)
                    }
                } else if !message.is_empty() {
                    message.to_string()
                } else {
                    // keep existing status
                    return;
                };
                self.status.insert(server.to_string(), status);
            }
            "end" => {
                self.status.insert(server.to_string(), "ready".to_string());
            }
            _ => {}
        }
    }

    /// Get the status string for a server that handles the given file extension.
    pub fn status_for_extension(&self, ext: &str, config: &Config) -> Option<(String, String)> {
        let server_config = config.server_for_extension(ext)?;
        let status = self
            .status
            .get(&server_config.name)
            .cloned()
            .unwrap_or_else(|| "stopped".to_string());
        Some((server_config.name.clone(), status))
    }

    fn log(&mut self, msg: String) {
        if self.message_log.len() >= 30 {
            self.message_log.remove(0);
        }
        self.message_log.push(msg);
    }

    /// Get debug info for all servers.
    pub fn debug_info(&self) -> Vec<String> {
        let mut lines = Vec::new();
        if self.clients.is_empty() {
            lines.push("No LSP servers running".to_string());
            return lines;
        }
        for (name, client) in &self.clients {
            let init = self
                .initialized
                .get(name)
                .copied()
                .unwrap_or(false);
            let status = self
                .status
                .get(name)
                .map(|s| s.as_str())
                .unwrap_or("unknown");
            let reader = if client.is_reader_alive() {
                "alive"
            } else {
                "DEAD"
            };
            let init_id = self.init_request_ids.get(name);
            lines.push(format!(
                "{}: {} | init:{} reader:{} init_req_id:{:?}",
                name, status, init, reader, init_id
            ));
            if let Some(docs) = self.open_documents.get(name) {
                for doc in docs {
                    lines.push(format!("  open: {}", doc));
                }
            }
            if let Some(stderr_path) = client.stderr_path() {
                // Show last few lines of stderr for quick diagnosis
                if let Ok(content) = std::fs::read_to_string(stderr_path) {
                    let stderr_lines: Vec<&str> = content.lines().collect();
                    if !stderr_lines.is_empty() {
                        lines.push("  stderr:".to_string());
                        for l in stderr_lines.iter().rev().take(5).rev() {
                            lines.push(format!("    {}", l));
                        }
                    }
                }
            }
        }
        if !self.pending_did_opens.is_empty() {
            lines.push(format!(
                "pending didOpens: {}",
                self.pending_did_opens.len()
            ));
        }
        // Show recent message log
        if !self.message_log.is_empty() {
            lines.push("--- message log ---".to_string());
            for entry in &self.message_log {
                lines.push(entry.clone());
            }
        }
        lines
    }

    /// Send textDocument/didChange (full document sync).
    pub fn did_change(&mut self, server_name: &str, uri: &str, version: i64, text: &str) {
        if let Some(client) = self.clients.get_mut(server_name) {
            client.send_notification(
                "textDocument/didChange",
                json!({
                    "textDocument": {
                        "uri": uri,
                        "version": version,
                    },
                    "contentChanges": [{
                        "text": text,
                    }]
                }),
            );
        }
    }

    /// Send textDocument/completion request. Returns the request ID if sent.
    pub fn request_completion(
        &mut self,
        server_name: &str,
        uri: &str,
        line: usize,
        character: usize,
    ) -> Option<i64> {
        if !self.is_server_ready(server_name) {
            return None;
        }
        if let Some(client) = self.clients.get_mut(server_name) {
            let id = client.send_request(
                "textDocument/completion",
                json!({
                    "textDocument": { "uri": uri },
                    "position": { "line": line, "character": character },
                }),
            );
            Some(id)
        } else {
            None
        }
    }

    /// Send textDocument/didOpen for a file. Queues if server isn't initialized yet.
    pub fn did_open(&mut self, server_name: &str, uri: &str, language_id: &str, text: &str) {
        if !self.is_server_ready(server_name) {
            self.pending_did_opens.push(PendingDidOpen {
                server_name: server_name.to_string(),
                uri: uri.to_string(),
                language_id: language_id.to_string(),
                text: text.to_string(),
            });
            return;
        }
        self.send_did_open_now(server_name, uri, language_id, text);
    }

    fn send_did_open_now(&mut self, server_name: &str, uri: &str, language_id: &str, text: &str) {
        if let Some(client) = self.clients.get_mut(server_name) {
            client.send_notification(
                "textDocument/didOpen",
                json!({
                    "textDocument": {
                        "uri": uri,
                        "languageId": language_id,
                        "version": 1,
                        "text": text,
                    }
                }),
            );
            self.open_documents
                .entry(server_name.to_string())
                .or_default()
                .push(uri.to_string());
        }
    }

    /// Drain any pending didOpen notifications for servers that are now ready.
    fn flush_pending_did_opens(&mut self) {
        let ready: Vec<PendingDidOpen> = self
            .pending_did_opens
            .drain(..)
            .collect();
        for pending in ready {
            if self.is_server_ready(&pending.server_name) {
                self.send_did_open_now(
                    &pending.server_name,
                    &pending.uri,
                    &pending.language_id,
                    &pending.text,
                );
            } else {
                // Still not ready, re-queue
                self.pending_did_opens.push(pending);
            }
        }
    }

    /// Send textDocument/formatting request. Returns the request ID if sent.
    pub fn request_formatting(
        &mut self,
        server_name: &str,
        uri: &str,
        tab_size: u32,
        insert_spaces: bool,
    ) -> Option<i64> {
        if !self.is_server_ready(server_name) {
            return None;
        }
        if let Some(client) = self.clients.get_mut(server_name) {
            let id = client.send_request(
                "textDocument/formatting",
                json!({
                    "textDocument": { "uri": uri },
                    "options": {
                        "tabSize": tab_size,
                        "insertSpaces": insert_spaces,
                    },
                }),
            );
            Some(id)
        } else {
            None
        }
    }

    /// Block until we receive a response with the given ID, or timeout.
    /// Returns the response result, or None on timeout/error.
    /// Also processes any other messages that arrive while waiting.
    pub fn wait_for_response(
        &mut self,
        server_name: &str,
        request_id: i64,
        timeout: std::time::Duration,
    ) -> Option<Value> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            if std::time::Instant::now() >= deadline {
                return None;
            }
            let messages = if let Some(client) = self.clients.get(server_name) {
                client.poll_messages()
            } else {
                return None;
            };

            for msg in messages {
                match msg {
                    LspMessage::Response { id, result, error } => {
                        if id == request_id {
                            if error.is_some() {
                                return None;
                            }
                            return result;
                        }
                        // Stash other responses
                        self.pending_responses
                            .push(LspResponse { id, result, error });
                    }
                    LspMessage::ServerRequest { id, .. } => {
                        if let Some(client) = self.clients.get_mut(server_name) {
                            client.send_response(id, Value::Null);
                        }
                    }
                    LspMessage::Notification { method, params } => {
                        self.handle_notification(server_name, &method, params.as_ref());
                    }
                }
            }

            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    pub fn is_server_ready(&self, name: &str) -> bool {
        self.initialized.get(name).copied().unwrap_or(false)
    }

    pub fn shutdown_all(&mut self) {
        for (_, mut client) in self.clients.drain() {
            client.shutdown();
        }
        self.initialized.clear();
        self.init_request_ids.clear();
        self.open_documents.clear();
        self.status.clear();
        self.pending_did_opens.clear();
        self.message_log.clear();
    }
}

impl Drop for LspManager {
    fn drop(&mut self) {
        self.shutdown_all();
    }
}
