//! AC5: `--policy lexical:<priority-order>`
//! - first decided framework in priority list wins
//! - when first framework is undetermined → falls through to next
//!
//! Uses `scenarios/trolley.ttl` where ousia-guard (or the embedded annotations)
//! give:  consequentialism=permissible, deontology=wrong, virtue-ethics=undetermined

use std::process::Command;

fn bin() -> String {
    env!("CARGO_BIN_EXE_doxa").to_string()
}

/// Trolley scenario: consequentialism=permissible is first decided in lexical order.
/// Policy: lexical:consequentialism,deontology → consequentialism wins → allow.
#[test]
fn lexical_consequentialism_decides_when_not_undetermined() {
    let output = Command::new(bin())
        .args([
            "guard",
            "--scenario",
            "scenarios/trolley.ttl",
            "--policy",
            "lexical:consequentialism,deontology",
        ])
        .output()
        .expect("failed to run doxa guard");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Consequentialism is permissible on trolley → allow wins over later deontology=wrong
    assert!(
        stdout.contains("verdict:  allow"),
        "lexical with consequentialism first (permissible) must allow; got:\n{stdout}\n{stderr}"
    );
    let code = output.status.code().unwrap_or(-1);
    assert_eq!(code, 0, "allow exit code must be 0; got {code}");
}

/// Trolley scenario: virtue-ethics is undetermined → falls through to deontology=wrong → deny.
/// Policy: lexical:virtue-ethics,deontology
#[test]
fn lexical_falls_through_to_deontology_when_first_undetermined() {
    let output = Command::new(bin())
        .args([
            "guard",
            "--scenario",
            "scenarios/trolley.ttl",
            "--policy",
            "lexical:virtue-ethics,deontology",
        ])
        .output()
        .expect("failed to run doxa guard");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // virtue-ethics is undetermined → falls through → deontology=wrong → deny
    assert!(
        stdout.contains("verdict:  deny"),
        "lexical fallthrough: virtue-ethics undetermined → deontology deny; got:\n{stdout}\n{stderr}"
    );
    let code = output.status.code().unwrap_or(-1);
    assert_eq!(code, 20, "deny exit code must be 20; got {code}");
}
