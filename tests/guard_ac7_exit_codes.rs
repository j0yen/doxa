//! AC7: Exit codes map allow/flag/deny → 0/10/20.
//! AC8: Unknown policy string → actionable error listing valid policies, non-zero exit.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn bin() -> String {
    env!("CARGO_BIN_EXE_doxa").to_string()
}

fn write_allow_scenario() -> PathBuf {
    let tmp = std::env::temp_dir().join(format!("doxa_allow_{}.ttl", std::process::id()));
    fs::write(
        &tmp,
        "# doxa-verdict: consequentialism=permissible\n\
         # doxa-verdict: deontology=permissible\n\
         # doxa-verdict: virtue-ethics=permissible\n\
         # doxa-verdict: contractualism=permissible\n",
    )
    .expect("write allow scenario");
    tmp
}

fn write_deny_scenario() -> PathBuf {
    let tmp = std::env::temp_dir().join(format!("doxa_deny_{}.ttl", std::process::id()));
    fs::write(
        &tmp,
        "# doxa-verdict: consequentialism=wrong\n\
         # doxa-verdict: deontology=wrong\n\
         # doxa-verdict: virtue-ethics=wrong\n\
         # doxa-verdict: contractualism=wrong\n",
    )
    .expect("write deny scenario");
    tmp
}

fn write_flag_scenario() -> PathBuf {
    let tmp = std::env::temp_dir().join(format!("doxa_flag_{}.ttl", std::process::id()));
    fs::write(
        &tmp,
        "# doxa-verdict: consequentialism=permissible\n\
         # doxa-verdict: deontology=wrong\n",
    )
    .expect("write flag scenario");
    tmp
}

#[test]
fn exit_code_allow_is_0() {
    let scenario = write_allow_scenario();
    let output = Command::new(bin())
        .args([
            "guard",
            "--scenario",
            scenario.to_str().unwrap(),
            "--policy",
            "unanimity",
        ])
        .output()
        .expect("run doxa guard");
    let _ = fs::remove_file(&scenario);
    let code = output.status.code().unwrap_or(-1);
    assert_eq!(code, 0, "all-permissible unanimity must exit 0; got {code}");
}

#[test]
fn exit_code_deny_is_20() {
    let scenario = write_deny_scenario();
    let output = Command::new(bin())
        .args([
            "guard",
            "--scenario",
            scenario.to_str().unwrap(),
            "--policy",
            "unanimity",
        ])
        .output()
        .expect("run doxa guard");
    let _ = fs::remove_file(&scenario);
    let code = output.status.code().unwrap_or(-1);
    assert_eq!(code, 20, "all-wrong unanimity must exit 20; got {code}");
}

#[test]
fn exit_code_flag_is_10() {
    let scenario = write_flag_scenario();
    let output = Command::new(bin())
        .args([
            "guard",
            "--scenario",
            scenario.to_str().unwrap(),
            "--policy",
            "majority",
            "--frameworks",
            "consequentialism,deontology",
        ])
        .output()
        .expect("run doxa guard");
    let _ = fs::remove_file(&scenario);
    let code = output.status.code().unwrap_or(-1);
    // Tie between allow and deny → flag → exit 10
    assert_eq!(code, 10, "tied majority must exit 10; got {code}");
}

#[test]
fn unknown_policy_gives_actionable_error() {
    let output = Command::new(bin())
        .args([
            "guard",
            "--scenario",
            "scenarios/trolley.ttl",
            "--policy",
            "notarealPolicy",
        ])
        .output()
        .expect("run doxa guard");

    let stderr = String::from_utf8_lossy(&output.stderr);
    let code = output.status.code().unwrap_or(0);

    // AC8: non-zero exit
    assert_ne!(code, 0, "unknown policy must exit non-zero; got {code}");

    // AC8: actionable error listing valid policies
    assert!(
        stderr.contains("unanimity") || stderr.contains("majority"),
        "error must list valid policies; got:\n{stderr}"
    );
    assert!(
        !stderr.is_empty(),
        "error message must be non-empty"
    );
}
