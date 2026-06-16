//! `doxa compare` — fan-out N frameworks on one scenario and emit an
//! agreement / conflict matrix.
//!
//! The module is intentionally decoupled from the live reasoning path: it
//! accepts pre-computed verdicts so the matrix / consensus logic can be
//! unit-tested without a real `ousia-reason` binary.

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};

// ── Verdict ──────────────────────────────────────────────────────────────────

/// The moral verdict returned by a framework for a given scenario.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Verdict {
    /// The action is obligatory (right).
    RightAction,
    /// The action is impermissible (wrong).
    WrongAction,
    /// The action is neither obligatory nor forbidden.
    PermissibleAction,
    /// The framework cannot determine a verdict for this scenario.
    Undetermined,
}

impl Verdict {
    /// Parse from the string token emitted by `ousia-reason`.
    #[must_use]
    pub fn from_str_token(s: &str) -> Self {
        match s.trim() {
            "RightAction" => Self::RightAction,
            "WrongAction" => Self::WrongAction,
            "PermissibleAction" => Self::PermissibleAction,
            _ => Self::Undetermined,
        }
    }

    /// Returns `true` if the framework reached a decision.
    #[must_use]
    pub const fn is_decided(&self) -> bool {
        !matches!(self, Self::Undetermined)
    }

    /// Short label for display.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::RightAction => "RightAction",
            Self::WrongAction => "WrongAction",
            Self::PermissibleAction => "PermissibleAction",
            Self::Undetermined => "undetermined",
        }
    }
}

impl fmt::Display for Verdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

// ── Structural feature descriptions (no-scenario mode) ───────────────────────

/// Returns the morally decisive feature each framework considers.
///
/// These descriptions are baked in as domain knowledge; they are not derived
/// from runtime reasoning.
fn decisive_feature(framework: &str) -> Option<&'static str> {
    match framework {
        "consequentialism" => Some("outcome: maximises aggregate welfare / minimises harm"),
        "deontology" => Some("maxim: conforms to universalisable duty / respects persons as ends"),
        "virtue-ethics" => {
            Some("character: expresses the virtues a person of practical wisdom would show")
        }
        "contractualism" => {
            Some("agreement: could not be reasonably rejected by any affected party")
        }
        "care-ethics" => {
            Some("relationship: sustains webs of care and particular responsibilities")
        }
        _ => None,
    }
}

// ── CompareResult ─────────────────────────────────────────────────────────────

/// The full output of a comparison run.
#[derive(Debug)]
pub struct CompareResult {
    /// Scenario path (if any).
    pub scenario: Option<String>,
    /// Per-framework verdict (ordered by insertion).
    pub verdicts: BTreeMap<String, Verdict>,
    /// The shared verdict if all *decided* frameworks agree; `None` on conflict or
    /// if every framework abstained.
    pub consensus: Option<Verdict>,
    /// `true` when decided frameworks disagree.
    pub conflict: bool,
    /// Frameworks that returned `Undetermined`.
    pub abstentions: Vec<String>,
}

impl CompareResult {
    /// Build a `CompareResult` from a map of `framework → verdict`.
    #[must_use]
    pub fn from_verdicts(
        scenario: Option<String>,
        verdicts: BTreeMap<String, Verdict>,
    ) -> Self {
        let decided: Vec<(&String, &Verdict)> = verdicts
            .iter()
            .filter(|(_, v)| v.is_decided())
            .collect();

        let abstentions: Vec<String> = verdicts
            .iter()
            .filter(|(_, v)| !v.is_decided())
            .map(|(k, _)| k.clone())
            .collect();

        // Consensus: all decided agree on the same verdict.
        let consensus = if decided.is_empty() {
            None
        } else {
            // Safe: we just checked `!decided.is_empty()`.
            let first = decided.first().map(|(_, v)| *v);
            if decided.iter().all(|(_, v)| Some(*v) == first) {
                first.cloned()
            } else {
                None
            }
        };

        let conflict = consensus.is_none() && !decided.is_empty();

        Self {
            scenario,
            verdicts,
            consensus,
            conflict,
            abstentions,
        }
    }
}

// ── JSON serialisation (manual — no serde dep) ────────────────────────────────

fn escape_json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Emit the result as a JSON object (documented shape).
#[must_use]
pub fn to_json(r: &CompareResult) -> String {
    let scenario_field = r
        .scenario
        .as_ref()
        .map_or_else(|| "null".to_owned(), |s| escape_json_str(s));

    let verdicts_inner: String = r
        .verdicts
        .iter()
        .map(|(k, v)| format!("{}:{}", escape_json_str(k), escape_json_str(v.label())))
        .collect::<Vec<_>>()
        .join(",");

    let consensus_field = r
        .consensus
        .as_ref()
        .map_or_else(|| "null".to_owned(), |v| escape_json_str(v.label()));

    let conflict_field = if r.conflict { "true" } else { "false" };

    let abstentions_inner: String = r
        .abstentions
        .iter()
        .map(|s| escape_json_str(s))
        .collect::<Vec<_>>()
        .join(",");

    format!(
        r#"{{"scenario":{scenario_field},"verdicts":{{{verdicts_inner}}},"consensus":{consensus_field},"conflict":{conflict_field},"abstentions":[{abstentions_inner}]}}"#
    )
}

// ── Text output ───────────────────────────────────────────────────────────────

/// Emit a human-readable matrix + summary line.
#[must_use]
pub fn to_text(r: &CompareResult) -> String {
    let mut out = String::new();

    // Matrix header
    out.push_str("framework            verdict\n");
    out.push_str("─────────────────── ────────────────────\n");
    for (fw, v) in &r.verdicts {
        out.push_str(&format!("{:<20} {}\n", fw, v.label()));
    }
    out.push('\n');

    // Summary
    if r.conflict {
        // Group by verdict
        let mut groups: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for (fw, v) in &r.verdicts {
            if v.is_decided() {
                groups.entry(v.label()).or_default().push(fw.as_str());
            }
        }
        let summary: Vec<String> = groups
            .iter()
            .map(|(vl, fws)| format!("{} {}", fws.len(), vl))
            .collect();
        out.push_str(&format!("conflict: {}\n", summary.join(" / ")));
    } else if let Some(ref v) = r.consensus {
        let decided_count = r.verdicts.values().filter(|x| x.is_decided()).count();
        let total = r.verdicts.len();
        out.push_str(&format!(
            "consensus: {} ({}/{})\n",
            v.label(),
            decided_count,
            total
        ));
    } else {
        out.push_str("result: all frameworks abstained\n");
    }

    if !r.abstentions.is_empty() {
        out.push_str(&format!("abstentions: {}\n", r.abstentions.join(", ")));
    }

    out
}

// ── Structural comparison (no-scenario mode) ──────────────────────────────────

/// Emit a structural comparison: decisive feature per framework.
#[must_use]
pub fn structural_compare(frameworks: &[String]) -> String {
    let mut out = String::new();
    out.push_str("framework            decisive feature\n");
    out.push_str(
        "─────────────────── ────────────────────────────────────────────────────\n",
    );
    for fw in frameworks {
        let feature = decisive_feature(fw.as_str())
            .unwrap_or("(no structural description available for this framework)");
        out.push_str(&format!("{fw:<20} {feature}\n"));
    }
    out
}

// ── Live reasoning via ousia-reason ──────────────────────────────────────────

/// Invoke `ousia-reason` to get the verdict for one framework on one scenario.
///
/// Returns `Undetermined` when `ousia-reason` is absent (graceful degradation).
///
/// # Errors
///
/// Returns an error only if `ousia-reason` was found but could not be executed.
pub fn reason_one(
    ousia_reason: Option<&Path>,
    framework: &str,
    scenario: &Path,
) -> Result<Verdict> {
    let bin = if let Some(p) = ousia_reason {
        p.to_path_buf()
    } else {
        // Try to find ousia-reason on PATH.
        let which_result = Command::new("which").arg("ousia-reason").output().ok();
        let path_opt = which_result.and_then(|o| {
            if o.status.success() {
                let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                if s.is_empty() { None } else { Some(s) }
            } else {
                None
            }
        });
        if let Some(path) = path_opt {
            std::path::PathBuf::from(path)
        } else {
            eprintln!("note: ousia-reason not on PATH; verdict = undetermined");
            return Ok(Verdict::Undetermined);
        }
    };

    let output = Command::new(&bin)
        .args(["reason", "--framework", framework, "--scenario"])
        .arg(scenario)
        .output()
        .with_context(|| format!("failed to run ousia-reason for framework {framework}"))?;

    if !output.status.success() {
        return Ok(Verdict::Undetermined);
    }

    // Parse last non-empty line for a verdict token.
    let stdout = String::from_utf8_lossy(&output.stdout);
    let verdict = stdout
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .map_or(Verdict::Undetermined, Verdict::from_str_token);

    Ok(verdict)
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_verdicts(pairs: &[(&str, Verdict)]) -> BTreeMap<String, Verdict> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), v.clone()))
            .collect()
    }

    // AC3: trolley-like scenario → conflict when frameworks disagree
    #[test]
    fn test_conflict_detected() {
        let verdicts = make_verdicts(&[
            ("consequentialism", Verdict::RightAction),
            ("deontology", Verdict::WrongAction),
            ("virtue-ethics", Verdict::WrongAction),
        ]);
        let r = CompareResult::from_verdicts(Some("trolley.ttl".into()), verdicts);
        assert!(r.conflict, "diverging frameworks must produce conflict=true");
        assert!(r.consensus.is_none(), "conflict implies no consensus");
    }

    // AC4: same framework twice → consensus (degenerate agreement)
    #[test]
    fn test_same_framework_twice_consensus() {
        let verdicts = make_verdicts(&[
            ("consequentialism", Verdict::WrongAction),
            ("consequentialism-alt", Verdict::WrongAction),
        ]);
        let r = CompareResult::from_verdicts(Some("s.ttl".into()), verdicts);
        assert!(!r.conflict, "identical verdicts must not conflict");
        assert_eq!(
            r.consensus,
            Some(Verdict::WrongAction),
            "identical verdicts must produce consensus"
        );
    }

    // AC5: all three agree → consensus
    #[test]
    fn test_all_agree_consensus() {
        let verdicts = make_verdicts(&[
            ("consequentialism", Verdict::WrongAction),
            ("deontology", Verdict::WrongAction),
            ("virtue-ethics", Verdict::WrongAction),
        ]);
        let r = CompareResult::from_verdicts(Some("agree.ttl".into()), verdicts);
        assert!(!r.conflict, "unanimous frameworks must not conflict");
        assert_eq!(r.consensus, Some(Verdict::WrongAction));
    }

    // AC7: JSON round-trip
    #[test]
    fn test_json_roundtrip_shape() {
        let verdicts = make_verdicts(&[
            ("consequentialism", Verdict::WrongAction),
            ("deontology", Verdict::RightAction),
        ]);
        let r = CompareResult::from_verdicts(Some("t.ttl".into()), verdicts);
        let json = to_json(&r);

        // Must contain the expected keys.
        assert!(json.contains("\"scenario\""), "JSON must have scenario key");
        assert!(json.contains("\"verdicts\""), "JSON must have verdicts key");
        assert!(json.contains("\"consensus\""), "JSON must have consensus key");
        assert!(json.contains("\"conflict\""), "JSON must have conflict key");
        assert!(
            json.contains("\"abstentions\""),
            "JSON must have abstentions key"
        );

        // The conflict case.
        assert!(json.contains("true"), "conflict flag must be true");

        // Verdict values present.
        assert!(json.contains("WrongAction"));
        assert!(json.contains("RightAction"));
    }

    // AC8: undetermined frameworks are abstentions, not counted as agreement.
    #[test]
    fn test_undetermined_is_abstention() {
        let verdicts = make_verdicts(&[
            ("consequentialism", Verdict::WrongAction),
            ("deontology", Verdict::WrongAction),
            ("virtue-ethics", Verdict::Undetermined),
        ]);
        let r = CompareResult::from_verdicts(Some("s.ttl".into()), verdicts);
        assert!(
            r.abstentions.contains(&"virtue-ethics".to_string()),
            "undetermined framework must appear in abstentions"
        );
        // Two decided frameworks agree → consensus even with abstention.
        assert_eq!(r.consensus, Some(Verdict::WrongAction));
        assert!(!r.conflict);
    }

    // Structural compare exits 0 and produces output (AC6).
    #[test]
    fn test_structural_compare_non_empty() {
        let fws: Vec<String> = vec!["consequentialism".into(), "deontology".into()];
        let out = structural_compare(&fws);
        assert!(
            out.contains("consequentialism"),
            "must describe consequentialism"
        );
        assert!(out.contains("deontology"), "must describe deontology");
        assert!(out.contains("outcome"), "consequentialism is outcome-based");
        assert!(out.contains("maxim"), "deontology is maxim-based");
    }

    // Verdict parsing.
    #[test]
    fn test_verdict_parsing() {
        assert_eq!(Verdict::from_str_token("RightAction"), Verdict::RightAction);
        assert_eq!(Verdict::from_str_token("WrongAction"), Verdict::WrongAction);
        assert_eq!(
            Verdict::from_str_token("PermissibleAction"),
            Verdict::PermissibleAction
        );
        assert_eq!(Verdict::from_str_token("other"), Verdict::Undetermined);
        assert_eq!(
            Verdict::from_str_token("  RightAction  "),
            Verdict::RightAction
        );
    }
}
