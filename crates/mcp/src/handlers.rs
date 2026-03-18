use crate::protocol::JsonRpcResponse;
use dac26_app::services::{
    blockize, coverage_report, slice_dynamic, slice_static, wave_value, BlockizeRequest,
    CoverageReportRequest, DynamicSliceRequest, StaticSliceRequest, WaveValueRequest,
};
use serde_json::{json, Value};
use std::path::PathBuf;

pub fn handle_initialize(id: Option<serde_json::Value>, _params: Value) -> Value {
    let resp = JsonRpcResponse::result(
        id,
        json!({
            "protocolVersion": "2024-11-05",
            "serverInfo": {
                "name": "dac26-mcp-server",
                "version": "0.1.0"
            },
            "capabilities": {
                "tools": {}
            }
        }),
    );
    serde_json::to_value(resp).unwrap()
}

pub fn handle_tools_list(id: Option<serde_json::Value>) -> Value {
    let resp = JsonRpcResponse::result(
        id,
        json!({
            "tools": [
                tool_schema("blockize", "Run dataflow blockization on SV files", &[
                    ("sv_files", "array of paths to SV files", "string[]")
                ]),
                tool_schema("slice_static", "Static backward slice from a signal", &[
                    ("sv_files", "array of paths to SV files", "string[]"),
                    ("signal", "signal name to slice from", "string")
                ]),
                tool_schema("slice_dynamic", "Dynamic slicing with waveform", &[
                    ("sv_files", "array of paths to SV files", "string[]"),
                    ("signal", "signal name to slice from", "string"),
                    ("vcd", "path to VCD waveform file", "string"),
                    ("time", "time to slice at", "number"),
                    ("min_time", "minimum time boundary", "number"),
                    ("clock", "clock signal name (optional)", "string"),
                    ("clk_step", "clock period (optional)", "number")
                ]),
                tool_schema("coverage_report", "Generate coverage report", &[
                    ("sv_files", "array of paths to SV files", "string[]"),
                    ("vcd", "path to VCD waveform file", "string"),
                    ("time", "time to evaluate at", "number")
                ]),
                tool_schema("wave_value", "Read signal value at time", &[
                    ("vcd", "path to VCD waveform file", "string"),
                    ("signal", "signal name", "string"),
                    ("time", "time to read", "number")
                ])
            ]
        }),
    );
    serde_json::to_value(resp).unwrap()
}

fn tool_schema(name: &str, description: &str, params: &[(&str, &str, &str)]) -> serde_json::Value {
    let mut input_props = serde_json::Map::new();
    for (pname, pdesc, ptype) in params {
        input_props.insert(
            pname.to_string(),
            json!({
                "type": ptype,
                "description": pdesc
            }),
        );
    }
    json!({
        "name": name,
        "description": description,
        "inputSchema": {
            "type": "object",
            "properties": input_props
        }
    })
}

pub fn handle_tools_call(id: Option<serde_json::Value>, params: Value) -> Result<Value, String> {
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or("missing tool name")?;

    let arguments = params
        .get("arguments")
        .and_then(|v| v.as_object())
        .ok_or("missing arguments")?;

    let result_json: Value = match name {
        "blockize" => {
            let sv_files: Vec<PathBuf> = arguments
                .get("sv_files")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .ok_or("invalid sv_files")?;
            let req = BlockizeRequest { sv_files };
            let result = blockize(req).map_err(|e| e.to_string())?;
            serde_json::to_value(&result).map_err(|e| e.to_string())?
        }
        "slice_static" => {
            let sv_files: Vec<PathBuf> = arguments
                .get("sv_files")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .ok_or("invalid sv_files")?;
            let signal = arguments
                .get("signal")
                .and_then(|v| v.as_str())
                .ok_or("invalid signal")?;
            let req = StaticSliceRequest {
                sv_files,
                signal: signal.to_string(),
            };
            let result = slice_static(req).map_err(|e| e.to_string())?;
            serde_json::to_value(&result).map_err(|e| e.to_string())?
        }
        "slice_dynamic" => {
            let sv_files: Vec<PathBuf> = arguments
                .get("sv_files")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .ok_or("invalid sv_files")?;
            let signal = arguments
                .get("signal")
                .and_then(|v| v.as_str())
                .ok_or("invalid signal")?;
            let vcd = arguments
                .get("vcd")
                .and_then(|v| v.as_str())
                .map(PathBuf::from)
                .ok_or("invalid vcd")?;
            let time = arguments
                .get("time")
                .and_then(|v| v.as_i64())
                .ok_or("invalid time")?;
            let min_time = arguments
                .get("min_time")
                .and_then(|v| v.as_i64())
                .ok_or("invalid min_time")?;
            let req = DynamicSliceRequest {
                sv_files,
                signal: signal.to_string(),
                vcd,
                time,
                min_time,
                clock: arguments
                    .get("clock")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                clk_step: arguments.get("clk_step").and_then(|v| v.as_i64()),
            };
            let result = slice_dynamic(req).map_err(|e| e.to_string())?;
            serde_json::to_value(&result).map_err(|e| e.to_string())?
        }
        "coverage_report" => {
            let sv_files: Vec<PathBuf> = arguments
                .get("sv_files")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .ok_or("invalid sv_files")?;
            let vcd = arguments
                .get("vcd")
                .and_then(|v| v.as_str())
                .map(PathBuf::from)
                .ok_or("invalid vcd")?;
            let time = arguments
                .get("time")
                .and_then(|v| v.as_i64())
                .ok_or("invalid time")?;
            let req = CoverageReportRequest {
                sv_files,
                vcd,
                time,
            };
            let result = coverage_report(req).map_err(|e| e.to_string())?;
            serde_json::to_value(&result).map_err(|e| e.to_string())?
        }
        "wave_value" => {
            let vcd = arguments
                .get("vcd")
                .and_then(|v| v.as_str())
                .map(PathBuf::from)
                .ok_or("invalid vcd")?;
            let signal = arguments
                .get("signal")
                .and_then(|v| v.as_str())
                .ok_or("invalid signal")?;
            let time = arguments
                .get("time")
                .and_then(|v| v.as_i64())
                .ok_or("invalid time")?;
            let req = WaveValueRequest {
                vcd,
                signal: signal.to_string(),
                time,
            };
            let result = wave_value(req).map_err(|e| e.to_string())?;
            match result {
                Some(val) => json!({ "value": format!("{:?}", val) }),
                None => json!({ "value": null }),
            }
        }
        _ => return Err(format!("unknown tool: {}", name)),
    };

    let resp = JsonRpcResponse::result(
        id,
        json!({
            "content": [{
                "type": "text",
                "text": serde_json::to_string_pretty(&result_json).unwrap_or_default()
            }]
        }),
    );
    serde_json::to_value(resp).map_err(|e| e.to_string())
}

pub fn handle_request(
    method: &str,
    id: Option<serde_json::Value>,
    params: Option<Value>,
) -> Option<Value> {
    match method {
        "initialize" => Some(handle_initialize(id, params.unwrap_or(json!({})))),
        "tools/list" => Some(handle_tools_list(id)),
        "tools/call" => handle_tools_call(id, params?).ok(),
        _ => None,
    }
}
