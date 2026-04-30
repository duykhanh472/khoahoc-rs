/// odin-ssg — Odin-style Curriculum Static Site Generator
///
/// Commands:
///   build  --source <path> --out <path> [--templates <path>]
///   serve  --source <path> --out <path> --port <u16> [--templates <path>]
mod manifest;
mod models;
mod parser;
mod renderer;
mod server;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

// ─────────────────────────────────────────────────────────────────────────────
// CLI definition (clap 4 derive)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(
    name = "odin-ssg",
    about = "Static site generator for Odin-style Markdown curriculum folders",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Parse a curriculum source directory and emit a static site.
    Build {
        /// Path to the curriculum source directory (containing manifest.yaml).
        #[arg(short, long, value_name = "PATH")]
        source: PathBuf,

        /// Path to write the generated site to.
        #[arg(short, long, value_name = "PATH", default_value = "out")]
        out: PathBuf,

        /// Path to the templates directory.
        /// Defaults to <binary-dir>/templates or ./templates.
        #[arg(short, long, value_name = "PATH")]
        templates: Option<PathBuf>,
    },

    /// Build and serve the site locally with auto-rebuild on changes.
    Serve {
        /// Path to the curriculum source directory.
        #[arg(short, long, value_name = "PATH")]
        source: PathBuf,

        /// Output directory (rebuilt on each change).
        #[arg(short, long, value_name = "PATH", default_value = "out")]
        out: PathBuf,

        /// Port to serve on.
        #[arg(short, long, default_value_t = 8080)]
        port: u16,

        /// Path to the templates directory.
        #[arg(short, long, value_name = "PATH")]
        templates: Option<PathBuf>,
    },
}

// ─────────────────────────────────────────────────────────────────────────────
// Entry point
// ─────────────────────────────────────────────────────────────────────────────

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Build {
            source,
            out,
            templates,
        } => {
            let templates_dir = resolve_templates_dir(templates)?;
            run_build(&source, &out, &templates_dir)?;
            println!(
                "✅  Built {} → {}",
                source.display(),
                out.display()
            );
        }

        Commands::Serve {
            source,
            out,
            port,
            templates,
        } => {
            let templates_dir = resolve_templates_dir(templates)?;

            // Initial build
            println!("🏗  Initial build…");
            run_build(&source, &out, &templates_dir)?;
            println!("✅  Initial build complete.");

            // Clone paths for the closure
            let source_clone = source.clone();
            let out_clone = out.clone();
            let templates_clone = templates_dir.clone();

            server::serve(&source, &out, port, move || {
                run_build(&source_clone, &out_clone, &templates_clone)
            })?;
        }
    }

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Build pipeline
// ─────────────────────────────────────────────────────────────────────────────

/// Full parse → resolve → render pipeline.
fn run_build(source: &std::path::Path, out: &std::path::Path, templates: &std::path::Path) -> Result<()> {
    // 1. Parse
    let (curriculum, search_entries) =
        parser::parse(source).with_context(|| format!("Parsing {}", source.display()))?;

    let n_lessons: usize = curriculum
        .paths
        .iter()
        .flat_map(|p| p.courses.iter())
        .flat_map(|c| c.sections.iter())
        .map(|s| s.lessons.len())
        .sum();

    println!(
        "📚  Parsed {} paths, {} courses, {} lessons",
        curriculum.paths.len(),
        curriculum.paths.iter().map(|p| p.courses.len()).sum::<usize>(),
        n_lessons,
    );

    // 2. Render
    renderer::render_site(&curriculum, &search_entries, templates, out)
        .with_context(|| format!("Rendering to {}", out.display()))?;

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Resolve the templates directory:
///   1. Explicit `--templates` flag
///   2. `templates/` relative to the current working directory
fn resolve_templates_dir(explicit: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(p) = explicit {
        if p.is_dir() {
            return Ok(p);
        }
        anyhow::bail!("Templates directory not found: {}", p.display());
    }
    let cwd_templates = std::env::current_dir()?.join("templates");
    if cwd_templates.is_dir() {
        return Ok(cwd_templates);
    }
    anyhow::bail!(
        "Templates directory not found. Use --templates <path> or create a `templates/` folder."
    );
}
