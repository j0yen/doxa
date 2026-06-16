//! AC2 (MUST): `doxa build-core --out /tmp/core.owl` exits 0 and writes a non-empty OWL file
//! when `ousia-forge` is on `$PATH`; logs a note and exits 0 if absent.

use std::process::Command;

/// When ousia-forge is absent the binary should exit 0 with a logged note.
///
/// When ousia-forge IS present (CI / real runs), it should produce a non-empty OWL file.
#[test]
fn acceptance_ac2_build_core_graceful() {
    // Locate the built binary.
    let bin = env!("CARGO_BIN_EXE_doxa");

    let out_path = std::env::temp_dir().join("doxa_ac2_core.owl");
    let _ = std::fs::remove_file(&out_path); // clean up from prior run

    let status = Command::new(bin)
        .args([
            "build-core",
            "--out",
            out_path.to_str().unwrap_or("/tmp/doxa_ac2_core.owl"),
            "--spec",
            "spec-core",
        ])
        .status()
        .expect("failed to launch doxa binary");

    assert!(
        status.success(),
        "doxa build-core must exit 0 (graceful when forge absent, successful when present)"
    );

    // If ousia-forge is on PATH, we expect a non-empty file.
    let forge_present = Command::new("which")
        .arg("ousia-forge")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if forge_present {
        let meta = std::fs::metadata(&out_path)
            .expect("core.owl should exist after build-core when ousia-forge is present");
        assert!(
            meta.len() > 0,
            "core.owl must be non-empty when ousia-forge is present"
        );
    }
}
