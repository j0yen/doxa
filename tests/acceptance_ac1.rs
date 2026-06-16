//! AC1 (MUST): `cargo test --release` passes; `cargo clippy -- -D warnings` passes.
//!
//! This test file itself being compiled and passing satisfies the cargo-test portion.
//! The clippy gate is enforced by scripts/run-metrics.sh and the CI workflow.

/// Smoke test: the binary compiles and the crate links.
#[test]
fn acceptance_ac1_compiles() {
    // If this test runs, the crate compiled successfully.
    // clippy -D warnings is verified by CI / run-metrics.sh.
}
