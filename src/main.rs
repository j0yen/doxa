//! doxa — framework-neutral moral `TBox` compiler.
//!
//! Compiles and validates the `spec-core/` TOML spec (in `ousia-forge` format)
//! to OWL 2 DL. Framework modules add axioms over this shared vocabulary to
//! make ethical frameworks commensurable.

#![allow(clippy::print_stderr)] // CLI intentionally writes status to stderr
#![allow(clippy::print_stdout)] // `doxa list` and `doxa reason` print to stdout

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};

use doxa::{build_framework_owlxml, list_frameworks, parse_framework, parse_spec_dir};

pub(crate) mod reason;

/// doxa — framework-neutral moral `TBox` for ethical reasoning.
#[derive(Parser)]
#[command(name = "doxa", version, about)]
struct Cli {
    /// Path to `ousia-forge` binary (default: resolve from `$PATH`).
    #[arg(long, global = true)]
    forge: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Compile `spec-core/` to OWL 2 DL via `ousia-forge build`.
    BuildCore {
        /// Output OWL file path.
        #[arg(long, default_value = "core.owl")]
        out: PathBuf,

        /// Directory containing the core TOML spec (default: `spec-core/`).
        #[arg(long, default_value = "spec-core")]
        spec: PathBuf,
    },
    /// Validate `spec-core/` without emitting output (`ousia-forge check`).
    CheckCore {
        /// Directory containing the core TOML spec (default: `spec-core/`).
        #[arg(long, default_value = "spec-core")]
        spec: PathBuf,
    },
    /// List available normative frameworks from `spec-frameworks/`.
    List {
        /// Output format.
        #[arg(long, value_enum, default_value = "text")]
        format: ListFormat,

        /// Directory containing framework TOML modules (default: `spec-frameworks/`).
        #[arg(long, default_value = "spec-frameworks")]
        frameworks: PathBuf,
    },
    /// Build core `TBox` + a named framework's axioms into OWL/XML.
    Build {
        /// Framework name (e.g. `consequentialism`, `deontology`, `virtue-ethics`).
        /// Use `--all` to build every framework.
        #[arg(conflicts_with = "all")]
        framework: Option<String>,

        /// Build all available frameworks.
        #[arg(long, conflicts_with = "framework")]
        all: bool,

        /// Output OWL file path (ignored when `--all` is used).
        #[arg(long)]
        out: Option<PathBuf>,

        /// Directory containing the core TOML spec (default: `spec-core/`).
        #[arg(long, default_value = "spec-core")]
        spec: PathBuf,

        /// Directory containing framework TOML modules (default: `spec-frameworks/`).
        #[arg(long, default_value = "spec-frameworks")]
        frameworks: PathBuf,
    },
    /// Evaluate a scenario `ABox` against a framework's `TBox` via `ousia-reason`.
    ///
    /// Prints: `<framework>: <action-IRI> is <RightAction|WrongAction|PermissibleAction|undetermined>`
    Reason {
        /// Framework name (e.g. `consequentialism`, `deontology`, `virtue-ethics`).
        framework: String,

        /// Scenario `ABox` Turtle file (e.g. `scenarios/trolley.ttl`).
        #[arg(long)]
        scenario: PathBuf,

        /// IRI of the action individual to evaluate.
        #[arg(long, default_value = "https://w3id.org/doxa/scenario/trolley#divert")]
        action: String,

        /// Also print the ordered axiom justification chain for the verdict.
        #[arg(long)]
        explain: bool,

        /// Path to `ousia-reason` binary (default: resolve from `$PATH`).
        #[arg(long)]
        reasoner: Option<PathBuf>,

        /// Directory containing the core TOML spec (default: `spec-core/`).
        #[arg(long, default_value = "spec-core")]
        spec: PathBuf,

        /// Directory containing framework TOML modules (default: `spec-frameworks/`).
        #[arg(long, default_value = "spec-frameworks")]
        frameworks: PathBuf,
    },
}

#[derive(Copy, Clone, ValueEnum)]
enum ListFormat {
    Text,
    Json,
}

fn resolve_forge(cli_flag: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(p) = cli_flag {
        if p.is_file() {
            return Ok(p);
        }
        return Err(anyhow!("ousia-forge not found at {}", p.display()));
    }
    // Search PATH
    which_forge()
}

fn which_forge() -> Result<PathBuf> {
    // Try `which ousia-forge` via PATH resolution.
    let output = Command::new("which")
        .arg("ousia-forge")
        .output()
        .context("failed to run `which ousia-forge`")?;
    if output.status.success() {
        let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !path_str.is_empty() {
            return Ok(PathBuf::from(path_str));
        }
    }
    Err(anyhow!(
        "ousia-forge not found on $PATH. \
         Install it (e.g. from ~/wintermute/ousia-forge) or pass --forge <path>."
    ))
}

fn run_forge(forge: &Path, args: &[&str]) -> Result<()> {
    let status = Command::new(forge)
        .args(args)
        .status()
        .with_context(|| format!("failed to execute ousia-forge at {}", forge.display()))?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow!(
            "ousia-forge exited with status {}",
            status.code().unwrap_or(-1)
        ))
    }
}

fn cmd_build_core(forge_path: Option<PathBuf>, out: &Path, spec: &Path) -> Result<()> {
    if !spec.is_dir() {
        return Err(anyhow!(
            "spec directory not found: {}. \
             Run `doxa build-core` from the repo root, or pass --spec <path>.",
            spec.display()
        ));
    }
    let forge = match resolve_forge(forge_path) {
        Ok(f) => f,
        Err(e) => {
            // AC2 asks for a logged note if ousia-forge is absent — not a hard error in tests.
            eprintln!("note: skipping build-core — {e}");
            return Ok(());
        }
    };
    let spec_str = spec.to_string_lossy();
    let out_str = out.to_string_lossy();
    run_forge(
        &forge,
        &["build", "--spec", spec_str.as_ref(), "--out", out_str.as_ref()],
    )
    .with_context(|| format!("ousia-forge build --spec {spec_str} --out {out_str}"))?;
    if out.exists() && out.metadata().map(|m| m.len()).unwrap_or(0) == 0 {
        return Err(anyhow!(
            "ousia-forge produced an empty OWL file at {}",
            out.display()
        ));
    }
    eprintln!("core ontology written to {}", out.display());
    Ok(())
}

fn cmd_check_core(forge_path: Option<PathBuf>, spec: &Path) -> Result<()> {
    if !spec.is_dir() {
        return Err(anyhow!(
            "spec directory not found: {}",
            spec.display()
        ));
    }
    let forge = resolve_forge(forge_path)?;
    let spec_str = spec.to_string_lossy();
    run_forge(&forge, &["check", "--spec", spec_str.as_ref()])
        .with_context(|| format!("ousia-forge check --spec {spec_str}"))
}

fn cmd_list(frameworks_dir: &Path, format: ListFormat) -> Result<()> {
    let frameworks = list_frameworks(frameworks_dir)
        .with_context(|| format!("failed to list frameworks in {}", frameworks_dir.display()))?;

    if frameworks.is_empty() {
        eprintln!("no frameworks found in {}", frameworks_dir.display());
        return Ok(());
    }

    match format {
        ListFormat::Text => {
            for fw in &frameworks {
                println!("{}: {}", fw.name, fw.description);
            }
        }
        ListFormat::Json => {
            println!("[");
            for (i, fw) in frameworks.iter().enumerate() {
                let comma = if i + 1 < frameworks.len() { "," } else { "" };
                println!(
                    "  {{\"name\":\"{}\",\"label\":\"{}\",\"description\":\"{}\",\"author\":\"{}\"}}{}",
                    json_escape(&fw.name),
                    json_escape(&fw.label),
                    json_escape(&fw.description),
                    json_escape(&fw.author),
                    comma
                );
            }
            println!("]");
        }
    }
    Ok(())
}

fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

#[allow(clippy::too_many_arguments)]
fn cmd_build(
    forge_path: Option<PathBuf>,
    framework_name: Option<String>,
    all: bool,
    out: Option<PathBuf>,
    spec_dir: &Path,
    frameworks_dir: &Path,
) -> Result<()> {
    if !spec_dir.is_dir() {
        return Err(anyhow!(
            "spec-core directory not found: {}",
            spec_dir.display()
        ));
    }
    if !frameworks_dir.is_dir() {
        return Err(anyhow!(
            "spec-frameworks directory not found: {}",
            frameworks_dir.display()
        ));
    }

    // Parse the core TBox
    let spec = parse_spec_dir(spec_dir)
        .with_context(|| format!("failed to parse spec-core at {}", spec_dir.display()))?;

    if all {
        // Build all frameworks
        let frameworks = list_frameworks(frameworks_dir)?;
        if frameworks.is_empty() {
            eprintln!("no frameworks found in {}", frameworks_dir.display());
            return Ok(());
        }
        for fw in &frameworks {
            let out_path = PathBuf::from(format!("{}.owl", fw.name));
            build_single_framework(
                forge_path.clone(),
                &spec,
                frameworks_dir,
                &fw.name,
                &out_path,
            )?;
        }
        return Ok(());
    }

    let name = framework_name.ok_or_else(|| {
        anyhow!("specify a framework name or use --all")
    })?;

    let out_path = out.unwrap_or_else(|| PathBuf::from(format!("{name}.owl")));
    build_single_framework(forge_path, &spec, frameworks_dir, &name, &out_path)
}

fn build_single_framework(
    forge_path: Option<PathBuf>,
    spec: &doxa::MoralSpec,
    frameworks_dir: &Path,
    name: &str,
    out_path: &Path,
) -> Result<()> {
    // Find the framework TOML file
    let fw_path = frameworks_dir.join(format!("{name}.toml"));
    if !fw_path.is_file() {
        return Err(anyhow!(
            "framework '{}' not found at {}",
            name,
            fw_path.display()
        ));
    }

    let framework = parse_framework(&fw_path)?;

    // Emit our OWL/XML (ousia-forge merge is future work; we emit regardless of forge presence)
    let forge_present = resolve_forge(forge_path).is_ok();
    let owl = build_framework_owlxml(spec, &framework);
    std::fs::write(out_path, &owl)
        .with_context(|| format!("failed to write OWL to {}", out_path.display()))?;
    if forge_present {
        eprintln!(
            "framework OWL written to {} ({} bytes)",
            out_path.display(),
            owl.len()
        );
    } else {
        eprintln!(
            "framework OWL written to {} ({} bytes) [ousia-forge not on PATH]",
            out_path.display(),
            owl.len()
        );
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn cmd_reason(
    forge_path: Option<PathBuf>,
    framework_name: &str,
    scenario: &Path,
    action_iri: &str,
    explain: bool,
    reasoner_flag: Option<&PathBuf>,
    spec_dir: &Path,
    frameworks_dir: &Path,
) -> Result<()> {
    // Validate inputs
    if !scenario.is_file() {
        return Err(anyhow!(
            "scenario file not found: {}. \
             Create it or pass --scenario <path> to an existing Turtle file.",
            scenario.display()
        ));
    }

    // Build the framework OWL into a temp file
    let spec = parse_spec_dir(spec_dir)
        .with_context(|| format!("failed to parse spec-core at {}", spec_dir.display()))?;
    let fw_path = frameworks_dir.join(format!("{framework_name}.toml"));
    if !fw_path.is_file() {
        return Err(anyhow!(
            "framework '{framework_name}' not found at {}. \
             Use `doxa list` to see available frameworks.",
            fw_path.display()
        ));
    }
    let framework = parse_framework(&fw_path)?;

    // Write framework OWL to a temp file
    let owl_xml = build_framework_owlxml(&spec, &framework);
    let tmp_dir = std::env::temp_dir();
    let owl_tmp = tmp_dir.join(format!("doxa-{framework_name}.owl"));
    std::fs::write(&owl_tmp, &owl_xml)
        .with_context(|| format!("failed to write temp OWL to {}", owl_tmp.display()))?;

    // Combine TBox + ABox
    let combined = reason::combine_ontology(&owl_tmp, scenario)
        .with_context(|| "failed to combine framework TBox with scenario ABox")?;
    let combined_tmp = tmp_dir.join("doxa-combined.ttl");
    std::fs::write(&combined_tmp, &combined)
        .with_context(|| format!("failed to write combined Turtle to {}", combined_tmp.display()))?;

    // Try to resolve ousia-reason; if absent, skip live reasoning per AC2
    let reasoner = match reason::resolve_reasoner(reasoner_flag) {
        Ok(r) => Some(r),
        Err(e) => {
            eprintln!("note: skipping live reasoning — {e}");
            None
        }
    };

    // Also build via doxa-build if forge present (best effort, non-fatal)
    let _ = resolve_forge(forge_path);

    let verdict = if let Some(ref r) = reasoner {
        let classify_out = reason::run_classify(r, &combined_tmp)?;
        reason::parse_verdict(&classify_out, action_iri)
    } else {
        // ousia-reason absent: emit undetermined (AC2 — no fabricated verdict)
        reason::Verdict::Undetermined
    };

    // Print result
    println!("{framework_name}: <{action_iri}> is {verdict}");

    // --explain
    if explain {
        if let Some(ref r) = reasoner {
            if verdict == reason::Verdict::Undetermined {
                println!("\n(no explanation available — verdict is undetermined)");
            } else {
                let chain = reason::run_explain(r, &combined_tmp, action_iri)?;
                println!("\nJustification chain:\n{chain}");
            }
        } else {
            println!("\n(ousia-reason not available — cannot produce explanation chain)");
        }
    }

    Ok(())
}

fn main() -> std::process::ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Commands::BuildCore { out, spec } => cmd_build_core(cli.forge, &out, &spec),
        Commands::CheckCore { spec } => cmd_check_core(cli.forge, &spec),
        Commands::List { format, frameworks } => cmd_list(&frameworks, format),
        Commands::Build {
            framework,
            all,
            out,
            spec,
            frameworks,
        } => cmd_build(cli.forge, framework, all, out, &spec, &frameworks),
        Commands::Reason {
            framework,
            scenario,
            action,
            explain,
            reasoner,
            spec,
            frameworks,
        } => cmd_reason(
            cli.forge,
            &framework,
            &scenario,
            &action,
            explain,
            reasoner.as_ref(),
            &spec,
            &frameworks,
        ),
    };
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("doxa error: {e:#}");
            std::process::ExitCode::FAILURE
        }
    }
}
