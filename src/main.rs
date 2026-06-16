//! doxa — framework-neutral moral `TBox` compiler.
//!
//! Compiles and validates the `spec-core/` TOML spec (in `ousia-forge` format)
//! to OWL 2 DL. Framework modules (next PRD) add axioms over this shared
//! vocabulary to make ethical frameworks commensurable.

#![allow(clippy::print_stderr)] // CLI intentionally writes status to stderr

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};

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

fn main() -> std::process::ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Commands::BuildCore { out, spec } => cmd_build_core(cli.forge, &out, &spec),
        Commands::CheckCore { spec } => cmd_check_core(cli.forge, &spec),
    };
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("doxa error: {e:#}");
            std::process::ExitCode::FAILURE
        }
    }
}
