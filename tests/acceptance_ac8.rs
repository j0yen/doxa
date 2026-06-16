//! AC8 (MUST): `doxa list` exits 0 and outputs at least the three framework names.

use std::process::Command;

#[test]
fn acceptance_ac8_list_exits_zero_and_shows_frameworks() {
    let bin = env!("CARGO_BIN_EXE_doxa");

    let output = Command::new(bin)
        .args(["list"])
        .output()
        .expect("failed to launch doxa binary");

    assert!(
        output.status.success(),
        "`doxa list` must exit 0; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("consequentialism"),
        "`doxa list` output must contain 'consequentialism'; got:\n{stdout}"
    );
    assert!(
        stdout.contains("deontology"),
        "`doxa list` output must contain 'deontology'; got:\n{stdout}"
    );
    assert!(
        stdout.contains("virtue-ethics"),
        "`doxa list` output must contain 'virtue-ethics'; got:\n{stdout}"
    );
}

#[test]
fn acceptance_ac8_list_json_exits_zero() {
    let bin = env!("CARGO_BIN_EXE_doxa");

    let output = Command::new(bin)
        .args(["list", "--format", "json"])
        .output()
        .expect("failed to launch doxa binary");

    assert!(
        output.status.success(),
        "`doxa list --format json` must exit 0; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should be valid-ish JSON array
    assert!(
        stdout.trim().starts_with('[') && stdout.trim().ends_with(']'),
        "`doxa list --format json` must output a JSON array; got:\n{stdout}"
    );
}
