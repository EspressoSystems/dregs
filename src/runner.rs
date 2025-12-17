use crate::generator::Mutant;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RunnerError {
    #[error("failed to run tests: {0}")]
    TestExecution(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to copy project: {0}")]
    ProjectCopy(String),
    #[error("failed to apply mutant: {0}")]
    MutantApplication(String),
}

pub type Result<T> = std::result::Result<T, RunnerError>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TestResult {
    pub mutant_id: u32,
    pub killed: bool,
    pub killed_by: Option<String>,
    pub duration: Duration,
}

pub fn run_mutant(mutant: &Mutant, project_root: &Path) -> Result<TestResult> {
    let start = Instant::now();

    let temp_dir = TempDir::new()?;
    let temp_project = temp_dir.path();

    copy_project_to_temp(project_root, temp_project)?;
    apply_mutant_to_project(mutant, project_root, temp_project)?;

    let (killed, killed_by) = run_forge_test(temp_project)?;

    let duration = start.elapsed();

    Ok(TestResult {
        mutant_id: mutant.id,
        killed,
        killed_by,
        duration,
    })
}

fn copy_project_to_temp(source: &Path, dest: &Path) -> Result<()> {
    copy_dir_recursive(source, dest).map_err(|e| RunnerError::ProjectCopy(e.to_string()))
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    if !dst.exists() {
        fs::create_dir_all(dst)?;
    }

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name();
        let file_name_str = file_name.to_string_lossy();

        if file_name_str.starts_with('.') {
            continue;
        }
        if file_name_str == "target"
            || file_name_str == "node_modules"
            || file_name_str == "cache"
            || file_name_str == "out"
            || file_name_str == "gambit_out"
        {
            continue;
        }

        let dest_path = dst.join(&file_name);

        if path.is_dir() {
            copy_dir_recursive(&path, &dest_path)?;
        } else {
            fs::copy(&path, &dest_path)?;
        }
    }

    Ok(())
}

fn apply_mutant_to_project(
    mutant: &Mutant,
    project_root: &Path,
    temp_project: &Path,
) -> Result<()> {
    let mutant_content = fs::read(&mutant.mutant_path).map_err(|e| {
        RunnerError::MutantApplication(format!("failed to read mutant file: {}", e))
    })?;

    let relative_source_path = mutant
        .source_path
        .strip_prefix(project_root)
        .unwrap_or(&mutant.source_path);

    let target_path = temp_project.join(relative_source_path);

    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            RunnerError::MutantApplication(format!("failed to create parent directories: {}", e))
        })?;
    }

    fs::write(&target_path, mutant_content).map_err(|e| {
        RunnerError::MutantApplication(format!("failed to write mutant to target: {}", e))
    })?;

    Ok(())
}

fn run_forge_test(project_root: &Path) -> Result<(bool, Option<String>)> {
    let output = Command::new("forge")
        .arg("test")
        .arg("--json")
        .current_dir(project_root)
        .output()?;

    if output.status.success() {
        return Ok((false, None));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    let killed_by = parse_failed_test_from_output(&stdout, &stderr);

    Ok((true, killed_by))
}

fn parse_failed_test_from_output(stdout: &str, stderr: &str) -> Option<String> {
    for line in stdout.lines().chain(stderr.lines()) {
        if line.contains("Failing tests:") {
            continue;
        }

        if line.contains("[FAIL")
            && let Some(test_name) = extract_test_name_from_fail_line(line)
        {
            return Some(test_name);
        }

        if let Ok(json) = serde_json::from_str::<serde_json::Value>(line)
            && let Some(test_results) = json.get("test_results")
            && let Some(obj) = test_results.as_object()
        {
            for (contract_path, contract_tests) in obj {
                if let Some(tests) = contract_tests.get("test_results")
                    && let Some(tests_obj) = tests.as_object()
                {
                    for (test_name, test_result) in tests_obj {
                        if let Some(status) = test_result.get("status")
                            && status.as_str() == Some("Failure")
                        {
                            let contract_name = extract_contract_name_from_path(contract_path);
                            return Some(format!("{}::{}", contract_name, test_name));
                        }
                    }
                }
            }
        }
    }

    None
}

fn extract_test_name_from_fail_line(line: &str) -> Option<String> {
    if let Some(start) = line.find(']') {
        let after_bracket = &line[start + 1..].trim();
        if let Some(test_end) = after_bracket.find('(') {
            let test_name = after_bracket[..test_end].trim();
            if !test_name.is_empty() {
                return Some(test_name.to_string());
            }
        }
    }
    None
}

fn extract_contract_name_from_path(path: &str) -> String {
    if let Some(colon_pos) = path.rfind(':') {
        return path[colon_pos + 1..].to_string();
    }

    PathBuf::from(path)
        .file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.trim_end_matches(".sol").to_string())
        .unwrap_or_else(|| path.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_copy_dir_recursive_skips_build_artifacts() {
        use assert_fs::TempDir;
        use assert_fs::prelude::*;

        let temp_src = TempDir::new().unwrap();
        temp_src.child("src/Contract.sol").touch().unwrap();
        temp_src.child("test/Test.sol").touch().unwrap();
        temp_src.child("target/build").touch().unwrap();
        temp_src.child("node_modules/pkg").touch().unwrap();
        temp_src.child("cache/data").touch().unwrap();
        temp_src.child("out/abi.json").touch().unwrap();
        temp_src.child("gambit_out/mutants").touch().unwrap();
        temp_src.child(".git/config").touch().unwrap();

        let temp_dst = TempDir::new().unwrap();
        copy_dir_recursive(temp_src.path(), temp_dst.path()).unwrap();

        assert!(temp_dst.child("src/Contract.sol").exists());
        assert!(temp_dst.child("test/Test.sol").exists());
        assert!(!temp_dst.child("target/build").exists());
        assert!(!temp_dst.child("node_modules/pkg").exists());
        assert!(!temp_dst.child("cache/data").exists());
        assert!(!temp_dst.child("out/abi.json").exists());
        assert!(!temp_dst.child("gambit_out/mutants").exists());
        assert!(!temp_dst.child(".git/config").exists());
    }

    #[test]
    fn test_parse_failed_test_from_output() {
        let stdout = "[FAIL. Reason: assertion failed] testIncrement() (gas: 12345)";

        let result = parse_failed_test_from_output(stdout, "");
        assert_eq!(result, Some("testIncrement".to_string()));
    }

    #[test]
    fn test_extract_contract_name_from_path() {
        assert_eq!(
            extract_contract_name_from_path("test/Counter.t.sol:CounterTest"),
            "CounterTest"
        );
        assert_eq!(extract_contract_name_from_path("Counter.sol"), "Counter");
        assert_eq!(
            extract_contract_name_from_path("test/MyTest.t.sol"),
            "MyTest.t"
        );
    }
}
