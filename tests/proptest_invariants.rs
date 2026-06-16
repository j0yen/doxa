//! Property-based invariants for `doxa`.
//!
//! READ-ONLY after scaffold. Edit-agent must not modify this file.

use proptest::prelude::*;

proptest! {
    /// The spec-core TOML file count is stable (at least 5 domain files + ontology.toml).
    #[test]
    fn prop_spec_core_has_expected_files(_seed in 0u64..1000u64) {
        let dir = std::path::Path::new("spec-core");
        let count = std::fs::read_dir(dir)
            .expect("spec-core/ must exist")
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("toml"))
            .count();
        prop_assert!(count >= 6, "spec-core must have ≥6 TOML files; found {count}");
    }
}
