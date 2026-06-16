//! AC6: Every output names the policy applied and lists per-framework verdicts —
//! no silent aggregation. Tests that breakdown is always present regardless of policy.

use std::process::Command;

fn bin() -> String {
    env!("CARGO_BIN_EXE_doxa").to_string()
}

fn run_guard(policy: &str) -> String {
    let output = Command::new(bin())
        .args([
            "guard",
            "--scenario",
            "scenarios/trolley.ttl",
            "--policy",
            policy,
        ])
        .output()
        .unwrap_or_else(|_| panic!("failed to run doxa guard --policy {policy}"));
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn breakdown_present_unanimity() {
    let out = run_guard("unanimity");
    assert!(out.contains("breakdown:"), "breakdown missing for unanimity:\n{out}");
    assert!(out.contains("policy:"), "policy label missing:\n{out}");
    assert!(out.contains("unanimity"), "policy name missing:\n{out}");
}

#[test]
fn breakdown_present_majority() {
    let out = run_guard("majority");
    assert!(out.contains("breakdown:"), "breakdown missing for majority:\n{out}");
    assert!(out.contains("policy:"), "policy label missing:\n{out}");
    assert!(out.contains("majority"), "policy name missing:\n{out}");
}

#[test]
fn breakdown_present_framework() {
    let out = run_guard("framework:deontology");
    assert!(out.contains("breakdown:"), "breakdown missing for framework:deontology:\n{out}");
    assert!(out.contains("framework:deontology"), "policy name missing:\n{out}");
}

#[test]
fn breakdown_present_lexical() {
    let out = run_guard("lexical:consequentialism,deontology");
    assert!(out.contains("breakdown:"), "breakdown missing for lexical:\n{out}");
    assert!(out.contains("lexical:consequentialism,deontology"), "policy name missing:\n{out}");
}
