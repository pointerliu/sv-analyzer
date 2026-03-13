use dac26_mcp::ast::{AstProvider, SvParserProvider};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_temp_dir() -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("dac26_mcp_parse_demo_sv_{suffix}"))
}

#[test]
fn parses_systemverilog_files_and_retains_blockization_inputs() {
    let temp_dir = unique_temp_dir();
    fs::create_dir_all(&temp_dir).unwrap();

    let design_path = temp_dir.join("dut_mod.sv");
    let include_path = temp_dir.join("included_defs.svh");
    let tb_path = temp_dir.join("tb.sv");

    fs::write(
        &include_path,
        "module helper_mod(sig);\n  input sig;\nendmodule\n",
    )
    .unwrap();
    fs::write(
        &design_path,
        "`include \"included_defs.svh\"\nmodule dut_mod(a);\n  input a;\n  helper_mod helper(.sig(a));\nendmodule\n",
    )
    .unwrap();
    fs::write(&tb_path, "module tb_top(clk);\n  input clk;\nendmodule\n").unwrap();

    let provider = SvParserProvider::default();
    let files = provider
        .parse_files(&[design_path.clone(), tb_path.clone()])
        .unwrap();

    assert_eq!(files.len(), 2);
    assert!(files.iter().any(|file| file.path == design_path));
    assert!(files.iter().any(|file| file.path == tb_path));
    assert!(files
        .iter()
        .any(|file| file.source_text.contains("`include \"included_defs.svh\"")));
    assert!(files
        .iter()
        .any(|file| format!("{:?}", file.syntax_tree).contains("helper_mod")));
    assert!(files
        .iter()
        .all(|file| !format!("{:?}", file.syntax_tree).is_empty()));

    fs::remove_dir_all(temp_dir).unwrap();
}
