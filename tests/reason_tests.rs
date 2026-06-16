//! Integration tests for `doxa reason` — fixture-based verdict classification.
//!
//! Tests exercise the offline (recorded fixture) path so they run without
//! a live `ousia-reason` binary. The scenario ABox files use embedded
//! `# doxa-verdict: <framework>=<verdict>` annotations that the reason module
//! reads as a fallback when the reasoner is absent or returns undetermined.

use std::io::Write;
use std::process::Command;

use tempfile::NamedTempFile;

fn bin() -> String {
    env!("CARGO_BIN_EXE_doxa").to_string()
}

/// Minimal Turtle ABox with embedded verdict annotations for all three built-in
/// frameworks. Used across multiple tests.
const TROLLEY_FIXTURE: &str = r#"
@prefix doxa: <https://wintermute.jyen.tech/doxa#> .
@prefix rdfs:  <http://www.w3.org/2000/01/rdf-schema#> .

# doxa-verdict: consequentialism=RightAction
# doxa-verdict: deontology=WrongAction
# doxa-verdict: virtue-ethics=PermissibleAction
# doxa-verdict: virtue-ethics-harm=WrongAction

doxa:PullLever a doxa:Action ; rdfs:label "Pull the lever" .
"#;

/// Write `content` to a fresh `NamedTempFile` and return it.
fn tmp_ttl(content: &str) -> NamedTempFile {
    let mut f = NamedTempFile::new().expect("tempfile");
    write!(f, "{content}").expect("write");
    f
}

/// Run `doxa reason <framework> --scenario <path>` and return stdout.
fn run_reason(framework: &str, scenario_path: &str) -> (String, std::process::ExitStatus) {
    let output = Command::new(bin())
        .args(["reason", framework, "--scenario", scenario_path])
        .output()
        .expect("failed to run doxa reason");
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    (stdout, output.status)
}

// ── 1. consequentialism + has_wellbeing=true, has_harm=false → RightAction ───

#[test]
fn consequentialism_right_action() {
    let f = tmp_ttl(TROLLEY_FIXTURE);
    let path = f.path().to_string_lossy().to_string();
    let (stdout, status) = run_reason("consequentialism", &path);
    assert!(
        status.success(),
        "doxa reason must exit 0 for known framework; got {status:?}\n{stdout}"
    );
    assert!(
        stdout.contains("RightAction"),
        "consequentialism on trolley must yield RightAction; got:\n{stdout}"
    );
}

// ── 2. consequentialism + has_harm=true → WrongAction ────────────────────────

#[test]
fn consequentialism_wrong_action() {
    let content = r#"
@prefix doxa: <https://wintermute.jyen.tech/doxa#> .
# doxa-verdict: consequentialism=WrongAction
doxa:Act a doxa:Action ; rdfs:label "harmful act" .
"#;
    let f = tmp_ttl(content);
    let path = f.path().to_string_lossy().to_string();
    let (stdout, status) = run_reason("consequentialism", &path);
    assert!(status.success(), "must exit 0; stdout:\n{stdout}");
    assert!(
        stdout.contains("WrongAction"),
        "consequentialism with harm annotation must yield WrongAction; got:\n{stdout}"
    );
}

// ── 3. deontology + duty_violated=Some → WrongAction ─────────────────────────

#[test]
fn deontology_wrong_action() {
    let f = tmp_ttl(TROLLEY_FIXTURE);
    let path = f.path().to_string_lossy().to_string();
    let (stdout, status) = run_reason("deontology", &path);
    assert!(status.success(), "must exit 0; stdout:\n{stdout}");
    assert!(
        stdout.contains("WrongAction"),
        "deontology annotation=WrongAction must yield WrongAction; got:\n{stdout}"
    );
}

// ── 4. deontology + maxim=Some, duty_violated=None → RightAction ─────────────

#[test]
fn deontology_right_action() {
    let content = r#"
@prefix doxa: <https://wintermute.jyen.tech/doxa#> .
# doxa-verdict: deontology=RightAction
doxa:Act a doxa:Action ; rdfs:label "dutiful act" .
"#;
    let f = tmp_ttl(content);
    let path = f.path().to_string_lossy().to_string();
    let (stdout, status) = run_reason("deontology", &path);
    assert!(status.success(), "must exit 0; stdout:\n{stdout}");
    assert!(
        stdout.contains("RightAction"),
        "deontology RightAction annotation must be returned; got:\n{stdout}"
    );
}

// ── 5. virtue-ethics + virtue_expressed=Some → RightAction ───────────────────

#[test]
fn virtue_ethics_right_action_via_permissible() {
    let f = tmp_ttl(TROLLEY_FIXTURE);
    let path = f.path().to_string_lossy().to_string();
    let (stdout, status) = run_reason("virtue-ethics", &path);
    assert!(status.success(), "must exit 0; stdout:\n{stdout}");
    // The trolley fixture annotates virtue-ethics=PermissibleAction
    assert!(
        stdout.contains("PermissibleAction") || stdout.contains("RightAction"),
        "virtue-ethics must yield a decided positive verdict; got:\n{stdout}"
    );
}

// ── 6. virtue-ethics + virtue_expressed=None, has_harm=false → Permissible ───

#[test]
fn virtue_ethics_permissible() {
    let content = r#"
@prefix doxa: <https://wintermute.jyen.tech/doxa#> .
# doxa-verdict: virtue-ethics=PermissibleAction
doxa:Act a doxa:Action ; rdfs:label "neutral act" .
"#;
    let f = tmp_ttl(content);
    let path = f.path().to_string_lossy().to_string();
    let (stdout, status) = run_reason("virtue-ethics", &path);
    assert!(status.success(), "must exit 0; stdout:\n{stdout}");
    assert!(
        stdout.contains("PermissibleAction"),
        "virtue-ethics PermissibleAction annotation must be returned; got:\n{stdout}"
    );
}

// ── 7. unknown framework → Undetermined (not an error) ───────────────────────

#[test]
fn unknown_framework_returns_undetermined() {
    let f = tmp_ttl(TROLLEY_FIXTURE);
    let path = f.path().to_string_lossy().to_string();
    let (stdout, status) = run_reason("existentialism", &path);
    // Must exit 0 (undetermined is not an error) and report undetermined
    assert!(
        status.success(),
        "unknown framework must exit 0 (undetermined is first-class); got {status:?}\n{stdout}"
    );
    assert!(
        stdout.contains("undetermined"),
        "unknown framework must produce 'undetermined'; got:\n{stdout}"
    );
}
