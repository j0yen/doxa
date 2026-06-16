//! AC2: `doxa guard --scenario scenarios/trolley.ttl --policy unanimity`
//! returns `flag` or `deny` (frameworks conflict on trolley) and shows per-framework split.
//!
//! Also exercises AC6 (breakdown always present) and AC7 (exit codes).

use std::process::Command;

fn bin() -> String {
    env!("CARGO_BIN_EXE_doxa").to_string()
}

fn trolley_path() -> String {
    // Run from repo root by cargo test
    "scenarios/trolley.ttl".to_string()
}

#[test]
fn guard_unanimity_trolley_not_allow() {
    let output = Command::new(bin())
        .args([
            "guard",
            "--scenario",
            &trolley_path(),
            "--policy",
            "unanimity",
        ])
        .output()
        .expect("failed to run doxa guard");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // AC2: must not be allow (trolley has conflicting verdicts)
    assert!(
        !stdout.contains("verdict:  allow"),
        "unanimity on trolley must not be allow; got:\n{stdout}\n{stderr}"
    );
    // Must be flag or deny
    let is_flag_or_deny =
        stdout.contains("verdict:  flag") || stdout.contains("verdict:  deny");
    assert!(
        is_flag_or_deny,
        "unanimity on trolley must be flag or deny; got:\n{stdout}\n{stderr}"
    );

    // AC6: breakdown always present
    assert!(
        stdout.contains("breakdown:"),
        "breakdown section must be present; got:\n{stdout}"
    );
    // Must list per-framework verdicts
    assert!(
        stdout.contains("consequentialism") || stdout.contains("deontology"),
        "per-framework verdicts must be shown; got:\n{stdout}"
    );

    // AC7: exit code must NOT be 0 (flag=10, deny=20)
    let code = output.status.code().unwrap_or(-1);
    assert!(
        code == 10 || code == 20,
        "exit code must be 10 (flag) or 20 (deny) for non-allow; got {code}"
    );
}
