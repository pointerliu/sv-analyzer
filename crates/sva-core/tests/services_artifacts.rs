use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::json;
use sva_core::ast::ParseOptions;
use sva_core::services::{
    blockize, blocks_query, create_blockize_json, create_slice_json_static,
    query_slice_block_drivers, query_slice_signal_drivers, slice_static, BlockizeRequest,
    BlocksQueryRequest, CreateBlockizeArtifactRequest, CreateStaticSliceArtifactRequest,
    SliceArtifactQueryRequest, StaticSliceRequest,
};
use sva_core::types::{BlockId, BlockJson, SignalNode, StableSliceEdgeJson, StableSliceGraphJson};

#[test]
fn create_blockize_json_writes_artifact_under_sva() {
    let dir = unique_temp_dir("sva_blockize_artifact");
    let source = dir.join("design.sv");
    fs::write(
        &source,
        "module top(input logic a, output logic y);\nassign y = a;\nendmodule\n",
    )
    .unwrap();

    let response = create_blockize_json(CreateBlockizeArtifactRequest {
        sv_files: vec![source],
        parse_options: ParseOptions::default(),
        artifact_dir: Some(dir.join(".sva")),
    })
    .unwrap();

    let artifact_path = PathBuf::from(&response.path);
    assert!(artifact_path.starts_with(dir.join(".sva")));
    assert!(artifact_path.exists());
    assert_eq!(response.mode, "blockize");

    let saved: serde_json::Value =
        serde_json::from_slice(&fs::read(&artifact_path).unwrap()).unwrap();
    assert!(saved["blocks"]
        .as_array()
        .is_some_and(|blocks| !blocks.is_empty()));
    assert!(saved["signal_to_drivers"].is_object());

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn create_static_slice_json_writes_artifact_under_sva() {
    let dir = unique_temp_dir("sva_static_artifact");
    let source = dir.join("design.sv");
    fs::write(
        &source,
        "module top(input logic a, output logic y);\nassign y = a;\nendmodule\n",
    )
    .unwrap();

    let response = create_slice_json_static(CreateStaticSliceArtifactRequest {
        sv_files: vec![source],
        parse_options: ParseOptions::default(),
        signal: "TOP.top.y".to_string(),
        artifact_dir: Some(dir.join(".sva")),
    })
    .unwrap();

    let artifact_path = PathBuf::from(&response.path);
    assert!(artifact_path.starts_with(dir.join(".sva")));
    assert!(artifact_path.exists());

    let saved: StableSliceGraphJson =
        serde_json::from_slice(&fs::read(&artifact_path).unwrap()).unwrap();
    assert_eq!(saved.target, "TOP.top.y");
    assert_eq!(response.mode, "static");

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn static_slice_preserves_block_id_from_blockize_for_matching_assign_block() {
    let dir = unique_temp_dir("sva_static_slice_block_id");
    let source = dir.join("design.sv");
    fs::write(
        &source,
        "module top(input logic a, input logic b, output logic y);\n  logic tmp;\n  assign tmp = a & b;\n  assign y = tmp;\nendmodule\n",
    )
    .unwrap();

    let block_set = blockize(BlockizeRequest {
        sv_files: vec![source.clone()],
        parse_options: ParseOptions::default(),
    })
    .unwrap();
    let static_graph = slice_static(StaticSliceRequest {
        sv_files: vec![source],
        parse_options: ParseOptions::default(),
        signal: "TOP.top.y".to_string(),
    })
    .unwrap();

    let blockize_assign = block_set
        .blocks()
        .iter()
        .find(|block| block.line_start() == 3 && block.line_end() == 4)
        .unwrap();
    let static_assign = static_graph
        .blocks
        .iter()
        .find(|block| block.line_start == 3 && block.line_end == 4)
        .unwrap();

    assert_eq!(blockize_assign.id().0, static_assign.id.0);

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn slice_signal_query_reads_latest_artifact_and_returns_distance_one_signals() {
    let dir = unique_temp_dir("sva_signal_query");
    let artifact_dir = dir.join(".sva");
    fs::create_dir_all(&artifact_dir).unwrap();

    fs::write(
        artifact_dir.join("slice-static-out-0001.json"),
        serde_json::to_vec_pretty(&slice_graph("old", &["old_sig"], &[1])).unwrap(),
    )
    .unwrap();
    fs::write(
        artifact_dir.join("slice-static-out-0002.json"),
        serde_json::to_vec_pretty(&distance_one_driver_graph()).unwrap(),
    )
    .unwrap();

    let response = query_slice_signal_drivers(SliceArtifactQueryRequest {
        slice_json: None,
        artifact_dir: Some(artifact_dir),
        signal: None,
    })
    .unwrap();

    assert_eq!(response.target, "d");
    let signal_names = response
        .signals
        .iter()
        .map(|signal| signal.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(signal_names, vec!["c"]);

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn slice_signal_query_can_override_artifact_target_with_requested_signal() {
    let dir = unique_temp_dir("sva_signal_query_override");
    let path = dir.join("slice-static-d.json");
    fs::write(
        &path,
        serde_json::to_vec_pretty(&distance_one_driver_graph()).unwrap(),
    )
    .unwrap();

    let response = query_slice_signal_drivers(SliceArtifactQueryRequest {
        slice_json: Some(path),
        artifact_dir: None,
        signal: Some("c".to_string()),
    })
    .unwrap();

    let signal_names = response
        .signals
        .iter()
        .map(|signal| signal.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(response.target, "c");
    assert_eq!(signal_names, vec!["a", "b"]);

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn slice_block_query_reads_artifact_and_returns_direct_driver_block() {
    let dir = unique_temp_dir("sva_block_query");
    let path = dir.join("slice-static-d.json");
    fs::write(
        &path,
        serde_json::to_vec_pretty(&distance_one_driver_graph()).unwrap(),
    )
    .unwrap();

    let response = query_slice_block_drivers(SliceArtifactQueryRequest {
        slice_json: Some(path),
        artifact_dir: None,
        signal: None,
    })
    .unwrap();

    let block_ids = response
        .blocks
        .iter()
        .map(|block| block.id.0)
        .collect::<Vec<_>>();
    assert_eq!(response.target, "d");
    assert_eq!(block_ids, vec![4]);

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn slice_block_query_can_override_artifact_target_with_requested_signal() {
    let dir = unique_temp_dir("sva_block_query_override");
    let path = dir.join("slice-static-d.json");
    fs::write(
        &path,
        serde_json::to_vec_pretty(&distance_one_driver_graph()).unwrap(),
    )
    .unwrap();

    let response = query_slice_block_drivers(SliceArtifactQueryRequest {
        slice_json: Some(path),
        artifact_dir: None,
        signal: Some("c".to_string()),
    })
    .unwrap();

    let block_ids = response
        .blocks
        .iter()
        .map(|block| block.id.0)
        .collect::<Vec<_>>();
    assert_eq!(response.target, "c");
    assert_eq!(block_ids, vec![3]);

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn blocks_query_matches_cli_filters() {
    let dir = unique_temp_dir("sva_blocks_query");
    let input = dir.join("blocks.json");
    fs::write(
        &input,
        serde_json::to_vec_pretty(&blocks_fixture()).unwrap(),
    )
    .unwrap();

    let response = blocks_query(BlocksQueryRequest {
        input,
        block_id: None,
        output_signals: vec!["TOP.a.b.out0".to_string(), "TOP.a.b.out1".to_string()],
        input_signals: vec!["TOP.a.b.in0".to_string(), "TOP.a.b.in1".to_string()],
        scope: Some("TOP.a".to_string()),
        block_type: Some("always".to_string()),
        circuit_type: Some("sequential".to_string()),
        source_file: Some("rtl/child.sv".to_string()),
    })
    .unwrap();

    assert_eq!(response.match_count, 1);
    assert_eq!(response.blocks[0]["id"], 7);
    assert_eq!(response.blocks[0]["module_scope"], "TOP.a.b");

    let _ = fs::remove_dir_all(dir);
}

fn slice_graph(target: &str, signals: &[&str], block_ids: &[u64]) -> StableSliceGraphJson {
    StableSliceGraphJson {
        target: target.to_string(),
        start_time: None,
        nodes: Vec::new(),
        edges: signals
            .iter()
            .enumerate()
            .map(|(index, signal)| StableSliceEdgeJson {
                from: index,
                to: index + 1,
                signal: Some(SignalNode::named(*signal)),
            })
            .collect(),
        blocks: block_ids
            .iter()
            .map(|id| BlockJson {
                id: BlockId(*id),
                scope: "TOP.dut".to_string(),
                block_type: "Assign".to_string(),
                source_file: "design.sv".to_string(),
                line_start: *id as usize,
                line_end: *id as usize,
                ast_line_start: *id as usize,
                ast_line_end: *id as usize,
                code_snippet: format!("assign b{id} = a{id};"),
            })
            .collect(),
    }
}

fn distance_one_driver_graph() -> StableSliceGraphJson {
    use sva_core::types::StableSliceNodeJson;

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
    let locate = json!({
        "offset": 0,
        "line": 0,
        "ast_line": 0,
        "len": 12
    });

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
                "input_signals": ["TOP.a.b.in0", "TOP.a.b.in1"],
                "output_signals": ["TOP.a.b.out0", "TOP.a.b.out1"],
                "dataflow": [
                    {
                        "output": [
                            {"kind": "variable", "name": "TOP.a.b.out0", "locate": locate},
                            {"kind": "variable", "name": "TOP.a.b.out1", "locate": locate}
                        ],
                        "inputs": [
                            {"kind": "variable", "name": "TOP.a.b.in0", "locate": locate},
                            {"kind": "variable", "name": "TOP.a.b.in1", "locate": locate}
                        ]
                    }
                ],
                "code_snippet": "always_ff @(posedge clk) begin out0 <= in0; out1 <= in1; end"
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

fn unique_temp_dir(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("{name}_{}_{}", std::process::id(), unique));
    fs::create_dir_all(&path).unwrap();
    path
}
