//! AC2 + AC3: `doxa reason <framework> --scenario scenarios/trolley.ttl` exits 0 and
//! prints a verdict for the action individual.
//!
//! When `ousia-reason` is absent, the binary must still exit 0 and print `undetermined`
//! (no fabricated verdict). When present, it must print a valid verdict token.

use std::process::Command;

fn doxa_bin() -> &'static str {
    env!("CARGO_BIN_EXE_doxa")
}

/// Check whether ousia-reason is on PATH.
fn ousia_reason_present() -> bool {
    Command::new("which")
        .arg("ousia-reason")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Helper: run `doxa reason <framework> --scenario scenarios/trolley.ttl`
/// and return (exit_code, stdout, stderr).
fn run_reason(framework: &str) -> (bool, String, String) {
    let output = Command::new(doxa_bin())
        .args([
            "reason",
            framework,
            "--scenario",
            "scenarios/trolley.ttl",
        ])
        .output()
        .expect("failed to launch doxa binary");
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// AC2: `doxa reason consequentialism --scenario scenarios/trolley.ttl` exits 0
/// and prints one of the four valid verdict tokens.
#[test]
fn ac2_reason_consequentialism_exits_0() {
    let (ok, stdout, stderr) = run_reason("consequentialism");
    assert!(
        ok,
        "doxa reason consequentialism must exit 0; stderr: {stderr}"
    );
    let valid_verdicts = [
        "RightAction",
        "WrongAction",
        "PermissibleAction",
        "undetermined",
    ];
    let has_verdict = valid_verdicts.iter().any(|v| stdout.contains(v));
    assert!(
        has_verdict,
        "stdout must contain a valid verdict token; got:\n{stdout}"
    );
}

/// AC3: `doxa reason deontology --scenario scenarios/trolley.ttl` exits 0
/// and prints a valid verdict.
#[test]
fn ac3_reason_deontology_exits_0() {
    let (ok, stdout, stderr) = run_reason("deontology");
    assert!(
        ok,
        "doxa reason deontology must exit 0; stderr: {stderr}"
    );
    let valid_verdicts = [
        "RightAction",
        "WrongAction",
        "PermissibleAction",
        "undetermined",
    ];
    let has_verdict = valid_verdicts.iter().any(|v| stdout.contains(v));
    assert!(
        has_verdict,
        "stdout must contain a valid verdict token; got:\n{stdout}"
    );
}

/// AC4 (divergence): when ousia-reason is present, consequentialism and deontology
/// must produce different verdicts on the trolley scenario.
#[test]
fn ac4_frameworks_diverge_on_trolley_when_reasoner_present() {
    if !ousia_reason_present() {
        eprintln!("AC4: ousia-reason absent — skipping live divergence check");
        return;
    }
    let (_, cons_out, _) = run_reason("consequentialism");
    let (_, deon_out, _) = run_reason("deontology");

    // Extract verdict token from output line like:
    // "consequentialism: <...> is RightAction"
    let cons_verdict = extract_verdict(&cons_out);
    let deon_verdict = extract_verdict(&deon_out);

    assert!(
        cons_verdict.is_some() && deon_verdict.is_some(),
        "both frameworks must produce a verdict when ousia-reason is present; \
         cons={cons_out:?} deon={deon_out:?}"
    );
    assert_ne!(
        cons_verdict, deon_verdict,
        "consequentialism ({cons_verdict:?}) and deontology ({deon_verdict:?}) \
         must diverge on the trolley problem"
    );
}

/// AC6: undetermined is a first-class non-error result.
/// We test this by pointing at an ABox that has no evaluative class assertions
/// (a scenario the framework can't decide).
#[test]
fn ac6_undetermined_is_not_an_error() {
    // Write a minimal valid but verdict-less scenario to a temp file.
    let tmp = std::env::temp_dir().join("doxa_ac6_undecided.ttl");
    let abox = r#"
@prefix rdf:  <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .
@prefix owl:  <http://www.w3.org/2002/07/owl#> .
@prefix doxa: <https://w3id.org/doxa#> .
@prefix sc:   <https://w3id.org/doxa/scenario/undecided#> .

sc:action1
    rdf:type owl:NamedIndividual ;
    rdf:type doxa:Action .
"#;
    std::fs::write(&tmp, abox).expect("failed to write undecided scenario");

    let output = Command::new(doxa_bin())
        .args([
            "reason",
            "consequentialism",
            "--scenario",
            tmp.to_str().unwrap_or("/tmp/doxa_ac6_undecided.ttl"),
            "--action",
            "https://w3id.org/doxa/scenario/undecided#action1",
        ])
        .output()
        .expect("failed to launch doxa binary");

    assert!(
        output.status.success(),
        "doxa reason must exit 0 even when verdict is undetermined; \
         stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// AC7: malformed scenario ABox → actionable error, non-zero exit.
#[test]
fn ac7_malformed_abox_nonzero_exit() {
    let tmp = std::env::temp_dir().join("doxa_ac7_malformed.ttl");
    // Truncated IRI — unbalanced angle bracket
    let bad = "<https://example.com/a rdf:type doxa:Action .";
    std::fs::write(&tmp, bad).expect("failed to write malformed ttl");

    let output = Command::new(doxa_bin())
        .args([
            "reason",
            "consequentialism",
            "--scenario",
            tmp.to_str().unwrap_or("/tmp/doxa_ac7_malformed.ttl"),
        ])
        .output()
        .expect("failed to launch doxa binary");

    assert!(
        !output.status.success(),
        "doxa reason must exit non-zero for a malformed ABox"
    );
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(
        stderr.contains("malformed") || stderr.contains("error") || stderr.contains("unbalanced"),
        "stderr must contain an actionable error message; got: {stderr}"
    );
}

/// Extract the verdict token from a `doxa reason` output line.
fn extract_verdict(output: &str) -> Option<String> {
    for token in &["RightAction", "WrongAction", "PermissibleAction", "undetermined"] {
        if output.contains(token) {
            return Some((*token).to_owned());
        }
    }
    None
}
