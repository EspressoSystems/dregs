use crate::runner::TestResult;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ReportError {
    #[error("failed to generate report: {0}")]
    Generation(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, ReportError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    pub total_mutants: u32,
    pub killed_mutants: u32,
    pub survived_mutants: u32,
    pub mutation_score: f64,
    pub results: Vec<TestResult>,
}

impl Report {
    pub fn new(results: Vec<TestResult>) -> Self {
        let total = results.len() as u32;
        let killed = results.iter().filter(|r| r.killed).count() as u32;
        let survived = total - killed;
        let score = if total > 0 {
            killed as f64 / total as f64
        } else {
            0.0
        };

        Self {
            total_mutants: total,
            killed_mutants: killed,
            survived_mutants: survived,
            mutation_score: score,
            results,
        }
    }

    pub fn print_summary(&self) {
        todo!("Implement summary printing")
    }

    pub fn write_json(&self, _path: &PathBuf) -> Result<()> {
        todo!("Implement JSON output")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_report_creation_all_killed() {
        let results = vec![
            TestResult {
                mutant_id: 1,
                killed: true,
                killed_by: Some("Test1".to_string()),
                duration: Duration::from_secs(1),
            },
            TestResult {
                mutant_id: 2,
                killed: true,
                killed_by: Some("Test2".to_string()),
                duration: Duration::from_secs(1),
            },
        ];

        let report = Report::new(results);
        assert_eq!(report.total_mutants, 2);
        assert_eq!(report.killed_mutants, 2);
        assert_eq!(report.survived_mutants, 0);
        assert_eq!(report.mutation_score, 1.0);
    }

    #[test]
    fn test_report_creation_partial_killed() {
        let results = vec![
            TestResult {
                mutant_id: 1,
                killed: true,
                killed_by: Some("Test1".to_string()),
                duration: Duration::from_secs(1),
            },
            TestResult {
                mutant_id: 2,
                killed: false,
                killed_by: None,
                duration: Duration::from_secs(1),
            },
        ];

        let report = Report::new(results);
        assert_eq!(report.total_mutants, 2);
        assert_eq!(report.killed_mutants, 1);
        assert_eq!(report.survived_mutants, 1);
        assert_eq!(report.mutation_score, 0.5);
    }

    #[test]
    fn test_report_creation_empty() {
        let results = vec![];
        let report = Report::new(results);
        assert_eq!(report.total_mutants, 0);
        assert_eq!(report.killed_mutants, 0);
        assert_eq!(report.survived_mutants, 0);
        assert_eq!(report.mutation_score, 0.0);
    }

    #[test]
    fn test_report_creation_all_survived() {
        let results = vec![
            TestResult {
                mutant_id: 1,
                killed: false,
                killed_by: None,
                duration: Duration::from_millis(100),
            },
            TestResult {
                mutant_id: 2,
                killed: false,
                killed_by: None,
                duration: Duration::from_millis(200),
            },
        ];

        let report = Report::new(results);
        assert_eq!(report.total_mutants, 2);
        assert_eq!(report.killed_mutants, 0);
        assert_eq!(report.survived_mutants, 2);
        assert_eq!(report.mutation_score, 0.0);
    }

    #[test]
    fn test_report_serialization() {
        let results = vec![TestResult {
            mutant_id: 1,
            killed: true,
            killed_by: Some("Test1".to_string()),
            duration: Duration::from_secs(1),
        }];

        let report = Report::new(results);
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("total_mutants"));
        assert!(json.contains("mutation_score"));
    }

    #[test]
    fn test_report_error_display() {
        let err = ReportError::Generation("write failed".to_string());
        assert_eq!(err.to_string(), "failed to generate report: write failed");
    }

    #[test]
    fn test_report_error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
        let err = ReportError::from(io_err);
        assert!(err.to_string().contains("io error"));
    }

    #[test]
    #[should_panic(expected = "not yet implemented")]
    fn test_print_summary_not_implemented() {
        let report = Report::new(vec![]);
        report.print_summary();
    }

    #[test]
    #[should_panic(expected = "not yet implemented")]
    fn test_write_json_not_implemented() {
        let report = Report::new(vec![]);
        let _ = report.write_json(&PathBuf::from("test.json"));
    }
}
