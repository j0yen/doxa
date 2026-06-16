//! doxa — framework-neutral moral `TBox` compiler.
//!
//! Compiles and validates the `spec-core/` TOML spec (in `ousia-forge` format)
//! to OWL 2 DL. Framework modules add axioms over this shared vocabulary to
//! make ethical frameworks commensurable.
//!
//! The `compare` subcommand (doxa-compare PRD) fans out N frameworks on one
//! scenario and emits an agreement/conflict matrix.

#![allow(clippy::print_stderr)] // CLI intentionally writes status to stderr
#![allow(clippy::print_stdout)] // `doxa list` / `doxa compare` print to stdout

pub mod compare;

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};

use doxa::{build_framework_owlxml, list_frameworks, parse_framework, parse_spec_dir};

/// doxa — framework-neutral moral `TBox` for ethical reasoning.
#[derive(Parser)]
#[command(name = "doxa", version, about)]
struct Cli {
    /// Path to `ousia-forge` binary (default: resolve from `$PATH`).
    #[arg(long, global = true)]
    forge: Option<PathBuf>,

    /// Path to `ousia-reason` binary (default: resolve from `$PATH`).
    #[arg(long, global = true)]
    reason: Option<PathBuf>,

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
    /// Compare how an action is evaluated across two or more frameworks.
    ///
    /// With `--scenario`: fan-out N frameworks on one scenario and emit an
    /// agreement/conflict matrix.
    ///
    /// Without `--scenario`: print the structural difference (the decisive
    /// feature each framework uses) independent of any case.
    Compare {
        /// Frameworks to compare (e.g. consequentialism deontology virtue-ethics).
        #[arg(required = true, num_args = 1..)]
        frameworks: Vec<String>,

        /// Path to the scenario `.ttl` (abox) file.
        /// Omit for a structural (principle-level) comparison.
        #[arg(long)]
        scenario: Option<PathBuf>,

        /// Output format.
        #[arg(long, default_value = "text", value_parser = ["text", "json"])]
        format: String,
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

fn cmd_compare(
    reason_bin: Option<&PathBuf>,
    frameworks: &[String],
    scenario: Option<&PathBuf>,
    format: &str,
) -> Result<()> {
    // No-scenario mode: structural comparison only.
    let Some(scenario_path) = scenario.map(PathBuf::as_path) else {
        let out = compare::structural_compare(frameworks);
        print!("{out}");
        return Ok(());
    };

    let scenario_label = scenario_path.to_string_lossy().into_owned();

    // Fan-out: run ousia-reason for each framework on the scenario.
    let mut verdicts = std::collections::BTreeMap::new();
    for fw in frameworks {
        let v = compare::reason_one(reason_bin.map(PathBuf::as_path), fw, scenario_path)?;
        verdicts.insert(fw.clone(), v);
    }

    let result = compare::CompareResult::from_verdicts(Some(scenario_label), verdicts);

    match format {
        "json" => println!("{}", compare::to_json(&result)),
        _ => print!("{}", compare::to_text(&result)),
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
        Commands::Compare {
            frameworks,
            scenario,
            format,
        } => cmd_compare(cli.reason.as_ref(), &frameworks, scenario.as_ref(), &format),
    };
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("doxa error: {e:#}");
            std::process::ExitCode::FAILURE
        }
    }
}
