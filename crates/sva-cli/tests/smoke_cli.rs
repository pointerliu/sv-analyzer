use std::process::Command;

fn main_bin() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_sva_cli"))
}

#[test]
fn cli_shows_help() {
    let output = Command::new(main_bin()).arg("--help").output().unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    assert!(stdout.contains("dataflow-engine"));
    assert!(stdout.contains("blockize"));
    assert!(stdout.contains("blocks"));
    assert!(stdout.contains("slice"));
    assert!(stdout.contains("coverage"));
    assert!(stdout.contains("wave"));
}
