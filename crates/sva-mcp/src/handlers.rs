use rmcp::model::{
    CallToolResult, Content, InitializeResult, ListToolsResult, ServerCapabilities, Tool,
};
use rmcp::schemars;
use rmcp::ServerHandler;
use serde::Deserialize;
use serde_json::Map;
use std::sync::Arc;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct BlockizeRequest {
    #[schemars(description = "paths to SV files")]
    pub sv_files: Vec<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct StaticSliceRequest {
    #[schemars(description = "paths to SV files")]
    pub sv_files: Vec<String>,
    #[schemars(description = "hierarchical signal name (e.g. 'tb.dut.u_stage3.result')")]
    pub signal: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DynamicSliceRequest {
    #[schemars(description = "paths to SV files")]
    pub sv_files: Vec<String>,
    #[schemars(description = "hierarchical signal name (e.g. 'tb.dut.u_stage3.result')")]
    pub signal: String,
    #[schemars(description = "path to VCD waveform file")]
    pub vcd: String,
    #[schemars(description = "time to slice at")]
    pub time: i64,
    #[schemars(description = "minimum time boundary")]
    pub min_time: i64,
    #[schemars(description = "clock signal name", default)]
    pub clock: Option<String>,
    #[schemars(description = "clock period (i.e., time interval between two posedge clock)", default)]
    pub clk_step: Option<i64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CoverageReportRequest {
    #[schemars(description = "paths to SV files")]
    pub sv_files: Vec<String>,
    #[schemars(description = "path to VCD waveform file")]
    pub vcd: String,
    #[schemars(description = "time to evaluate at")]
    pub time: i64,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct WaveValueRequest {
    #[schemars(description = "path to VCD waveform file")]
    pub vcd: String,
    #[schemars(description = "hierarchical signal name (e.g. 'tb.dut.u_stage3.result')")]
    pub signal: String,
    #[schemars(description = "time to read")]
    pub time: i64,
}

#[derive(Debug, Clone)]
pub struct SvaMcpServer;

impl SvaMcpServer {
    fn blockize_impl(&self, req: BlockizeRequest) -> Result<String, String> {
        let sv_files: Vec<std::path::PathBuf> = req
            .sv_files
            .into_iter()
            .map(std::path::PathBuf::from)
            .collect();
        let result = sva_core::services::blockize(sva_core::services::BlockizeRequest {
            sv_files,
        })
        .map_err(|e| e.to_string())?;
        serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
    }

    fn slice_static_impl(&self, req: StaticSliceRequest) -> Result<String, String> {
        let sv_files: Vec<std::path::PathBuf> = req
            .sv_files
            .into_iter()
            .map(std::path::PathBuf::from)
            .collect();
        let result = sva_core::services::slice_static(sva_core::services::StaticSliceRequest {
            sv_files,
            signal: req.signal,
        })
        .map_err(|e| e.to_string())?;
        serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
    }

    fn slice_dynamic_impl(&self, req: DynamicSliceRequest) -> Result<String, String> {
        let sv_files: Vec<std::path::PathBuf> = req
            .sv_files
            .into_iter()
            .map(std::path::PathBuf::from)
            .collect();
        let result = sva_core::services::slice_dynamic(sva_core::services::DynamicSliceRequest {
            sv_files,
            signal: req.signal,
            vcd: std::path::PathBuf::from(req.vcd),
            time: req.time,
            min_time: req.min_time,
            clock: req.clock,
            clk_step: req.clk_step,
        })
        .map_err(|e| e.to_string())?;
        serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
    }

    fn coverage_report_impl(&self, req: CoverageReportRequest) -> Result<String, String> {
        let sv_files: Vec<std::path::PathBuf> = req
            .sv_files
            .into_iter()
            .map(std::path::PathBuf::from)
            .collect();
        let result = sva_core::services::coverage_report(
            sva_core::services::CoverageReportRequest {
                sv_files,
                vcd: std::path::PathBuf::from(req.vcd),
                time: req.time,
            },
        )
        .map_err(|e| e.to_string())?;
        serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
    }

    fn wave_value_impl(&self, req: WaveValueRequest) -> Result<String, String> {
        let result = sva_core::services::wave_value(sva_core::services::WaveValueRequest {
            vcd: std::path::PathBuf::from(req.vcd),
            signal: req.signal,
            time: req.time,
        })
        .map_err(|e| e.to_string())?;
        Ok(match result {
            Some(val) => format!("{:?}", val),
            None => "null".to_string(),
        })
    }
}

fn build_input_schema(props: Map<String, serde_json::Value>) -> Arc<Map<String, serde_json::Value>> {
    let mut schema = Map::new();
    schema.insert("type".to_string(), serde_json::json!("object"));
    schema.insert("properties".to_string(), serde_json::json!(props));
    Arc::new(schema)
}

impl ServerHandler for SvaMcpServer {
    fn get_info(&self) -> InitializeResult {
        let caps: ServerCapabilities = ServerCapabilities::builder()
            .enable_tools()
            .build();

        InitializeResult::new(caps)
            .with_instructions("sva-mcp: HDL dataflow analysis engine")
            .with_server_info(rmcp::model::Implementation::new("sva-mcp", "0.1.0"))
    }

    async fn list_tools(
        &self,
        _pagination: Option<rmcp::model::PaginatedRequestParams>,
        _ctx: rmcp::service::RequestContext<rmcp::service::RoleServer>,
    ) -> Result<ListToolsResult, rmcp::ErrorData> {
        let mut props = Map::new();
        props.insert(
            "sv_files".to_string(),
            serde_json::json!({
                "type": "array",
                "items": { "type": "string" },
                "description": "paths to SV files"
            }),
        );
        let blockize_schema = build_input_schema(props);

        let mut props = Map::new();
        props.insert(
            "sv_files".to_string(),
            serde_json::json!({
                "type": "array",
                "items": { "type": "string" },
                "description": "paths to SV files"
            }),
        );
        props.insert(
            "signal".to_string(),
            serde_json::json!({
                "type": "string",
                "description": "hierarchical signal name (e.g. 'tb.dut.u_stage3.result')"
            }),
        );
        let slice_static_schema = build_input_schema(props);

        let mut props = Map::new();
        props.insert(
            "sv_files".to_string(),
            serde_json::json!({
                "type": "array",
                "items": { "type": "string" },
                "description": "paths to SV files"
            }),
        );
        props.insert(
            "signal".to_string(),
            serde_json::json!({
                "type": "string",
                "description": "hierarchical signal name (e.g. 'tb.dut.u_stage3.result')"
            }),
        );
        props.insert(
            "vcd".to_string(),
            serde_json::json!({
                "type": "string",
                "description": "path to VCD waveform file"
            }),
        );
        props.insert(
            "time".to_string(),
            serde_json::json!({
                "type": "integer",
                "description": "time to slice at"
            }),
        );
        props.insert(
            "min_time".to_string(),
            serde_json::json!({
                "type": "integer",
                "description": "minimum time boundary"
            }),
        );
        props.insert(
            "clock".to_string(),
            serde_json::json!({
                "type": "string",
                "description": "clock signal name (optional)"
            }),
        );
        props.insert(
            "clk_step".to_string(),
            serde_json::json!({
                "type": "integer",
                "description": "clock period (i.e., time interval between two posedge clock) (optional)"
            }),
        );
        let slice_dynamic_schema = build_input_schema(props);

        let mut props = Map::new();
        props.insert(
            "sv_files".to_string(),
            serde_json::json!({
                "type": "array",
                "items": { "type": "string" },
                "description": "paths to SV files"
            }),
        );
        props.insert(
            "vcd".to_string(),
            serde_json::json!({
                "type": "string",
                "description": "path to VCD waveform file"
            }),
        );
        props.insert(
            "time".to_string(),
            serde_json::json!({
                "type": "integer",
                "description": "time to evaluate at"
            }),
        );
        let coverage_report_schema = build_input_schema(props);

        let mut props = Map::new();
        props.insert(
            "vcd".to_string(),
            serde_json::json!({
                "type": "string",
                "description": "path to VCD waveform file"
            }),
        );
        props.insert(
            "signal".to_string(),
            serde_json::json!({
                "type": "string",
                "description": "hierarchical signal name (e.g. 'tb.dut.u_stage3.result')"
            }),
        );
        props.insert(
            "time".to_string(),
            serde_json::json!({
                "type": "integer",
                "description": "time to read"
            }),
        );
        let wave_value_schema = build_input_schema(props);

        Ok(ListToolsResult {
            next_cursor: None,
            meta: None,
            tools: vec![
                Tool::new(
                    "blockize",
                    "Run dataflow blockization on SystemVerilog files",
                    blockize_schema,
                ),
                Tool::new(
                    "slice_static",
                    "Static backward slice from a signal",
                    slice_static_schema,
                ),
                Tool::new(
                    "slice_dynamic",
                    "Dynamic slicing with waveform",
                    slice_dynamic_schema,
                ),
                Tool::new(
                    "coverage_report",
                    "Generate coverage report",
                    coverage_report_schema,
                ),
                Tool::new(
                    "wave_value",
                    "Read signal value at specific time",
                    wave_value_schema,
                ),
            ],
        })
    }

    async fn call_tool(
        &self,
        request: rmcp::model::CallToolRequestParams,
        _ctx: rmcp::service::RequestContext<rmcp::service::RoleServer>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let name = request.name.to_string();
        let arguments = request.arguments.as_ref().ok_or_else(|| {
            rmcp::ErrorData::invalid_params("missing arguments", None)
        })?;

        let result_text = match name.as_str() {
            "blockize" => {
                let req: BlockizeRequest = serde_json::from_value(serde_json::Value::Object(arguments.clone())).map_err(|e| {
                    rmcp::ErrorData::invalid_params(format!("invalid blockize request: {}", e), None)
                })?;
                self.blockize_impl(req).map_err(|e| rmcp::ErrorData::internal_error(e, None))?
            }
            "slice_static" => {
                let req: StaticSliceRequest = serde_json::from_value(serde_json::Value::Object(arguments.clone())).map_err(|e| {
                    rmcp::ErrorData::invalid_params(format!("invalid slice_static request: {}", e), None)
                })?;
                self.slice_static_impl(req).map_err(|e| rmcp::ErrorData::internal_error(e, None))?
            }
            "slice_dynamic" => {
                let req: DynamicSliceRequest = serde_json::from_value(serde_json::Value::Object(arguments.clone())).map_err(|e| {
                    rmcp::ErrorData::invalid_params(format!("invalid slice_dynamic request: {}", e), None)
                })?;
                self.slice_dynamic_impl(req).map_err(|e| rmcp::ErrorData::internal_error(e, None))?
            }
            "coverage_report" => {
                let req: CoverageReportRequest = serde_json::from_value(serde_json::Value::Object(arguments.clone())).map_err(|e| {
                    rmcp::ErrorData::invalid_params(format!("invalid coverage_report request: {}", e), None)
                })?;
                self.coverage_report_impl(req).map_err(|e| rmcp::ErrorData::internal_error(e, None))?
            }
            "wave_value" => {
                let req: WaveValueRequest = serde_json::from_value(serde_json::Value::Object(arguments.clone())).map_err(|e| {
                    rmcp::ErrorData::invalid_params(format!("invalid wave_value request: {}", e), None)
                })?;
                self.wave_value_impl(req).map_err(|e| rmcp::ErrorData::internal_error(e, None))?
            }
            _ => {
                return Err(rmcp::ErrorData::invalid_params(
                    format!("unknown tool: {}", name),
                    None,
                ))
            }
        };

        Ok(CallToolResult::success(vec![Content::text(result_text)]))
    }
}
