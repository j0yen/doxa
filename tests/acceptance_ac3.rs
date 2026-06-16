//! AC3 (MUST): `ousia-forge check --spec spec-core/` reports the spec valid.
//! (Equivalently: `doxa check-core` exits 0.)

use std::process::Command;

#[test]
fn acceptance_ac3_check_core_valid() {
    // Skip gracefully if ousia-forge is absent.
    let forge_present = Command::new("which")
        .arg("ousia-forge")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !forge_present {
        eprintln!("AC3: ousia-forge not on PATH — skipping live check-core validation");
        return;
    }

    let bin = env!("CARGO_BIN_EXE_doxa");
    let status = Command::new(bin)
        .args(["check-core", "--spec", "spec-core"])
        .status()
        .expect("failed to launch doxa binary");

    assert!(
        status.success(),
        "doxa check-core must exit 0 — spec-core/ must be a valid ousia-forge spec"
    );
}
