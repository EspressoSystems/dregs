use crate::generator::Mutant;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use thiserror::Error;

#[derive(Error, Debug)]
pub(crate) enum RunnerError {
    #[error("failed to run tests: {0}")]
    TestExecution(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to copy project: {0}")]
    ProjectCopy(String),
    #[error("failed to apply mutant: {0}")]
    MutantApplication(String),
}

pub(crate) type Result<T> = std::result::Result<T, RunnerError>;

#[derive(Debug)]
pub(crate) struct ForgeTestResult {
    pub(crate) failed: bool,
    pub(crate) killed_by: Option<String>,
    pub(crate) stderr: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct TestResult {
    pub(crate) mutant_id: u32,
    pub(crate) killed: bool,
    pub(crate) killed_by: Option<String>,
    pub(crate) duration: Duration,
}

pub(crate) fn run_mutant(mutant: &Mutant, project_root: &Path) -> Result<TestResult> {
    let start = Instant::now();

    let temp_dir = TempDir::new()?;
    let temp_project = temp_dir.path();

    copy_project_to_temp(project_root, temp_project)?;
    apply_mutant_to_project(mutant, temp_project)?;

    let forge_result = run_forge_test(temp_project, &mutant.forge_args)?;
    let killed = forge_result.failed;
    let killed_by = forge_result.killed_by;

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

pub(crate) fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
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

fn apply_mutant_to_project(mutant: &Mutant, temp_project: &Path) -> Result<()> {
    let mutant_content = fs::read(&mutant.mutant_path).map_err(|e| {
        RunnerError::MutantApplication(format!("failed to read mutant file: {}", e))
    })?;

    let target_path = temp_project.join(&mutant.relative_source_path);

    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent).expect("failed to create parent directories");
    }

    fs::write(&target_path, mutant_content).expect("failed to write mutant to target");

    Ok(())
}

pub(crate) fn run_forge_test(
    project_root: &Path,
    extra_args: &[String],
) -> Result<ForgeTestResult> {
    let output = Command::new("forge")
        .arg("test")
        .arg("--json")
        .args(extra_args)
        .current_dir(project_root)
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

    if output.status.success() {
        return Ok(ForgeTestResult {
            failed: false,
            killed_by: None,
            stderr,
        });
    }

    // Check if this is a compilation/setup error vs test failure
    if stderr.contains("Compiler run failed")
        || stderr.contains("could not compile")
        || stderr.contains("Failed to resolve")
    {
        return Err(RunnerError::TestExecution(format!(
            "forge compilation failed:\nstdout: {}\nstderr: {}",
            stdout, stderr
        )));
    }

    let killed_by = parse_failed_test_from_output(&stdout, &stderr);

    Ok(ForgeTestResult {
        failed: true,
        killed_by,
        stderr,
    })
}

pub(crate) fn list_forge_tests(project_root: &Path, extra_args: &[String]) -> Result<Vec<String>> {
    let output = Command::new("forge")
        .arg("test")
        .arg("--json")
        .arg("--list")
        .args(extra_args)
        .current_dir(project_root)
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(RunnerError::TestExecution(format!(
            "forge test --list failed:\nstdout: {}\nstderr: {}",
            stdout, stderr
        )));
    }

    parse_list_output(&stdout).map_err(RunnerError::TestExecution)
}

fn parse_list_output(stdout: &str) -> std::result::Result<Vec<String>, String> {
    let json: serde_json::Value =
        serde_json::from_str(stdout).map_err(|e| format!("failed to parse JSON: {}", e))?;

    let obj = json
        .as_object()
        .ok_or_else(|| "expected JSON object".to_string())?;

    let mut names = Vec::new();
    for (_file_path, contracts) in obj {
        let Some(contracts_obj) = contracts.as_object() else {
            continue;
        };
        for (contract_name, tests) in contracts_obj {
            let Some(tests_arr) = tests.as_array() else {
                continue;
            };
            for test in tests_arr {
                if let Some(test_name) = test.as_str() {
                    names.push(format!("{}::{}", contract_name, test_name));
                }
            }
        }
    }
    if names.is_empty() && !obj.is_empty() {
        return Err("JSON contained entries but no test names could be extracted".to_string());
    }
    names.sort();
    Ok(names)
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

        if let Some(result) = parse_json_test_output(line) {
            return Some(result);
        }
    }

    None
}

fn parse_json_test_output(line: &str) -> Option<String> {
    let json = serde_json::from_str::<serde_json::Value>(line).ok()?;
    let obj = json.as_object()?;

    for (contract_path, contract_data) in obj {
        let tests = contract_data.get("test_results")?.as_object()?;
        for (test_name, test_result) in tests {
            if test_result.get("status")?.as_str() == Some("Failure") {
                let contract_name = extract_contract_name_from_path(contract_path);
                return Some(format!("{}::{}", contract_name, test_name));
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
        .expect("invalid contract path")
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

    #[test]
    fn test_parse_failed_test_from_output_failing_tests_line() {
        let stdout = "Failing tests:\n[FAIL] testSomething() (gas: 100)";
        let result = parse_failed_test_from_output(stdout, "");
        assert_eq!(result, Some("testSomething".to_string()));
    }

    #[test]
    fn test_parse_failed_test_from_output_no_match() {
        let stdout = "All tests passed!";
        let result = parse_failed_test_from_output(stdout, "");
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_failed_test_from_output_json_format() {
        let json_output = r#"{"test/Counter.t.sol:CounterTest":{"test_results":{"testIncrement":{"status":"Failure"}}}}"#;
        let result = parse_failed_test_from_output(json_output, "");
        assert_eq!(result, Some("CounterTest::testIncrement".to_string()));
    }

    #[test]
    fn test_parse_json_test_output_no_failures() {
        let json_output = r#"{"test/Counter.t.sol:CounterTest":{"test_results":{"testIncrement":{"status":"Success"}}}}"#;
        let result = parse_json_test_output(json_output);
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_failed_test_from_output_from_stderr() {
        let result = parse_failed_test_from_output("", "[FAIL] testFromStderr() (gas: 50)");
        assert_eq!(result, Some("testFromStderr".to_string()));
    }

    #[test]
    fn test_extract_test_name_no_bracket() {
        let result = extract_test_name_from_fail_line("no bracket here");
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_test_name_no_paren() {
        let result = extract_test_name_from_fail_line("[FAIL] no_paren");
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_test_name_empty_name() {
        let result = extract_test_name_from_fail_line("[FAIL] ()");
        assert!(result.is_none());
    }

    #[test]
    fn test_copy_project_to_temp_error() {
        let result = copy_project_to_temp(Path::new("/nonexistent/path"), Path::new("/tmp/dest"));
        pretty_assertions::assert_matches!(result, Err(RunnerError::ProjectCopy(_)));
    }

    #[test]
    fn test_apply_mutant_to_project() {
        use assert_fs::TempDir;
        use assert_fs::prelude::*;

        let project_dir = TempDir::new().unwrap();
        project_dir
            .child("src/Counter.sol")
            .write_str("original")
            .unwrap();

        let mutant_dir = TempDir::new().unwrap();
        mutant_dir.child("mutant.sol").write_str("mutated").unwrap();

        let temp_project = TempDir::new().unwrap();
        temp_project.child("src").create_dir_all().unwrap();
        temp_project
            .child("src/Counter.sol")
            .write_str("original")
            .unwrap();

        let mutant = Mutant {
            id: 1,
            source_path: project_dir.path().join("src/Counter.sol"),
            relative_source_path: PathBuf::from("src/Counter.sol"),
            mutant_path: mutant_dir.path().join("mutant.sol"),
            operator: "test".to_string(),
            original: "original".to_string(),
            replacement: "mutated".to_string(),
            line: 1,
            forge_args: vec![],
        };

        let result = apply_mutant_to_project(&mutant, temp_project.path());
        assert!(result.is_ok());

        let content =
            std::fs::read_to_string(temp_project.child("src/Counter.sol").path()).unwrap();
        assert_eq!(content, "mutated");
    }

    #[test]
    fn test_apply_mutant_to_project_missing_mutant_file() {
        use assert_fs::TempDir;

        let temp_project = TempDir::new().unwrap();

        let mutant = Mutant {
            id: 1,
            source_path: PathBuf::from("src/Counter.sol"),
            relative_source_path: PathBuf::from("src/Counter.sol"),
            mutant_path: PathBuf::from("/nonexistent/mutant.sol"),
            operator: "test".to_string(),
            original: "original".to_string(),
            replacement: "mutated".to_string(),
            line: 1,
            forge_args: vec![],
        };

        let result = apply_mutant_to_project(&mutant, temp_project.path());
        pretty_assertions::assert_matches!(result, Err(RunnerError::MutantApplication(_)));
    }

    #[test]
    fn test_apply_mutant_creates_parent_dirs() {
        use assert_fs::TempDir;
        use assert_fs::prelude::*;

        let mutant_dir = TempDir::new().unwrap();
        mutant_dir.child("mutant.sol").write_str("mutated").unwrap();
        let temp_project = TempDir::new().unwrap();

        let mutant = Mutant {
            id: 1,
            source_path: PathBuf::from("deep/nested/Contract.sol"),
            relative_source_path: PathBuf::from("deep/nested/Contract.sol"),
            mutant_path: mutant_dir.path().join("mutant.sol"),
            operator: "test".to_string(),
            original: "original".to_string(),
            replacement: "mutated".to_string(),
            line: 1,
            forge_args: vec![],
        };

        let result = apply_mutant_to_project(&mutant, temp_project.path());
        assert!(result.is_ok());
        assert!(temp_project.child("deep/nested/Contract.sol").exists());
    }

    #[test]
    fn test_runner_error_project_copy_display() {
        let err = RunnerError::ProjectCopy("copy failed".to_string());
        assert_eq!(err.to_string(), "failed to copy project: copy failed");
    }

    #[test]
    fn test_runner_error_mutant_application_display() {
        let err = RunnerError::MutantApplication("apply failed".to_string());
        assert_eq!(err.to_string(), "failed to apply mutant: apply failed");
    }

    #[test]
    fn test_run_forge_test_with_passing_tests() {
        let (_temp, project_root) = crate::test_utils::fixture_to_temp("simple");
        let result = run_forge_test(&project_root, &[]).unwrap();
        assert!(!result.failed);
        assert!(result.killed_by.is_none());
    }

    #[test]
    fn test_run_forge_test_with_failing_tests() {
        use assert_fs::prelude::*;

        let project = assert_fs::TempDir::new().unwrap();

        project
            .child("foundry.toml")
            .write_str(
                r#"[profile.default]
src = "src"
test = "test"
solc = "0.8.30"
"#,
            )
            .unwrap();

        project
            .child("src/Dummy.sol")
            .write_str(
                r#"// SPDX-License-Identifier: MIT
pragma solidity ^0.8.30;
contract Dummy {}
"#,
            )
            .unwrap();

        project
            .child("test/Fail.t.sol")
            .write_str(
                r#"// SPDX-License-Identifier: MIT
pragma solidity ^0.8.30;
contract FailTest {
    function test_fail() public pure {
        assert(false);
    }
}
"#,
            )
            .unwrap();

        let result = run_forge_test(project.path(), &[]).unwrap();
        assert!(result.failed);
    }

    #[test]
    fn test_run_forge_test_compilation_error() {
        use assert_fs::prelude::*;

        let project = assert_fs::TempDir::new().unwrap();

        project
            .child("foundry.toml")
            .write_str(
                r#"[profile.default]
src = "src"
test = "test"
solc = "0.8.30"
"#,
            )
            .unwrap();

        project
            .child("src/Invalid.sol")
            .write_str("this is not valid solidity code")
            .unwrap();

        let result = run_forge_test(project.path(), &[]);
        pretty_assertions::assert_matches!(result, Err(RunnerError::TestExecution(_)));
    }

    #[test]
    fn test_parse_list_output() {
        let json_output = r#"{"test/Counter.t.sol":{"CounterTest":["testIncrement","testDecrement"]},"test/Other.t.sol":{"OtherTest":["testOther"]}}"#;
        let result = parse_list_output(json_output).unwrap();
        assert_eq!(
            result,
            vec![
                "CounterTest::testDecrement",
                "CounterTest::testIncrement",
                "OtherTest::testOther",
            ]
        );
    }

    #[test]
    fn test_parse_list_output_empty() {
        let result = parse_list_output("{}").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_list_output_invalid_json() {
        let result = parse_list_output("not json at all");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_list_output_unexpected_structure_not_object() {
        let json_output = r#"{"test/Counter.t.sol": "not_an_object"}"#;
        let result = parse_list_output(json_output);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("no test names could be extracted")
        );
    }

    #[test]
    fn test_parse_list_output_unexpected_structure_not_array() {
        let json_output = r#"{"test/Counter.t.sol": {"CounterTest": "not_an_array"}}"#;
        let result = parse_list_output(json_output);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("no test names could be extracted")
        );
    }

    #[test]
    fn test_parse_list_output_non_string_items() {
        // Array items that aren't strings should be skipped
        let json_output = r#"{"test/A.sol":{"ATest":[123, null, "testReal"]}}"#;
        let result = parse_list_output(json_output).unwrap();
        assert_eq!(result, vec!["ATest::testReal"]);
    }

    #[test]
    fn test_parse_list_output_top_level_not_object() {
        let result = parse_list_output("[1, 2, 3]");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("expected JSON object"));
    }

    #[test]
    fn test_list_forge_tests_compilation_error() {
        use assert_fs::prelude::*;

        let project = assert_fs::TempDir::new().unwrap();
        project
            .child("foundry.toml")
            .write_str("[profile.default]\nsrc = \"src\"\nsolc = \"0.8.30\"\n")
            .unwrap();
        project
            .child("src/Invalid.sol")
            .write_str("this is not valid solidity")
            .unwrap();

        let result = list_forge_tests(project.path(), &[]);
        pretty_assertions::assert_matches!(result, Err(RunnerError::TestExecution(_)));
    }

    #[test]
    fn test_list_forge_tests_integration() {
        let (_temp, project_root) = crate::test_utils::fixture_to_temp("simple");
        let result = list_forge_tests(&project_root, &[]).unwrap();
        assert!(result.contains(&"CounterTest::test_Increment".to_string()));
        assert!(result.contains(&"CounterTest::test_Decrement".to_string()));
        assert!(result.contains(&"CounterTest::test_SetNumber".to_string()));
    }

    #[test]
    fn test_list_forge_tests_with_filter() {
        let (_temp, project_root) = crate::test_utils::fixture_to_temp("simple");
        let args = vec!["--match-test".to_string(), "Increment".to_string()];
        let result = list_forge_tests(&project_root, &args).unwrap();
        assert_eq!(result, vec!["CounterTest::test_Increment"]);
    }

    #[test]
    fn test_run_forge_test_with_filter() {
        let (_temp, project_root) = crate::test_utils::fixture_to_temp("simple");
        let args = vec!["--match-test".to_string(), "Increment".to_string()];
        let result = run_forge_test(&project_root, &args).unwrap();
        assert!(!result.failed);
    }

    #[test]
    fn test_run_mutant_integration() {
        use crate::generator::gambit::GambitGenerator;
        use crate::generator::{FileTarget, GeneratorConfig, MutationGenerator};
        use tempfile::TempDir;

        let (_fixture_temp, project_root) = crate::test_utils::fixture_to_temp("simple");
        let temp_dir = TempDir::new().unwrap();
        let output_dir = temp_dir.path().join("gambit_out");

        let generator = GambitGenerator::new();
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

        let mutants = generator.generate(&config).unwrap();
        assert!(!mutants.is_empty());

        let result = run_mutant(&mutants[0], &project_root).unwrap();
        assert_eq!(result.mutant_id, 1);
        assert!(result.killed);
        assert!(result.killed_by.as_ref().unwrap().contains("CounterTest"));
    }
}
