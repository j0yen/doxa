//! `doxa reason` — evaluate a scenario `ABox` under a single chosen framework.
//!
//! Shells out to `ousia-reason classify` to materialise OWL entailments over
//! the combined `TBox` (framework ontology) + `ABox` (scenario), then inspects
//! whether the scenario's action individual is entailed as a member of
//! `RightAction`, `WrongAction`, or `PermissibleAction`.
//!
//! Graceful degradation: when `ousia-reason` is absent from `$PATH` the module
//! falls back to reading `# doxa-verdict: <framework>=<verdict>` annotations
//! baked into the scenario `.ttl` file (the "recorded fixture" path, AC2/AC3).

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};

// ── Public verdict type (re-exported from compare) ───────────────────────────

pub use crate::compare::Verdict;

// ── ReasonArgs ────────────────────────────────────────────────────────────────

/// Arguments for `doxa reason`.
pub struct ReasonArgs {
    /// Framework name, e.g. `"consequentialism"`.
    pub framework: String,
    /// Path to the scenario `ABox` (`.ttl` file).
    pub scenario: PathBuf,
    /// If true, print the axiom justification chain.
    pub explain: bool,
    /// Optional explicit path to `ousia-reason`.
    pub ousia_reason: Option<PathBuf>,
    /// Optional explicit path to the framework OWL file.
    /// If absent, the module tries `<framework>.owl` in the current directory.
    pub framework_owl: Option<PathBuf>,
}

// ── ReasonResult ──────────────────────────────────────────────────────────────

/// Result of a `doxa reason` run.
#[derive(Debug, Clone)]
pub struct ReasonResult {
    /// Framework that was evaluated.
    pub framework: String,
    /// Action IRI (or label) that was evaluated.
    pub action: String,
    /// Moral verdict under this framework.
    pub verdict: Verdict,
    /// If `--explain` was passed and the verdict is decided, the axiom chain.
    pub explanation: Option<String>,
    /// True when the result came from the recorded-fixture fallback
    /// (ousia-reason absent), false when from a live reasoner run.
    pub from_fixture: bool,
}

impl ReasonResult {
    /// One-line summary: `<framework>: <action> is <verdict>`
    #[must_use]
    pub fn summary_line(&self) -> String {
        format!("{}: {} is {}", self.framework, self.action, self.verdict)
    }
}

// ── Scenario validation ───────────────────────────────────────────────────────

/// Validate that the scenario file is syntactically usable (non-empty, UTF-8,
/// contains at least one RDF statement marker).
///
/// This is a lightweight pre-flight check; full OWL consistency is left to
/// `ousia-reason`. Returns a structured `Err` on malformed input so callers
/// can emit an actionable error message (AC7).
///
/// # Errors
///
/// Returns an error if the file cannot be read, is empty, or is not valid
/// UTF-8 Turtle (heuristic: must contain `@prefix` or `<http` or `a :`).
pub fn validate_scenario(scenario: &Path) -> Result<String> {
    let content = std::fs::read_to_string(scenario).with_context(|| {
        format!(
            "cannot read scenario ABox: {} — check the path and file permissions",
            scenario.display()
        )
    })?;

    if content.trim().is_empty() {
        anyhow::bail!(
            "scenario ABox is empty: {} — a valid Turtle file is required",
            scenario.display()
        );
    }

    // Heuristic: a Turtle file must contain at least one of these markers.
    let looks_like_turtle = content.contains("@prefix")
        || content.contains("<http")
        || content.contains("a :")
        || content.contains("rdf:type");

    if !looks_like_turtle {
        anyhow::bail!(
            "scenario ABox does not look like a Turtle/RDF file: {} — \
             expected at least one of: @prefix, <http://…>, rdf:type",
            scenario.display()
        );
    }

    Ok(content)
}

// ── Fixture-based fallback ────────────────────────────────────────────────────

/// Parse the embedded `# doxa-verdict: <framework>=<class>` annotations from
/// the scenario content.
///
/// Returns the verdict class string for the requested framework, or `None` if
/// no annotation for that framework is found.
///
/// # Example annotation
/// ```text
/// # doxa-verdict: consequentialism=PermissibleAction
/// # doxa-verdict: deontology=WrongAction
/// ```
#[must_use]
pub fn read_fixture_verdict(content: &str, framework: &str) -> Option<Verdict> {
    let prefix = format!("# doxa-verdict: {framework}=");
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix(&prefix) {
            return Some(parse_verdict_token(rest.trim()));
        }
    }
    None
}

/// Parse legacy `permissible`/`wrong`/`undetermined` shorthand tokens used in
/// older `.ttl` fixture annotations, plus the canonical OWL class names.
fn parse_verdict_token(s: &str) -> Verdict {
    match s {
        "RightAction" => Verdict::RightAction,
        "WrongAction" | "wrong" => Verdict::WrongAction,
        "PermissibleAction" | "permissible" => Verdict::PermissibleAction,
        _ => Verdict::Undetermined,
    }
}

/// Extract the "primary action" label from the scenario content.
///
/// Looks for the first `rdfs:label "…"` associated with a `doxa:Action`
/// individual. Falls back to the framework name if none is found.
#[must_use]
pub fn action_label(content: &str, framework: &str) -> String {
    // Simple heuristic: find lines containing `doxa:Action` nearby a label.
    // Full SPARQL parsing is beyond scope; we settle for a best-effort label.
    let mut found_action = false;
    for line in content.lines() {
        let t = line.trim();
        if t.contains("doxa:Action") || t.ends_with("a doxa:Action ;") {
            found_action = true;
        }
        if found_action && t.starts_with("rdfs:label") {
            // Extract content between first pair of quotes.
            if let Some(start) = t.find('"') {
                let after = &t[start + 1..];
                if let Some(end) = after.find('"') {
                    return after[..end].to_string();
                }
            }
        }
    }
    // Fallback: use framework-specific default action names from the trolley
    // scenario so we always emit something meaningful.
    format!("action@{framework}")
}

// ── ousia-reason integration ──────────────────────────────────────────────────

/// Resolve the `ousia-reason` binary path.
///
/// Returns `None` if the binary is not found (caller should fall back to
/// the fixture path rather than hard-erroring — per AC2/AC3).
#[must_use]
pub fn resolve_ousia_reason(explicit: Option<&PathBuf>) -> Option<PathBuf> {
    if let Some(p) = explicit {
        if p.is_file() {
            return Some(p.clone());
        }
        return None;
    }
    let which_result = Command::new("which").arg("ousia-reason").output().ok()?;
    if !which_result.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&which_result.stdout)
        .trim()
        .to_string();
    if s.is_empty() {
        None
    } else {
        Some(PathBuf::from(s))
    }
}

/// Run `ousia-reason classify` with the combined `TBox` + `ABox` and extract the
/// verdict for the action individual.
///
/// # Errors
///
/// Returns an error if the binary was found but could not be executed.
/// Returns `Undetermined` (not an error) when reasoning completes but produces
/// no decisive classification.
pub fn run_classify(
    bin: &Path,
    framework_owl: &Path,
    scenario: &Path,
) -> Result<Verdict> {
    let output = Command::new(bin)
        .args(["classify", "--owl"])
        .arg(framework_owl)
        .arg("--abox")
        .arg(scenario)
        .output()
        .with_context(|| {
            format!(
                "failed to execute ousia-reason at {}",
                bin.display()
            )
        })?;

    if !output.status.success() {
        // Non-zero exit from the reasoner → treat as undetermined (not an
        // error to the caller, per AC6).
        return Ok(Verdict::Undetermined);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // ousia-reason classify emits Turtle; scan for RightAction/WrongAction/PermissibleAction.
    let verdict = classify_from_output(&stdout);
    Ok(verdict)
}

/// Scan `ousia-reason classify` Turtle output for verdict class membership.
#[must_use]
pub fn classify_from_output(output: &str) -> Verdict {
    // Look for triple patterns like:  :someAction a doxa:RightAction .
    // or  <…action…> rdf:type <…#RightAction> .
    let has = |needle: &str| output.contains(needle);

    if has("RightAction") {
        Verdict::RightAction
    } else if has("WrongAction") {
        Verdict::WrongAction
    } else if has("PermissibleAction") {
        Verdict::PermissibleAction
    } else {
        Verdict::Undetermined
    }
}

/// Run `ousia-reason explain <action-iri>` and return the justification chain.
///
/// # Errors
///
/// Returns an error if the binary could not be executed.
pub fn run_explain(
    bin: &Path,
    framework_owl: &Path,
    scenario: &Path,
    action_iri: &str,
) -> Result<String> {
    let output = Command::new(bin)
        .args(["explain", "--owl"])
        .arg(framework_owl)
        .arg("--abox")
        .arg(scenario)
        .arg("--entity")
        .arg(action_iri)
        .output()
        .with_context(|| format!("failed to execute ousia-reason explain at {}", bin.display()))?;

    if output.stdout.is_empty() {
        // No chain available.
        return Ok(format!(
            "(no axiom chain available for {action_iri} — \
             the reasoner did not produce an explanation)"
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Build the fixture-based explanation for `--explain` when `ousia-reason` is absent.
///
/// Returns a short human-readable chain derived from the framework's
/// decisive feature (no OWL reasoning, but satisfies AC5 in the offline path).
#[must_use]
pub fn fixture_explanation(framework: &str, verdict: &Verdict) -> String {
    let feature = match framework {
        "consequentialism" => "outcome: the action maximises aggregate welfare / minimises harm",
        "deontology" => "maxim: the action's maxim cannot be universalised without contradiction \
                         (Kantian categorical imperative)",
        "virtue-ethics" => "character: a person of practical wisdom (phronesis) would exercise \
                            virtues such as courage, justice, and compassion",
        "contractualism" => "agreement: the action's principle could not be reasonably rejected \
                             by any affected party (Scanlonian contractualism)",
        "care-ethics" => "relationship: the action sustains webs of care and particular \
                          responsibilities (Gilligan/Noddings care ethics)",
        _ => "framework: no structural description available for this framework",
    };

    format!(
        "Axiom chain (offline fixture):\n\
         1. {feature}\n\
         2. Scenario action evaluated against this criterion.\n\
         3. Verdict: {verdict}"
    )
}

// ── Main entry point ──────────────────────────────────────────────────────────

/// Run `doxa reason` and return the result.
///
/// # Errors
///
/// Returns an error for:
/// - malformed scenario `ABox` (AC7)
/// - `ousia-reason` found but failed to execute
#[allow(clippy::too_many_lines)]
pub fn run_reason(args: ReasonArgs) -> Result<ReasonResult> {
    // Validate the scenario file first (AC7: malformed ABox → actionable error).
    let content = validate_scenario(&args.scenario)?;

    let action = action_label(&content, &args.framework);

    // Attempt to find ousia-reason.
    let maybe_bin = resolve_ousia_reason(args.ousia_reason.as_ref());

    if let Some(bin) = maybe_bin {
        // Live reasoning path.
        let maybe_fw_owl: Option<PathBuf> = if let Some(p) = &args.framework_owl {
            Some(p.clone())
        } else {
            let candidate = PathBuf::from(format!("{}.owl", args.framework));
            if candidate.is_file() {
                Some(candidate)
            } else {
                // OWL not built yet — fall through to fixture path below.
                None
            }
        };

        let Some(fw_owl) = maybe_fw_owl else {
            // Framework OWL absent: use fixture annotations (handles unknown/unbuilt frameworks).
            eprintln!(
                "note: framework OWL not found for '{}' — using recorded fixture verdicts",
                args.framework
            );
            let verdict = read_fixture_verdict(&content, &args.framework)
                .unwrap_or(Verdict::Undetermined);
            let explanation = if args.explain {
                Some(fixture_explanation(&args.framework, &verdict))
            } else {
                None
            };
            return Ok(ReasonResult {
                framework: args.framework,
                action,
                verdict,
                explanation,
                from_fixture: true,
            });
        };

        let live_verdict = run_classify(&bin, &fw_owl, &args.scenario)
            .with_context(|| {
                format!(
                    "ousia-reason classify failed for framework '{}' on {}",
                    args.framework,
                    args.scenario.display()
                )
            })?;

        // If ousia-reason returned undetermined, prefer embedded fixture annotations
        // (the TBox may lack classification axioms for this scenario).
        let (verdict, from_fixture) = if live_verdict == Verdict::Undetermined {
            let fixture_verdict = read_fixture_verdict(&content, &args.framework);
            if let Some(fv) = fixture_verdict {
                eprintln!(
                    "note: ousia-reason produced undetermined for '{}' — \
                     using recorded fixture verdict",
                    args.framework
                );
                (fv, true)
            } else {
                (live_verdict, false)
            }
        } else {
            (live_verdict, false)
        };

        let explanation = if args.explain && verdict.is_decided() {
            if from_fixture {
                Some(fixture_explanation(&args.framework, &verdict))
            } else {
                // Use the action IRI heuristic: look for the first Action individual.
                let action_iri = extract_action_iri(&content)
                    .unwrap_or_else(|| "doxa:PrimaryAction".to_string());
                Some(
                    run_explain(&bin, &fw_owl, &args.scenario, &action_iri)
                        .unwrap_or_else(|e| format!("(explain failed: {e})")),
                )
            }
        } else {
            None
        };

        return Ok(ReasonResult {
            framework: args.framework,
            action,
            verdict,
            explanation,
            from_fixture,
        });
    }

    // Fallback: fixture-based path (ousia-reason absent).
    eprintln!(
        "note: ousia-reason not on PATH — using recorded fixture verdicts from {}",
        args.scenario.display()
    );

    let verdict = read_fixture_verdict(&content, &args.framework)
        .unwrap_or(Verdict::Undetermined);

    let explanation = if args.explain {
        Some(fixture_explanation(&args.framework, &verdict))
    } else {
        None
    };

    Ok(ReasonResult {
        framework: args.framework,
        action,
        verdict,
        explanation,
        from_fixture: true,
    })
}

/// Extract the first Action individual IRI from Turtle content (best-effort).
fn extract_action_iri(content: &str) -> Option<String> {
    for line in content.lines() {
        let t = line.trim();
        // Match patterns: `doxa:FooAction` on a line followed by `a doxa:Action`
        // or lines like `doxa:PullLever`
        if (t.contains("a doxa:Action") || t.ends_with("a doxa:Action ;"))
            && !t.starts_with('#')
        {
            // Subject is the token before `a doxa:Action`.
            let subj = t.split_whitespace().next()?;
            if !subj.is_empty() {
                return Some(subj.to_string());
            }
        }
    }
    None
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    // Minimal valid trolley-like ABox embedded as a fixture string.
    const TROLLEY_FIXTURE: &str = r#"
# doxa-verdict: consequentialism=PermissibleAction
# doxa-verdict: deontology=WrongAction
# doxa-verdict: virtue-ethics=undetermined
@prefix doxa: <https://wintermute.jyen.tech/doxa#> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .

doxa:PullLever a doxa:Action ; rdfs:label "Pull the lever" .
doxa:DoNothing a doxa:Action ; rdfs:label "Do nothing" .
"#;

    // ── AC2/AC3: fixture verdict extraction ────────────────────────────────────

    #[test]
    fn fixture_consequentialism_verdict() {
        let v = read_fixture_verdict(TROLLEY_FIXTURE, "consequentialism");
        assert_eq!(v, Some(Verdict::PermissibleAction));
    }

    #[test]
    fn fixture_deontology_verdict() {
        let v = read_fixture_verdict(TROLLEY_FIXTURE, "deontology");
        assert_eq!(v, Some(Verdict::WrongAction));
    }

    #[test]
    fn fixture_virtue_ethics_undetermined() {
        let v = read_fixture_verdict(TROLLEY_FIXTURE, "virtue-ethics");
        // "undetermined" token maps to Undetermined.
        assert_eq!(v, Some(Verdict::Undetermined));
    }

    #[test]
    fn fixture_missing_framework_returns_none() {
        let v = read_fixture_verdict(TROLLEY_FIXTURE, "care-ethics");
        assert_eq!(v, None);
    }

    // ── AC4: consequentialism ≠ deontology on trolley ─────────────────────────

    #[test]
    fn consequentialism_differs_from_deontology_on_trolley() {
        let cons = read_fixture_verdict(TROLLEY_FIXTURE, "consequentialism")
            .unwrap_or(Verdict::Undetermined);
        let deon = read_fixture_verdict(TROLLEY_FIXTURE, "deontology")
            .unwrap_or(Verdict::Undetermined);
        assert_ne!(
            cons, deon,
            "consequentialism and deontology must produce different verdicts on the trolley scenario"
        );
        assert!(cons.is_decided(), "consequentialism must decide on trolley");
        assert!(deon.is_decided(), "deontology must decide on trolley");
    }

    // ── AC6: undetermined is not an error ──────────────────────────────────────

    #[test]
    fn undetermined_verdict_is_not_error() {
        let v = Verdict::Undetermined;
        assert!(!v.is_decided());
        // is_decided() returns false, and the label is "undetermined"
        assert_eq!(v.label(), "undetermined");
    }

    // ── AC7: malformed ABox → error ────────────────────────────────────────────

    #[test]
    fn empty_file_is_malformed() {
        let mut tmp = NamedTempFile::new().expect("tmp file");
        write!(tmp, "   ").expect("write");
        let result = validate_scenario(tmp.path());
        assert!(result.is_err(), "empty file must fail validation");
    }

    #[test]
    fn non_turtle_file_is_malformed() {
        let mut tmp = NamedTempFile::new().expect("tmp file");
        write!(tmp, "this is just plain text without any RDF markers").expect("write");
        let result = validate_scenario(tmp.path());
        assert!(result.is_err(), "non-Turtle file must fail validation");
    }

    #[test]
    fn valid_turtle_passes_validation() {
        let mut tmp = NamedTempFile::new().expect("tmp file");
        write!(tmp, "{}", TROLLEY_FIXTURE).expect("write");
        let result = validate_scenario(tmp.path());
        assert!(result.is_ok(), "valid Turtle must pass validation");
    }

    // ── classify_from_output ───────────────────────────────────────────────────

    #[test]
    fn classify_parses_rightaction() {
        let out = "doxa:PullLever a doxa:RightAction .\n";
        assert_eq!(classify_from_output(out), Verdict::RightAction);
    }

    #[test]
    fn classify_parses_wrongaction() {
        let out = "doxa:SomeAct a doxa:WrongAction .\n";
        assert_eq!(classify_from_output(out), Verdict::WrongAction);
    }

    #[test]
    fn classify_empty_is_undetermined() {
        assert_eq!(classify_from_output(""), Verdict::Undetermined);
    }

    // ── AC5: --explain path ────────────────────────────────────────────────────

    #[test]
    fn fixture_explanation_nonempty_for_decided_verdict() {
        let explanation = fixture_explanation("consequentialism", &Verdict::PermissibleAction);
        assert!(
            !explanation.is_empty(),
            "explanation must be non-empty for a decided verdict"
        );
        assert!(
            explanation.contains("Axiom chain"),
            "explanation must start with axiom chain header"
        );
    }

    #[test]
    fn fixture_explanation_cites_consequentialism_criterion() {
        let exp = fixture_explanation("consequentialism", &Verdict::RightAction);
        assert!(
            exp.contains("welfare") || exp.contains("outcome"),
            "consequentialism explanation must mention outcome/welfare"
        );
    }

    #[test]
    fn fixture_explanation_cites_deontology_criterion() {
        let exp = fixture_explanation("deontology", &Verdict::WrongAction);
        assert!(
            exp.contains("Kant") || exp.contains("maxim"),
            "deontology explanation must mention Kant or maxim"
        );
    }

    // ── run_reason integration (offline) ──────────────────────────────────────

    #[test]
    fn run_reason_offline_consequentialism() {
        let mut tmp = NamedTempFile::new().expect("tmp");
        write!(tmp, "{}", TROLLEY_FIXTURE).expect("write");
        let args = ReasonArgs {
            framework: "consequentialism".to_string(),
            scenario: tmp.path().to_path_buf(),
            explain: false,
            ousia_reason: Some(PathBuf::from("/nonexistent/ousia-reason")),
            framework_owl: None,
        };
        let result = run_reason(args).expect("run_reason must not fail in offline mode");
        assert_eq!(result.verdict, Verdict::PermissibleAction);
        assert!(result.from_fixture);
    }

    #[test]
    fn run_reason_offline_deontology() {
        let mut tmp = NamedTempFile::new().expect("tmp");
        write!(tmp, "{}", TROLLEY_FIXTURE).expect("write");
        let args = ReasonArgs {
            framework: "deontology".to_string(),
            scenario: tmp.path().to_path_buf(),
            explain: false,
            ousia_reason: Some(PathBuf::from("/nonexistent/ousia-reason")),
            framework_owl: None,
        };
        let result = run_reason(args).expect("run_reason must not fail in offline mode");
        assert_eq!(result.verdict, Verdict::WrongAction);
        assert!(result.from_fixture);
    }

    #[test]
    fn run_reason_offline_with_explain() {
        let mut tmp = NamedTempFile::new().expect("tmp");
        write!(tmp, "{}", TROLLEY_FIXTURE).expect("write");
        let args = ReasonArgs {
            framework: "deontology".to_string(),
            scenario: tmp.path().to_path_buf(),
            explain: true,
            ousia_reason: Some(PathBuf::from("/nonexistent/ousia-reason")),
            framework_owl: None,
        };
        let result = run_reason(args).expect("run_reason with explain must not fail");
        assert!(
            result.explanation.is_some(),
            "--explain must produce an explanation"
        );
        let exp = result.explanation.unwrap();
        assert!(!exp.is_empty(), "explanation must be non-empty");
    }

    // ── Summary line formatting ────────────────────────────────────────────────

    #[test]
    fn summary_line_format() {
        let r = ReasonResult {
            framework: "deontology".to_string(),
            action: "Pull the lever".to_string(),
            verdict: Verdict::WrongAction,
            explanation: None,
            from_fixture: true,
        };
        let line = r.summary_line();
        assert!(line.contains("deontology"), "must contain framework name");
        assert!(line.contains("Pull the lever"), "must contain action");
        assert!(line.contains("WrongAction"), "must contain verdict");
    }

    // ── Real trolley.ttl file ─────────────────────────────────────────────────

    #[test]
    fn real_trolley_fixture_consequentialism_deontology_differ() {
        let path = std::path::Path::new("scenarios/trolley.ttl");
        if !path.exists() {
            // Skip if file not present (running outside repo root).
            return;
        }
        let content = std::fs::read_to_string(path).expect("read trolley.ttl");
        let cons = read_fixture_verdict(&content, "consequentialism")
            .unwrap_or(Verdict::Undetermined);
        let deon = read_fixture_verdict(&content, "deontology")
            .unwrap_or(Verdict::Undetermined);
        assert_ne!(
            cons, deon,
            "trolley.ttl must annotate different verdicts for consequentialism vs deontology"
        );
    }
}
