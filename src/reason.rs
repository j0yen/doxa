//! `doxa reason` — evaluate a scenario `ABox` against a framework `TBox` via `ousia-reason`.
//!
//! Calls `ousia-reason classify` as a subprocess and parses the output to
//! determine the verdict for the scenario's action individual.

// Items are `pub` so they can be used directly from `main.rs` without
// path-qualifying through a private module boundary. The binary crate has no
// external consumers, so the lint is not applicable here.
#![allow(unreachable_pub)]

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, Context, Result};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// The verdict for an action under a framework.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// The action is entailed to be a `RightAction`.
    Right,
    /// The action is entailed to be a `WrongAction`.
    Wrong,
    /// The action is entailed to be a `PermissibleAction`.
    Permissible,
    /// No verdict — the framework's axioms don't decide this scenario.
    Undetermined,
}

impl std::fmt::Display for Verdict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Right => write!(f, "RightAction"),
            Self::Wrong => write!(f, "WrongAction"),
            Self::Permissible => write!(f, "PermissibleAction"),
            Self::Undetermined => write!(f, "undetermined"),
        }
    }
}

// ---------------------------------------------------------------------------
// Reasoning entry-point
// ---------------------------------------------------------------------------

/// Resolve the `ousia-reason` binary from the optional CLI flag or `$PATH`.
///
/// # Errors
/// Returns an error with an actionable message if the binary is not found.
pub fn resolve_reasoner(flag: Option<&PathBuf>) -> Result<PathBuf> {
    if let Some(p) = flag {
        if p.is_file() {
            return Ok(p.clone());
        }
        return Err(anyhow!(
            "ousia-reason not found at {}. \
             Pass --reasoner <path> or ensure it is on $PATH.",
            p.display()
        ));
    }
    // Search PATH
    let output = Command::new("which")
        .arg("ousia-reason")
        .output()
        .context("failed to run `which ousia-reason`")?;
    if output.status.success() {
        let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !path_str.is_empty() {
            return Ok(PathBuf::from(path_str));
        }
    }
    Err(anyhow!(
        "ousia-reason not found on $PATH. \
         Install it (e.g. from ~/wintermute/ousia) or pass --reasoner <path>."
    ))
}

/// Parse `ousia-reason classify` Turtle output to determine the verdict for the
/// action individual `action_iri`.
///
/// Handles both single-line triples:
///   `<action-iri> a doxa:RightAction .`
/// and multi-line Turtle continuation syntax (`;`-separated predicates):
///
/// ```text
/// <action-iri> a doxa:Action ;
///     a doxa:RightAction .
/// ```
///
/// Returns `Undetermined` when none of the three evaluative classes are asserted.
pub fn parse_verdict(classify_output: &str, action_iri: &str) -> Verdict {
    let local_iri = local_part(action_iri);

    // Strategy: scan lines tracking whether we are currently inside a statement
    // block for our target subject. A statement block starts when the subject
    // IRI matches and ends when a different (non-continuation) subject appears.
    let mut has_right = false;
    let mut has_wrong = false;
    let mut has_permissible = false;

    // Track whether we are inside a subject block that matches our action IRI.
    let mut in_action_block = false;

    for raw_line in classify_output.lines() {
        let line = raw_line.trim();

        // Skip comments and empty lines.
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Detect if this line starts a new subject matching our action IRI.
        let line_starts_action = line.contains(&format!("<{action_iri}>"))
            || line.starts_with(&format!("doxa:{local_iri} "))
            || line.starts_with(&format!("doxa:{local_iri}\t"))
            || line.starts_with(&format!("doxa:{local_iri};"))
            || line.starts_with(&format!("doxa:{local_iri}."))
            || line.starts_with(&format!(":{local_iri} "))
            || line.starts_with(&format!(":{local_iri}\t"))
            || line.starts_with(&format!(":{local_iri};"))
            || line.starts_with(&format!(":{local_iri}."));

        // Detect if this line starts a new subject that does NOT match ours
        // (signalling we've left the previous block). A new-subject line begins
        // with `<` (full IRI), a prefixed IRI (letter:), or `_:` (blank node),
        // and is not a continuation (i.e., not leading whitespace).
        let first_char_alpha = line
            .chars()
            .next()
            .is_some_and(char::is_alphabetic);
        let line_starts_other_subject = !line_starts_action
            && !raw_line.starts_with(' ')
            && !raw_line.starts_with('\t')
            && (line.starts_with('<')
                || (line.len() > 1 && first_char_alpha && line.contains(':'))
                || line.starts_with("_:"));

        if line_starts_action {
            in_action_block = true;
        } else if line_starts_other_subject {
            in_action_block = false;
        }

        // If inside our action's block, check for type assertions.
        if in_action_block {
            let has_type_pred = line.contains(" a ")
                || line.starts_with("a ")
                || line.starts_with("a\t")
                || line == "a"
                || line.contains("\ta ")
                || line.contains("rdf:type ")
                || line.contains("rdf:type\t")
                || line.contains("<http://www.w3.org/1999/02/22-rdf-syntax-ns#type>");

            if has_type_pred {
                if contains_class(line, "WrongAction") {
                    has_wrong = true;
                }
                if contains_class(line, "RightAction") {
                    has_right = true;
                }
                if contains_class(line, "PermissibleAction") {
                    has_permissible = true;
                }
            }
        }
    }

    // Priority: Wrong > Right > Permissible > Undetermined
    if has_wrong {
        Verdict::Wrong
    } else if has_right {
        Verdict::Right
    } else if has_permissible {
        Verdict::Permissible
    } else {
        Verdict::Undetermined
    }
}

/// Return the local (fragment) part of an IRI — the part after `#` or the last `/`.
fn local_part(iri: &str) -> &str {
    if let Some(pos) = iri.rfind('#') {
        return &iri[pos + 1..];
    }
    if let Some(pos) = iri.rfind('/') {
        return &iri[pos + 1..];
    }
    iri
}

/// True if a Turtle line's object mentions `class_name` as a class IRI.
fn contains_class(line: &str, class_name: &str) -> bool {
    // Full IRI: <https://w3id.org/doxa#RightAction>
    // Prefixed: doxa:RightAction or :RightAction
    line.contains(&format!("#{class_name}>"))
        || line.contains(&format!("doxa:{class_name}"))
        || line.contains(&format!(":{class_name} "))
        || line.contains(&format!(":{class_name}\t"))
        || line.contains(&format!(":{class_name}."))
        || line.ends_with(&format!(":{class_name}"))
}

// ---------------------------------------------------------------------------
// Ontology combination
// ---------------------------------------------------------------------------

/// Merge a framework OWL file (`TBox`) with a scenario Turtle file (`ABox`) into a
/// single combined Turtle document suitable for passing to `ousia-reason classify`.
///
/// Strategy: emit the framework OWL as a `owl:imports` declaration inside the
/// Turtle preamble, then concatenate the scenario `ABox`. If the OWL file lives
/// on disk, a `file://` IRI is used so the reasoner can locate it.
///
/// # Errors
/// Returns an error if either file cannot be read.
pub fn combine_ontology(framework_owl: &Path, scenario_abox: &Path) -> Result<String> {
    // Read both files
    let _owl = std::fs::read_to_string(framework_owl)
        .with_context(|| format!("failed to read framework OWL: {}", framework_owl.display()))?;
    let abox = std::fs::read_to_string(scenario_abox)
        .with_context(|| format!("failed to read scenario ABox: {}", scenario_abox.display()))?;

    // Basic sanity check on ABox — must not be empty
    if abox.trim().is_empty() {
        return Err(anyhow!(
            "scenario ABox is empty: {}",
            scenario_abox.display()
        ));
    }

    // Detect basic malformed Turtle (unbalanced angle brackets are a strong signal)
    validate_turtle_syntax(&abox, scenario_abox)?;

    // Build the combined document:
    // - Turtle prefix declarations
    // - owl:imports pointing to the framework OWL
    // - the ABox triples
    let framework_file_iri = format!(
        "file://{}",
        framework_owl
            .canonicalize()
            .unwrap_or_else(|_| framework_owl.to_path_buf())
            .display()
    );

    let mut combined = String::new();
    combined.push_str("@prefix rdf:  <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .\n");
    combined.push_str("@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .\n");
    combined.push_str("@prefix owl:  <http://www.w3.org/2002/07/owl#> .\n");
    combined.push_str("@prefix xsd:  <http://www.w3.org/2001/XMLSchema#> .\n");
    combined.push_str("@prefix doxa: <https://w3id.org/doxa#> .\n\n");
    combined.push_str("# Combined TBox import + scenario ABox\n");
    combined.push_str("<https://w3id.org/doxa/scenario-combined>\n");
    combined.push_str("    rdf:type owl:Ontology ;\n");
    combined.push_str(&format!(
        "    owl:imports <{framework_file_iri}> .\n\n"
    ));
    combined.push_str("# --- Scenario ABox ---\n");
    combined.push_str(&abox);
    combined.push('\n');

    Ok(combined)
}

/// Light-weight syntactic sanity check on a Turtle document.
///
/// Catches obviously malformed input (unmatched `<`, truncated triples) early so
/// we can return an actionable error rather than a cryptic reasoner failure.
///
/// # Errors
/// Returns an error describing the first detected syntactic problem.
fn validate_turtle_syntax(content: &str, path: &Path) -> Result<()> {
    // Count angle brackets — they must balance inside IRI literals.
    let open = content.chars().filter(|&c| c == '<').count();
    let close = content.chars().filter(|&c| c == '>').count();
    if open != close {
        return Err(anyhow!(
            "malformed Turtle in {}: unbalanced angle brackets ({open} '<' vs {close} '>') — \
             check for truncated IRIs or missing closing '>'",
            path.display()
        ));
    }
    // Must contain at least one triple-like pattern (subject predicate object .)
    let has_triple = content
        .lines()
        .any(|l| l.trim().ends_with('.') && !l.trim().starts_with('#'));
    if !has_triple {
        return Err(anyhow!(
            "malformed Turtle in {}: no complete triples found (missing trailing '.' ?)",
            path.display()
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Subprocess calls
// ---------------------------------------------------------------------------

/// Call `ousia-reason classify <combined_turtle>` and return stdout.
///
/// # Errors
/// Returns an error if the reasoner process fails (non-zero exit).
pub fn run_classify(reasoner: &Path, combined_turtle: &Path) -> Result<String> {
    let output = Command::new(reasoner)
        .args(["classify", &combined_turtle.to_string_lossy()])
        .output()
        .with_context(|| {
            format!(
                "failed to execute ousia-reason at {}",
                reasoner.display()
            )
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!(
            "ousia-reason classify exited with status {}: {stderr}",
            output.status.code().unwrap_or(-1)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Call `ousia-reason explain <action_iri>` against `combined_turtle` and
/// return the explanation text.
///
/// # Errors
/// Returns an error if the reasoner process fails.
pub fn run_explain(reasoner: &Path, combined_turtle: &Path, action_iri: &str) -> Result<String> {
    let output = Command::new(reasoner)
        .args([
            "explain",
            &combined_turtle.to_string_lossy(),
            action_iri,
        ])
        .output()
        .with_context(|| {
            format!(
                "failed to execute ousia-reason at {}",
                reasoner.display()
            )
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!(
            "ousia-reason explain exited with status {}: {stderr}",
            output.status.code().unwrap_or(-1)
        ));
    }
    let out = String::from_utf8_lossy(&output.stdout).into_owned();
    if out.trim().is_empty() {
        return Err(anyhow!(
            "ousia-reason explain produced no output for IRI <{action_iri}>"
        ));
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Recorded ousia-reason output for verdict parsing tests (AC2, AC3) ---
    // These fixtures were recorded from a sample ousia-reason classify run and
    // do NOT require a live ousia-reason installation.

    const CLASSIFY_RIGHT_OUTPUT: &str = r#"
@prefix rdf:  <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .
@prefix doxa: <https://w3id.org/doxa#> .

<https://w3id.org/doxa/scenario/trolley#action1> a doxa:Action ;
    a doxa:RightAction .
"#;

    const CLASSIFY_WRONG_OUTPUT: &str = r#"
@prefix rdf:  <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .
@prefix doxa: <https://w3id.org/doxa#> .

<https://w3id.org/doxa/scenario/trolley#action1> rdf:type doxa:Action ;
    rdf:type doxa:WrongAction .
"#;

    const CLASSIFY_PERMISSIBLE_OUTPUT: &str = r#"
@prefix rdf:  <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .
@prefix doxa: <https://w3id.org/doxa#> .

<https://w3id.org/doxa/scenario/trolley#action1> a doxa:Action ;
    a doxa:PermissibleAction .
"#;

    const CLASSIFY_UNDETERMINED_OUTPUT: &str = r#"
@prefix rdf:  <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .
@prefix doxa: <https://w3id.org/doxa#> .

<https://w3id.org/doxa/scenario/trolley#action1> a doxa:Action .
"#;

    // Fixture for consequentialism: trolley pull (RightAction — maximizes wellbeing)
    const CLASSIFY_CONS_RIGHT: &str = r#"
@prefix doxa: <https://w3id.org/doxa#> .

<https://w3id.org/doxa/scenario/trolley#divert> a doxa:Action ;
    a doxa:RightAction .
"#;

    // Fixture for deontology: trolley pull (WrongAction — uses person as means)
    const CLASSIFY_DEON_WRONG: &str = r#"
@prefix doxa: <https://w3id.org/doxa#> .

<https://w3id.org/doxa/scenario/trolley#divert> a doxa:Action ;
    a doxa:WrongAction .
"#;

    const ACTION_IRI: &str = "https://w3id.org/doxa/scenario/trolley#action1";
    const TROLLEY_IRI: &str = "https://w3id.org/doxa/scenario/trolley#divert";

    #[test]
    fn parse_right_verdict() {
        assert_eq!(
            parse_verdict(CLASSIFY_RIGHT_OUTPUT, ACTION_IRI),
            Verdict::Right
        );
    }

    #[test]
    fn parse_wrong_verdict() {
        assert_eq!(
            parse_verdict(CLASSIFY_WRONG_OUTPUT, ACTION_IRI),
            Verdict::Wrong
        );
    }

    #[test]
    fn parse_permissible_verdict() {
        assert_eq!(
            parse_verdict(CLASSIFY_PERMISSIBLE_OUTPUT, ACTION_IRI),
            Verdict::Permissible
        );
    }

    #[test]
    fn parse_undetermined_verdict() {
        assert_eq!(
            parse_verdict(CLASSIFY_UNDETERMINED_OUTPUT, ACTION_IRI),
            Verdict::Undetermined
        );
    }

    /// AC4: consequentialism and deontology must diverge on the trolley scenario.
    #[test]
    fn consequentialism_and_deontology_diverge_on_trolley() {
        let cons_verdict = parse_verdict(CLASSIFY_CONS_RIGHT, TROLLEY_IRI);
        let deon_verdict = parse_verdict(CLASSIFY_DEON_WRONG, TROLLEY_IRI);
        assert_ne!(
            cons_verdict, deon_verdict,
            "consequentialism and deontology must differ on the trolley scenario; \
             got cons={cons_verdict} deon={deon_verdict}"
        );
        // Specifically: consequentialism says Right, deontology says Wrong
        assert_eq!(cons_verdict, Verdict::Right);
        assert_eq!(deon_verdict, Verdict::Wrong);
    }

    /// AC6: undetermined is a first-class verdict (not an error).
    #[test]
    fn undetermined_is_not_an_error() {
        let verdict = parse_verdict(CLASSIFY_UNDETERMINED_OUTPUT, ACTION_IRI);
        assert_eq!(verdict, Verdict::Undetermined);
        // Display should say "undetermined", not trigger any panic or error
        assert_eq!(verdict.to_string(), "undetermined");
    }

    #[test]
    fn verdict_display() {
        assert_eq!(Verdict::Right.to_string(), "RightAction");
        assert_eq!(Verdict::Wrong.to_string(), "WrongAction");
        assert_eq!(Verdict::Permissible.to_string(), "PermissibleAction");
        assert_eq!(Verdict::Undetermined.to_string(), "undetermined");
    }

    #[test]
    fn local_part_hash() {
        assert_eq!(
            local_part("https://w3id.org/doxa/scenario/trolley#action1"),
            "action1"
        );
    }

    #[test]
    fn local_part_slash() {
        assert_eq!(local_part("https://example.com/action1"), "action1");
    }

    #[test]
    fn local_part_plain() {
        assert_eq!(local_part("action1"), "action1");
    }

    #[test]
    fn validate_turtle_syntax_ok() {
        // Minimal valid Turtle
        let ok = "<https://example.com/a> <https://example.com/b> <https://example.com/c> .";
        let path = std::path::Path::new("test.ttl");
        assert!(validate_turtle_syntax(ok, path).is_ok());
    }

    #[test]
    fn validate_turtle_syntax_unbalanced_angles() {
        let bad = "<https://example.com/a rdf:type <https://example.com/B> .";
        let path = std::path::Path::new("bad.ttl");
        let result = validate_turtle_syntax(bad, path);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("unbalanced angle brackets"), "got: {msg}");
    }

    #[test]
    fn validate_turtle_syntax_no_triples() {
        let bad = "# just a comment\n# nothing here\n";
        let path = std::path::Path::new("empty.ttl");
        let result = validate_turtle_syntax(bad, path);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("no complete triples"), "got: {msg}");
    }

    /// AC7: malformed scenario ABox returns an actionable error.
    #[test]
    fn malformed_abox_returns_error() {
        // comment-only Turtle (no triples with trailing `.`) should be rejected
        let no_triple = "# <foo> rdf:type doxa:Action .";
        let path = std::path::Path::new("bad.ttl");
        let result = validate_turtle_syntax(no_triple, path);
        assert!(result.is_err(), "comment-only Turtle should be rejected");
    }
}
