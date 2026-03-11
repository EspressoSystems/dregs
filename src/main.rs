use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use mutr::config::{find_project_root, parse_foundry_toml, resolve_remappings};
use mutr::generator::gambit::GambitGenerator;
use mutr::generator::{GeneratorConfig, MutationGenerator};
use mutr::report::Report;
use mutr::runner::{run_forge_test, run_mutant};
use std::path::PathBuf;
use std::process;
use std::time::Instant;

#[derive(Parser)]
#[command(name = "mutr")]
#[command(about = "Mutation testing for Solidity projects", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Run {
        #[arg(help = "Solidity files to mutate (default: src/**/*.sol)")]
        files: Vec<PathBuf>,

        #[arg(short, long, default_value = ".", help = "Project root")]
        project: PathBuf,

        #[arg(short, long, help = "Output report path (JSON)")]
        output: Option<PathBuf>,

        #[arg(
            long,
            help = "Fail if mutation score below threshold (0.0-1.0)",
            value_name = "SCORE"
        )]
        fail_under: Option<f64>,

        #[arg(long, help = "Path to solc binary")]
        solc: Option<PathBuf>,

        #[arg(
            long,
            help = "Comma-separated mutation operators (default: all)",
            value_delimiter = ','
        )]
        mutations: Vec<String>,

        #[arg(
            long,
            default_value = "60",
            help = "Test timeout per mutant in seconds"
        )]
        timeout: u64,

        #[arg(
            long,
            help = "Skip gambit's mutant validation (workaround for via_ir projects)"
        )]
        skip_validate: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Run {
            files,
            project,
            output,
            fail_under,
            solc: _solc,
            mutations,
            timeout: _timeout,
            skip_validate,
        } => {
            run_mutation_testing(files, project, output, fail_under, mutations, skip_validate)?;
        }
    }

    Ok(())
}

fn run_mutation_testing(
    files: Vec<PathBuf>,
    project: PathBuf,
    output: Option<PathBuf>,
    fail_under: Option<f64>,
    mutations: Vec<String>,
    skip_validate: bool,
) -> Result<()> {
    let project_root = resolve_project_root(&files, &project)?;

    let target_files = if files.is_empty() {
        discover_solidity_files(&project_root)?
    } else {
        files
            .into_iter()
            .map(|f| {
                if f.is_absolute() {
                    Ok(f)
                } else {
                    project_root
                        .join(&f)
                        .canonicalize()
                        .with_context(|| format!("failed to resolve file path: {}", f.display()))
                }
            })
            .collect::<Result<Vec<_>>>()?
    };

    if target_files.is_empty() {
        anyhow::bail!("no Solidity files found to mutate");
    }

    let foundry_config =
        parse_foundry_toml(&project_root).context("failed to parse foundry.toml")?;

    // Resolve remappings via forge if not explicitly set in foundry.toml
    let foundry_config = foundry_config.map(|mut fc| {
        if fc.remappings.is_empty() {
            fc.remappings = resolve_remappings(&project_root);
        }
        fc
    });

    eprintln!("Running baseline tests...");
    let baseline_start = Instant::now();
    let (has_failures, _) =
        run_forge_test(&project_root).context("failed to run baseline tests")?;
    if has_failures {
        anyhow::bail!("baseline tests failed - fix tests before running mutation testing");
    }
    eprintln!(
        "Baseline tests passed ({:.1}s)",
        baseline_start.elapsed().as_secs_f64()
    );

    let output_dir = project_root.join("gambit_out");
    let config = GeneratorConfig {
        project_root: project_root.clone(),
        files: target_files,
        operators: mutations,
        output_dir,
        foundry_config,
        skip_validate,
    };

    let generator = GambitGenerator::new();
    eprintln!("Generating mutants...");
    let mutants = generator
        .generate(&config)
        .context("failed to generate mutants")?;

    if mutants.is_empty() {
        println!("No mutants generated");
        return Ok(());
    }

    eprintln!("Generated {} mutants", mutants.len());

    let mut results = Vec::new();
    for (i, mutant) in mutants.iter().enumerate() {
        eprintln!(
            "[{}/{}] {}:{} {}",
            i + 1,
            mutants.len(),
            mutant.relative_source_path.display(),
            mutant.line,
            mutant.operator
        );
        let result = run_mutant(mutant, &project_root).context("failed to run mutant")?;
        let status = if result.killed {
            format!(
                "KILLED{}",
                result
                    .killed_by
                    .as_deref()
                    .map(|t| format!(" by {}", t))
                    .unwrap_or_default()
            )
        } else {
            "SURVIVED".to_string()
        };
        eprintln!("  -> {} ({:.1}s)", status, result.duration.as_secs_f64());
        results.push(result);
    }

    let report = Report::new(results);
    report.print_summary(&mutants);

    if let Some(output_path) = output {
        report
            .write_json(&output_path)
            .context("failed to write JSON report")?;
        println!("Report written to: {}", output_path.display());
    }

    if let Some(threshold) = fail_under
        && report.mutation_score < threshold
    {
        eprintln!(
            "Mutation score {:.0}% is below threshold {:.0}%",
            report.mutation_score * 100.0,
            threshold * 100.0
        );
        process::exit(1);
    }

    Ok(())
}

fn resolve_project_root(files: &[PathBuf], explicit_project: &PathBuf) -> Result<PathBuf> {
    let default_project = PathBuf::from(".");
    let is_explicit = explicit_project != &default_project;

    if is_explicit || files.is_empty() {
        if !explicit_project.exists() {
            anyhow::bail!("invalid project path: {}", explicit_project.display());
        }
        return explicit_project
            .canonicalize()
            .context("failed to canonicalize project path");
    }

    let first_file = &files[0];
    let first_file_abs = if first_file.is_absolute() {
        first_file.clone()
    } else {
        std::env::current_dir()?.join(first_file)
    };

    if let Some(root) = find_project_root(&first_file_abs) {
        for file in files.iter().skip(1) {
            let file_abs = if file.is_absolute() {
                file.clone()
            } else {
                std::env::current_dir()?.join(file)
            };
            if let Some(other_root) = find_project_root(&file_abs)
                && other_root != root
            {
                anyhow::bail!(
                    "files have different project roots: {} vs {}",
                    root.display(),
                    other_root.display()
                );
            }
        }
        return Ok(root);
    }

    explicit_project
        .canonicalize()
        .context("failed to canonicalize project path")
}

fn discover_solidity_files(project_root: &std::path::Path) -> Result<Vec<PathBuf>> {
    let pattern = project_root.join("src/**/*.sol");
    let pattern_str = pattern.to_str().context("invalid path for glob pattern")?;

    let mut files = Vec::new();
    for entry in glob::glob(pattern_str).context("failed to read glob pattern")? {
        let path = entry.context("failed to read glob entry")?;
        files.push(path);
    }

    Ok(files)
}
