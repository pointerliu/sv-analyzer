mod handlers;
mod protocol;

use anyhow::Result;
use serde::Deserialize;
use serde_json::Value;
use std::io::{self, BufRead, Write};

fn main() -> Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let lines = stdin.lock().lines();
    let mut initialized = false;

    for line in lines {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        #[derive(Deserialize)]
        #[allow(dead_code)]
        struct JsonRpcRequest {
            jsonrpc: String,
            id: Option<serde_json::Value>,
            method: String,
            #[serde(default)]
            params: Option<Value>,
        }

        let request: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("[mcp] failed to parse: {}", e);
                continue;
            }
        };

        let is_notification = request.id.is_none();

        if !initialized && request.method != "initialize" {
            eprintln!("[mcp] not initialized yet, ignoring: {}", request.method);
            continue;
        }

        if request.method == "initialize" {
            initialized = true;
        }

        let response =
            handlers::handle_request(&request.method, request.id.clone(), request.params);

        if !is_notification {
            if let Some(resp) = response {
                writeln!(stdout, "{}", serde_json::to_string(&resp)?)?;
                stdout.flush()?;
            }
        }

        if request.method == "shutdown" {
            break;
        }
    }
    Ok(())
}
