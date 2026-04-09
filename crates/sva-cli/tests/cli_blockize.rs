use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

fn main_bin() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_sva_cli"))
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

#[test]
fn cli_blockize_outputs_json() {
    let fixture = write_fixture(
        "module demo(input logic a, input logic b, output logic y);\n  assign y = a & b;\nendmodule\n",
    );

    let output = Command::new(main_bin())
        .args(["blockize", "--sv", fixture.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    let object = json.as_object().unwrap();

    assert_eq!(object.len(), 2, "unexpected top-level shape: {object:?}");
    assert!(object.contains_key("blocks"));
    assert!(object.contains_key("signal_to_drivers"));
    assert!(json["blocks"].is_array());
    assert!(json["signal_to_drivers"].is_object());

    let _ = fs::remove_file(fixture);
}

#[test]
fn cli_blockize_requires_sv_argument() {
    let output = Command::new(main_bin())
        .args(["blockize"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("at least one SystemVerilog source is required via --sv or --project-path"));
}

#[test]
fn cli_blockize_accepts_repeated_sv_arguments() {
    let fixture_a = write_fixture(
        "module demo_a(input logic a, output logic y);\n  assign y = a;\nendmodule\n",
    );
    let fixture_b = write_fixture(
        "module demo_b(input logic b, output logic z);\n  assign z = b;\nendmodule\n",
    );

    let output = Command::new(main_bin())
        .args([
            "blockize",
            "--sv",
            fixture_a.to_str().unwrap(),
            "--sv",
            fixture_b.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    let blocks = json["blocks"].as_array().unwrap();
    assert!(
        blocks.len() >= 4,
        "expected combined blocks from both files: {blocks:?}"
    );

    let _ = fs::remove_file(fixture_a);
    let _ = fs::remove_file(fixture_b);
}

#[test]
fn cli_blockize_processes_multi_submodule_demo_modules() {
    let design = workspace_root().join("demo/multi_submodule_demo/design.sv");
    let testbench = workspace_root().join("demo/multi_submodule_demo/tb.sv");

    let output = Command::new(main_bin())
        .args([
            "blockize",
            "--sv",
            design.to_str().unwrap(),
            "--sv",
            testbench.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    let scopes = json["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|block| block["module_scope"].as_str())
        .collect::<std::collections::BTreeSet<_>>();

    assert!(scopes.contains("top"), "missing top blocks: {json:?}");
    assert!(
        scopes.contains("submodule1"),
        "missing submodule1 blocks: {json:?}"
    );
    assert!(
        scopes.contains("submodule2"),
        "missing submodule2 blocks: {json:?}"
    );

    assert!(
        json["blocks"].as_array().unwrap().iter().any(|block| {
            block["module_scope"] == "submodule1" && block["block_type"] == "Assign"
        }),
        "expected assign logic in submodule1: {json:?}"
    );
    assert!(
        json["blocks"].as_array().unwrap().iter().any(|block| {
            block["module_scope"] == "submodule1" && block["block_type"] == "Always"
        }),
        "expected always/always_comb logic in submodule1: {json:?}"
    );
    assert!(
        json["blocks"].as_array().unwrap().iter().any(|block| {
            block["module_scope"] == "submodule2" && block["block_type"] == "Always"
        }),
        "expected sequential always logic in submodule2: {json:?}"
    );
    assert!(
        json["blocks"].as_array().unwrap().iter().any(|block| {
            block["module_scope"] == "submodule2" && block["block_type"] == "Assign"
        }),
        "expected assign logic in submodule2: {json:?}"
    );
}

#[test]
fn cli_blockize_supports_project_path_and_external_include_paths() {
    let fixture = write_project_fixture();
    let include_paths = format!(
        "{},{}",
        fixture.primary_include.display(),
        fixture.secondary_include.display()
    );

    let output = Command::new(main_bin())
        .arg("blockize")
        .arg("--project-path")
        .arg(&fixture.src_dir)
        .arg("--include-paths")
        .arg(&include_paths)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    let scopes = json["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|block| block["module_scope"].as_str())
        .collect::<std::collections::BTreeSet<_>>();

    assert!(
        scopes.contains("demo_top"),
        "missing top module blocks: {json:?}"
    );
    assert!(
        scopes.contains("helper_mod"),
        "missing discovered helper module blocks: {json:?}"
    );
    assert!(
        json["signal_to_drivers"].get("rvfi_valid").is_some(),
        "expected RVFI-gated driver in blockized output: {json:?}"
    );

    fixture.cleanup();
}

fn write_fixture(contents: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "sva_task13_cli_blockize_{}_{}.sv",
        std::process::id(),
        unique
    ));

    fs::write(&path, contents).unwrap();
    path
}

struct ProjectFixture {
    root: PathBuf,
    src_dir: PathBuf,
    primary_include: PathBuf,
    secondary_include: PathBuf,
}

impl ProjectFixture {
    fn cleanup(self) {
        let _ = fs::remove_dir_all(self.root);
    }
}

fn write_project_fixture() -> ProjectFixture {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "sva_task13_cli_blockize_project_{}_{}",
        std::process::id(),
        unique
    ));
    let src_dir = root.join("src");
    let primary_include = root.join("include_a");
    let secondary_include = root.join("include_b");

    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&primary_include).unwrap();
    fs::create_dir_all(&secondary_include).unwrap();

    fs::write(
        primary_include.join("shared_macros.svh"),
        "`define PRIMARY_BUF(signal_) (signal_)\n",
    )
    .unwrap();
    fs::write(
        secondary_include.join("dv_like.svh"),
        "`define SECONDARY_OR(lhs_, rhs_) ((lhs_) | (rhs_))\n",
    )
    .unwrap();
    fs::write(
        src_dir.join("helper_mod.sv"),
        "module helper_mod(input logic a, output logic y);\n  assign y = a;\nendmodule\n",
    )
    .unwrap();
    fs::write(src_dir.join("demo_top.f"), "demo_top.sv\nhelper_mod.sv\n").unwrap();
    fs::write(
        src_dir.join("demo_top.sv"),
        "`include \"shared_macros.svh\"\n`include \"dv_like.svh\"\nmodule demo_top(\n  input logic a,\n  output logic y,\n`ifdef RVFI\n  output logic rvfi_valid,\n`endif\n  output logic helper_y\n);\n  helper_mod u_helper(.a(a), .y(helper_y));\n  assign y = `PRIMARY_BUF(`SECONDARY_OR(a, helper_y));\n`ifdef RVFI\n  assign rvfi_valid = helper_y;\n`endif\nendmodule\n",
    )
    .unwrap();

    ProjectFixture {
        root,
        src_dir,
        primary_include,
        secondary_include,
    }
}
