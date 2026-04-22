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

    // After elaboration, scopes are hierarchical paths like TOP.tb.u_top, TOP.tb.u_top.u_sub1, etc.
    assert!(
        scopes.iter().any(|s| s.ends_with("u_top")),
        "missing top blocks: {scopes:?}"
    );
    assert!(
        scopes.iter().any(|s| s.ends_with("u_sub1")),
        "missing submodule1 blocks: {scopes:?}"
    );
    assert!(
        scopes.iter().any(|s| s.ends_with("u_sub2")),
        "missing submodule2 blocks: {scopes:?}"
    );

    assert!(
        json["blocks"].as_array().unwrap().iter().any(|block| {
            block["module_scope"]
                .as_str()
                .is_some_and(|s| s.ends_with("u_sub1"))
                && block["block_type"] == "Assign"
        }),
        "expected assign logic in submodule1: {json:?}"
    );
    assert!(
        json["blocks"].as_array().unwrap().iter().any(|block| {
            block["module_scope"]
                .as_str()
                .is_some_and(|s| s.ends_with("u_sub1"))
                && block["block_type"] == "Always"
        }),
        "expected always/always_comb logic in submodule1: {json:?}"
    );
    assert!(
        json["blocks"].as_array().unwrap().iter().any(|block| {
            block["module_scope"]
                .as_str()
                .is_some_and(|s| s.ends_with("u_sub2"))
                && block["block_type"] == "Always"
        }),
        "expected sequential always logic in submodule2: {json:?}"
    );
    assert!(
        json["blocks"].as_array().unwrap().iter().any(|block| {
            block["module_scope"]
                .as_str()
                .is_some_and(|s| s.ends_with("u_sub2"))
                && block["block_type"] == "Assign"
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

    // After elaboration, scopes are hierarchical like TOP.demo_top, TOP.demo_top.u_helper
    assert!(
        scopes.iter().any(|s| s.ends_with("demo_top")),
        "missing top module blocks: {scopes:?}"
    );
    assert!(
        scopes.iter().any(|s| s.ends_with("u_helper")),
        "missing discovered helper module blocks: {scopes:?}"
    );
    // Signal names are now fully qualified after elaboration
    let drivers = json["signal_to_drivers"].as_object().unwrap();
    assert!(
        drivers.keys().any(|k| k.ends_with("rvfi_valid")),
        "expected RVFI-gated driver in blockized output: {drivers:?}"
    );

    fixture.cleanup();
}

#[test]
fn cli_blockize_keeps_block_ids_stable_across_processes() {
    let fixture = write_fixture(
        "module demo(\n  input logic a,\n  input logic b,\n  input logic c,\n  input logic d,\n  output logic y,\n  output logic z\n);\n  logic tmp_y;\n  logic tmp_z;\n\n  assign tmp_y = a & b;\n  assign y = tmp_y;\n\n  assign tmp_z = c | d;\n  assign z = tmp_z;\nendmodule\n",
    );

    let baseline = block_ids_by_identity(&fixture);
    assert!(
        baseline.len() >= 8,
        "expected port and assign blocks in blockize output: {baseline:?}"
    );

    for _ in 0..24 {
        let current = block_ids_by_identity(&fixture);
        assert_eq!(
            current, baseline,
            "expected stable block IDs across repeated sva_cli blockize runs"
        );
    }

    let _ = fs::remove_file(fixture);
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

fn block_ids_by_identity(path: &Path) -> std::collections::BTreeMap<String, u64> {
    let output = Command::new(main_bin())
        .args(["blockize", "--sv", path.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    json["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|block| {
            let identity = format!(
                "{}|{}|{}|{}|{}",
                block["module_scope"].as_str().unwrap(),
                block["block_type"].as_str().unwrap(),
                block["line_start"].as_u64().unwrap(),
                block["line_end"].as_u64().unwrap(),
                block["code_snippet"].as_str().unwrap()
            );
            (identity, block["id"].as_u64().unwrap())
        })
        .collect()
}
