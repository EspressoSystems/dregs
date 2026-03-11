use crate::generator::Mutant;
use crate::runner::TestResult;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
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

    /// Merge multiple partial result files into combined results.
    pub fn merge(result_files: &[PathBuf]) -> Result<Vec<TestResult>> {
        if result_files.is_empty() {
            return Err(ReportError::Generation(
                "no result files provided".to_string(),
            ));
        }

        let mut all_results = Vec::new();
        let mut seen_ids = HashSet::new();

        for file in result_files {
            let content = fs::read_to_string(file)
                .map_err(|e| ReportError::Generation(format!("{}: {}", file.display(), e)))?;
            let results: Vec<TestResult> = serde_json::from_str(&content)
                .map_err(|e| ReportError::Generation(format!("{}: {}", file.display(), e)))?;
            for result in results {
                if !seen_ids.insert(result.mutant_id) {
                    return Err(ReportError::Generation(format!(
                        "duplicate mutant_id {} in {}",
                        result.mutant_id,
                        file.display()
                    )));
                }
                all_results.push(result);
            }
        }

        all_results.sort_by_key(|r| r.mutant_id);
        Ok(all_results)
    }

    pub fn print_summary(&self, mutants: &[Mutant]) {
        self.write_summary(&mut std::io::stdout(), mutants)
            .expect("failed to write summary to stdout");
    }

    pub fn write_summary(&self, w: &mut impl Write, mutants: &[Mutant]) -> std::io::Result<()> {
        let mutant_map: HashMap<u32, &Mutant> = mutants.iter().map(|m| (m.id, m)).collect();

        for result in &self.results {
            let status = if result.killed {
                format!(
                    "KILLED by {}",
                    result.killed_by.as_ref().unwrap_or(&"unknown".to_string())
                )
            } else {
                "SURVIVED".to_string()
            };

            if let Some(mutant) = mutant_map.get(&result.mutant_id) {
                writeln!(
                    w,
                    "[{}/{}] {}:{} {}: {}",
                    result.mutant_id,
                    self.total_mutants,
                    mutant.source_path.display(),
                    mutant.line,
                    mutant.operator,
                    status
                )?;
            } else {
                writeln!(
                    w,
                    "[{}/{}]: {}",
                    result.mutant_id, self.total_mutants, status
                )?;
            }
        }

        writeln!(
            w,
            "Mutation score: {}/{} ({:.0}%)",
            self.killed_mutants,
            self.total_mutants,
            self.mutation_score * 100.0
        )?;

        if self.survived_mutants > 0 {
            writeln!(w, "Surviving mutants:")?;
            for result in &self.results {
                if !result.killed
                    && let Some(mutant) = mutant_map.get(&result.mutant_id)
                {
                    writeln!(
                        w,
                        "  [{}] {}:{} {}",
                        result.mutant_id,
                        mutant.source_path.display(),
                        mutant.line,
                        mutant.operator
                    )?;
                    writeln!(w, "     `{}` -> `{}`", mutant.original, mutant.replacement)?;
                }
            }
        }

        Ok(())
    }

    pub fn write_json(&self, path: &Path) -> Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        fs::write(path, json)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator::Mutant;
    use std::time::Duration;

    fn sample_mixed_results() -> (Vec<TestResult>, Vec<Mutant>) {
        let results = vec![
            TestResult {
                mutant_id: 1,
                killed: true,
                killed_by: Some("CounterTest::test_increment".to_string()),
                duration: Duration::from_secs(1),
            },
            TestResult {
                mutant_id: 2,
                killed: false,
                killed_by: None,
                duration: Duration::from_millis(500),
            },
        ];

        let mutants = vec![
            Mutant {
                id: 1,
                source_path: PathBuf::from("src/Counter.sol"),
                relative_source_path: PathBuf::from("src/Counter.sol"),
                mutant_path: PathBuf::from("gambit_out/mutants/1/Counter.sol"),
                operator: "binary-op-mutation".to_string(),
                original: "+".to_string(),
                replacement: "-".to_string(),
                line: 12,
            },
            Mutant {
                id: 2,
                source_path: PathBuf::from("src/Counter.sol"),
                relative_source_path: PathBuf::from("src/Counter.sol"),
                mutant_path: PathBuf::from("gambit_out/mutants/2/Counter.sol"),
                operator: "require-mutation".to_string(),
                original: "require(true)".to_string(),
                replacement: "require(false)".to_string(),
                line: 15,
            },
        ];

        (results, mutants)
    }

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
    fn test_write_summary_empty() {
        let report = Report::new(vec![]);
        let mut output = Vec::new();
        report.write_summary(&mut output, &[]).unwrap();
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("Mutation score: 0/0"));
    }

    #[test]
    fn test_write_json_success() {
        use assert_fs::TempDir;

        let results = vec![TestResult {
            mutant_id: 1,
            killed: true,
            killed_by: Some("Test1".to_string()),
            duration: Duration::from_secs(1),
        }];

        let report = Report::new(results);
        let temp = TempDir::new().unwrap();
        let output_path = temp.path().join("report.json");

        let result = report.write_json(&output_path);
        assert!(result.is_ok());
        assert!(output_path.exists());

        let content = std::fs::read_to_string(&output_path).unwrap();
        assert!(content.contains("total_mutants"));
        assert!(content.contains("mutation_score"));
    }

    #[test]
    fn test_write_json_io_error() {
        let report = Report::new(vec![]);
        let result = report.write_json(&PathBuf::from("/nonexistent/path/report.json"));
        pretty_assertions::assert_matches!(result, Err(ReportError::Io(_)));
    }

    #[test]
    fn test_survivor_diff_in_summary() {
        let (results, mutants) = sample_mixed_results();
        let report = Report::new(results);

        let mut output = Vec::new();
        report.write_summary(&mut output, &mutants).unwrap();
        let output = String::from_utf8(output).unwrap();

        assert!(output.contains("Surviving mutants:"));
        assert!(output.contains("src/Counter.sol:15 require-mutation"));
        assert!(output.contains("`require(true)` -> `require(false)`"));
        assert!(!output.contains("`+` -> `-`"));
    }

    #[test]
    fn test_no_survivors_no_diff_section() {
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

        let mutants = vec![
            Mutant {
                id: 1,
                source_path: PathBuf::from("src/A.sol"),
                relative_source_path: PathBuf::from("src/A.sol"),
                mutant_path: PathBuf::from("gambit_out/mutants/1/A.sol"),
                operator: "op1".to_string(),
                original: "a".to_string(),
                replacement: "b".to_string(),
                line: 1,
            },
            Mutant {
                id: 2,
                source_path: PathBuf::from("src/B.sol"),
                relative_source_path: PathBuf::from("src/B.sol"),
                mutant_path: PathBuf::from("gambit_out/mutants/2/B.sol"),
                operator: "op2".to_string(),
                original: "c".to_string(),
                replacement: "d".to_string(),
                line: 2,
            },
        ];

        let report = Report::new(results);

        let mut output = Vec::new();
        report.write_summary(&mut output, &mutants).unwrap();
        let output = String::from_utf8(output).unwrap();

        assert!(!output.contains("Surviving mutants:"));
    }

    #[test]
    fn test_write_summary_killed_without_test_name() {
        let results = vec![TestResult {
            mutant_id: 1,
            killed: true,
            killed_by: None,
            duration: Duration::from_secs(1),
        }];

        let mutants = vec![Mutant {
            id: 1,
            source_path: PathBuf::from("src/Test.sol"),
            relative_source_path: PathBuf::from("src/Test.sol"),
            mutant_path: PathBuf::from("gambit_out/mutants/1/Test.sol"),
            operator: "test-op".to_string(),
            original: "old".to_string(),
            replacement: "new".to_string(),
            line: 5,
        }];

        let report = Report::new(results);
        let mut output = Vec::new();
        report.write_summary(&mut output, &mutants).unwrap();
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("KILLED by unknown"));
    }

    #[test]
    fn test_write_summary_io_error() {
        let (results, mutants) = sample_mixed_results();
        let report = Report::new(results);
        let mut buf = [0u8; 0];
        let result = report.write_summary(&mut buf.as_mut_slice(), &mutants);
        assert!(result.is_err());
    }

    #[test]
    fn test_merge_ok() {
        use assert_fs::TempDir;

        let dir = TempDir::new().unwrap();

        let file1 = dir.path().join("part1.json");
        let file2 = dir.path().join("part2.json");

        let results1 = vec![TestResult {
            mutant_id: 3,
            killed: true,
            killed_by: Some("Test1".to_string()),
            duration: Duration::from_secs(1),
        }];
        let results2 = vec![TestResult {
            mutant_id: 1,
            killed: false,
            killed_by: None,
            duration: Duration::from_millis(500),
        }];

        std::fs::write(&file1, serde_json::to_string(&results1).unwrap()).unwrap();
        std::fs::write(&file2, serde_json::to_string(&results2).unwrap()).unwrap();

        let merged = Report::merge(&[file1, file2]).unwrap();
        assert_eq!(merged.len(), 2);
        // Sorted by mutant_id
        assert_eq!(merged[0].mutant_id, 1);
        assert_eq!(merged[1].mutant_id, 3);
    }

    #[test]
    fn test_write_summary_unknown_mutant_id() {
        let results = vec![TestResult {
            mutant_id: 999,
            killed: true,
            killed_by: Some("SomeTest".to_string()),
            duration: Duration::from_secs(1),
        }];

        let report = Report::new(results);
        let mut output = Vec::new();
        // Pass empty mutants so mutant_map won't find id 999
        report.write_summary(&mut output, &[]).unwrap();
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("[999/1]: KILLED"));
    }

    #[test]
    fn test_merge_no_files_fails() {
        let result = Report::merge(&[]);
        pretty_assertions::assert_matches!(result, Err(ReportError::Generation(_)));
    }

    #[test]
    fn test_merge_duplicate_ids_fails() {
        use assert_fs::TempDir;

        let dir = TempDir::new().unwrap();

        let file1 = dir.path().join("part1.json");
        let file2 = dir.path().join("part2.json");

        let results = vec![TestResult {
            mutant_id: 1,
            killed: true,
            killed_by: Some("Test1".to_string()),
            duration: Duration::from_secs(1),
        }];

        let json = serde_json::to_string(&results).unwrap();
        std::fs::write(&file1, &json).unwrap();
        std::fs::write(&file2, &json).unwrap();

        let result = Report::merge(&[file1, file2]);
        pretty_assertions::assert_matches!(result, Err(ReportError::Generation(_)));
    }
}
