use rmcp::model::{
    CallToolResult, Content, InitializeResult, ListToolsResult, ServerCapabilities, Tool,
};
use rmcp::schemars;
use rmcp::ServerHandler;
use serde::{Deserialize, Serialize};
use serde_json::Map;
use std::sync::Arc;
use sva_core::ast::ParseOptions;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct BlockizeRequest {
    #[serde(default)]
    #[schemars(description = "paths to SV files; optional when project_path is provided")]
    pub sv_files: Vec<String>,
    #[serde(default)]
    #[schemars(description = "directory of .sv sources to parse recursively", default)]
    pub project_path: Option<String>,
    #[serde(default)]
    #[schemars(description = "include search paths for sv_parser", default)]
    pub include_paths: Vec<String>,
    #[serde(default)]
    #[schemars(description = "blockize artifact directory; defaults to .sva", default)]
    pub artifact_dir: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct StaticSliceRequest {
    #[serde(default)]
    #[schemars(description = "paths to SV files; optional when project_path is provided")]
    pub sv_files: Vec<String>,
    #[serde(default)]
    #[schemars(description = "directory of .sv sources to parse recursively", default)]
    pub project_path: Option<String>,
    #[serde(default)]
    #[schemars(description = "include search paths for sv_parser", default)]
    pub include_paths: Vec<String>,
    #[schemars(
        description = "hierarchical scoped signal name; use full hierarchy when passing it (e.g. 'TOP.tb.dut.result')"
    )]
    pub signal: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DynamicSliceRequest {
    #[serde(default)]
    #[schemars(description = "paths to SV files; optional when project_path is provided")]
    pub sv_files: Vec<String>,
    #[serde(default)]
    #[schemars(description = "directory of .sv sources to parse recursively", default)]
    pub project_path: Option<String>,
    #[serde(default)]
    #[schemars(description = "include search paths for sv_parser", default)]
    pub include_paths: Vec<String>,
    #[schemars(
        description = "hierarchical scoped signal name; use full hierarchy when passing it (e.g. 'TOP.tb.dut.result')"
    )]
    pub signal: String,
    #[schemars(description = "path to VCD or FST waveform file")]
    pub vcd: String,
    #[serde(default)]
    #[schemars(
        description = "native Verilator tree JSON used to prune non-elaborated dynamic slice blocks",
        default
    )]
    pub tree_json: Option<String>,
    #[serde(default)]
    #[schemars(
        description = "native Verilator tree metadata JSON used to resolve loc file IDs",
        default
    )]
    pub tree_meta_json: Option<String>,
    #[schemars(description = "time to slice at")]
    pub time: i64,
    #[schemars(description = "minimum time boundary")]
    pub min_time: i64,
    #[schemars(description = "clock signal name", default)]
    pub clock: Option<String>,
    #[schemars(
        description = "clock period (i.e., time interval between two posedge clock)",
        default
    )]
    pub clk_step: Option<i64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateStaticSliceArtifactRequest {
    #[serde(default)]
    #[schemars(description = "paths to SV files; optional when project_path is provided")]
    pub sv_files: Vec<String>,
    #[serde(default)]
    #[schemars(description = "directory of .sv sources to parse recursively", default)]
    pub project_path: Option<String>,
    #[serde(default)]
    #[schemars(description = "include search paths for sv_parser", default)]
    pub include_paths: Vec<String>,
    #[schemars(
        description = "hierarchical scoped signal name; use full hierarchy when passing it (e.g. 'TOP.tb.dut.result')"
    )]
    pub signal: String,
    #[serde(default)]
    #[schemars(description = "slice artifact directory; defaults to .sva", default)]
    pub artifact_dir: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateDynamicSliceArtifactRequest {
    #[serde(default)]
    #[schemars(description = "paths to SV files; optional when project_path is provided")]
    pub sv_files: Vec<String>,
    #[serde(default)]
    #[schemars(description = "directory of .sv sources to parse recursively", default)]
    pub project_path: Option<String>,
    #[serde(default)]
    #[schemars(description = "include search paths for sv_parser", default)]
    pub include_paths: Vec<String>,
    #[schemars(
        description = "hierarchical scoped signal name; use full hierarchy when passing it (e.g. 'TOP.tb.dut.result')"
    )]
    pub signal: String,
    #[schemars(description = "path to VCD or FST waveform file")]
    pub vcd: String,
    #[serde(default)]
    #[schemars(
        description = "native Verilator tree JSON used to prune non-elaborated dynamic slice blocks",
        default
    )]
    pub tree_json: Option<String>,
    #[serde(default)]
    #[schemars(
        description = "native Verilator tree metadata JSON used to resolve loc file IDs",
        default
    )]
    pub tree_meta_json: Option<String>,
    #[schemars(description = "time to slice at")]
    pub time: i64,
    #[schemars(description = "minimum time boundary")]
    pub min_time: i64,
    #[schemars(description = "clock signal name", default)]
    pub clock: Option<String>,
    #[schemars(
        description = "clock period (i.e., time interval between two posedge clock)",
        default
    )]
    pub clk_step: Option<i64>,
    #[serde(default)]
    #[schemars(description = "slice artifact directory; defaults to .sva", default)]
    pub artifact_dir: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SliceArtifactQueryRequest {
    #[serde(default)]
    #[schemars(description = "path to an existing slice JSON artifact", default)]
    pub slice_json: Option<String>,
    #[serde(default)]
    #[schemars(description = "slice artifact directory; defaults to .sva", default)]
    pub artifact_dir: Option<String>,
    #[serde(default)]
    #[schemars(
        description = "hierarchical scoped signal to query; defaults to the artifact target (e.g. 'TOP.tb.dut.result')",
        default
    )]
    pub signal: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct BlocksQueryRequest {
    #[schemars(description = "path to a saved blockize JSON file")]
    pub input: String,
    #[serde(default)]
    #[schemars(description = "exact block id match", default)]
    pub block_id: Option<u64>,
    #[serde(default)]
    #[schemars(description = "output signal filters; all must match", default)]
    pub output_signals: Vec<String>,
    #[serde(default)]
    #[schemars(description = "input signal filters; all must match", default)]
    pub input_signals: Vec<String>,
    #[serde(default)]
    #[schemars(description = "hierarchical scope prefix filter", default)]
    pub scope: Option<String>,
    #[serde(default)]
    #[schemars(
        description = "block type filter: mod-input, mod-output, always, assign",
        default
    )]
    pub block_type: Option<String>,
    #[serde(default)]
    #[schemars(
        description = "circuit type filter: combinational, sequential",
        default
    )]
    pub circuit_type: Option<String>,
    #[serde(default)]
    #[schemars(description = "source file suffix filter", default)]
    pub source_file: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CoverageReportRequest {
    #[serde(default)]
    #[schemars(description = "paths to SV files; optional when project_path is provided")]
    pub sv_files: Vec<String>,
    #[serde(default)]
    #[schemars(description = "directory of .sv sources to parse recursively", default)]
    pub project_path: Option<String>,
    #[serde(default)]
    #[schemars(description = "include search paths for sv_parser", default)]
    pub include_paths: Vec<String>,
    #[schemars(description = "path to VCD or FST waveform file")]
    pub vcd: String,
    #[schemars(description = "time to evaluate at")]
    pub time: i64,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct WaveValueRequest {
    #[schemars(description = "path to VCD or FST waveform file")]
    pub vcd: String,
    #[schemars(
        description = "hierarchical scoped signal name; use full hierarchy when passing it (e.g. 'TOP.tb.dut.result')"
    )]
    pub signal: String,
    #[schemars(description = "time to read")]
    pub time: i64,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct WaveSignalSearchRequest {
    #[schemars(description = "path to VCD or FST waveform file")]
    pub vcd: String,
    #[schemars(description = "fuzzy signal-name search query")]
    pub query: String,
    #[serde(default)]
    #[schemars(description = "maximum number of matches; defaults to 5", default)]
    pub limit: Option<usize>,
}

#[derive(Debug, Serialize)]
struct WaveOutput {
    signal: String,
    time: i64,
    value: WaveValueOutput,
}

#[derive(Debug, Serialize)]
struct WaveValueOutput {
    raw_bits: String,
    pretty_hex: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SvaMcpServer;

impl SvaMcpServer {
    fn parse_options(project_path: Option<String>, include_paths: Vec<String>) -> ParseOptions {
        ParseOptions {
            project_path: project_path.map(std::path::PathBuf::from),
            include_paths: include_paths
                .into_iter()
                .map(std::path::PathBuf::from)
                .collect(),
        }
    }

    fn path_bufs(paths: Vec<String>) -> Vec<std::path::PathBuf> {
        paths.into_iter().map(std::path::PathBuf::from).collect()
    }

    fn blockize_impl(&self, req: BlockizeRequest) -> Result<String, String> {
        let result = sva_core::services::create_blockize_json(
            sva_core::services::CreateBlockizeArtifactRequest {
                sv_files: Self::path_bufs(req.sv_files),
                parse_options: Self::parse_options(req.project_path, req.include_paths),
                artifact_dir: req.artifact_dir.map(std::path::PathBuf::from),
            },
        )
        .map_err(|e| e.to_string())?;
        serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
    }

    fn slice_static_impl(&self, req: StaticSliceRequest) -> Result<String, String> {
        let result = sva_core::services::slice_static(sva_core::services::StaticSliceRequest {
            sv_files: Self::path_bufs(req.sv_files),
            parse_options: Self::parse_options(req.project_path, req.include_paths),
            signal: req.signal,
        })
        .map_err(|e| e.to_string())?;
        serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
    }

    fn slice_dynamic_impl(&self, req: DynamicSliceRequest) -> Result<String, String> {
        let result = sva_core::services::slice_dynamic(sva_core::services::DynamicSliceRequest {
            sv_files: Self::path_bufs(req.sv_files),
            parse_options: Self::parse_options(req.project_path, req.include_paths),
            signal: req.signal,
            vcd: std::path::PathBuf::from(req.vcd),
            tree_json: req.tree_json.map(std::path::PathBuf::from),
            tree_meta_json: req.tree_meta_json.map(std::path::PathBuf::from),
            time: req.time,
            min_time: req.min_time,
            clock: req.clock,
            clk_step: req.clk_step,
        })
        .map_err(|e| e.to_string())?;
        serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
    }

    fn create_slice_json_static_impl(
        &self,
        req: CreateStaticSliceArtifactRequest,
    ) -> Result<String, String> {
        let result = sva_core::services::create_slice_json_static(
            sva_core::services::CreateStaticSliceArtifactRequest {
                sv_files: Self::path_bufs(req.sv_files),
                parse_options: Self::parse_options(req.project_path, req.include_paths),
                signal: req.signal,
                artifact_dir: req.artifact_dir.map(std::path::PathBuf::from),
            },
        )
        .map_err(|e| e.to_string())?;
        serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
    }

    fn create_slice_json_dynamic_impl(
        &self,
        req: CreateDynamicSliceArtifactRequest,
    ) -> Result<String, String> {
        let result = sva_core::services::create_slice_json_dynamic(
            sva_core::services::CreateDynamicSliceArtifactRequest {
                sv_files: Self::path_bufs(req.sv_files),
                parse_options: Self::parse_options(req.project_path, req.include_paths),
                signal: req.signal,
                vcd: std::path::PathBuf::from(req.vcd),
                tree_json: req.tree_json.map(std::path::PathBuf::from),
                tree_meta_json: req.tree_meta_json.map(std::path::PathBuf::from),
                time: req.time,
                min_time: req.min_time,
                clock: req.clock,
                clk_step: req.clk_step,
                artifact_dir: req.artifact_dir.map(std::path::PathBuf::from),
            },
        )
        .map_err(|e| e.to_string())?;
        serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
    }

    fn query_signal_drivers_impl(&self, req: SliceArtifactQueryRequest) -> Result<String, String> {
        let result = sva_core::services::query_slice_signal_drivers(
            sva_core::services::SliceArtifactQueryRequest {
                slice_json: req.slice_json.map(std::path::PathBuf::from),
                artifact_dir: req.artifact_dir.map(std::path::PathBuf::from),
                signal: req.signal,
            },
        )
        .map_err(|e| e.to_string())?;
        serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
    }

    fn query_block_drivers_impl(&self, req: SliceArtifactQueryRequest) -> Result<String, String> {
        let result = sva_core::services::query_slice_block_drivers(
            sva_core::services::SliceArtifactQueryRequest {
                slice_json: req.slice_json.map(std::path::PathBuf::from),
                artifact_dir: req.artifact_dir.map(std::path::PathBuf::from),
                signal: req.signal,
            },
        )
        .map_err(|e| e.to_string())?;
        serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
    }

    fn blocks_query_impl(&self, req: BlocksQueryRequest) -> Result<String, String> {
        let result = sva_core::services::blocks_query(sva_core::services::BlocksQueryRequest {
            input: std::path::PathBuf::from(req.input),
            block_id: req.block_id,
            output_signals: req.output_signals,
            input_signals: req.input_signals,
            scope: req.scope,
            block_type: req.block_type,
            circuit_type: req.circuit_type,
            source_file: req.source_file,
        })
        .map_err(|e| e.to_string())?;
        serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
    }

    fn coverage_report_impl(&self, req: CoverageReportRequest) -> Result<String, String> {
        let result =
            sva_core::services::coverage_report(sva_core::services::CoverageReportRequest {
                sv_files: Self::path_bufs(req.sv_files),
                parse_options: Self::parse_options(req.project_path, req.include_paths),
                vcd: std::path::PathBuf::from(req.vcd),
                time: req.time,
            })
            .map_err(|e| e.to_string())?;
        serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
    }

    fn wave_value_impl(&self, req: WaveValueRequest) -> Result<String, String> {
        let signal = req.signal;
        let result = sva_core::services::wave_value(sva_core::services::WaveValueRequest {
            vcd: std::path::PathBuf::from(req.vcd),
            signal: signal.clone(),
            time: req.time,
        })
        .map_err(|e| e.to_string())?;
        let output = WaveOutput {
            signal,
            time: req.time,
            value: WaveValueOutput {
                raw_bits: result.raw_bits,
                pretty_hex: result.pretty_hex,
            },
        };
        serde_json::to_string_pretty(&output).map_err(|e| e.to_string())
    }

    fn wave_signal_search_impl(&self, req: WaveSignalSearchRequest) -> Result<String, String> {
        let result =
            sva_core::services::wave_signal_search(sva_core::services::WaveSignalSearchRequest {
                vcd: std::path::PathBuf::from(req.vcd),
                query: req.query,
                limit: req.limit.unwrap_or(5),
            })
            .map_err(|e| e.to_string())?;
        serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
    }
}

fn build_input_schema(
    props: Map<String, serde_json::Value>,
) -> Arc<Map<String, serde_json::Value>> {
    let mut schema = Map::new();
    schema.insert("type".to_string(), serde_json::json!("object"));
    schema.insert("properties".to_string(), serde_json::json!(props));
    Arc::new(schema)
}

impl ServerHandler for SvaMcpServer {
    fn get_info(&self) -> InitializeResult {
        let caps: ServerCapabilities = ServerCapabilities::builder().enable_tools().build();

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
                "description": "paths to SV files; optional when project_path is provided"
            }),
        );
        props.insert(
            "project_path".to_string(),
            serde_json::json!({
                "type": "string",
                "description": "directory of .sv sources to parse recursively"
            }),
        );
        props.insert(
            "include_paths".to_string(),
            serde_json::json!({
                "type": "array",
                "items": { "type": "string" },
                "description": "include search paths for sv_parser"
            }),
        );
        props.insert(
            "artifact_dir".to_string(),
            serde_json::json!({
                "type": "string",
                "description": "blockize artifact directory; defaults to .sva"
            }),
        );
        let blockize_schema = build_input_schema(props);

        let mut props = Map::new();
        props.insert(
            "sv_files".to_string(),
            serde_json::json!({
                "type": "array",
                "items": { "type": "string" },
                "description": "paths to SV files; optional when project_path is provided"
            }),
        );
        props.insert(
            "project_path".to_string(),
            serde_json::json!({
                "type": "string",
                "description": "directory of .sv sources to parse recursively"
            }),
        );
        props.insert(
            "include_paths".to_string(),
            serde_json::json!({
                "type": "array",
                "items": { "type": "string" },
                "description": "include search paths for sv_parser"
            }),
        );
        props.insert(
            "signal".to_string(),
            serde_json::json!({
                "type": "string",
                "description": "hierarchical scoped signal name; use full hierarchy when passing it (e.g. 'TOP.tb.dut.result')"
            }),
        );
        let slice_static_schema = build_input_schema(props);

        let mut props = Map::new();
        props.insert(
            "sv_files".to_string(),
            serde_json::json!({
                "type": "array",
                "items": { "type": "string" },
                "description": "paths to SV files; optional when project_path is provided"
            }),
        );
        props.insert(
            "project_path".to_string(),
            serde_json::json!({
                "type": "string",
                "description": "directory of .sv sources to parse recursively"
            }),
        );
        props.insert(
            "include_paths".to_string(),
            serde_json::json!({
                "type": "array",
                "items": { "type": "string" },
                "description": "include search paths for sv_parser"
            }),
        );
        props.insert(
            "signal".to_string(),
            serde_json::json!({
                "type": "string",
                "description": "hierarchical scoped signal name; use full hierarchy when passing it (e.g. 'TOP.tb.dut.result')"
            }),
        );
        props.insert(
            "vcd".to_string(),
            serde_json::json!({
                "type": "string",
                "description": "path to VCD or FST waveform file"
            }),
        );
        props.insert(
            "tree_json".to_string(),
            serde_json::json!({
                "type": "string",
                "description": "native Verilator tree JSON used to prune non-elaborated dynamic slice blocks (optional)"
            }),
        );
        props.insert(
            "tree_meta_json".to_string(),
            serde_json::json!({
                "type": "string",
                "description": "native Verilator tree metadata JSON used to resolve loc file IDs (optional)"
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
                "description": "paths to SV files; optional when project_path is provided"
            }),
        );
        props.insert(
            "project_path".to_string(),
            serde_json::json!({
                "type": "string",
                "description": "directory of .sv sources to parse recursively"
            }),
        );
        props.insert(
            "include_paths".to_string(),
            serde_json::json!({
                "type": "array",
                "items": { "type": "string" },
                "description": "include search paths for sv_parser"
            }),
        );
        props.insert(
            "signal".to_string(),
            serde_json::json!({
                "type": "string",
                "description": "hierarchical scoped signal name; use full hierarchy when passing it (e.g. 'TOP.tb.dut.result')"
            }),
        );
        props.insert(
            "artifact_dir".to_string(),
            serde_json::json!({
                "type": "string",
                "description": "slice artifact directory; defaults to .sva"
            }),
        );
        let create_slice_json_static_schema = build_input_schema(props);

        let mut props = Map::new();
        props.insert(
            "sv_files".to_string(),
            serde_json::json!({
                "type": "array",
                "items": { "type": "string" },
                "description": "paths to SV files; optional when project_path is provided"
            }),
        );
        props.insert(
            "project_path".to_string(),
            serde_json::json!({
                "type": "string",
                "description": "directory of .sv sources to parse recursively"
            }),
        );
        props.insert(
            "include_paths".to_string(),
            serde_json::json!({
                "type": "array",
                "items": { "type": "string" },
                "description": "include search paths for sv_parser"
            }),
        );
        props.insert(
            "signal".to_string(),
            serde_json::json!({
                "type": "string",
                "description": "hierarchical scoped signal name; use full hierarchy when passing it (e.g. 'TOP.tb.dut.result')"
            }),
        );
        props.insert(
            "vcd".to_string(),
            serde_json::json!({
                "type": "string",
                "description": "path to VCD or FST waveform file"
            }),
        );
        props.insert(
            "tree_json".to_string(),
            serde_json::json!({
                "type": "string",
                "description": "native Verilator tree JSON used to prune non-elaborated dynamic slice blocks (optional)"
            }),
        );
        props.insert(
            "tree_meta_json".to_string(),
            serde_json::json!({
                "type": "string",
                "description": "native Verilator tree metadata JSON used to resolve loc file IDs (optional)"
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
        props.insert(
            "artifact_dir".to_string(),
            serde_json::json!({
                "type": "string",
                "description": "slice artifact directory; defaults to .sva"
            }),
        );
        let create_slice_json_dynamic_schema = build_input_schema(props);

        let mut props = Map::new();
        props.insert(
            "slice_json".to_string(),
            serde_json::json!({
                "type": "string",
                "description": "path to an existing slice JSON artifact"
            }),
        );
        props.insert(
            "artifact_dir".to_string(),
            serde_json::json!({
                "type": "string",
                "description": "slice artifact directory; defaults to .sva"
            }),
        );
        props.insert(
            "signal".to_string(),
            serde_json::json!({
                "type": "string",
                "description": "hierarchical scoped signal to query; defaults to the artifact target (e.g. 'TOP.tb.dut.result')"
            }),
        );
        let slice_artifact_query_schema = build_input_schema(props);

        let mut props = Map::new();
        props.insert(
            "input".to_string(),
            serde_json::json!({
                "type": "string",
                "description": "path to a saved blockize JSON file"
            }),
        );
        props.insert(
            "block_id".to_string(),
            serde_json::json!({
                "type": "integer",
                "description": "exact block id match"
            }),
        );
        props.insert(
            "output_signals".to_string(),
            serde_json::json!({
                "type": "array",
                "items": { "type": "string" },
                "description": "output signal filters; all must match"
            }),
        );
        props.insert(
            "input_signals".to_string(),
            serde_json::json!({
                "type": "array",
                "items": { "type": "string" },
                "description": "input signal filters; all must match"
            }),
        );
        props.insert(
            "scope".to_string(),
            serde_json::json!({
                "type": "string",
                "description": "hierarchical scope prefix filter"
            }),
        );
        props.insert(
            "block_type".to_string(),
            serde_json::json!({
                "type": "string",
                "description": "block type filter: mod-input, mod-output, always, assign"
            }),
        );
        props.insert(
            "circuit_type".to_string(),
            serde_json::json!({
                "type": "string",
                "description": "circuit type filter: combinational, sequential"
            }),
        );
        props.insert(
            "source_file".to_string(),
            serde_json::json!({
                "type": "string",
                "description": "source file suffix filter"
            }),
        );
        let blocks_query_schema = build_input_schema(props);

        let mut props = Map::new();
        props.insert(
            "sv_files".to_string(),
            serde_json::json!({
                "type": "array",
                "items": { "type": "string" },
                "description": "paths to SV files; optional when project_path is provided"
            }),
        );
        props.insert(
            "project_path".to_string(),
            serde_json::json!({
                "type": "string",
                "description": "directory of .sv sources to parse recursively"
            }),
        );
        props.insert(
            "include_paths".to_string(),
            serde_json::json!({
                "type": "array",
                "items": { "type": "string" },
                "description": "include search paths for sv_parser"
            }),
        );
        props.insert(
            "vcd".to_string(),
            serde_json::json!({
                "type": "string",
                "description": "path to VCD or FST waveform file"
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
                "description": "path to VCD or FST waveform file"
            }),
        );
        props.insert(
            "signal".to_string(),
            serde_json::json!({
                "type": "string",
                "description": "hierarchical scoped signal name; use full hierarchy when passing it (e.g. 'TOP.tb.dut.result')"
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

        let mut props = Map::new();
        props.insert(
            "vcd".to_string(),
            serde_json::json!({
                "type": "string",
                "description": "path to VCD or FST waveform file"
            }),
        );
        props.insert(
            "query".to_string(),
            serde_json::json!({
                "type": "string",
                "description": "fuzzy signal-name search query"
            }),
        );
        props.insert(
            "limit".to_string(),
            serde_json::json!({
                "type": "integer",
                "description": "maximum number of matches; defaults to 5"
            }),
        );
        let wave_signal_search_schema = build_input_schema(props);

        Ok(ListToolsResult {
            next_cursor: None,
            meta: None,
            tools: vec![
                Tool::new(
                    "blockize",
                    "Run dataflow blockization on SystemVerilog files and save JSON under .sva",
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
                    "create_slice_json_static",
                    "Create a static slice JSON artifact under .sva",
                    create_slice_json_static_schema,
                ),
                Tool::new(
                    "create_slice_json_dynamic",
                    "Create a dynamic slice JSON artifact under .sva",
                    create_slice_json_dynamic_schema,
                ),
                Tool::new(
                    "query_signal_drivers",
                    "Query direct distance-1 signal drivers from a slice JSON artifact",
                    slice_artifact_query_schema.clone(),
                ),
                Tool::new(
                    "query_block_drivers",
                    "Query direct distance-1 block drivers from a slice JSON artifact",
                    slice_artifact_query_schema,
                ),
                Tool::new(
                    "blocks_query",
                    "Filter a saved blockize JSON file",
                    blocks_query_schema,
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
                Tool::new(
                    "wave_signal_search",
                    "Fuzzy search waveform signal names",
                    wave_signal_search_schema,
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
        let arguments = request
            .arguments
            .as_ref()
            .ok_or_else(|| rmcp::ErrorData::invalid_params("missing arguments", None))?;

        let result_text =
            match name.as_str() {
                "blockize" => {
                    let req: BlockizeRequest =
                        serde_json::from_value(serde_json::Value::Object(arguments.clone()))
                            .map_err(|e| {
                                rmcp::ErrorData::invalid_params(
                                    format!("invalid blockize request: {}", e),
                                    None,
                                )
                            })?;
                    self.blockize_impl(req)
                        .map_err(|e| rmcp::ErrorData::internal_error(e, None))?
                }
                "slice_static" => {
                    let req: StaticSliceRequest =
                        serde_json::from_value(serde_json::Value::Object(arguments.clone()))
                            .map_err(|e| {
                                rmcp::ErrorData::invalid_params(
                                    format!("invalid slice_static request: {}", e),
                                    None,
                                )
                            })?;
                    self.slice_static_impl(req)
                        .map_err(|e| rmcp::ErrorData::internal_error(e, None))?
                }
                "slice_dynamic" => {
                    let req: DynamicSliceRequest =
                        serde_json::from_value(serde_json::Value::Object(arguments.clone()))
                            .map_err(|e| {
                                rmcp::ErrorData::invalid_params(
                                    format!("invalid slice_dynamic request: {}", e),
                                    None,
                                )
                            })?;
                    self.slice_dynamic_impl(req)
                        .map_err(|e| rmcp::ErrorData::internal_error(e, None))?
                }
                "create_slice_json_static" => {
                    let req: CreateStaticSliceArtifactRequest =
                        serde_json::from_value(serde_json::Value::Object(arguments.clone()))
                            .map_err(|e| {
                                rmcp::ErrorData::invalid_params(
                                    format!("invalid create_slice_json_static request: {}", e),
                                    None,
                                )
                            })?;
                    self.create_slice_json_static_impl(req)
                        .map_err(|e| rmcp::ErrorData::internal_error(e, None))?
                }
                "create_slice_json_dynamic" => {
                    let req: CreateDynamicSliceArtifactRequest =
                        serde_json::from_value(serde_json::Value::Object(arguments.clone()))
                            .map_err(|e| {
                                rmcp::ErrorData::invalid_params(
                                    format!("invalid create_slice_json_dynamic request: {}", e),
                                    None,
                                )
                            })?;
                    self.create_slice_json_dynamic_impl(req)
                        .map_err(|e| rmcp::ErrorData::internal_error(e, None))?
                }
                "query_signal_drivers" => {
                    let req: SliceArtifactQueryRequest =
                        serde_json::from_value(serde_json::Value::Object(arguments.clone()))
                            .map_err(|e| {
                                rmcp::ErrorData::invalid_params(
                                    format!("invalid query_signal_drivers request: {}", e),
                                    None,
                                )
                            })?;
                    self.query_signal_drivers_impl(req)
                        .map_err(|e| rmcp::ErrorData::internal_error(e, None))?
                }
                "query_block_drivers" => {
                    let req: SliceArtifactQueryRequest =
                        serde_json::from_value(serde_json::Value::Object(arguments.clone()))
                            .map_err(|e| {
                                rmcp::ErrorData::invalid_params(
                                    format!("invalid query_block_drivers request: {}", e),
                                    None,
                                )
                            })?;
                    self.query_block_drivers_impl(req)
                        .map_err(|e| rmcp::ErrorData::internal_error(e, None))?
                }
                "blocks_query" => {
                    let req: BlocksQueryRequest =
                        serde_json::from_value(serde_json::Value::Object(arguments.clone()))
                            .map_err(|e| {
                                rmcp::ErrorData::invalid_params(
                                    format!("invalid blocks_query request: {}", e),
                                    None,
                                )
                            })?;
                    self.blocks_query_impl(req)
                        .map_err(|e| rmcp::ErrorData::internal_error(e, None))?
                }
                "coverage_report" => {
                    let req: CoverageReportRequest =
                        serde_json::from_value(serde_json::Value::Object(arguments.clone()))
                            .map_err(|e| {
                                rmcp::ErrorData::invalid_params(
                                    format!("invalid coverage_report request: {}", e),
                                    None,
                                )
                            })?;
                    self.coverage_report_impl(req)
                        .map_err(|e| rmcp::ErrorData::internal_error(e, None))?
                }
                "wave_value" => {
                    let req: WaveValueRequest =
                        serde_json::from_value(serde_json::Value::Object(arguments.clone()))
                            .map_err(|e| {
                                rmcp::ErrorData::invalid_params(
                                    format!("invalid wave_value request: {}", e),
                                    None,
                                )
                            })?;
                    self.wave_value_impl(req)
                        .map_err(|e| rmcp::ErrorData::internal_error(e, None))?
                }
                "wave_signal_search" => {
                    let req: WaveSignalSearchRequest =
                        serde_json::from_value(serde_json::Value::Object(arguments.clone()))
                            .map_err(|e| {
                                rmcp::ErrorData::invalid_params(
                                    format!("invalid wave_signal_search request: {}", e),
                                    None,
                                )
                            })?;
                    self.wave_signal_search_impl(req)
                        .map_err(|e| rmcp::ErrorData::internal_error(e, None))?
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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use serde_json::json;
    use sva_core::types::{
        BlockId, BlockJson, SignalNode, StableSliceEdgeJson, StableSliceGraphJson,
        StableSliceNodeJson,
    };

    use super::{
        BlockizeRequest, BlocksQueryRequest, CoverageReportRequest,
        CreateDynamicSliceArtifactRequest, CreateStaticSliceArtifactRequest, DynamicSliceRequest,
        SliceArtifactQueryRequest, StaticSliceRequest, SvaMcpServer, WaveSignalSearchRequest,
        WaveValueRequest,
    };

    #[test]
    fn source_parsing_requests_default_missing_sv_files_to_empty() {
        let blockize: BlockizeRequest = serde_json::from_value(json!({
            "project_path": "rtl"
        }))
        .unwrap();
        let static_slice: StaticSliceRequest = serde_json::from_value(json!({
            "project_path": "rtl",
            "signal": "TOP.top.y"
        }))
        .unwrap();
        let dynamic_slice: DynamicSliceRequest = serde_json::from_value(json!({
            "project_path": "rtl",
            "signal": "TOP.top.y",
            "vcd": "wave.fst",
            "time": 10,
            "min_time": 0
        }))
        .unwrap();
        let create_static: CreateStaticSliceArtifactRequest = serde_json::from_value(json!({
            "project_path": "rtl",
            "signal": "TOP.top.y"
        }))
        .unwrap();
        let create_dynamic: CreateDynamicSliceArtifactRequest = serde_json::from_value(json!({
            "project_path": "rtl",
            "signal": "TOP.top.y",
            "vcd": "wave.fst",
            "time": 10,
            "min_time": 0
        }))
        .unwrap();
        let coverage: CoverageReportRequest = serde_json::from_value(json!({
            "project_path": "rtl",
            "vcd": "wave.fst",
            "time": 10
        }))
        .unwrap();

        assert!(blockize.sv_files.is_empty());
        assert!(static_slice.sv_files.is_empty());
        assert!(dynamic_slice.sv_files.is_empty());
        assert!(create_static.sv_files.is_empty());
        assert!(create_dynamic.sv_files.is_empty());
        assert!(coverage.sv_files.is_empty());
    }

    #[test]
    fn blockize_impl_writes_artifact_and_returns_path() {
        let dir = unique_temp_dir("sva_mcp_blockize_artifact");
        let source = dir.join("design.sv");
        fs::write(
            &source,
            "module top(input logic a, output logic y);\nassign y = a;\nendmodule\n",
        )
        .unwrap();

        let output = SvaMcpServer
            .blockize_impl(BlockizeRequest {
                sv_files: vec![source.to_string_lossy().into_owned()],
                project_path: None,
                include_paths: Vec::new(),
                artifact_dir: Some(dir.join(".sva").to_string_lossy().into_owned()),
            })
            .unwrap();
        let json: serde_json::Value = serde_json::from_str(&output).unwrap();

        let path = json["path"].as_str().unwrap();
        assert!(PathBuf::from(path).exists());
        assert_eq!(json["mode"], "blockize");
        assert!(json["block_set"]["blocks"]
            .as_array()
            .is_some_and(|blocks| !blocks.is_empty()));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn query_signal_drivers_impl_reads_slice_artifact_json() {
        let dir = unique_temp_dir("sva_mcp_signal_query");
        let path = dir.join("slice-static-d.json");
        fs::write(
            &path,
            serde_json::to_vec_pretty(&distance_one_driver_graph()).unwrap(),
        )
        .unwrap();

        let output = SvaMcpServer
            .query_signal_drivers_impl(SliceArtifactQueryRequest {
                slice_json: Some(path.to_string_lossy().into_owned()),
                artifact_dir: None,
                signal: None,
            })
            .unwrap();
        let json: serde_json::Value = serde_json::from_str(&output).unwrap();

        assert_eq!(json["target"], "d");
        assert_eq!(json["signals"][0]["name"], "c");

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn blocks_query_impl_filters_saved_blockize_json() {
        let dir = unique_temp_dir("sva_mcp_blocks_query");
        let path = dir.join("blocks.json");
        fs::write(&path, serde_json::to_vec_pretty(&blocks_fixture()).unwrap()).unwrap();

        let output = SvaMcpServer
            .blocks_query_impl(BlocksQueryRequest {
                input: path.to_string_lossy().into_owned(),
                block_id: Some(8),
                output_signals: Vec::new(),
                input_signals: Vec::new(),
                scope: None,
                block_type: None,
                circuit_type: None,
                source_file: None,
            })
            .unwrap();
        let json: serde_json::Value = serde_json::from_str(&output).unwrap();

        assert_eq!(json["match_count"], 1);
        assert_eq!(json["blocks"][0]["id"], 8);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn wave_signal_search_impl_returns_fuzzy_matches() {
        let dir = unique_temp_dir("sva_mcp_wave_search");
        let path = dir.join("wave.vcd");
        fs::write(&path, wave_vcd()).unwrap();

        let output = SvaMcpServer
            .wave_signal_search_impl(WaveSignalSearchRequest {
                vcd: path.to_string_lossy().into_owned(),
                query: "tb sig".to_string(),
                limit: Some(3),
            })
            .unwrap();
        let json: serde_json::Value = serde_json::from_str(&output).unwrap();

        assert_eq!(json["query"], "tb sig");
        assert_eq!(json["matches"][0], "tb.sig");

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn wave_value_impl_returns_structured_json() {
        let dir = unique_temp_dir("sva_mcp_wave");
        let path = dir.join("wave.vcd");
        fs::write(&path, wave_vcd()).unwrap();

        let output = SvaMcpServer
            .wave_value_impl(WaveValueRequest {
                vcd: path.to_string_lossy().into_owned(),
                signal: "tb.sig".to_string(),
                time: 5,
            })
            .unwrap();
        let json: serde_json::Value = serde_json::from_str(&output).unwrap();

        assert_eq!(json["signal"], "tb.sig");
        assert_eq!(json["time"], 5);
        assert_eq!(json["value"]["raw_bits"], "1010");
        assert_eq!(json["value"]["pretty_hex"], "0xa");

        let _ = fs::remove_dir_all(dir);
    }

    fn distance_one_driver_graph() -> StableSliceGraphJson {
        StableSliceGraphJson {
            target: "d".to_string(),
            start_time: None,
            nodes: vec![
                StableSliceNodeJson::Block {
                    id: 0,
                    block_id: BlockId(1),
                    time: None,
                },
                StableSliceNodeJson::Block {
                    id: 1,
                    block_id: BlockId(2),
                    time: None,
                },
                StableSliceNodeJson::Block {
                    id: 2,
                    block_id: BlockId(3),
                    time: None,
                },
                StableSliceNodeJson::Block {
                    id: 3,
                    block_id: BlockId(4),
                    time: None,
                },
            ],
            edges: vec![
                StableSliceEdgeJson {
                    from: 0,
                    to: 2,
                    signal: Some(SignalNode::named("a")),
                },
                StableSliceEdgeJson {
                    from: 1,
                    to: 2,
                    signal: Some(SignalNode::named("b")),
                },
                StableSliceEdgeJson {
                    from: 2,
                    to: 3,
                    signal: Some(SignalNode::named("c")),
                },
            ],
            blocks: vec![block_json(1), block_json(2), block_json(3), block_json(4)],
        }
    }

    fn block_json(id: u64) -> BlockJson {
        BlockJson {
            id: BlockId(id),
            scope: "TOP.dut".to_string(),
            block_type: "Assign".to_string(),
            source_file: "design.sv".to_string(),
            line_start: id as usize,
            line_end: id as usize,
            ast_line_start: id as usize,
            ast_line_end: id as usize,
            code_snippet: format!("assign b{id} = a{id};"),
        }
    }

    fn blocks_fixture() -> serde_json::Value {
        json!({
            "blocks": [
                {
                    "id": 7,
                    "block_type": "Always",
                    "circuit_type": "Sequential",
                    "module_scope": "TOP.a.b",
                    "source_file": "/tmp/project/rtl/child.sv",
                    "line_start": 10,
                    "line_end": 20,
                    "ast_line_start": 10,
                    "ast_line_end": 20,
                    "input_signals": ["TOP.a.b.in0"],
                    "output_signals": ["TOP.a.b.out0"],
                    "dataflow": [],
                    "code_snippet": "always_ff @(posedge clk) out0 <= in0;"
                },
                {
                    "id": 8,
                    "block_type": "Assign",
                    "circuit_type": "Combinational",
                    "module_scope": "TOP.a.b",
                    "source_file": "/tmp/project/rtl/child.sv",
                    "line_start": 22,
                    "line_end": 22,
                    "ast_line_start": 22,
                    "ast_line_end": 22,
                    "input_signals": ["TOP.a.b.in0"],
                    "output_signals": ["TOP.a.b.out2"],
                    "dataflow": [],
                    "code_snippet": "assign out2 = in0;"
                }
            ]
        })
    }

    fn wave_vcd() -> &'static str {
        "$date\n    today\n$end\n\
         $version\n    test\n$end\n\
         $timescale 1ns $end\n\
         $scope module tb $end\n\
         $var wire 4 ! sig $end\n\
         $upscope $end\n\
         $enddefinitions $end\n\
         #0\n\
         b0000 !\n\
         #5\n\
         b1010 !\n"
    }

    fn unique_temp_dir(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("{name}_{}_{}", std::process::id(), unique));
        fs::create_dir_all(&path).unwrap();
        path
    }
}
