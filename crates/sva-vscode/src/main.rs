mod handlers;
mod protocol;
mod types;

use anyhow::Result;
use serde::Deserialize;
use serde_json::Value;
use std::io::{self, BufRead, Write};

fn main() -> Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let lines = stdin.lock().lines();

    for line in lines {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        #[derive(Deserialize)]
        #[allow(dead_code)]
        struct RawRequest {
            jsonrpc: String,
            id: Option<serde_json::Value>,
            method: String,
            #[serde(default)]
            params: Option<Value>,
        }

        let request: RawRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("[vscode-backend] failed to parse: {}", e);
                continue;
            }
        };

        let response = handlers::handle(&request.method, request.params);

        if response.id.is_some() {
            writeln!(stdout, "{}", serde_json::to_string(&response)?)?;
            stdout.flush()?;
        }
    }
    Ok(())
}
