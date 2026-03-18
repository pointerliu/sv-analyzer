use crate::protocol::{JsonRpcError, JsonRpcResponse};
use crate::types::{BlockizeParams, CoverageParams, SliceParams, WaveParams};
use dac26_app::services::{
    blockize, coverage_report, slice_dynamic, slice_static, wave_value, BlockizeRequest,
    CoverageReportRequest, DynamicSliceRequest, StaticSliceRequest, WaveValueRequest,
};
use serde_json::Value;

pub fn handle(method: &str, params: Option<Value>) -> JsonRpcResponse {
    match method {
        "initialize" => handle_initialize(),
        "shutdown" => handle_shutdown(),
        "blockize" => handle_blockize(params),
        "slice" => handle_slice(params),
        "coverage" => handle_coverage(params),
        "wave" => handle_wave(params),
        _ => JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id: None,
            result: None,
            error: Some(JsonRpcError::method_not_found()),
        },
    }
}

fn handle_initialize() -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0".into(),
        id: None,
        result: Some(serde_json::json!({
            "protocolVersion": "1.0",
            "serverName": "dac26_vscode_backend",
            "capabilities": {}
        })),
        error: None,
    }
}

fn handle_shutdown() -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0".into(),
        id: None,
        result: Some(serde_json::json!({"ok": true})),
        error: None,
    }
}

fn handle_blockize(params: Option<Value>) -> JsonRpcResponse {
    let params: BlockizeParams = match params.and_then(|p| serde_json::from_value(p).ok()) {
        Some(p) => p,
        None => return bad_params("invalid params"),
    };
    let req = BlockizeRequest {
        sv_files: params
            .sv_files
            .iter()
            .map(std::path::PathBuf::from)
            .collect(),
    };
    match blockize(req) {
        Ok(result) => JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id: None,
            result: Some(serde_json::to_value(result).unwrap_or_default()),
            error: None,
        },
        Err(e) => internal_error(e),
    }
}

fn handle_slice(params: Option<Value>) -> JsonRpcResponse {
    let params: SliceParams = match params.and_then(|p| serde_json::from_value(p).ok()) {
        Some(p) => p,
        None => return bad_params("invalid params"),
    };

    if params.r#static {
        let req = StaticSliceRequest {
            sv_files: params
                .sv_files
                .iter()
                .map(std::path::PathBuf::from)
                .collect(),
            signal: params.signal,
        };
        match slice_static(req) {
            Ok(result) => JsonRpcResponse {
                jsonrpc: "2.0".into(),
                id: None,
                result: Some(serde_json::to_value(result).unwrap_or_default()),
                error: None,
            },
            Err(e) => internal_error(e),
        }
    } else {
        let vcd = match params.vcd.as_ref() {
            Some(v) => v,
            None => return bad_params("vcd required for dynamic slice"),
        };
        let time = match params.time {
            Some(t) => t,
            None => return bad_params("time required for dynamic slice"),
        };
        let min_time = match params.min_time {
            Some(m) => m,
            None => return bad_params("min_time required for dynamic slice"),
        };

        let req = DynamicSliceRequest {
            sv_files: params
                .sv_files
                .iter()
                .map(std::path::PathBuf::from)
                .collect(),
            signal: params.signal,
            vcd: std::path::PathBuf::from(vcd.clone()),
            time,
            min_time,
            clock: params.clock,
            clk_step: params.clk_step,
        };
        match slice_dynamic(req) {
            Ok(result) => JsonRpcResponse {
                jsonrpc: "2.0".into(),
                id: None,
                result: Some(serde_json::to_value(result).unwrap_or_default()),
                error: None,
            },
            Err(e) => internal_error(e),
        }
    }
}

fn handle_coverage(params: Option<Value>) -> JsonRpcResponse {
    let params: CoverageParams = match params.and_then(|p| serde_json::from_value(p).ok()) {
        Some(p) => p,
        None => return bad_params("invalid params"),
    };
    let req = CoverageReportRequest {
        sv_files: params
            .sv_files
            .iter()
            .map(std::path::PathBuf::from)
            .collect(),
        vcd: std::path::PathBuf::from(params.vcd),
        time: params.time,
    };
    match coverage_report(req) {
        Ok(result) => JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id: None,
            result: Some(serde_json::to_value(result).unwrap_or_default()),
            error: None,
        },
        Err(e) => internal_error(e),
    }
}

fn handle_wave(params: Option<Value>) -> JsonRpcResponse {
    let params: WaveParams = match params.and_then(|p| serde_json::from_value(p).ok()) {
        Some(p) => p,
        None => return bad_params("invalid params"),
    };
    let req = WaveValueRequest {
        vcd: std::path::PathBuf::from(params.vcd),
        signal: params.signal,
        time: params.time,
    };
    match wave_value(req) {
        Ok(Some(result)) => JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id: None,
            result: Some(serde_json::json!({
                "raw_bits": result.raw_bits,
                "pretty_hex": result.pretty_hex
            })),
            error: None,
        },
        Ok(None) => JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id: None,
            result: Some(serde_json::Value::Null),
            error: None,
        },
        Err(e) => internal_error(e),
    }
}

fn bad_params(msg: &str) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0".into(),
        id: None,
        result: None,
        error: Some(JsonRpcError::invalid_params(msg)),
    }
}

fn internal_error(e: anyhow::Error) -> JsonRpcResponse {
    eprintln!("[vscode-backend error] {}", e);
    JsonRpcResponse {
        jsonrpc: "2.0".into(),
        id: None,
        result: None,
        error: Some(JsonRpcError::internal(e.to_string())),
    }
}
