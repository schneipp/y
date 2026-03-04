use serde_json::{json, Value};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;

#[derive(Debug)]
pub enum LspMessage {
    Response {
        id: i64,
        result: Option<Value>,
        error: Option<Value>,
    },
    Notification {
        method: String,
        params: Option<Value>,
    },
    /// Server-initiated request (has id + method, expects a response)
    ServerRequest {
        id: i64,
        method: String,
        params: Option<Value>,
    },
}

pub struct LspClient {
    process: Child,
    writer: BufWriter<std::process::ChildStdin>,
    pub rx: mpsc::Receiver<LspMessage>,
    next_id: AtomicI64,
    reader_alive: Arc<AtomicBool>,
    stderr_path: Option<String>,
}

impl LspClient {
    pub fn start(binary: &str, args: &[String]) -> Result<Self, String> {
        // Capture stderr to a temp file for diagnostics
        let stderr_path = format!("/tmp/y-lsp-{}.log", binary.replace('/', "_"));
        let stderr_file = std::fs::File::create(&stderr_path).ok();
        let stderr = stderr_file
            .map(Stdio::from)
            .unwrap_or(Stdio::null());

        let mut child = Command::new(binary)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(stderr)
            .spawn()
            .map_err(|e| format!("Failed to spawn {}: {}", binary, e))?;

        let stdin = child.stdin.take().ok_or("Failed to capture stdin")?;
        let stdout = child.stdout.take().ok_or("Failed to capture stdout")?;

        let writer = BufWriter::new(stdin);
        let (tx, rx) = mpsc::channel();

        let reader_alive = Arc::new(AtomicBool::new(true));
        let reader_alive_clone = reader_alive.clone();

        // Background reader thread
        thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                match read_message(&mut reader) {
                    Ok(msg) => {
                        if tx.send(msg).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            reader_alive_clone.store(false, Ordering::SeqCst);
        });

        Ok(Self {
            process: child,
            writer,
            rx,
            next_id: AtomicI64::new(1),
            reader_alive,
            stderr_path: Some(stderr_path),
        })
    }

    pub fn is_reader_alive(&self) -> bool {
        self.reader_alive.load(Ordering::SeqCst)
    }

    pub fn stderr_path(&self) -> Option<&str> {
        self.stderr_path.as_deref()
    }

    pub fn send_request(&mut self, method: &str, params: Value) -> i64 {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let msg = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        self.send_raw(&msg);
        id
    }

    /// Send a response to a server-initiated request.
    pub fn send_response(&mut self, id: i64, result: Value) {
        let msg = json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": result,
        });
        self.send_raw(&msg);
    }

    pub fn send_notification(&mut self, method: &str, params: Value) {
        let msg = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });
        self.send_raw(&msg);
    }

    fn send_raw(&mut self, msg: &Value) {
        let body = serde_json::to_string(msg).unwrap();
        let header = format!("Content-Length: {}\r\n\r\n", body.len());
        let _ = self.writer.write_all(header.as_bytes());
        let _ = self.writer.write_all(body.as_bytes());
        let _ = self.writer.flush();
    }

    /// Non-blocking drain of all pending messages.
    pub fn poll_messages(&self) -> Vec<LspMessage> {
        let mut msgs = Vec::new();
        while let Ok(msg) = self.rx.try_recv() {
            msgs.push(msg);
        }
        msgs
    }

    pub fn shutdown(&mut self) {
        let _ = self.send_request("shutdown", Value::Null);
        self.send_notification("exit", Value::Null);
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}

impl Drop for LspClient {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Read a single Content-Length framed JSON-RPC message from the reader.
fn read_message<R: BufRead>(reader: &mut R) -> Result<LspMessage, String> {
    // Read headers
    let mut content_length: usize = 0;
    loop {
        let mut line = String::new();
        let bytes_read = reader.read_line(&mut line).map_err(|e| e.to_string())?;
        if bytes_read == 0 {
            return Err("EOF".into());
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            break;
        }
        // Case-insensitive Content-Length parsing
        let lower = trimmed.to_lowercase();
        if let Some(len_str) = lower.strip_prefix("content-length:") {
            let len_str = len_str.trim();
            content_length = len_str
                .parse()
                .map_err(|e: std::num::ParseIntError| e.to_string())?;
        }
    }

    if content_length == 0 {
        return Err("No Content-Length header".into());
    }

    // Read body
    let mut body = vec![0u8; content_length];
    reader.read_exact(&mut body).map_err(|e| e.to_string())?;
    let body_str = String::from_utf8(body).map_err(|e| e.to_string())?;
    let value: Value = serde_json::from_str(&body_str).map_err(|e| e.to_string())?;

    // Parse as response, server request, or notification
    if let Some(id) = value.get("id") {
        if value.get("method").is_some() {
            // Server-initiated request (has both id and method)
            Ok(LspMessage::ServerRequest {
                id: id.as_i64().unwrap_or(0),
                method: value["method"].as_str().unwrap_or("").to_string(),
                params: value.get("params").cloned(),
            })
        } else {
            Ok(LspMessage::Response {
                id: id.as_i64().unwrap_or(0),
                result: value.get("result").cloned(),
                error: value.get("error").cloned(),
            })
        }
    } else {
        Ok(LspMessage::Notification {
            method: value["method"].as_str().unwrap_or("").to_string(),
            params: value.get("params").cloned(),
        })
    }
}
