use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use mutr::generator::gambit::GambitGenerator;
use mutr::generator::{GeneratorConfig, MutationGenerator};
use mutr::report::Report;
use mutr::runner::run_mutant;
use std::path::PathBuf;
use std::process;

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
            solc,
            mutations,
            timeout: _timeout,
        } => {
            run_mutation_testing(files, project, output, fail_under, solc, mutations)?;
        }
    }

    Ok(())
}

fn run_mutation_testing(
    files: Vec<PathBuf>,
    project: PathBuf,
    output: Option<PathBuf>,
    fail_under: Option<f64>,
    _solc: Option<PathBuf>,
    mutations: Vec<String>,
) -> Result<()> {
    if !project.exists() {
        anyhow::bail!("invalid project path: {}", project.display());
    }

    let target_files = if files.is_empty() {
        discover_solidity_files(&project)?
    } else {
        files
            .into_iter()
            .map(|f| {
                if f.is_absolute() {
                    Ok(f)
                } else {
                    project
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

    let output_dir = project.join("gambit_out");
    let config = GeneratorConfig {
        project_root: project.clone(),
        files: target_files,
        operators: mutations,
        output_dir,
    };

    let generator = GambitGenerator::new();
    let mutants = generator
        .generate(&config)
        .context("failed to generate mutants")?;

    if mutants.is_empty() {
        println!("No mutants generated");
        return Ok(());
    }

    let mut results = Vec::new();
    for mutant in &mutants {
        let result = run_mutant(mutant, &project).context("failed to run mutant")?;
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
