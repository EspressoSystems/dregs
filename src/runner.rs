use crate::generator::Mutant;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RunnerError {
    #[error("failed to run tests: {0}")]
    TestExecution(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, RunnerError>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TestResult {
    pub mutant_id: u32,
    pub killed: bool,
    pub killed_by: Option<String>,
    pub duration: Duration,
}

pub fn run_mutant(_mutant: &Mutant, _project_root: &Path) -> Result<TestResult> {
    todo!("Implement test runner")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_test_result_killed() {
        let result = TestResult {
            mutant_id: 1,
            killed: true,
            killed_by: Some("CounterTest::test_increment".to_string()),
            duration: Duration::from_secs(1),
        };
        assert_eq!(result.mutant_id, 1);
        assert!(result.killed);
        assert_eq!(
            result.killed_by,
            Some("CounterTest::test_increment".to_string())
        );
    }

    #[test]
    fn test_test_result_survived() {
        let result = TestResult {
            mutant_id: 2,
            killed: false,
            killed_by: None,
            duration: Duration::from_millis(500),
        };
        assert_eq!(result.mutant_id, 2);
        assert!(!result.killed);
        assert!(result.killed_by.is_none());
    }

    #[test]
    fn test_runner_error_display() {
        let err = RunnerError::TestExecution("forge failed".to_string());
        assert_eq!(err.to_string(), "failed to run tests: forge failed");
    }

    #[test]
    fn test_runner_error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
        let err = RunnerError::from(io_err);
        assert!(err.to_string().contains("io error"));
    }

    #[test]
    #[should_panic(expected = "not yet implemented")]
    fn test_run_mutant_not_implemented() {
        use crate::generator::Mutant;

        let mutant = Mutant {
            id: 1,
            source_path: PathBuf::from("src/Counter.sol"),
            mutant_path: PathBuf::from("gambit_out/mutants/1/Counter.sol"),
            operator: "binary-op-mutation".to_string(),
            original: "+".to_string(),
            replacement: "-".to_string(),
            line: 12,
        };
        let _ = run_mutant(&mutant, Path::new("."));
    }
}
