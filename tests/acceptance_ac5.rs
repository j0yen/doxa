//! AC5 (MUST): `RightAction`, `WrongAction`, `PermissibleAction` are declared as subclasses of
//! `Action` but carry **no** equivalence/definition axiom in core.

use std::process::Command;

#[test]
fn acceptance_ac5_no_equivalence_axiom_in_core() {
    let forge_present = Command::new("which")
        .arg("ousia-forge")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if forge_present {
        // Build and check the OWL XML directly.
        let out_path = std::env::temp_dir().join("doxa_ac5_core.owl");
        let bin = env!("CARGO_BIN_EXE_doxa");
        let build_status = Command::new(bin)
            .args([
                "build-core",
                "--out",
                out_path.to_str().unwrap_or("/tmp/doxa_ac5_core.owl"),
                "--spec",
                "spec-core",
            ])
            .status()
            .expect("failed to run doxa build-core");
        assert!(build_status.success(), "doxa build-core must succeed for AC5");

        let owl = std::fs::read_to_string(&out_path).expect("core.owl must be readable");

        // The OWL should declare the three classes but NOT include EquivalentClasses for them.
        for cls in &["RightAction", "WrongAction", "PermissibleAction"] {
            assert!(
                owl.contains(cls),
                "core OWL must declare class {cls} (as subclass of Action)"
            );
        }

        // EquivalentClasses blocks for these three must be absent.
        assert!(
            !owl_has_equivalence_for(&owl, "RightAction"),
            "core OWL must NOT have an EquivalentClasses axiom for RightAction"
        );
        assert!(
            !owl_has_equivalence_for(&owl, "WrongAction"),
            "core OWL must NOT have an EquivalentClasses axiom for WrongAction"
        );
        assert!(
            !owl_has_equivalence_for(&owl, "PermissibleAction"),
            "core OWL must NOT have an EquivalentClasses axiom for PermissibleAction"
        );
    } else {
        // Without ousia-forge, inspect the TOML spec files directly.
        // None of the spec files should contain `equivalent_to` or `equivalent_classes` for the three.
        let spec_content = read_all_spec_toml("spec-core");
        for cls in &["RightAction", "WrongAction", "PermissibleAction"] {
            assert!(
                spec_content.contains(cls),
                "spec-core must declare class {cls}"
            );
            // Ensure no equivalence axiom is present in the spec for these three.
            // A correct spec will have no `equivalent_to` key under these classes.
            assert_no_equivalence_in_spec(&spec_content, cls);
        }
    }
}

fn owl_has_equivalence_for(owl: &str, class_local_name: &str) -> bool {
    // OWL/XML encodes EquivalentClasses as <EquivalentClasses> containing the class IRI.
    // Only match on actual XML element open tags, not annotation literal text.
    let mut inside_equiv = false;
    for line in owl.lines() {
        let trimmed = line.trim();
        // Only enter equiv block on the actual XML tag, not on text inside a <Literal>
        if trimmed.starts_with("<EquivalentClasses>") || trimmed == "<EquivalentClasses>" {
            inside_equiv = true;
        }
        if inside_equiv && line.contains(class_local_name) {
            return true;
        }
        if inside_equiv && line.contains("</EquivalentClasses>") {
            inside_equiv = false;
        }
    }
    false
}

fn read_all_spec_toml(spec_dir: &str) -> String {
    let dir = std::path::Path::new(spec_dir);
    let mut combined = String::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("toml") {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    combined.push_str(&content);
                }
            }
        }
    }
    combined
}

fn assert_no_equivalence_in_spec(spec_content: &str, class_name: &str) {
    // Search for any `equivalent_to` key that appears after the class iri = "X" entry.
    let mut found_class = false;
    for line in spec_content.lines() {
        if line.contains(&format!(r#"iri = "{class_name}""#)) {
            found_class = true;
        }
        if found_class {
            if line.trim_start().starts_with("iri =") && !line.contains(class_name) {
                found_class = false;
            }
            assert!(
                !line.contains("equivalent_to") && !line.contains("equivalent_classes"),
                "spec-core must NOT declare an equivalence for {class_name}; found: {line}"
            );
        }
    }
}
