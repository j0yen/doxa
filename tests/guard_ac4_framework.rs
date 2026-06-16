//! AC4: `--policy framework:deontology` returns exactly deontology's verdict
//! and output states the policy.

use std::process::Command;

fn bin() -> String {
    env!("CARGO_BIN_EXE_doxa").to_string()
}

#[test]
fn guard_framework_deontology_deny_and_policy_named() {
    let output = Command::new(bin())
        .args([
            "guard",
            "--scenario",
            "scenarios/trolley.ttl",
            "--policy",
            "framework:deontology",
        ])
        .output()
        .expect("failed to run doxa guard");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // deontology says wrong → deny
    assert!(
        stdout.contains("verdict:  deny"),
        "deontology verdict on trolley must be deny; got:\n{stdout}\n{stderr}"
    );

    // Policy must be named in output (AC4 + AC6)
    assert!(
        stdout.contains("framework:deontology"),
        "policy name must appear in output; got:\n{stdout}"
    );

    // AC7: deny exit code = 20
    let code = output.status.code().unwrap_or(-1);
    assert_eq!(
        code, 20,
        "framework:deontology on trolley must exit 20 (deny); got {code}"
    );
}
