use std::process::Command;

#[test]
fn cli_shows_help() {
    let output = Command::new(env!("CARGO_BIN_EXE_main"))
        .arg("--help")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    assert!(stdout.contains("dataflow-engine"));
    assert!(stdout.contains("blockize"));
    assert!(stdout.contains("slice"));
    assert!(stdout.contains("coverage"));
    assert!(stdout.contains("wave"));
}
