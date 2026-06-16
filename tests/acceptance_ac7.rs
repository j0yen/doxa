//! AC7 (MUST): `doxa check-core` exits non-zero on a deliberately malformed spec fixture.

use std::process::Command;

#[test]
fn acceptance_ac7_check_core_rejects_malformed_spec() {
    let forge_present = Command::new("which")
        .arg("ousia-forge")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !forge_present {
        eprintln!("AC7: ousia-forge not on PATH — skipping malformed-spec rejection test");
        return;
    }

    // Create a temp directory with a deliberately malformed spec.
    let tmp = std::env::temp_dir().join("doxa_ac7_malformed_spec");
    std::fs::create_dir_all(&tmp).expect("failed to create temp spec dir");

    // Write a malformed ontology.toml (missing required `iri` field in a class block).
    let malformed = r#"
# malformed spec — missing iri field in class block
domain = "test"

[[classes]]
label = "BrokenClass"
parent = "BFO:role"
definition = "This class has no iri, making the spec invalid."
"#;
    std::fs::write(tmp.join("broken.toml"), malformed)
        .expect("failed to write malformed spec file");

    // Also write a minimal ontology.toml.
    let ontology = r#"
iri = "https://example.org/malformed-test"
version_iri = "https://example.org/malformed-test/0.1"
"#;
    std::fs::write(tmp.join("ontology.toml"), ontology)
        .expect("failed to write malformed ontology.toml");

    let bin = env!("CARGO_BIN_EXE_doxa");
    let status = Command::new(bin)
        .args([
            "check-core",
            "--spec",
            tmp.to_str().unwrap_or("/tmp/doxa_ac7_malformed_spec"),
        ])
        .status()
        .expect("failed to launch doxa binary");

    assert!(
        !status.success(),
        "doxa check-core must exit non-zero on a malformed spec fixture"
    );

    // Cleanup.
    let _ = std::fs::remove_dir_all(&tmp);
}
