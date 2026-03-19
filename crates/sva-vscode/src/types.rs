use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct BlockizeParams {
    pub sv_files: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct SliceParams {
    pub sv_files: Vec<String>,
    pub signal: String,
    #[serde(default)]
    pub vcd: Option<String>,
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
    pub vcd: String,
    pub time: i64,
}

#[derive(Debug, Deserialize)]
pub struct WaveParams {
    pub vcd: String,
    pub signal: String,
    pub time: i64,
}
