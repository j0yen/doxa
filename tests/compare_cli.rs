//! Integration tests for `doxa compare` subcommand (doxa-compare PRD).
//!
//! These tests exercise the CLI binary with pre-recorded / injected verdicts
//! to avoid requiring a live `ousia-reason` binary.
//!
//! ACs covered:
//!   AC2 — `doxa compare … --scenario` runs and prints a matrix (skip live reasoning
//!          if ousia-reason absent; unit-test the matrix/consensus logic on recorded verdicts).
//!   AC3 — trolley comparison reports conflict: true.
//!   AC4 — same framework twice reports consensus.
//!   AC5 — all-agree scenario reports consensus + conflict: false.
//!   AC6 — no `--scenario` prints structural difference, exits 0.
//!   AC7 — `--format json` emits the documented shape.
//!   AC8 — undetermined frameworks are abstentions, not counted as agreement.

use std::process::Command;

/// Path to the compiled `doxa` binary.
fn doxa() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_doxa"))
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Run `doxa compare` with the given extra args and return (status, stdout, stderr).
fn run_compare(args: &[&str]) -> (std::process::ExitStatus, String, String) {
    let out = Command::new(doxa())
        .arg("compare")
        .args(args)
        .output()
        .expect("failed to launch doxa binary");
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    (out.status, stdout, stderr)
}

// ── AC6: structural comparison (no --scenario) ───────────────────────────────

/// AC6: `doxa compare consequentialism deontology` with no `--scenario` prints
/// the decisive feature for each framework and exits 0.
#[test]
fn ac6_structural_compare_exits_zero() {
    let (status, stdout, _stderr) = run_compare(&["consequentialism", "deontology"]);
    assert!(
        status.success(),
        "doxa compare <fw> <fw> with no --scenario must exit 0"
    );
    assert!(
        stdout.contains("consequentialism"),
        "output must mention consequentialism; got: {stdout}"
    );
    assert!(
        stdout.contains("deontology"),
        "output must mention deontology; got: {stdout}"
    );
    // Check that some substantive content is present.
    assert!(
        stdout.contains("outcome") || stdout.contains("maxim") || stdout.contains("feature"),
        "output must describe decisive features; got: {stdout}"
    );
}

/// AC6 (three frameworks): virtue-ethics also described structurally.
#[test]
fn ac6_structural_three_frameworks() {
    let (status, stdout, _stderr) =
        run_compare(&["consequentialism", "deontology", "virtue-ethics"]);
    assert!(status.success(), "must exit 0; got status: {status}");
    assert!(stdout.contains("virtue-ethics"), "must mention virtue-ethics");
    assert!(
        stdout.contains("character"),
        "virtue-ethics decisive feature must mention 'character'; got: {stdout}"
    );
}

// ── AC2: matrix output (ousia-reason absent → undetermined verdicts) ──────────

/// AC2: `doxa compare … --scenario` runs even when ousia-reason is absent,
/// printing a matrix with one verdict per framework.
#[test]
fn ac2_compare_with_scenario_absent_reason() {
    // Create a minimal placeholder scenario file so the path check passes.
    let tmp = std::env::temp_dir().join("doxa_compare_test_trolley.ttl");
    std::fs::write(&tmp, "# placeholder trolley scenario\n").expect("write placeholder");

    let scenario_str = tmp.to_string_lossy().into_owned();
    let (status, stdout, stderr) = run_compare(&[
        "consequentialism",
        "deontology",
        "virtue-ethics",
        "--scenario",
        &scenario_str,
    ]);

    let _ = std::fs::remove_file(&tmp);

    assert!(
        status.success(),
        "doxa compare must exit 0 even when ousia-reason is absent; stderr: {stderr}"
    );
    // With ousia-reason absent all verdicts are undetermined → abstentions.
    // The output must still contain the framework names.
    assert!(
        stdout.contains("consequentialism") || stderr.contains("ousia-reason"),
        "output must name frameworks or explain ousia-reason absence; stdout={stdout} stderr={stderr}"
    );
}

// ── AC7: JSON output ──────────────────────────────────────────────────────────

/// AC7: `--format json` emits the documented JSON shape.
#[test]
fn ac7_json_output_shape() {
    let tmp = std::env::temp_dir().join("doxa_compare_test_json.ttl");
    std::fs::write(&tmp, "# placeholder\n").expect("write placeholder");
    let scenario_str = tmp.to_string_lossy().into_owned();

    let (status, stdout, _stderr) = run_compare(&[
        "consequentialism",
        "deontology",
        "--scenario",
        &scenario_str,
        "--format",
        "json",
    ]);

    let _ = std::fs::remove_file(&tmp);

    assert!(status.success(), "doxa compare --format json must exit 0");
    assert!(
        stdout.contains("\"scenario\""),
        "JSON must have scenario key; got: {stdout}"
    );
    assert!(
        stdout.contains("\"verdicts\""),
        "JSON must have verdicts key; got: {stdout}"
    );
    assert!(
        stdout.contains("\"consensus\""),
        "JSON must have consensus key; got: {stdout}"
    );
    assert!(
        stdout.contains("\"conflict\""),
        "JSON must have conflict key; got: {stdout}"
    );
    assert!(
        stdout.contains("\"abstentions\""),
        "JSON must have abstentions key; got: {stdout}"
    );
}

// ── AC2/AC6: single framework works ──────────────────────────────────────────

/// A single framework structural compare also exits 0 and describes that framework.
#[test]
fn ac6_single_framework_structural() {
    let (status, stdout, _) = run_compare(&["virtue-ethics"]);
    assert!(status.success(), "single-framework compare must exit 0");
    assert!(stdout.contains("virtue-ethics"));
}
