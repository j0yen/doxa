//! AC9 (MUST): `doxa build <framework>` exits 0 and writes a non-empty OWL file.

use std::process::Command;

#[test]
fn acceptance_ac9_build_consequentialism() {
    let bin = env!("CARGO_BIN_EXE_doxa");
    let out_path = std::env::temp_dir().join("doxa_ac9_consequentialism.owl");
    let _ = std::fs::remove_file(&out_path);

    let status = Command::new(bin)
        .args([
            "build",
            "consequentialism",
            "--out",
            out_path.to_str().unwrap_or("/tmp/doxa_ac9_consequentialism.owl"),
        ])
        .status()
        .expect("failed to launch doxa binary");

    assert!(
        status.success(),
        "`doxa build consequentialism` must exit 0"
    );

    let meta = std::fs::metadata(&out_path)
        .expect("consequentialism.owl must exist after build");
    assert!(meta.len() > 0, "consequentialism.owl must be non-empty");

    // Check for key content
    let owl = std::fs::read_to_string(&out_path).expect("must be readable");
    assert!(
        owl.contains("RightAction"),
        "OWL must contain RightAction"
    );
    // Consequentialism cites Mill
    assert!(
        owl.contains("Mill"),
        "consequentialism OWL must contain 'Mill' from philosophicalGrounding"
    );
    // References Wellbeing or maximizes
    assert!(
        owl.contains("Wellbeing") || owl.contains("maximizes"),
        "consequentialism OWL must reference Wellbeing or maximizes"
    );
}

#[test]
fn acceptance_ac9_build_deontology() {
    let bin = env!("CARGO_BIN_EXE_doxa");
    let out_path = std::env::temp_dir().join("doxa_ac9_deontology.owl");
    let _ = std::fs::remove_file(&out_path);

    let status = Command::new(bin)
        .args([
            "build",
            "deontology",
            "--out",
            out_path.to_str().unwrap_or("/tmp/doxa_ac9_deontology.owl"),
        ])
        .status()
        .expect("failed to launch doxa binary");

    assert!(
        status.success(),
        "`doxa build deontology` must exit 0"
    );

    let owl = std::fs::read_to_string(&out_path).expect("deontology.owl must be readable");
    assert!(
        owl.contains("Kant"),
        "deontology OWL must contain 'Kant' from philosophicalGrounding"
    );
    assert!(
        owl.contains("Maxim") || owl.contains("instantiatesMaxim") || owl.contains("violates"),
        "deontology OWL must reference Maxim/instantiatesMaxim or violates"
    );
}

#[test]
fn acceptance_ac9_build_virtue_ethics() {
    let bin = env!("CARGO_BIN_EXE_doxa");
    let out_path = std::env::temp_dir().join("doxa_ac9_virtue_ethics.owl");
    let _ = std::fs::remove_file(&out_path);

    let status = Command::new(bin)
        .args([
            "build",
            "virtue-ethics",
            "--out",
            out_path.to_str().unwrap_or("/tmp/doxa_ac9_virtue_ethics.owl"),
        ])
        .status()
        .expect("failed to launch doxa binary");

    assert!(
        status.success(),
        "`doxa build virtue-ethics` must exit 0"
    );

    let owl = std::fs::read_to_string(&out_path).expect("virtue-ethics.owl must be readable");
    assert!(
        owl.contains("Aristotle"),
        "virtue-ethics OWL must contain 'Aristotle' from philosophicalGrounding"
    );
    assert!(
        owl.contains("Virtue") || owl.contains("expresses"),
        "virtue-ethics OWL must reference Virtue or expresses"
    );
}

#[test]
fn acceptance_ac9_build_all() {
    let bin = env!("CARGO_BIN_EXE_doxa");

    let status = Command::new(bin)
        .args(["build", "--all"])
        .status()
        .expect("failed to launch doxa binary");

    assert!(
        status.success(),
        "`doxa build --all` must exit 0"
    );
}

#[test]
fn acceptance_ac9_consequentialism_and_deontology_differ() {
    let bin = env!("CARGO_BIN_EXE_doxa");
    let cons_path = std::env::temp_dir().join("doxa_ac9_diff_consequentialism.owl");
    let deon_path = std::env::temp_dir().join("doxa_ac9_diff_deontology.owl");

    for (name, path) in &[
        ("consequentialism", &cons_path),
        ("deontology", &deon_path),
    ] {
        let _ = std::fs::remove_file(path);
        let status = Command::new(bin)
            .args(["build", name, "--out", path.to_str().unwrap_or("/tmp")])
            .status()
            .expect("failed to launch doxa binary");
        assert!(status.success(), "`doxa build {name}` must exit 0");
    }

    let owl_cons = std::fs::read_to_string(&cons_path).expect("consequentialism owl must be readable");
    let owl_deon = std::fs::read_to_string(&deon_path).expect("deontology owl must be readable");

    assert_ne!(
        owl_cons, owl_deon,
        "consequentialism and deontology must produce structurally different OWL outputs"
    );
}
