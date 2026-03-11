use crate::config::FoundryConfig;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

pub mod gambit;

#[derive(Error, Debug)]
pub enum GeneratorError {
    #[error("failed to generate mutants: {0}")]
    Generation(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, GeneratorError>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Mutant {
    pub id: u32,
    pub source_path: PathBuf,
    pub relative_source_path: PathBuf,
    pub mutant_path: PathBuf,
    pub operator: String,
    pub original: String,
    pub replacement: String,
    pub line: u32,
}

#[derive(Debug, Clone)]
pub struct GeneratorConfig {
    pub project_root: PathBuf,
    pub files: Vec<PathBuf>,
    pub operators: Vec<String>,
    pub output_dir: PathBuf,
    pub foundry_config: Option<FoundryConfig>,
    pub skip_validate: bool,
}

pub trait MutationGenerator {
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
        };
        assert_eq!(mutant.id, 1);
        assert_eq!(mutant.operator, "binary-op-mutation");
        assert_eq!(mutant.line, 12);
    }

    #[test]
    fn test_generator_config_creation() {
        let config = GeneratorConfig {
            project_root: PathBuf::from("."),
            files: vec![PathBuf::from("src/Counter.sol")],
            operators: vec!["binary-op-mutation".to_string()],
            output_dir: PathBuf::from("gambit_out"),
            foundry_config: None,
            skip_validate: false,
        };
        assert_eq!(config.files.len(), 1);
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
