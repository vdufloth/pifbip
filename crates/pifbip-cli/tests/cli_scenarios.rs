use std::path::Path;
use std::process::Command;

#[test]
fn interactive_cli_scenarios() {
    // This crate lives at <workspace>/crates/pifbip-cli; the scenario harness
    // and demo assets live at the workspace root.
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("workspace root is two levels above the crate manifest");
    let binary = env!("CARGO_BIN_EXE_pifbip-cli");

    let status = Command::new("python3")
        .current_dir(workspace_root)
        .arg("tests/cli_scenarios.py")
        .arg("--binary")
        .arg(binary)
        .status()
        .expect("failed to run tests/cli_scenarios.py with python3");

    assert!(status.success(), "interactive CLI scenarios failed");
}
