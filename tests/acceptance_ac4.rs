//! AC4 (MUST): The built core ontology contains ≥12 moral-domain classes and ≥8 object properties.

use std::process::Command;

/// Count classes and object properties in the built OWL via `ousia-forge stats`.
#[test]
fn acceptance_ac4_class_and_property_counts() {
    let forge_present = Command::new("which")
        .arg("ousia-forge")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !forge_present {
        // Without ousia-forge, verify the spec-core TOML files directly.
        // Count [[classes]] entries across all toml files in spec-core/.
        let class_count = count_toml_classes("spec-core");
        let prop_count = count_toml_object_properties("spec-core/ontology.toml");
        assert!(
            class_count >= 12,
            "spec-core must declare ≥12 moral-domain classes; found {class_count}"
        );
        assert!(
            prop_count >= 8,
            "spec-core must declare ≥8 object properties; found {prop_count}"
        );
        return;
    }

    // Build first, then stats.
    let out_path = std::env::temp_dir().join("doxa_ac4_core.owl");
    let bin = env!("CARGO_BIN_EXE_doxa");
    let build_status = Command::new(bin)
        .args([
            "build-core",
            "--out",
            out_path.to_str().unwrap_or("/tmp/doxa_ac4_core.owl"),
            "--spec",
            "spec-core",
        ])
        .status()
        .expect("failed to run doxa build-core");
    assert!(build_status.success(), "doxa build-core must succeed for AC4");

    let stats_output = Command::new("ousia-forge")
        .args([
            "stats",
            "--out",
            out_path.to_str().unwrap_or("/tmp/doxa_ac4_core.owl"),
        ])
        .output()
        .expect("failed to run ousia-forge stats");

    let stdout = String::from_utf8_lossy(&stats_output.stdout);
    // Parse "Classes: N" and "Object properties: N" lines.
    let class_count = parse_stat_line(&stdout, "Classes");
    let prop_count = parse_stat_line(&stdout, "Object properties");
    assert!(
        class_count >= 12,
        "core OWL must contain ≥12 classes; ousia-forge stats reported {class_count}\nstats output:\n{stdout}"
    );
    assert!(
        prop_count >= 8,
        "core OWL must contain ≥8 object properties; ousia-forge stats reported {prop_count}\nstats output:\n{stdout}"
    );
}

fn parse_stat_line(text: &str, label: &str) -> usize {
    for line in text.lines() {
        if line.trim_start().starts_with(label) {
            if let Some(n_str) = line.split(':').nth(1) {
                if let Ok(n) = n_str.trim().parse::<usize>() {
                    return n;
                }
            }
        }
    }
    0
}

fn count_toml_classes(spec_dir: &str) -> usize {
    let dir = std::path::Path::new(spec_dir);
    let mut count = 0usize;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("toml") {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    count += content.matches("[[classes]]").count();
                }
            }
        }
    }
    count
}

fn count_toml_object_properties(toml_path: &str) -> usize {
    if let Ok(content) = std::fs::read_to_string(toml_path) {
        return content.matches("[[object_properties]]").count();
    }
    0
}
