use super::{GeneratorConfig, Mutant, MutationGenerator, Result};
use gambit::{MutateParams, run_mutate};

pub struct GambitGenerator;

impl GambitGenerator {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GambitGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl MutationGenerator for GambitGenerator {
    fn generate(&self, config: &GeneratorConfig) -> Result<Vec<Mutant>> {
        let mut mutate_params_list = Vec::new();

        for file in &config.files {
            let params = MutateParams {
                json: None,
                filename: Some(file.to_string_lossy().to_string()),
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
                solc: "solc".to_string(),
                solc_optimize: false,
                solc_evm_version: None,
                functions: None,
                contract: None,
                solc_base_path: None,
                solc_allow_paths: None,
                solc_include_path: None,
                solc_remappings: None,
                skip_validate: false,
            };
            mutate_params_list.push(params);
        }

        let gambit_results = run_mutate(mutate_params_list).expect("gambit run_mutate failed");

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
                });

                mutant_id += 1;
            }
        }

        Ok(mutants)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

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
            files: vec![],
            operators: vec![],
            output_dir: PathBuf::from("gambit_out"),
        };
        let result = generator.generate(&config);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[test]
    fn test_generate_with_solidity_file() {
        use tempfile::TempDir;

        let generator = GambitGenerator::new();
        let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/simple");
        let temp_dir = TempDir::new().unwrap();
        let output_dir = temp_dir.path().join("gambit_out");

        let config = GeneratorConfig {
            project_root: project_root.clone(),
            files: vec![project_root.join("src/Counter.sol")],
            operators: vec![],
            output_dir,
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
        let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/simple");
        let temp_dir = TempDir::new().unwrap();
        let output_dir = temp_dir.path().join("gambit_out");

        let config = GeneratorConfig {
            project_root: project_root.clone(),
            files: vec![project_root.join("src/Counter.sol")],
            operators: vec!["binary-op-mutation".to_string()],
            output_dir,
        };

        let result = generator.generate(&config);
        assert!(result.is_ok());
    }
}
