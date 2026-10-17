//! A minimal MCP client over a child process's stdio, for the integration
//! tests. It speaks just enough JSON-RPC to initialize the server, list its
//! tools and call them, which is exactly what an agent's host does.

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use base64::Engine as _;
use serde_json::{json, Value};

/// What a tool call returned.
#[derive(Debug, Default)]
pub struct Output {
    /// Every text block, joined by newlines.
    pub text: String,
    /// Every image block, decoded.
    pub images: Vec<Vec<u8>>,
}

/// A running server.
pub struct Client {
    child: Child,
    stdin: ChildStdin,
    reader: BufReader<ChildStdout>,
    next_id: u64,
    /// The instructions string the server sent when it initialized.
    pub instructions: String,
}

impl Client {
    /// Start the server binary with `args` and the given extra environment.
    pub fn start(args: &[&str], env: &[(&str, &Path)], cwd: &Path) -> Self {
        let mut command = Command::new(env!("CARGO_BIN_EXE_psxed-mcp"));
        command
            .args(args)
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        for (key, value) in env {
            command.env(key, value);
        }
        let mut child = command.spawn().expect("spawn psxed-mcp");
        let stdin = child.stdin.take().expect("child stdin");
        let reader = BufReader::new(child.stdout.take().expect("child stdout"));
        let mut client = Self {
            child,
            stdin,
            reader,
            next_id: 1,
            instructions: String::new(),
        };
        client.initialize();
        client
    }

    fn send(&mut self, message: &Value) {
        let line = serde_json::to_string(message).expect("serialize");
        writeln!(self.stdin, "{line}").expect("write to the server");
        self.stdin.flush().expect("flush");
    }

    fn request(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        loop {
            let mut line = String::new();
            let read = self
                .reader
                .read_line(&mut line)
                .expect("read from the server");
            assert!(
                read > 0,
                "the server closed its stdout while waiting for {method}"
            );
            let value: Value = match serde_json::from_str(&line) {
                Ok(v) => v,
                Err(_) => continue,
            };
            if value.get("id") != Some(&json!(id)) {
                // A notification or a server request; not ours.
                continue;
            }
            if let Some(error) = value.get("error") {
                return Err(error
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown error")
                    .to_string());
            }
            return Ok(value["result"].clone());
        }
    }

    fn initialize(&mut self) {
        let reply = self
            .request(
                "initialize",
                json!({
                    "protocolVersion": "2025-03-26",
                    "capabilities": {},
                    "clientInfo": {"name": "psxed-mcp-test", "version": "0"}
                }),
            )
            .expect("initialize");
        self.instructions = reply["instructions"].as_str().unwrap_or("").to_string();
        self.send(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
    }

    /// Every tool the server lists.
    #[allow(dead_code)]
    pub fn tools(&mut self) -> Vec<Value> {
        self.request("tools/list", json!({})).expect("tools/list")["tools"]
            .as_array()
            .cloned()
            .unwrap_or_default()
    }

    /// Call a tool. `Err` is the tool's error message.
    pub fn call(&mut self, tool: &str, arguments: Value) -> Result<Output, String> {
        let result = self.request("tools/call", json!({"name": tool, "arguments": arguments}))?;
        let mut out = Output::default();
        if result.get("isError") == Some(&json!(true)) {
            let text = result["content"][0]["text"]
                .as_str()
                .unwrap_or("tool error");
            return Err(text.to_string());
        }
        for block in result["content"].as_array().cloned().unwrap_or_default() {
            match block["type"].as_str() {
                Some("text") => {
                    if !out.text.is_empty() {
                        out.text.push('\n');
                    }
                    out.text.push_str(block["text"].as_str().unwrap_or(""));
                }
                Some("image") => {
                    let data = block["data"].as_str().unwrap_or("");
                    out.images.push(
                        base64::engine::general_purpose::STANDARD
                            .decode(data)
                            .expect("base64 image"),
                    );
                }
                _ => {}
            }
        }
        Ok(out)
    }

    /// Call a tool that must succeed, with a message naming it on failure.
    pub fn ok(&mut self, tool: &str, arguments: Value) -> Output {
        match self.call(tool, arguments) {
            Ok(out) => out,
            Err(error) => panic!("{tool} failed: {error}"),
        }
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
