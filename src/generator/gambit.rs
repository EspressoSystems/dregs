use super::{GeneratorConfig, Mutant, MutationGenerator, Result};
use gambit::{MutateParams, run_mutate};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Default)]
pub(crate) struct GambitGenerator;

impl GambitGenerator {
    pub(crate) fn new() -> Self {
        Self
    }
}

impl MutationGenerator for GambitGenerator {
    fn generate(&self, config: &GeneratorConfig) -> Result<Vec<Mutant>> {
        let foundry = config.foundry_config.as_ref();

        if let Some(fc) = foundry
            && fc.via_ir
        {
            eprintln!(
                "Warning: via_ir=true detected in foundry.toml but gambit doesn't support it. \
                 Consider using --skip-validate if mutation generation fails."
            );
        }

        let mut mutate_params_list = Vec::new();
        let mut file_forge_args: HashMap<PathBuf, Vec<String>> = HashMap::new();

        for target in &config.targets {
            // Canonicalize to handle path representation differences between our input and gambit output
            let key = target.file.canonicalize().unwrap_or(target.file.clone());
            file_forge_args.insert(key, target.forge_args.clone());

            let contracts: Vec<Option<&String>> = if target.contracts.is_empty() {
                vec![None]
            } else {
                target.contracts.iter().map(Some).collect()
            };

            let functions = if target.functions.is_empty() {
                None
            } else {
                Some(target.functions.clone())
            };

            for contract in &contracts {
                let params = MutateParams {
                    json: None,
                    filename: Some(target.file.to_string_lossy().to_string()),
                    num_mutants: None,
                    random_seed: false,
                    seed: 0,
                    outdir: Some(config.output_dir.to_string_lossy().to_string()),
                    sourceroot: Some(config.project_root.to_string_lossy().to_string()),
                    mutations: if config.operators.is_empty() {
                        None
                    } else {
                        Some(config.operators.clone())
                    },
                    no_export: false,
                    no_overwrite: false,
                    solc: foundry
                        .and_then(|f| f.solc.clone())
                        .filter(|s| {
                            let is_path = s.contains('/') || s.starts_with("solc");
                            if !is_path {
                                eprintln!(
                                    "Warning: ignoring solc value '{}' from foundry.toml \
                                     (gambit expects a binary path, not a version string)",
                                    s
                                );
                            }
                            is_path
                        })
                        .unwrap_or_else(|| "solc".to_string()),
                    solc_optimize: foundry.is_some_and(|f| f.optimizer),
                    solc_evm_version: foundry.and_then(|f| f.evm_version.clone()),
                    functions: functions.clone(),
                    contract: contract.cloned(),
                    solc_base_path: None,
                    solc_allow_paths: None,
                    solc_include_path: None,
                    solc_remappings: foundry
                        .map(|f| f.remappings.clone())
                        .filter(|r| !r.is_empty()),
                    skip_validate: config.skip_validate,
                };
                mutate_params_list.push(params);
            }
        }

        let gambit_results = run_mutate(mutate_params_list)
            .map_err(|e| super::GeneratorError::Generation(e.to_string()))?;

        let mut mutants = Vec::new();
        let mut mutant_id = 1u32;

        for (_outdir, gambit_mutants) in gambit_results {
            for gambit_mutant in gambit_mutants {
                let source = &gambit_mutant.source;
                let source_path = source.filename();
                let relative_path = source
                    .relative_filename()
                    .expect("gambit returned invalid source without relative filename");

                let (line, _col) = source
                    .get_line_column(gambit_mutant.start)
                    .expect("gambit returned invalid mutant position");

                let mutant_path = config
                    .output_dir
                    .join("mutants")
                    .join(mutant_id.to_string())
                    .join(&relative_path);

                mutants.push(Mutant {
                    id: mutant_id,
                    source_path: source_path.to_path_buf(),
                    relative_source_path: relative_path.clone(),
                    mutant_path,
                    operator: format!("{:?}", gambit_mutant.op),
                    original: gambit_mutant.orig.clone(),
                    replacement: gambit_mutant.repl.clone(),
                    line: line as u32,
                    forge_args: file_forge_args
                        .get(
                            &source_path
                                .canonicalize()
                                .unwrap_or(source_path.to_path_buf()),
                        )
                        .cloned()
                        .unwrap_or_default(),
                });

                mutant_id += 1;
            }
        }

        Ok(mutants)
    }
}

#[cfg(test)]
mod tests {
    use super::super::FileTarget;
    use super::*;

    #[test]
    fn test_gambit_generator_new() {
        let _generator = GambitGenerator::new();
    }

    #[test]
    fn test_gambit_generator_default() {
        let _generator: GambitGenerator = Default::default();
    }

    #[test]
    fn test_generate_empty_files() {
        let generator = GambitGenerator::new();
        let config = GeneratorConfig {
            project_root: PathBuf::from("."),
            targets: vec![],
            operators: vec![],
            output_dir: PathBuf::from("gambit_out"),
            foundry_config: None,
            skip_validate: false,
        };
        let result = generator.generate(&config);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[test]
    fn test_generate_with_solidity_file() {
        use tempfile::TempDir;

        let generator = GambitGenerator::new();
        let (_fixture_temp, project_root) = crate::test_utils::fixture_to_temp("simple");
        let temp_dir = TempDir::new().unwrap();
        let output_dir = temp_dir.path().join("gambit_out");

        let config = GeneratorConfig {
            project_root: project_root.clone(),
            targets: vec![FileTarget {
                file: project_root.join("src/Counter.sol"),
                contracts: vec![],
                functions: vec![],
                forge_args: vec![],
            }],
            operators: vec![],
            output_dir,
            foundry_config: None,
            skip_validate: false,
        };

        let result = generator.generate(&config);
        assert!(result.is_ok());

        let mutants = result.unwrap();
        assert!(!mutants.is_empty());

        let first_mutant = &mutants[0];
        assert_eq!(first_mutant.id, 1);
        assert!(first_mutant.source_path.exists());
        assert!(!first_mutant.operator.is_empty());
        assert!(first_mutant.line > 0);
    }

    #[test]
    fn test_generate_with_specific_operators() {
        use tempfile::TempDir;

        let generator = GambitGenerator::new();
        let (_fixture_temp, project_root) = crate::test_utils::fixture_to_temp("simple");
        let temp_dir = TempDir::new().unwrap();
        let output_dir = temp_dir.path().join("gambit_out");

        let config = GeneratorConfig {
            project_root: project_root.clone(),
            targets: vec![FileTarget {
                file: project_root.join("src/Counter.sol"),
                contracts: vec![],
                functions: vec![],
                forge_args: vec![],
            }],
            operators: vec!["binary-op-mutation".to_string()],
            output_dir,
            foundry_config: None,
            skip_validate: false,
        };

        let result = generator.generate(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_generate_with_via_ir_warning() {
        use crate::config::FoundryConfig;
        use tempfile::TempDir;

        let generator = GambitGenerator::new();
        let (_fixture_temp, project_root) = crate::test_utils::fixture_to_temp("simple");
        let temp_dir = TempDir::new().unwrap();
        let output_dir = temp_dir.path().join("gambit_out");

        let config = GeneratorConfig {
            project_root: project_root.clone(),
            targets: vec![FileTarget {
                file: project_root.join("src/Counter.sol"),
                contracts: vec![],
                functions: vec![],
                forge_args: vec![],
            }],
            operators: vec![],
            output_dir,
            foundry_config: Some(FoundryConfig {
                via_ir: true,
                ..Default::default()
            }),
            skip_validate: true,
        };

        let result = generator.generate(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_generate_with_contract_filter() {
        use tempfile::TempDir;

        let generator = GambitGenerator::new();
        let (_fixture_temp, project_root) = crate::test_utils::fixture_to_temp("simple");
        let temp_dir = TempDir::new().unwrap();
        let output_dir = temp_dir.path().join("gambit_out");

        let config = GeneratorConfig {
            project_root: project_root.clone(),
            targets: vec![FileTarget {
                file: project_root.join("src/Counter.sol"),
                contracts: vec!["Counter".to_string()],
                functions: vec!["increment".to_string()],
                forge_args: vec!["--match-contract".to_string(), "CounterTest".to_string()],
            }],
            operators: vec![],
            output_dir,
            foundry_config: None,
            skip_validate: false,
        };

        let result = generator.generate(&config);
        assert!(result.is_ok());

        let mutants = result.unwrap();
        assert!(!mutants.is_empty());
        for mutant in &mutants {
            assert_eq!(mutant.forge_args, vec!["--match-contract", "CounterTest"]);
        }
    }

    #[test]
    fn test_generate_with_multiple_contracts() {
        use tempfile::TempDir;

        let generator = GambitGenerator::new();
        let (_fixture_temp, project_root) = crate::test_utils::fixture_to_temp("simple");
        let temp_dir = TempDir::new().unwrap();
        let output_dir = temp_dir.path().join("gambit_out");

        let config = GeneratorConfig {
            project_root: project_root.clone(),
            targets: vec![FileTarget {
                file: project_root.join("src/Counter.sol"),
                contracts: vec!["Counter".to_string(), "NonExistent".to_string()],
                functions: vec![],
                forge_args: vec![],
            }],
            operators: vec![],
            output_dir,
            foundry_config: None,
            skip_validate: false,
        };

        let result = generator.generate(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_generate_filters_solc_version_string() {
        use crate::config::FoundryConfig;
        use tempfile::TempDir;

        let generator = GambitGenerator::new();
        let (_fixture_temp, project_root) = crate::test_utils::fixture_to_temp("simple");
        let temp_dir = TempDir::new().unwrap();
        let output_dir = temp_dir.path().join("gambit_out");

        let config = GeneratorConfig {
            project_root: project_root.clone(),
            targets: vec![FileTarget {
                file: project_root.join("src/Counter.sol"),
                contracts: vec![],
                functions: vec![],
                forge_args: vec![],
            }],
            operators: vec![],
            output_dir,
            foundry_config: Some(FoundryConfig {
                solc: Some("0.8.30".to_string()),
                ..Default::default()
            }),
            skip_validate: false,
        };

        let result = generator.generate(&config);
        assert!(result.is_ok());
    }
}
