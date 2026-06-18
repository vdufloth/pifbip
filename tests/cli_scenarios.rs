use std::path::Path;
use std::process::Command;

#[test]
fn interactive_cli_scenarios() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let binary = env!("CARGO_BIN_EXE_pifbip");

    let status = Command::new("python3")
        .current_dir(manifest_dir)
        .arg("tests/cli_scenarios.py")
        .arg("--binary")
        .arg(binary)
        .status()
        .expect("failed to run tests/cli_scenarios.py with python3");

    assert!(status.success(), "interactive CLI scenarios failed");
}
