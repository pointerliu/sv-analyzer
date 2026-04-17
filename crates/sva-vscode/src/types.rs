use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct BlockizeParams {
    pub sv_files: Vec<String>,
    #[serde(default)]
    pub project_path: Option<String>,
    #[serde(default)]
    pub include_paths: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct SliceParams {
    pub sv_files: Vec<String>,
    #[serde(default)]
    pub project_path: Option<String>,
    #[serde(default)]
    pub include_paths: Vec<String>,
    /// Hierarchical signal name, e.g. "tb.dut.u_stage3.result"
    pub signal: String,
    #[serde(default)]
    pub vcd: Option<String>,
    #[serde(default)]
    pub tree_json: Option<String>,
    #[serde(default)]
    pub tree_meta_json: Option<String>,
    #[serde(default)]
    pub time: Option<i64>,
    #[serde(default)]
    pub min_time: Option<i64>,
    #[serde(default)]
    pub r#static: bool,
    #[serde(default)]
    pub clock: Option<String>,
    #[serde(default)]
    pub clk_step: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct CoverageParams {
    pub sv_files: Vec<String>,
    #[serde(default)]
    pub project_path: Option<String>,
    #[serde(default)]
    pub include_paths: Vec<String>,
    pub vcd: String,
    pub time: i64,
}

#[derive(Debug, Deserialize)]
pub struct WaveParams {
    pub vcd: String,
    /// Hierarchical signal name, e.g. "tb.dut.u_stage3.result"
    pub signal: String,
    pub time: i64,
}
