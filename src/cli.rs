use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Instant;

use crate::config::{
    DregsConfig, FoundryConfig, find_project_root, parse_dregs_toml, parse_foundry_toml,
    resolve_remappings,
};
use crate::diff::{
    DiffRange, filter_mutants, filter_targets_by_diff, parse_diff_from_reader, parse_git_diff,
};
use crate::generator::gambit::GambitGenerator;
use crate::generator::{FileTarget, GeneratorConfig, Mutant, MutationGenerator};
use crate::manifest::Manifest;
use crate::partition::Partition;
use crate::report::Report;
use crate::runner::{TestResult, list_forge_tests, run_forge_test_baseline, run_mutant};

pub(crate) fn parse_workers(s: &str) -> std::result::Result<usize, String> {
    let n: usize = s.parse().map_err(|e| format!("{e}"))?;
    if n == 0 {
        return Err("workers must be at least 1".to_string());
    }
    Ok(n)
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub(crate) enum OutputFormat {
    Text,
    Markdown,
}

#[derive(Parser)]
#[command(name = "dregs", version)]
#[command(about = "Mutation testing for Solidity projects", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
pub(crate) enum Commands {
    /// Run full mutation testing (generate + test + report)
    Run {
        #[arg(help = "Solidity files to mutate (default: src/**/*.sol)")]
        files: Vec<PathBuf>,

        #[arg(short, long, default_value = ".", help = "Project root")]
        project: PathBuf,

        #[arg(long, help = "Path to dregs.toml config file")]
        config: Option<PathBuf>,

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

        #[arg(short, long, default_value = "1", help = "Number of parallel workers", value_parser = parse_workers)]
        workers: usize,

        #[arg(
            long,
            help = "Only mutate lines changed since this git ref (merge-base)",
            value_name = "REF",
            conflicts_with = "diff_file"
        )]
        diff_base: Option<String>,

        #[arg(
            long,
            help = "Read unified diff from file (use - for stdin)",
            value_name = "PATH"
        )]
        diff_file: Option<PathBuf>,

        #[arg(last = true, help = "Extra arguments passed to forge test (after --)")]
        forge_args: Vec<String>,
    },

    /// Generate mutants and write manifest
    Generate {
        #[arg(help = "Solidity files to mutate (default: src/**/*.sol)")]
        files: Vec<PathBuf>,

        #[arg(short, long, default_value = ".", help = "Project root")]
        project: PathBuf,

        #[arg(long, help = "Path to dregs.toml config file")]
        config: Option<PathBuf>,

        #[arg(
            short,
            long,
            default_value = "./mutants",
            help = "Output directory for manifest and mutants"
        )]
        output: PathBuf,

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
            help = "Skip gambit's mutant validation (workaround for via_ir projects)"
        )]
        skip_validate: bool,

        #[arg(
            long,
            help = "Only mutate lines changed since this git ref (merge-base)",
            value_name = "REF",
            conflicts_with = "diff_file"
        )]
        diff_base: Option<String>,

        #[arg(
            long,
            help = "Read unified diff from file (use - for stdin)",
            value_name = "PATH"
        )]
        diff_file: Option<PathBuf>,
    },

    /// Test mutants from a manifest
    Test {
        #[arg(long, help = "Path to manifest.json")]
        manifest: PathBuf,

        #[arg(short, long, default_value = ".", help = "Project root")]
        project: PathBuf,

        #[arg(short, long, default_value = "1", help = "Number of parallel workers", value_parser = parse_workers)]
        workers: usize,

        #[arg(long, help = "Partition spec (e.g., slice:1/4)")]
        partition: Option<String>,

        #[arg(short, long, help = "Output results path (JSON)")]
        output: Option<PathBuf>,

        #[arg(last = true, help = "Extra arguments passed to forge test (after --)")]
        forge_args: Vec<String>,
    },

    /// Merge results and generate report
    #[command(name = "report")]
    ReportCmd {
        #[arg(help = "Path to manifest.json")]
        manifest: PathBuf,

        #[arg(help = "Result files to merge")]
        result_files: Vec<PathBuf>,

        #[arg(short, long, help = "Output report path (JSON)")]
        output: Option<PathBuf>,

        #[arg(
            long,
            help = "Fail if mutation score below threshold (0.0-1.0)",
            value_name = "SCORE"
        )]
        fail_under: Option<f64>,

        #[arg(
            long,
            default_value = "text",
            help = "Output format (text or markdown)"
        )]
        format: OutputFormat,
    },
}

pub fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::Run {
            files,
            project,
            config,
            output,
            fail_under,
            solc: _solc,
            mutations,
            timeout: _timeout,
            skip_validate,
            workers,
            diff_base,
            diff_file,
            forge_args,
        } => {
            run_mutation_testing(
                files,
                project,
                config,
                output,
                fail_under,
                mutations,
                skip_validate,
                workers,
                diff_base,
                diff_file,
                &forge_args,
            )?;
        }
        Commands::Generate {
            files,
            project,
            config,
            output,
            solc: _solc,
            mutations,
            skip_validate,
            diff_base,
            diff_file,
        } => {
            cmd_generate(
                files,
                project,
                config,
                output,
                mutations,
                skip_validate,
                diff_base,
                diff_file,
            )?;
        }
        Commands::Test {
            manifest,
            project,
            workers,
            partition,
            output,
            forge_args,
        } => {
            cmd_test(manifest, project, workers, partition, output, &forge_args)?;
        }
        Commands::ReportCmd {
            manifest,
            result_files,
            output,
            fail_under,
            format,
        } => {
            cmd_report(manifest, result_files, output, fail_under, format)?;
        }
    }

    Ok(())
}

fn resolve_targets(
    dregs_config: Option<DregsConfig>,
    cli_files: Vec<PathBuf>,
    forge_args: &[String],
    project_root: &Path,
) -> Result<Vec<FileTarget>> {
    if let Some(config) = dregs_config {
        if !cli_files.is_empty() || !forge_args.is_empty() {
            anyhow::bail!(
                "dregs.toml defines targets; do not pass files or -- forge_args on the command line"
            );
        }
        let mut targets = Vec::new();
        for tc in config.targets {
            let resolved_files = resolve_glob_patterns(&tc.files, project_root)?;
            for file in resolved_files {
                targets.push(FileTarget {
                    file,
                    contracts: tc.contracts.clone().unwrap_or_default(),
                    functions: tc.functions.clone().unwrap_or_default(),
                    forge_args: tc.forge_args.clone().unwrap_or_default(),
                });
            }
        }
        if targets.is_empty() {
            anyhow::bail!("dregs.toml targets matched no files");
        }
        Ok(targets)
    } else {
        let target_files = if cli_files.is_empty() {
            discover_solidity_files(project_root)?
        } else {
            cli_files
                .into_iter()
                .map(|f| {
                    if f.is_absolute() {
                        Ok(f)
                    } else {
                        project_root.join(&f).canonicalize().with_context(|| {
                            format!("failed to resolve file path: {}", f.display())
                        })
                    }
                })
                .collect::<Result<Vec<_>>>()?
        };
        if target_files.is_empty() {
            anyhow::bail!("no Solidity files found to mutate");
        }
        Ok(paths_to_targets(target_files, forge_args))
    }
}

fn resolve_glob_patterns(patterns: &[String], project_root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for pattern in patterns {
        let full_pattern = project_root.join(pattern);
        let pattern_str = full_pattern
            .to_str()
            .context("invalid path for glob pattern")?;
        let mut matched = false;
        for entry in glob::glob(pattern_str).context("failed to read glob pattern")? {
            let path = entry.context("failed to read glob entry")?;
            files.push(path);
            matched = true;
        }
        if !matched {
            anyhow::bail!("no files matched pattern: {}", pattern);
        }
    }
    Ok(files)
}

fn paths_to_targets(files: Vec<PathBuf>, forge_args: &[String]) -> Vec<FileTarget> {
    files
        .into_iter()
        .map(|file| FileTarget {
            file,
            contracts: vec![],
            functions: vec![],
            forge_args: forge_args.to_vec(),
        })
        .collect()
}

fn resolve_diff_ranges(
    diff_base: Option<&str>,
    diff_file: Option<&Path>,
    project_root: &Path,
) -> Result<Option<Vec<DiffRange>>> {
    if let Some(base_ref) = diff_base {
        let ranges = parse_git_diff(project_root, base_ref).context("failed to parse git diff")?;
        return Ok(Some(ranges));
    }
    if let Some(path) = diff_file {
        let reader: Box<dyn std::io::Read> = if path == Path::new("-") {
            Box::new(std::io::stdin().lock())
        } else {
            let file = std::fs::File::open(path)
                .context(format!("failed to open diff file: {}", path.display()))?;
            Box::new(std::io::BufReader::new(file))
        };
        let ranges = parse_diff_from_reader(reader).context("failed to read diff")?;
        return Ok(Some(ranges));
    }
    Ok(None)
}

fn apply_diff_target_filter(
    targets: Vec<FileTarget>,
    diff_ranges: &[DiffRange],
    project_root: &Path,
) -> Vec<FileTarget> {
    if diff_ranges.is_empty() {
        eprintln!("No Solidity files changed in diff");
        return vec![];
    }
    let filtered = filter_targets_by_diff(targets, diff_ranges, project_root);
    if filtered.is_empty() {
        eprintln!("No target files overlap with diff");
    } else {
        eprintln!(
            "Diff filter: {} target files overlap with changes",
            filtered.len()
        );
    }
    filtered
}

fn apply_diff_mutant_filter(
    mutants: Vec<Mutant>,
    diff_ranges: Option<&[DiffRange]>,
) -> Vec<Mutant> {
    let Some(diff_ranges) = diff_ranges else {
        return mutants;
    };
    let before = mutants.len();
    let filtered = filter_mutants(mutants, diff_ranges);
    if filtered.is_empty() {
        eprintln!("No mutants on changed lines (filtered {before} mutants)");
    } else {
        eprintln!(
            "Diff filter: {}/{before} mutants on changed lines",
            filtered.len()
        );
    }
    filtered
}

fn generate_mutants(
    project_root: &Path,
    targets: Vec<FileTarget>,
    mutations: Vec<String>,
    foundry_config: Option<FoundryConfig>,
    skip_validate: bool,
) -> Result<Vec<Mutant>> {
    let output_dir = project_root.join("gambit_out");
    let config = GeneratorConfig {
        project_root: project_root.to_path_buf(),
        targets,
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

    Ok(mutants)
}

fn run_mutants_parallel(
    mutants: &[Mutant],
    project_root: &Path,
    workers: usize,
) -> Result<Vec<TestResult>> {
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(workers)
        .build()
        .context("failed to build thread pool")?;

    let completed = AtomicU32::new(0);
    let total = mutants.len();

    let mut results: Vec<_> = pool.install(|| {
        mutants
            .par_iter()
            .map(|mutant| {
                let result = run_mutant(mutant, project_root)
                    .with_context(|| format!("failed to run mutant {}", mutant.id))?;
                let done = completed.fetch_add(1, Ordering::Relaxed) + 1;
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
                let mut msg = format!(
                    "[{}/{}] {}:{} {} -> {} ({:.1}s)",
                    done,
                    total,
                    mutant.relative_source_path.display(),
                    mutant.line,
                    mutant.operator,
                    status,
                    result.duration.as_secs_f64()
                );
                if !result.killed {
                    msg.push_str(&format!(
                        "\n     `{}` -> `{}`",
                        mutant.original, mutant.replacement
                    ));
                }
                eprintln!("{msg}");
                Ok(result)
            })
            .collect::<Result<Vec<_>>>()
    })?;

    results.sort_by_key(|r| r.mutant_id);
    Ok(results)
}

fn baseline_forge_arg_sets(
    dregs_config: Option<&DregsConfig>,
    cli_forge_args: &[String],
) -> Vec<Vec<String>> {
    let Some(config) = dregs_config else {
        return vec![cli_forge_args.to_vec()];
    };
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();
    for target in &config.targets {
        let args = target.forge_args.clone().unwrap_or_default();
        if seen.insert(args.clone()) {
            result.push(args);
        }
    }
    result
}

fn run_baseline_tests(project_root: &Path, forge_args: &[String]) -> Result<()> {
    if !forge_args.is_empty() {
        let test_names =
            list_forge_tests(project_root, forge_args).context("failed to list matching tests")?;
        if test_names.is_empty() {
            anyhow::bail!("no tests matched the provided filters");
        }
        eprintln!("Matched {} tests:", test_names.len());
        for name in &test_names {
            eprintln!("  {}", name);
        }
    }

    eprintln!("Running baseline tests...");
    let baseline_start = Instant::now();
    if let Err(e) = run_forge_test_baseline(project_root, forge_args) {
        eprintln!("{}", e);
        anyhow::bail!("baseline tests failed - fix tests before running mutation testing");
    }
    eprintln!(
        "Baseline tests passed ({:.1}s)",
        baseline_start.elapsed().as_secs_f64()
    );

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_mutation_testing(
    files: Vec<PathBuf>,
    project: PathBuf,
    config: Option<PathBuf>,
    output: Option<PathBuf>,
    fail_under: Option<f64>,
    mutations: Vec<String>,
    skip_validate: bool,
    workers: usize,
    diff_base: Option<String>,
    diff_file: Option<PathBuf>,
    forge_args: &[String],
) -> Result<()> {
    let project_root = resolve_project_root(&files, &project)?;
    let foundry_config =
        parse_foundry_toml(&project_root).context("failed to parse foundry.toml")?;
    if foundry_config.is_some() {
        eprintln!(
            "Using foundry.toml: {}",
            project_root.join("foundry.toml").display()
        );
    }
    let foundry_config = foundry_config.map(|mut fc| {
        if fc.remappings.is_empty() {
            fc.remappings = resolve_remappings(&project_root);
        }
        fc
    });

    let dregs_config =
        parse_dregs_toml(&project_root, config.as_deref()).context("failed to parse dregs.toml")?;
    if dregs_config.is_some() {
        let default_path = project_root.join("dregs.toml");
        let dregs_path = config.as_deref().unwrap_or(&default_path);
        eprintln!("Using dregs.toml: {}", dregs_path.display());
    }

    let baseline_arg_sets = baseline_forge_arg_sets(dregs_config.as_ref(), forge_args);
    for args in &baseline_arg_sets {
        run_baseline_tests(&project_root, args)?;
    }

    let targets = resolve_targets(dregs_config, files, forge_args, &project_root)?;
    let diff_ranges =
        resolve_diff_ranges(diff_base.as_deref(), diff_file.as_deref(), &project_root)?;
    let targets = if let Some(ranges) = &diff_ranges {
        apply_diff_target_filter(targets, ranges, &project_root)
    } else {
        targets
    };
    if targets.is_empty() {
        println!("Mutation score: 100.0% (0 mutants)");
        return Ok(());
    }

    let mutants = generate_mutants(
        &project_root,
        targets,
        mutations,
        foundry_config,
        skip_validate,
    )?;
    let mutants = apply_diff_mutant_filter(mutants, diff_ranges.as_deref());
    if mutants.is_empty() {
        if diff_ranges.is_some() {
            println!("Mutation score: 100.0% (0 mutants)");
        } else {
            println!("No mutants generated");
        }
        return Ok(());
    }

    eprintln!(
        "Testing {} mutants with {} workers...",
        mutants.len(),
        workers
    );

    let results = run_mutants_parallel(&mutants, &project_root, workers)?;

    let report = Report::new(results);
    report.print_summary(&mutants);

    if let Some(output_path) = output {
        report
            .write_json(&output_path)
            .context("failed to write JSON report")?;
        eprintln!("Report written to: {}", output_path.display());
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

#[allow(clippy::too_many_arguments)]
fn cmd_generate(
    files: Vec<PathBuf>,
    project: PathBuf,
    config: Option<PathBuf>,
    output: PathBuf,
    mutations: Vec<String>,
    skip_validate: bool,
    diff_base: Option<String>,
    diff_file: Option<PathBuf>,
) -> Result<()> {
    let project_root = resolve_project_root(&files, &project)?;
    let foundry_config =
        parse_foundry_toml(&project_root).context("failed to parse foundry.toml")?;
    if foundry_config.is_some() {
        eprintln!(
            "Using foundry.toml: {}",
            project_root.join("foundry.toml").display()
        );
    }
    let foundry_config = foundry_config.map(|mut fc| {
        if fc.remappings.is_empty() {
            fc.remappings = resolve_remappings(&project_root);
        }
        fc
    });

    let dregs_config =
        parse_dregs_toml(&project_root, config.as_deref()).context("failed to parse dregs.toml")?;
    if dregs_config.is_some() {
        let default_path = project_root.join("dregs.toml");
        let dregs_path = config.as_deref().unwrap_or(&default_path);
        eprintln!("Using dregs.toml: {}", dregs_path.display());
    }
    let targets = resolve_targets(dregs_config, files, &[], &project_root)?;
    let diff_ranges =
        resolve_diff_ranges(diff_base.as_deref(), diff_file.as_deref(), &project_root)?;
    let targets = if let Some(ranges) = &diff_ranges {
        apply_diff_target_filter(targets, ranges, &project_root)
    } else {
        targets
    };
    if targets.is_empty() {
        println!("No mutants generated");
        return Ok(());
    }

    let mutants = generate_mutants(
        &project_root,
        targets,
        mutations,
        foundry_config,
        skip_validate,
    )?;
    let mutants = apply_diff_mutant_filter(mutants, diff_ranges.as_deref());
    if mutants.is_empty() {
        println!("No mutants generated");
        return Ok(());
    }

    let manifest = Manifest::write(&output, mutants).context("failed to write manifest")?;
    eprintln!(
        "Generated {} mutants to {}",
        manifest.mutants.len(),
        output.display()
    );
    Ok(())
}

fn cmd_test(
    manifest_path: PathBuf,
    project: PathBuf,
    workers: usize,
    partition: Option<String>,
    output: Option<PathBuf>,
    forge_args: &[String],
) -> Result<()> {
    let project_root = resolve_project_root(&[], &project)?;

    let manifest = Manifest::read(&manifest_path).context("failed to read manifest")?;

    // cmd_test doesn't load dregs.toml, so baseline runs with CLI forge_args only
    run_baseline_tests(&project_root, forge_args)?;

    let mutants_to_test: Vec<&Mutant> = if let Some(partition_str) = &partition {
        let p: Partition = partition_str.parse().context("failed to parse partition")?;
        p.filter(&manifest.mutants, |m| m.id)
    } else {
        manifest.mutants.iter().collect()
    };

    if mutants_to_test.is_empty() {
        eprintln!("No mutants in this partition");
        if let Some(output_path) = &output {
            let empty: Vec<TestResult> = vec![];
            let json = serde_json::to_string_pretty(&empty)?;
            std::fs::write(output_path, json)?;
        }
        return Ok(());
    }

    eprintln!(
        "Testing {} mutants (of {} total)",
        mutants_to_test.len(),
        manifest.mutants.len()
    );

    let owned_mutants: Vec<Mutant> = mutants_to_test
        .into_iter()
        .cloned()
        .map(|mut m| {
            if !forge_args.is_empty() {
                m.forge_args = forge_args.to_vec();
            }
            m
        })
        .collect();
    let results = run_mutants_parallel(&owned_mutants, &project_root, workers)?;

    if let Some(output_path) = &output {
        let json = serde_json::to_string_pretty(&results)?;
        std::fs::write(output_path, json)?;
        eprintln!("Results written to {}", output_path.display());
    } else {
        let json = serde_json::to_string_pretty(&results)?;
        println!("{json}");
    }

    Ok(())
}

fn cmd_report(
    manifest_path: PathBuf,
    result_files: Vec<PathBuf>,
    output: Option<PathBuf>,
    fail_under: Option<f64>,
    format: OutputFormat,
) -> Result<()> {
    let manifest = Manifest::read(&manifest_path).context("failed to read manifest")?;

    let results = Report::merge(&result_files).context("failed to merge results")?;

    if results.len() < manifest.mutants.len() {
        eprintln!(
            "Warning: results cover {}/{} mutants",
            results.len(),
            manifest.mutants.len()
        );
    }

    let report = Report::new(results);
    match format {
        OutputFormat::Markdown => report.print_summary_markdown(&manifest.mutants),
        OutputFormat::Text => report.print_summary(&manifest.mutants),
    }

    if let Some(output_path) = output {
        report
            .write_json(&output_path)
            .context("failed to write JSON report")?;
        eprintln!("Report written to: {}", output_path.display());
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

#[cfg(test)]
#[allow(clippy::single_range_in_vec_init)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_workers_valid() {
        assert_eq!(parse_workers("4").unwrap(), 4);
        assert_eq!(parse_workers("1").unwrap(), 1);
    }

    #[test]
    fn test_parse_workers_zero() {
        let err = parse_workers("0").unwrap_err();
        assert!(err.contains("at least 1"));
    }

    #[test]
    fn test_parse_workers_non_numeric() {
        let err = parse_workers("abc").unwrap_err();
        assert!(!err.is_empty());
    }

    #[test]
    fn test_parse_workers_negative() {
        let err = parse_workers("-1").unwrap_err();
        assert!(!err.is_empty());
    }

    #[test]
    fn test_resolve_project_root_explicit_path() {
        let temp = assert_fs::TempDir::new().unwrap();
        let root = resolve_project_root(&[], &temp.path().to_path_buf()).unwrap();
        assert_eq!(root, temp.path().canonicalize().unwrap());
    }

    #[test]
    fn test_resolve_project_root_invalid_path() {
        let result = resolve_project_root(&[], &PathBuf::from("/nonexistent/path"));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("invalid project path")
        );
    }

    #[test]
    fn test_resolve_project_root_fallback_to_default() {
        use assert_fs::prelude::*;
        // File with no foundry.toml above it -> falls back to canonicalize(".")
        let temp = assert_fs::TempDir::new().unwrap();
        temp.child("src/A.sol").touch().unwrap();
        let file = temp.path().join("src/A.sol");
        let result = resolve_project_root(&[file], &PathBuf::from("."));
        assert!(result.is_ok());
    }

    #[test]
    fn test_discover_solidity_files_empty() {
        let temp = assert_fs::TempDir::new().unwrap();
        let files = discover_solidity_files(temp.path()).unwrap();
        assert!(files.is_empty());
    }

    #[test]
    fn test_discover_solidity_files_found() {
        use assert_fs::prelude::*;
        let temp = assert_fs::TempDir::new().unwrap();
        temp.child("src/A.sol").write_str("// sol").unwrap();
        temp.child("src/nested/B.sol").write_str("// sol").unwrap();
        let files = discover_solidity_files(temp.path()).unwrap();
        assert_eq!(files.len(), 2);
    }

    #[test]
    fn test_resolve_targets_no_files_found() {
        let temp = assert_fs::TempDir::new().unwrap();
        let result = resolve_targets(None, vec![], &[], temp.path());
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("no Solidity files")
        );
    }

    #[test]
    fn test_resolve_targets_nonexistent_file() {
        let temp = assert_fs::TempDir::new().unwrap();
        let result = resolve_targets(
            None,
            vec![PathBuf::from("nonexistent.sol")],
            &[],
            temp.path(),
        );
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("failed to resolve file path")
        );
    }

    #[test]
    fn test_resolve_targets_from_dregs_config() {
        use crate::config::{DregsConfig, TargetConfig};
        use assert_fs::prelude::*;

        let temp = assert_fs::TempDir::new().unwrap();
        temp.child("src/Token.sol").write_str("// sol").unwrap();

        let config = DregsConfig {
            targets: vec![TargetConfig {
                files: vec!["src/Token.sol".to_string()],
                contracts: Some(vec!["Token".to_string()]),
                functions: None,
                forge_args: Some(vec![
                    "--match-contract".to_string(),
                    "TokenTest".to_string(),
                ]),
            }],
        };

        let targets = resolve_targets(Some(config), vec![], &[], temp.path()).unwrap();
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].contracts, vec!["Token"]);
        assert_eq!(targets[0].forge_args, vec!["--match-contract", "TokenTest"]);
    }

    #[test]
    fn test_resolve_targets_config_conflict_with_files() {
        use crate::config::{DregsConfig, TargetConfig};

        let temp = assert_fs::TempDir::new().unwrap();
        let config = DregsConfig {
            targets: vec![TargetConfig {
                files: vec!["src/Token.sol".to_string()],
                contracts: None,
                functions: None,
                forge_args: None,
            }],
        };

        let result = resolve_targets(
            Some(config),
            vec![PathBuf::from("src/Other.sol")],
            &[],
            temp.path(),
        );
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("dregs.toml defines targets")
        );
    }

    #[test]
    fn test_resolve_targets_config_conflict_with_forge_args() {
        use crate::config::{DregsConfig, TargetConfig};

        let temp = assert_fs::TempDir::new().unwrap();
        let config = DregsConfig {
            targets: vec![TargetConfig {
                files: vec!["src/Token.sol".to_string()],
                contracts: None,
                functions: None,
                forge_args: None,
            }],
        };

        let result = resolve_targets(
            Some(config),
            vec![],
            &["--match-test".to_string(), "test_foo".to_string()],
            temp.path(),
        );
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("dregs.toml defines targets")
        );
    }

    #[test]
    fn test_resolve_targets_config_glob_pattern() {
        use crate::config::{DregsConfig, TargetConfig};
        use assert_fs::prelude::*;

        let temp = assert_fs::TempDir::new().unwrap();
        temp.child("src/A.sol").write_str("// sol").unwrap();
        temp.child("src/B.sol").write_str("// sol").unwrap();
        temp.child("src/nested/C.sol").write_str("// sol").unwrap();

        let config = DregsConfig {
            targets: vec![TargetConfig {
                files: vec!["src/**/*.sol".to_string()],
                contracts: None,
                functions: None,
                forge_args: None,
            }],
        };

        let targets = resolve_targets(Some(config), vec![], &[], temp.path()).unwrap();
        assert_eq!(targets.len(), 3);
    }

    #[test]
    fn test_resolve_targets_config_no_matching_files() {
        use crate::config::{DregsConfig, TargetConfig};

        let temp = assert_fs::TempDir::new().unwrap();
        let config = DregsConfig {
            targets: vec![TargetConfig {
                files: vec!["nonexistent/**/*.sol".to_string()],
                contracts: None,
                functions: None,
                forge_args: None,
            }],
        };

        let result = resolve_targets(Some(config), vec![], &[], temp.path());
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("no files matched pattern")
        );
    }

    #[test]
    fn test_resolve_targets_config_missing_literal_file() {
        use crate::config::{DregsConfig, TargetConfig};

        let temp = assert_fs::TempDir::new().unwrap();
        let config = DregsConfig {
            targets: vec![TargetConfig {
                files: vec!["src/Missing.sol".to_string()],
                contracts: None,
                functions: None,
                forge_args: None,
            }],
        };

        let result = resolve_targets(Some(config), vec![], &[], temp.path());
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("no files matched pattern")
        );
    }

    #[test]
    fn test_resolve_targets_no_config_no_files() {
        let temp = assert_fs::TempDir::new().unwrap();
        let result = resolve_targets(None, vec![], &[], temp.path());
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("no Solidity files")
        );
    }

    #[test]
    fn test_cli_accepts_diff_base() {
        use clap::Parser;
        let cli = Cli::parse_from(["dregs", "run", "--diff-base", "main"]);
        let Commands::Run { diff_base, .. } = cli.command else {
            panic!("expected Run command")
        };
        assert_eq!(diff_base, Some("main".to_string()));
    }

    #[test]
    fn test_cli_generate_accepts_diff_base() {
        use clap::Parser;
        let cli = Cli::parse_from(["dregs", "generate", "--diff-base", "HEAD~1"]);
        let Commands::Generate { diff_base, .. } = cli.command else {
            panic!("expected Generate command")
        };
        assert_eq!(diff_base, Some("HEAD~1".to_string()));
    }

    #[test]
    fn test_resolve_targets_no_config_with_files() {
        use assert_fs::prelude::*;

        let temp = assert_fs::TempDir::new().unwrap();
        temp.child("src/A.sol").write_str("// sol").unwrap();

        let file = temp.path().join("src/A.sol");
        let targets = resolve_targets(
            None,
            vec![file],
            &["--match-test".to_string(), "foo".to_string()],
            temp.path(),
        )
        .unwrap();
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].forge_args, vec!["--match-test", "foo"]);
    }

    #[test]
    fn test_apply_diff_target_filter_with_diff() {
        let targets = vec![
            FileTarget {
                file: PathBuf::from("/project/src/A.sol"),
                contracts: vec![],
                functions: vec![],
                forge_args: vec![],
            },
            FileTarget {
                file: PathBuf::from("/project/src/B.sol"),
                contracts: vec![],
                functions: vec![],
                forge_args: vec![],
            },
        ];
        let diff_ranges = vec![DiffRange {
            file: PathBuf::from("src/A.sol"),
            lines: vec![1..5],
        }];
        let filtered = apply_diff_target_filter(targets, &diff_ranges, Path::new("/project"));
        assert_eq!(filtered.len(), 1);
        assert!(filtered[0].file.to_string_lossy().contains("src/A.sol"));
    }

    #[test]
    fn test_apply_diff_target_filter_empty_diff() {
        let targets = vec![FileTarget {
            file: PathBuf::from("src/A.sol"),
            contracts: vec![],
            functions: vec![],
            forge_args: vec![],
        }];
        let diff_ranges: Vec<DiffRange> = vec![];
        let filtered = apply_diff_target_filter(targets, &diff_ranges, Path::new(""));
        assert!(filtered.is_empty());
    }

    #[test]
    fn test_apply_diff_target_filter_no_overlap() {
        let targets = vec![FileTarget {
            file: PathBuf::from("/project/src/B.sol"),
            contracts: vec![],
            functions: vec![],
            forge_args: vec![],
        }];
        let diff_ranges = vec![DiffRange {
            file: PathBuf::from("src/A.sol"),
            lines: vec![1..5],
        }];
        let filtered = apply_diff_target_filter(targets, &diff_ranges, Path::new("/project"));
        assert!(filtered.is_empty());
    }

    #[test]
    fn test_resolve_diff_ranges_from_file() {
        let temp = assert_fs::TempDir::new().unwrap();
        let diff_path = temp.path().join("test.diff");
        std::fs::write(
            &diff_path,
            "\
diff --git a/src/Counter.sol b/src/Counter.sol
--- a/src/Counter.sol
+++ b/src/Counter.sol
@@ -10,3 +10,5 @@ contract Counter {
",
        )
        .unwrap();

        let ranges = resolve_diff_ranges(None, Some(diff_path.as_path()), temp.path()).unwrap();
        let ranges = ranges.unwrap();
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].file, PathBuf::from("src/Counter.sol"));
        assert_eq!(ranges[0].lines, vec![10..15]);
    }

    #[test]
    fn test_resolve_diff_ranges_none() {
        let temp = assert_fs::TempDir::new().unwrap();
        let result = resolve_diff_ranges(None, None, temp.path()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_cli_accepts_diff_file() {
        use clap::Parser;
        let cli = Cli::parse_from(["dregs", "run", "--diff-file", "changes.diff"]);
        let Commands::Run {
            diff_file,
            diff_base,
            ..
        } = cli.command
        else {
            panic!("expected Run command")
        };
        assert_eq!(diff_file, Some(PathBuf::from("changes.diff")));
        assert!(diff_base.is_none());
    }

    #[test]
    fn test_cli_generate_accepts_diff_file() {
        use clap::Parser;
        let cli = Cli::parse_from(["dregs", "generate", "--diff-file", "-"]);
        let Commands::Generate {
            diff_file,
            diff_base,
            ..
        } = cli.command
        else {
            panic!("expected Generate command")
        };
        assert_eq!(diff_file, Some(PathBuf::from("-")));
        assert!(diff_base.is_none());
    }

    #[test]
    fn test_apply_diff_mutant_filter_none() {
        let mutants = vec![Mutant {
            id: 1,
            source_path: PathBuf::from("src/A.sol"),
            relative_source_path: PathBuf::from("src/A.sol"),
            mutant_path: PathBuf::from("out/1.sol"),
            operator: "op".to_string(),
            original: "a".to_string(),
            replacement: "b".to_string(),
            line: 10,
            forge_args: vec![],
        }];
        let result = apply_diff_mutant_filter(mutants, None);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_apply_diff_mutant_filter_with_ranges() {
        let mutants = vec![
            Mutant {
                id: 1,
                source_path: PathBuf::from("src/A.sol"),
                relative_source_path: PathBuf::from("src/A.sol"),
                mutant_path: PathBuf::from("out/1.sol"),
                operator: "op".to_string(),
                original: "a".to_string(),
                replacement: "b".to_string(),
                line: 10,
                forge_args: vec![],
            },
            Mutant {
                id: 2,
                source_path: PathBuf::from("src/A.sol"),
                relative_source_path: PathBuf::from("src/A.sol"),
                mutant_path: PathBuf::from("out/2.sol"),
                operator: "op".to_string(),
                original: "a".to_string(),
                replacement: "b".to_string(),
                line: 50,
                forge_args: vec![],
            },
        ];
        let ranges = Some(vec![DiffRange {
            file: PathBuf::from("src/A.sol"),
            lines: vec![8..15],
        }]);
        let result = apply_diff_mutant_filter(mutants, ranges.as_deref());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, 1);
    }

    #[test]
    fn test_apply_diff_mutant_filter_all_filtered() {
        let mutants = vec![Mutant {
            id: 1,
            source_path: PathBuf::from("src/A.sol"),
            relative_source_path: PathBuf::from("src/A.sol"),
            mutant_path: PathBuf::from("out/1.sol"),
            operator: "op".to_string(),
            original: "a".to_string(),
            replacement: "b".to_string(),
            line: 50,
            forge_args: vec![],
        }];
        let ranges = Some(vec![DiffRange {
            file: PathBuf::from("src/A.sol"),
            lines: vec![1..5],
        }]);
        let result = apply_diff_mutant_filter(mutants, ranges.as_deref());
        assert!(result.is_empty());
    }

    #[test]
    fn test_baseline_forge_arg_sets_no_config() {
        let args = vec!["--match-test".to_string(), "Foo".to_string()];
        let result = baseline_forge_arg_sets(None, &args);
        assert_eq!(result, vec![args]);
    }

    #[test]
    fn test_baseline_forge_arg_sets_no_config_empty() {
        let result = baseline_forge_arg_sets(None, &[]);
        assert_eq!(result, vec![Vec::<String>::new()]);
    }

    #[test]
    fn test_baseline_forge_arg_sets_from_dregs_config() {
        use crate::config::{DregsConfig, TargetConfig};

        let config = DregsConfig {
            targets: vec![
                TargetConfig {
                    files: vec!["src/A.sol".to_string()],
                    contracts: None,
                    functions: None,
                    forge_args: Some(vec!["--match-contract".to_string(), "ATest".to_string()]),
                },
                TargetConfig {
                    files: vec!["src/B.sol".to_string()],
                    contracts: None,
                    functions: None,
                    forge_args: Some(vec!["--match-contract".to_string(), "BTest".to_string()]),
                },
            ],
        };

        let result = baseline_forge_arg_sets(Some(&config), &[]);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], vec!["--match-contract", "ATest"]);
        assert_eq!(result[1], vec!["--match-contract", "BTest"]);
    }

    #[test]
    fn test_baseline_forge_arg_sets_deduplicates() {
        use crate::config::{DregsConfig, TargetConfig};

        let args = vec!["--match-contract".to_string(), "ATest".to_string()];
        let config = DregsConfig {
            targets: vec![
                TargetConfig {
                    files: vec!["src/A.sol".to_string()],
                    contracts: None,
                    functions: None,
                    forge_args: Some(args.clone()),
                },
                TargetConfig {
                    files: vec!["src/B.sol".to_string()],
                    contracts: None,
                    functions: None,
                    forge_args: Some(args),
                },
            ],
        };

        let result = baseline_forge_arg_sets(Some(&config), &[]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], vec!["--match-contract", "ATest"]);
    }

    #[test]
    fn test_baseline_forge_arg_sets_none_treated_as_empty() {
        use crate::config::{DregsConfig, TargetConfig};

        let config = DregsConfig {
            targets: vec![TargetConfig {
                files: vec!["src/A.sol".to_string()],
                contracts: None,
                functions: None,
                forge_args: None,
            }],
        };

        let result = baseline_forge_arg_sets(Some(&config), &[]);
        assert_eq!(result, vec![Vec::<String>::new()]);
    }
}
