//! `doxa guard` — pluralist action gating under an explicit, inspectable policy.
//!
//! Aggregates per-framework verdicts into a single `allow | flag | deny` under
//! a user-chosen aggregation policy (unanimity, majority, framework:<name>,
//! lexical:<a,b,c>). Every output names the policy and shows the per-framework
//! breakdown so the ethics applied is never hidden.

use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use std::process::Command;

use anyhow::{anyhow, bail, Context, Result};

// ── Verdict triad ──────────────────────────────────────────────────────────────

/// Three-valued guard verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Action is permitted — exit code 0.
    Allow,
    /// Action requires review — exit code 10.
    Flag,
    /// Action is prohibited — exit code 20.
    Deny,
}

impl Verdict {
    /// POSIX exit code: allow→0, flag→10, deny→20.
    #[must_use]
    pub const fn exit_code(self) -> u8 {
        match self {
            Self::Allow => 0,
            Self::Flag => 10,
            Self::Deny => 20,
        }
    }
}

impl fmt::Display for Verdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Allow => write!(f, "allow"),
            Self::Flag => write!(f, "flag"),
            Self::Deny => write!(f, "deny"),
        }
    }
}

// ── Per-framework raw verdict ──────────────────────────────────────────────────

/// A framework's raw moral verdict (OWL class membership).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameworkVerdict {
    /// `RightAction` or `PermissibleAction` — maps to `allow`.
    Permissible,
    /// `WrongAction` — maps to `deny`.
    Wrong,
    /// `undetermined` / no decisive classification.
    Undetermined,
}

impl FrameworkVerdict {
    /// Map framework verdict → guard triad (None = undetermined).
    #[must_use]
    pub const fn to_guard(self) -> Option<Verdict> {
        match self {
            Self::Permissible => Some(Verdict::Allow),
            Self::Wrong => Some(Verdict::Deny),
            Self::Undetermined => None,
        }
    }

    /// Parse from string (case-insensitive).
    #[must_use]
    pub fn parse(s: &str) -> Self {
        match s.to_ascii_lowercase().trim() {
            "rightaction" | "permissibleaction" | "permissible" | "allow" => Self::Permissible,
            "wrongaction" | "wrong" | "deny" => Self::Wrong,
            _ => Self::Undetermined,
        }
    }
}

impl fmt::Display for FrameworkVerdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Permissible => write!(f, "permissible"),
            Self::Wrong => write!(f, "wrong"),
            Self::Undetermined => write!(f, "undetermined"),
        }
    }
}

// ── Aggregation policy ─────────────────────────────────────────────────────────

/// User-chosen aggregation policy.
#[derive(Debug, Clone)]
pub enum Policy {
    /// `allow` only if every decided framework says permissible; `deny` if any says wrong; else `flag`.
    Unanimity,
    /// Majority of decided frameworks; ties → `flag`.
    Majority,
    /// Defer to one named framework.
    Framework(String),
    /// Apply frameworks in priority order; first decided wins.
    Lexical(Vec<String>),
}

impl Policy {
    /// Parse a policy string into a `Policy`.
    ///
    /// # Errors
    /// Returns an error if the policy string is not recognized.
    pub fn parse(s: &str) -> Result<Self> {
        if s == "unanimity" {
            return Ok(Self::Unanimity);
        }
        if s == "majority" {
            return Ok(Self::Majority);
        }
        if let Some(name) = s.strip_prefix("framework:") {
            if name.is_empty() {
                bail!("framework name must not be empty in policy 'framework:<name>'");
            }
            return Ok(Self::Framework(name.to_string()));
        }
        if let Some(list) = s.strip_prefix("lexical:") {
            let names: Vec<String> = list.split(',').map(|n| n.trim().to_string()).collect();
            if names.is_empty() || names.iter().any(String::is_empty) {
                bail!("lexical policy must list at least one non-empty framework name");
            }
            return Ok(Self::Lexical(names));
        }
        bail!(
            "unknown policy '{s}'. Valid policies: unanimity, majority, \
             framework:<name>, lexical:<a,b,...>"
        )
    }

    /// Human-readable display.
    #[must_use]
    pub fn display_name(&self) -> String {
        match self {
            Self::Unanimity => "unanimity".to_string(),
            Self::Majority => "majority".to_string(),
            Self::Framework(n) => format!("framework:{n}"),
            Self::Lexical(ns) => format!("lexical:{}", ns.join(",")),
        }
    }
}

// ── Per-framework result ───────────────────────────────────────────────────────

/// One framework's verdict plus optional axiom chain.
#[derive(Debug, Clone)]
pub struct FrameworkResult {
    /// Framework name (e.g. `"consequentialism"`).
    pub name: String,
    /// Moral verdict under this framework.
    pub verdict: FrameworkVerdict,
    /// Axiom chain from `ousia-guard --explain`, if requested.
    pub axiom_chain: Option<String>,
}

// ── Guard result ───────────────────────────────────────────────────────────────

/// Full guard result: policy, per-framework breakdown, aggregate verdict, dissenters.
#[derive(Debug)]
pub struct GuardResult {
    /// Aggregation policy used.
    pub policy: Policy,
    /// Per-framework verdict breakdown.
    pub frameworks: Vec<FrameworkResult>,
    /// Aggregate guard verdict.
    pub verdict: Verdict,
    /// Names of frameworks that disagreed with the aggregate verdict.
    pub dissenters: Vec<String>,
}

impl GuardResult {
    /// Format for stdout display.
    #[must_use]
    pub fn display(&self, explain: bool) -> String {
        let mut out = String::new();
        out.push_str(&format!("verdict:  {}\n", self.verdict));
        out.push_str(&format!("policy:   {}\n", self.policy.display_name()));
        out.push_str("breakdown:\n");
        for fr in &self.frameworks {
            out.push_str(&format!("  {}: {}\n", fr.name, fr.verdict));
            if explain {
                if let Some(chain) = &fr.axiom_chain {
                    for line in chain.lines() {
                        out.push_str(&format!("    {line}\n"));
                    }
                }
            }
        }
        if self.dissenters.is_empty() {
            out.push_str("dissenters: none\n");
        } else {
            out.push_str(&format!("dissenters: {}\n", self.dissenters.join(", ")));
        }
        out
    }
}

// ── Aggregation logic ──────────────────────────────────────────────────────────

/// Aggregate per-framework results under a policy into a guard verdict.
///
/// # Errors
/// Returns an error if the named framework (for `framework:` policy) is not
/// in the set of available results.
pub fn aggregate(
    results: &[FrameworkResult],
    policy: &Policy,
) -> Result<(Verdict, Vec<String>)> {
    match policy {
        Policy::Unanimity => Ok(aggregate_unanimity(results)),
        Policy::Majority => Ok(aggregate_majority(results)),
        Policy::Framework(name) => aggregate_single_framework(results, name),
        Policy::Lexical(names) => aggregate_lexical(results, names),
    }
}

fn aggregate_unanimity(results: &[FrameworkResult]) -> (Verdict, Vec<String>) {
    let decided: Vec<&FrameworkResult> = results
        .iter()
        .filter(|r| r.verdict != FrameworkVerdict::Undetermined)
        .collect();

    if decided.is_empty() {
        // No framework decided → flag (cautious)
        return (Verdict::Flag, vec![]);
    }

    let any_deny = decided.iter().any(|r| r.verdict == FrameworkVerdict::Wrong);
    let all_allow = decided.iter().all(|r| r.verdict == FrameworkVerdict::Permissible);

    let verdict = if any_deny {
        Verdict::Deny
    } else if all_allow && decided.len() == results.len() {
        // All frameworks (including undetermined-free) allow
        Verdict::Allow
    } else if all_allow {
        // Some undetermined, none deny — cautious: flag
        Verdict::Flag
    } else {
        Verdict::Flag
    };

    let dissenters = compute_dissenters(results, verdict);
    (verdict, dissenters)
}

fn aggregate_majority(results: &[FrameworkResult]) -> (Verdict, Vec<String>) {
    let decided: Vec<&FrameworkResult> = results
        .iter()
        .filter(|r| r.verdict != FrameworkVerdict::Undetermined)
        .collect();

    if decided.is_empty() {
        return (Verdict::Flag, vec![]);
    }

    let allow_count = decided
        .iter()
        .filter(|r| r.verdict == FrameworkVerdict::Permissible)
        .count();
    let deny_count = decided
        .iter()
        .filter(|r| r.verdict == FrameworkVerdict::Wrong)
        .count();

    let verdict = match allow_count.cmp(&deny_count) {
        std::cmp::Ordering::Greater => Verdict::Allow,
        std::cmp::Ordering::Less => Verdict::Deny,
        std::cmp::Ordering::Equal => Verdict::Flag, // Tie
    };

    let dissenters = compute_dissenters(results, verdict);
    (verdict, dissenters)
}

fn aggregate_single_framework(
    results: &[FrameworkResult],
    name: &str,
) -> Result<(Verdict, Vec<String>)> {
    let fr = results
        .iter()
        .find(|r| r.name.eq_ignore_ascii_case(name))
        .ok_or_else(|| {
            let available: Vec<&str> = results.iter().map(|r| r.name.as_str()).collect();
            anyhow!(
                "framework '{}' not found. Available frameworks: {}",
                name,
                available.join(", ")
            )
        })?;

    let verdict = fr.verdict.to_guard().unwrap_or(Verdict::Flag);
    let dissenters = compute_dissenters(results, verdict);
    Ok((verdict, dissenters))
}

fn aggregate_lexical(
    results: &[FrameworkResult],
    names: &[String],
) -> Result<(Verdict, Vec<String>)> {
    // Build a lookup map
    let map: HashMap<&str, &FrameworkResult> = results.iter().map(|r| (r.name.as_str(), r)).collect();

    for name in names {
        let fr = map.get(name.as_str()).ok_or_else(|| {
            let available: Vec<&str> = results.iter().map(|r| r.name.as_str()).collect();
            anyhow!(
                "framework '{}' in lexical policy not found. Available: {}",
                name,
                available.join(", ")
            )
        })?;

        if fr.verdict != FrameworkVerdict::Undetermined {
            let verdict = fr.verdict.to_guard().unwrap_or(Verdict::Flag);
            let dissenters = compute_dissenters(results, verdict);
            return Ok((verdict, dissenters));
        }
    }

    // All listed frameworks undetermined → flag
    Ok((Verdict::Flag, vec![]))
}

/// Frameworks whose guard verdict differs from the aggregate.
fn compute_dissenters(results: &[FrameworkResult], aggregate: Verdict) -> Vec<String> {
    results
        .iter()
        .filter_map(|r| {
            let fv = r.verdict.to_guard();
            match fv {
                Some(v) if v != aggregate => Some(r.name.clone()),
                None if aggregate != Verdict::Flag => {
                    // Undetermined frameworks dissent from allow/deny
                    Some(r.name.clone())
                }
                _ => None,
            }
        })
        .collect()
}

// ── Live reasoning via ousia-guard / doxa-reason ──────────────────────────────

/// Known ethical frameworks doxa ships with.
pub const DEFAULT_FRAMEWORKS: &[&str] = &[
    "consequentialism",
    "deontology",
    "virtue-ethics",
    "contractualism",
];

/// Collect per-framework verdicts, using `ousia-guard` if available, else stub.
///
/// # Errors
/// Returns an error if the scenario file is not found.
pub fn collect_verdicts(
    scenario: &std::path::Path,
    frameworks: &[String],
    ousia_guard: Option<&std::path::Path>,
    explain: bool,
) -> Result<Vec<FrameworkResult>> {
    if !scenario.exists() {
        bail!(
            "scenario file not found: {}",
            scenario.display()
        );
    }

    // If ousia-guard is available, use it; otherwise fall back to stub verdicts
    // from the scenario file (checking for embedded framework annotations).
    let guard_bin = ousia_guard
        .map(std::path::Path::to_path_buf)
        .or_else(|| find_on_path("ousia-guard"));

    let mut results = Vec::new();
    for fw_name in frameworks {
        let result = if let Some(ref bin) = guard_bin {
            // Fall back to stub if ousia-guard exits non-zero (incompatible CLI).
            if let Ok(r) = call_ousia_guard(bin, scenario, fw_name, explain) {
                r
            } else {
                eprintln!(
                    "note: ousia-guard failed for framework '{fw_name}' — \
                     falling back to scenario annotations"
                );
                stub_verdict(scenario, fw_name)?
            }
        } else {
            stub_verdict(scenario, fw_name)?
        };
        results.push(result);
    }
    Ok(results)
}

fn call_ousia_guard(
    bin: &std::path::Path,
    scenario: &std::path::Path,
    framework: &str,
    explain: bool,
) -> Result<FrameworkResult> {
    let mut cmd = Command::new(bin);
    cmd.arg("--scenario").arg(scenario);
    cmd.arg("--framework").arg(framework);
    if explain {
        cmd.arg("--explain");
    }
    let output = cmd
        .output()
        .with_context(|| format!("failed to run ousia-guard for framework '{framework}'"))?;

    if !output.status.success() {
        bail!(
            "ousia-guard exited with status {} for framework '{framework}'",
            output.status.code().unwrap_or(-1)
        );
    }

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let verdict = parse_ousia_guard_output(&stdout);
    let axiom_chain = if explain { Some(stdout) } else { None };

    Ok(FrameworkResult {
        name: framework.to_string(),
        verdict,
        axiom_chain,
    })
}

fn parse_ousia_guard_output(output: &str) -> FrameworkVerdict {
    // ousia-guard outputs a line like: "verdict: allow" or "RightAction" etc.
    for line in output.lines() {
        let line = line.trim().to_ascii_lowercase();
        if line.contains("rightaction") || line.contains("permissible") || line.contains("allow") {
            return FrameworkVerdict::Permissible;
        }
        if line.contains("wrongaction") || line.contains("wrong") || line.contains("deny") {
            return FrameworkVerdict::Wrong;
        }
    }
    FrameworkVerdict::Undetermined
}

/// Stub verdict — reads embedded `# doxa-verdict: <fw>=<v>` annotations from the
/// scenario file, falling back to `Undetermined` when absent.
fn stub_verdict(scenario: &std::path::Path, framework: &str) -> Result<FrameworkResult> {
    let content = std::fs::read_to_string(scenario)
        .with_context(|| format!("reading scenario {}", scenario.display()))?;

    let verdict = content
        .lines()
        .find_map(|line| {
            // Format: # doxa-verdict: <framework>=<verdict>
            let line = line.trim();
            let body = line.strip_prefix("# doxa-verdict:")?;
            let (fw, vstr) = body.trim().split_once('=')?;
            if fw.trim().eq_ignore_ascii_case(framework) {
                Some(FrameworkVerdict::parse(vstr.trim()))
            } else {
                None
            }
        })
        .unwrap_or(FrameworkVerdict::Undetermined);

    Ok(FrameworkResult {
        name: framework.to_string(),
        verdict,
        axiom_chain: None,
    })
}

fn find_on_path(bin: &str) -> Option<PathBuf> {
    Command::new("which")
        .arg(bin)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if s.is_empty() { None } else { Some(PathBuf::from(s)) }
        })
}

// ── Public entry point ─────────────────────────────────────────────────────────

/// Arguments for the `guard` subcommand.
#[derive(Debug)]
pub struct GuardArgs {
    /// Path to the scenario `ABox` (`.ttl` file).
    pub scenario: PathBuf,
    /// Aggregation policy string (e.g. `"unanimity"`, `"framework:deontology"`).
    pub policy: String,
    /// Frameworks to evaluate (default: all built-in).
    pub frameworks: Option<Vec<String>>,
    /// Explicit path to `ousia-guard` binary.
    pub ousia_guard: Option<PathBuf>,
    /// Include per-framework axiom chains in the output.
    pub explain: bool,
}

/// Run the guard subcommand end-to-end.
///
/// # Errors
/// Returns an error if the policy is invalid, the scenario is missing, or a
/// named framework in a `framework:`/`lexical:` policy is not available.
pub fn run_guard(args: GuardArgs) -> Result<GuardResult> {
    let policy = Policy::parse(&args.policy)?;

    let fw_names: Vec<String> = args.frameworks.unwrap_or_else(|| {
        DEFAULT_FRAMEWORKS
            .iter()
            .map(|s| (*s).to_string())
            .collect()
    });

    let framework_results = collect_verdicts(
        &args.scenario,
        &fw_names,
        args.ousia_guard.as_deref(),
        args.explain,
    )?;

    let (verdict, dissenters) = aggregate(&framework_results, &policy)?;

    Ok(GuardResult {
        policy,
        frameworks: framework_results,
        verdict,
        dissenters,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn make_results(pairs: &[(&str, &str)]) -> Vec<FrameworkResult> {
        pairs
            .iter()
            .map(|(name, verdict_str)| FrameworkResult {
                name: (*name).to_string(),
                verdict: FrameworkVerdict::parse(verdict_str),
                axiom_chain: None,
            })
            .collect()
    }

    #[test]
    fn policy_parse_unanimity() {
        let p = Policy::parse("unanimity").unwrap();
        assert!(matches!(p, Policy::Unanimity));
    }

    #[test]
    fn policy_parse_majority() {
        let p = Policy::parse("majority").unwrap();
        assert!(matches!(p, Policy::Majority));
    }

    #[test]
    fn policy_parse_framework() {
        let p = Policy::parse("framework:deontology").unwrap();
        assert!(matches!(p, Policy::Framework(ref n) if n == "deontology"));
    }

    #[test]
    fn policy_parse_lexical() {
        let p = Policy::parse("lexical:consequentialism,deontology").unwrap();
        assert!(matches!(p, Policy::Lexical(ref ns) if ns[0] == "consequentialism" && ns[1] == "deontology"));
    }

    #[test]
    fn policy_parse_unknown_errors() {
        let err = Policy::parse("bogus").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("unknown policy"), "expected 'unknown policy' in: {msg}");
        assert!(msg.contains("unanimity"), "expected 'unanimity' listed in: {msg}");
    }

    #[test]
    fn unanimity_deny_when_any_wrong() {
        let results = make_results(&[
            ("consequentialism", "permissible"),
            ("deontology", "wrong"),
            ("virtue-ethics", "permissible"),
        ]);
        let (verdict, dissenters) = aggregate(&results, &Policy::Unanimity).unwrap();
        assert_eq!(verdict, Verdict::Deny);
        // consequentialism and virtue-ethics dissent
        assert!(dissenters.contains(&"consequentialism".to_string()));
        assert!(dissenters.contains(&"virtue-ethics".to_string()));
    }

    #[test]
    fn unanimity_allow_when_all_permissible() {
        let results = make_results(&[
            ("consequentialism", "permissible"),
            ("deontology", "permissible"),
            ("virtue-ethics", "permissible"),
        ]);
        let (verdict, _) = aggregate(&results, &Policy::Unanimity).unwrap();
        assert_eq!(verdict, Verdict::Allow);
    }

    #[test]
    fn unanimity_flag_when_conflict() {
        // Conflict: some permissible, some wrong → deny (not allow), but let's check mixed
        // Actually: any wrong → deny under unanimity; this tests undetermined
        let results = make_results(&[
            ("consequentialism", "permissible"),
            ("deontology", "undetermined"),
        ]);
        let (verdict, _) = aggregate(&results, &Policy::Unanimity).unwrap();
        // Some undetermined, none deny → flag (cautious)
        assert_eq!(verdict, Verdict::Flag);
    }

    #[test]
    fn majority_returns_majority_verdict() {
        let results = make_results(&[
            ("consequentialism", "permissible"),
            ("deontology", "wrong"),
            ("virtue-ethics", "permissible"),
        ]);
        let (verdict, dissenters) = aggregate(&results, &Policy::Majority).unwrap();
        assert_eq!(verdict, Verdict::Allow);
        assert!(dissenters.contains(&"deontology".to_string()));
    }

    #[test]
    fn majority_tie_gives_flag() {
        let results = make_results(&[
            ("consequentialism", "permissible"),
            ("deontology", "wrong"),
        ]);
        let (verdict, _) = aggregate(&results, &Policy::Majority).unwrap();
        assert_eq!(verdict, Verdict::Flag);
    }

    #[test]
    fn framework_policy_deontology() {
        let results = make_results(&[
            ("consequentialism", "permissible"),
            ("deontology", "wrong"),
        ]);
        let (verdict, _) =
            aggregate(&results, &Policy::Framework("deontology".to_string())).unwrap();
        assert_eq!(verdict, Verdict::Deny);
    }

    #[test]
    fn framework_policy_unknown_errors() {
        let results = make_results(&[("consequentialism", "permissible")]);
        let err = aggregate(&results, &Policy::Framework("unknown".to_string())).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("not found"), "expected 'not found' in: {msg}");
        assert!(msg.contains("consequentialism"), "should list available: {msg}");
    }

    #[test]
    fn lexical_first_decided_wins() {
        let results = make_results(&[
            ("consequentialism", "permissible"),
            ("deontology", "wrong"),
        ]);
        let policy = Policy::Lexical(vec![
            "consequentialism".to_string(),
            "deontology".to_string(),
        ]);
        let (verdict, _) = aggregate(&results, &policy).unwrap();
        // consequentialism decides → allow
        assert_eq!(verdict, Verdict::Allow);
    }

    #[test]
    fn lexical_falls_through_to_second_when_first_undetermined() {
        let results = make_results(&[
            ("consequentialism", "undetermined"),
            ("deontology", "wrong"),
        ]);
        let policy = Policy::Lexical(vec![
            "consequentialism".to_string(),
            "deontology".to_string(),
        ]);
        let (verdict, _) = aggregate(&results, &policy).unwrap();
        // consequentialism undetermined → fall through to deontology → deny
        assert_eq!(verdict, Verdict::Deny);
    }

    #[test]
    fn lexical_all_undetermined_gives_flag() {
        let results = make_results(&[
            ("consequentialism", "undetermined"),
            ("deontology", "undetermined"),
        ]);
        let policy = Policy::Lexical(vec![
            "consequentialism".to_string(),
            "deontology".to_string(),
        ]);
        let (verdict, _) = aggregate(&results, &policy).unwrap();
        assert_eq!(verdict, Verdict::Flag);
    }

    #[test]
    fn exit_codes_correct() {
        assert_eq!(Verdict::Allow.exit_code(), 0);
        assert_eq!(Verdict::Flag.exit_code(), 10);
        assert_eq!(Verdict::Deny.exit_code(), 20);
    }

    #[test]
    fn guard_result_display_always_has_breakdown() {
        let policy = Policy::Unanimity;
        let frameworks = vec![
            FrameworkResult {
                name: "consequentialism".to_string(),
                verdict: FrameworkVerdict::Permissible,
                axiom_chain: None,
            },
            FrameworkResult {
                name: "deontology".to_string(),
                verdict: FrameworkVerdict::Wrong,
                axiom_chain: None,
            },
        ];
        let result = GuardResult {
            policy,
            frameworks,
            verdict: Verdict::Deny,
            dissenters: vec!["consequentialism".to_string()],
        };
        let display = result.display(false);
        // AC6: breakdown always present
        assert!(display.contains("breakdown:"), "breakdown section missing: {display}");
        assert!(display.contains("consequentialism"), "per-fw breakdown missing: {display}");
        assert!(display.contains("deontology"), "per-fw breakdown missing: {display}");
        assert!(display.contains("policy:"), "policy section missing: {display}");
        assert!(display.contains("unanimity"), "policy name missing: {display}");
    }
}
