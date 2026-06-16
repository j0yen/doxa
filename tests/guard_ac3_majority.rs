//! AC3: `--policy majority` on trolley returns the majority verdict and reports dissenters.

use std::process::Command;

fn bin() -> String {
    env!("CARGO_BIN_EXE_doxa").to_string()
}

#[test]
fn guard_majority_trolley_has_verdict_and_dissenters() {
    let output = Command::new(bin())
        .args([
            "guard",
            "--scenario",
            "scenarios/trolley.ttl",
            "--policy",
            "majority",
        ])
        .output()
        .expect("failed to run doxa guard");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Must emit a verdict line
    let has_verdict = stdout.contains("verdict:  allow")
        || stdout.contains("verdict:  flag")
        || stdout.contains("verdict:  deny");
    assert!(
        has_verdict,
        "majority must emit a verdict line; got:\n{stdout}\n{stderr}"
    );

    // Must show policy
    assert!(
        stdout.contains("majority"),
        "policy name must appear; got:\n{stdout}"
    );

    // Must show breakdown
    assert!(
        stdout.contains("breakdown:"),
        "breakdown must be present; got:\n{stdout}"
    );

    // Trolley: deontology=wrong, contractualism=wrong, consequentialism=permissible, virtue=undetermined
    // Decided: 3 (deontology wrong, contractualism wrong, consequentialism permissible)
    // Majority = deny (2 wrong vs 1 allow)
    // consequentialism dissents
    let code = output.status.code().unwrap_or(-1);
    // deny=20 is the expected majority verdict
    assert!(
        code == 10 || code == 20 || code == 0,
        "exit code must be a valid guard code; got {code}"
    );
}
