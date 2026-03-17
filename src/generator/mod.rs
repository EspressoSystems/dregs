use crate::config::{FoundryConfig, TestCommand};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

pub(crate) mod gambit;

#[derive(Error, Debug)]
pub(crate) enum GeneratorError {
    #[error("failed to generate mutants: {0}")]
    Generation(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

pub(crate) type Result<T> = std::result::Result<T, GeneratorError>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct Mutant {
    pub(crate) id: u32,
    pub(crate) source_path: PathBuf,
    pub(crate) relative_source_path: PathBuf,
    pub(crate) mutant_path: PathBuf,
    pub(crate) operator: String,
    pub(crate) original: String,
    pub(crate) replacement: String,
    pub(crate) line: u32,
    #[serde(default)]
    pub(crate) test_commands: Vec<TestCommand>,
}

/// Invariant: `functions` and `exclude_functions` are mutually exclusive (enforced in config parsing).
#[derive(Debug, Clone, Default)]
pub(crate) struct FileTarget {
    pub(crate) file: PathBuf,
    pub(crate) contracts: Vec<String>,
    pub(crate) functions: Vec<String>,
    pub(crate) exclude_functions: Vec<String>,
    pub(crate) test_commands: Vec<TestCommand>,
}

impl FileTarget {
    pub(crate) fn new(file: PathBuf) -> Self {
        Self {
            file,
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct GeneratorConfig {
    pub(crate) project_root: PathBuf,
    pub(crate) targets: Vec<FileTarget>,
    pub(crate) operators: Vec<String>,
    pub(crate) output_dir: PathBuf,
    pub(crate) foundry_config: Option<FoundryConfig>,
    pub(crate) skip_validate: bool,
}

pub(crate) trait MutationGenerator {
    fn generate(&self, config: &GeneratorConfig) -> Result<Vec<Mutant>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mutant_creation() {
        let mutant = Mutant {
            id: 1,
            source_path: PathBuf::from("src/Counter.sol"),
            relative_source_path: PathBuf::from("src/Counter.sol"),
            mutant_path: PathBuf::from("gambit_out/mutants/1/Counter.sol"),
            operator: "binary-op-mutation".to_string(),
            original: "+".to_string(),
            replacement: "-".to_string(),
            line: 12,
            test_commands: vec![],
        };
        assert_eq!(mutant.id, 1);
        assert_eq!(mutant.operator, "binary-op-mutation");
        assert_eq!(mutant.line, 12);
    }

    #[test]
    fn test_generator_config_creation() {
        let config = GeneratorConfig {
            project_root: PathBuf::from("."),
            targets: vec![FileTarget::new(PathBuf::from("src/Counter.sol"))],
            operators: vec!["binary-op-mutation".to_string()],
            output_dir: PathBuf::from("gambit_out"),
            foundry_config: None,
            skip_validate: false,
        };
        assert_eq!(config.targets.len(), 1);
        assert_eq!(config.operators.len(), 1);
    }

    #[test]
    fn test_generator_error_display() {
        let err = GeneratorError::Generation("test error".to_string());
        assert_eq!(err.to_string(), "failed to generate mutants: test error");
    }

    #[test]
    fn test_generator_error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err = GeneratorError::from(io_err);
        assert!(err.to_string().contains("io error"));
    }
}
