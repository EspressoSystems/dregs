use crate::config::TestCommand;
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
    #[error("{0}")]
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
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct TestResult {
    pub(crate) mutant_id: u32,
    pub(crate) killed: bool,
    pub(crate) killed_by: Option<String>,
    pub(crate) duration: Duration,
}

/// A temporary copy of the project for running tests.
/// The directory is cleaned up when this struct is dropped.
pub(crate) struct Workspace {
    _temp_dir: TempDir,
    path: PathBuf,
}

impl Workspace {
    pub(crate) fn new(project_root: &Path, symlinks: &[String]) -> Result<Self> {
        let temp_dir = TempDir::new()?;
        let path = temp_dir.path().to_path_buf();
        copy_project_to_temp(project_root, &path, symlinks)?;
        Ok(Self {
            _temp_dir: temp_dir,
            path,
        })
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

pub(crate) fn run_mutant(mutant: &Mutant, project_root: &Path) -> Result<TestResult> {
    let start = Instant::now();

    let symlinks = collect_symlinks(&mutant.test_commands);
    let workspace = Workspace::new(project_root, &symlinks)?;
    let ws = workspace.path();
    apply_mutant_to_project(mutant, ws)?;

    if mutant.test_commands.is_empty() {
        // Backward compat: no test_commands means default foundry with no args
        let forge_result = run_forge_test(ws, &[])?;
        return Ok(TestResult {
            mutant_id: mutant.id,
            killed: forge_result.failed,
            killed_by: forge_result.killed_by,
            duration: start.elapsed(),
        });
    }

    // Run commands in user-specified order. Fail-fast on first kill.
    for cmd in &mutant.test_commands {
        match cmd {
            TestCommand::Foundry { args } => {
                let forge_result = run_forge_test(ws, args)?;
                if forge_result.failed {
                    return Ok(TestResult {
                        mutant_id: mutant.id,
                        killed: true,
                        killed_by: forge_result.killed_by,
                        duration: start.elapsed(),
                    });
                }
            }
            TestCommand::Custom { command, .. } => {
                let killed = run_custom_test(ws, command)?;
                if killed {
                    return Ok(TestResult {
                        mutant_id: mutant.id,
                        killed: true,
                        killed_by: None,
                        duration: start.elapsed(),
                    });
                }
            }
        }
    }

    Ok(TestResult {
        mutant_id: mutant.id,
        killed: false,
        killed_by: None,
        duration: start.elapsed(),
    })
}

pub(crate) fn run_custom_test(project_root: &Path, command: &[String]) -> Result<bool> {
    let output = Command::new(&command[0])
        .args(&command[1..])
        .current_dir(project_root)
        .output()?;

    // Non-zero exit means mutant was killed
    Ok(!output.status.success())
}

pub(crate) fn run_custom_test_baseline(project_root: &Path, command: &[String]) -> Result<()> {
    eprintln!(
        "Running: {} (in {})",
        command.join(" "),
        project_root.display()
    );
    let output = Command::new(&command[0])
        .args(&command[1..])
        .current_dir(project_root)
        .output()?;

    if output.status.success() {
        return Ok(());
    }

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

    let combined = [stdout, stderr]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    Err(RunnerError::TestExecution(combined))
}

fn copy_project_to_temp(source: &Path, dest: &Path, symlinks: &[String]) -> Result<()> {
    let symlink_set: std::collections::HashSet<&str> =
        symlinks.iter().map(|s| s.as_str()).collect();

    copy_dir_impl(source, dest, &symlink_set)
        .map_err(|e| RunnerError::ProjectCopy(e.to_string()))?;

    for name in &symlink_set {
        if !dest.join(name).exists() {
            return Err(RunnerError::ProjectCopy(format!(
                "configured symlink directory not found in project: {name}"
            )));
        }
    }

    Ok(())
}

pub(crate) fn collect_symlinks(test_commands: &[TestCommand]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();
    for cmd in test_commands {
        if let TestCommand::Custom { symlinks, .. } = cmd {
            for s in symlinks {
                if seen.insert(s.clone()) {
                    result.push(s.clone());
                }
            }
        }
    }
    result
}

/// Recursively copy a directory, preserving symlinks.
/// Skips `.git` and `gambit_out` at every level.
/// Top-level entries in `symlink_names` are symlinked instead of copied.
pub(crate) fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    copy_dir_impl(src, dst, &std::collections::HashSet::new())
}

fn copy_dir_impl(
    src: &Path,
    dst: &Path,
    symlink_names: &std::collections::HashSet<&str>,
) -> std::io::Result<()> {
    if !dst.exists() {
        fs::create_dir_all(dst)?;
    }

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        if name_str == ".git" || name_str == "gambit_out" {
            continue;
        }

        let dest_path = dst.join(&name);

        if symlink_names.contains(name_str.as_ref()) {
            std::os::unix::fs::symlink(entry.path(), &dest_path)?;
            continue;
        }

        let metadata = fs::symlink_metadata(entry.path())?;
        if metadata.is_symlink() {
            let target = fs::read_link(entry.path())?;
            std::os::unix::fs::symlink(&target, &dest_path)?;
        } else if metadata.is_dir() {
            // Recursive calls never pass symlink_names (top-level only)
            copy_dir_recursive(&entry.path(), &dest_path)?;
        } else {
            fs::copy(entry.path(), &dest_path)?;
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
    })
}

pub(crate) fn run_forge_test_baseline(project_root: &Path, extra_args: &[String]) -> Result<()> {
    eprintln!("Running: forge test {}", extra_args.join(" "));
    let output = Command::new("forge")
        .arg("test")
        .args(extra_args)
        .current_dir(project_root)
        .output()?;

    if output.status.success() {
        return Ok(());
    }

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

    let combined = [stdout, stderr]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    Err(RunnerError::TestExecution(combined))
}

pub(crate) fn list_forge_tests(project_root: &Path, extra_args: &[String]) -> Result<Vec<String>> {
    eprintln!("Running: forge test --json --list {}", extra_args.join(" "));
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

    let stderr = String::from_utf8_lossy(&output.stderr);
    parse_list_output(&stdout)
        .map_err(|e| RunnerError::TestExecution(format!("{e}\nstdout: {stdout}\nstderr: {stderr}")))
}

fn parse_list_output(stdout: &str) -> std::result::Result<Vec<String>, String> {
    // Find first '{' at the start of a line (or start of string) to skip
    // non-JSON prefix (compilation progress, warnings) that forge may emit.
    let json_start = stdout
        .match_indices('{')
        .find(|&(pos, _)| pos == 0 || stdout.as_bytes()[pos - 1] == b'\n')
        .map(|(pos, _)| pos);
    let json_str = match json_start {
        Some(offset) => &stdout[offset..],
        None => return Err(format!("no JSON object found in forge output: {}", stdout)),
    };
    // Use streaming deserializer to parse only the first JSON value, ignoring
    // trailing content (e.g. RUST_LOG debug output from forge/solar on stdout).
    let mut de = serde_json::Deserializer::from_str(json_str);
    let json = serde_json::Value::deserialize(&mut de)
        .map_err(|e| format!("failed to parse JSON: {e}\ninput: {json_str}"))?;

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
        assert_eq!(err.to_string(), "forge failed");
    }

    #[test]
    fn test_runner_error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
        let err = RunnerError::from(io_err);
        assert!(err.to_string().contains("io error"));
    }

    #[test]
    fn test_copy_dir_recursive_copies_project() {
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
        temp_src.child(".env").touch().unwrap();

        // Add a symlink inside src/
        std::os::unix::fs::symlink("Contract.sol", temp_src.path().join("src/Link.sol")).unwrap();

        let temp_dst = TempDir::new().unwrap();
        copy_dir_recursive(temp_src.path(), temp_dst.path()).unwrap();

        // Copied
        assert!(temp_dst.child("src/Contract.sol").exists());
        assert!(temp_dst.child("test/Test.sol").exists());
        assert!(temp_dst.child("target/build").exists());
        assert!(temp_dst.child("node_modules/pkg").exists());
        assert!(temp_dst.child("cache/data").exists());
        assert!(temp_dst.child("out/abi.json").exists());
        assert!(temp_dst.child(".env").exists());

        // Skipped
        assert!(!temp_dst.child("gambit_out/mutants").exists());
        assert!(!temp_dst.child(".git/config").exists());

        // Symlink preserved
        let link = temp_dst.path().join("src/Link.sol");
        assert!(link.symlink_metadata().unwrap().file_type().is_symlink());
        assert_eq!(
            std::fs::read_link(&link).unwrap(),
            PathBuf::from("Contract.sol")
        );
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
        let result =
            copy_project_to_temp(Path::new("/nonexistent/path"), Path::new("/tmp/dest"), &[]);
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
            test_commands: vec![],
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
            test_commands: vec![],
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
            test_commands: vec![],
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
    fn test_run_forge_test_failing_captures_stdout() {
        let (_temp, project_root) = crate::test_utils::fixture_to_temp("simple");

        // Add a failing test to the working fixture
        let fail_test = project_root.join("test/Fail.t.sol");
        fs::write(
            &fail_test,
            r#"// SPDX-License-Identifier: MIT
pragma solidity ^0.8.30;
import "../src/Counter.sol";
contract FailTest {
    function test_alwaysFails() public pure {
        assert(false);
    }
}
"#,
        )
        .unwrap();

        let err = run_forge_test_baseline(&project_root, &[]).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("test_alwaysFails"),
            "error should contain the failing test name, got: {}",
            msg
        );
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
    fn test_parse_list_output_no_json_at_all() {
        let result = parse_list_output("just some text with no braces");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("no JSON object found"));
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
        assert!(result.unwrap_err().contains("no JSON object found"));
    }

    /// Regression: forge outputs compilation progress to stdout before JSON when
    /// stdout is a pipe (not a TTY). parse_list_output must skip the prefix.
    #[test]
    fn test_parse_list_output_regression_forge_compilation_prefix() {
        // Real-world forge output when compiling before listing
        let output = concat!(
            "Compiling 2 files with Solc 0.8.30\n",
            "Solc 0.8.30 finished in 1.23s\n",
            "Compiler run successful!\n",
            r#"{"test/foundry/SequencerInbox.t.sol":{"SequencerInboxTest":["testAddSequencerL2BatchFromOrigin","testFuzz_addSequencerBatch_FeeToken"]}}"#,
        );
        let result = parse_list_output(output).unwrap();
        assert_eq!(
            result,
            vec![
                "SequencerInboxTest::testAddSequencerL2BatchFromOrigin",
                "SequencerInboxTest::testFuzz_addSequencerBatch_FeeToken",
            ]
        );
    }

    /// Regression: RUST_LOG=debug causes forge/solar to write debug logs to
    /// stdout after the JSON. The streaming deserializer must ignore trailing content.
    #[test]
    fn test_parse_list_output_with_trailing_debug_logs() {
        let output = concat!(
            r#"{"test/Counter.t.sol":{"CounterTest":["testIncrement"]}}"#,
            "\n",
            "2026-03-17T10:19:03.311119Z DEBUG Compiler::drop: solar_sema::compiler: asts_allocated=11.49 MiB\n",
            "2026-03-17T10:19:03.311127Z DEBUG Compiler::drop: solar_sema::compiler: hir_allocated=7.25 KiB\n",
        );
        let result = parse_list_output(output).unwrap();
        assert_eq!(result, vec!["CounterTest::testIncrement"]);
    }

    /// Regression: forge may output both a compilation prefix AND trailing debug
    /// logs when RUST_LOG is set.
    #[test]
    fn test_parse_list_output_with_prefix_and_trailing_logs() {
        let output = concat!(
            "2026-03-17T10:19:02.176239Z DEBUG solar_interface::session: created new session\n",
            r#"{"test/Counter.t.sol":{"CounterTest":["testIncrement"]}}"#,
            "\n",
            "2026-03-17T10:19:03.311119Z DEBUG Compiler::drop: solar_sema::compiler: done\n",
        );
        let result = parse_list_output(output).unwrap();
        assert_eq!(result, vec!["CounterTest::testIncrement"]);
    }

    #[test]
    fn test_parse_list_output_error_includes_input() {
        let result = parse_list_output("{invalid json}");
        let err = result.unwrap_err();
        assert!(err.contains("failed to parse JSON"), "got: {err}");
        assert!(
            err.contains("{invalid json}"),
            "error should include the input, got: {err}"
        );
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
            targets: vec![FileTarget::new(project_root.join("src/Counter.sol"))],
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

    #[test]
    fn test_run_custom_test_success() {
        let temp = tempfile::TempDir::new().unwrap();
        let command = vec!["sh".to_string(), "-c".to_string(), "exit 0".to_string()];
        let killed = run_custom_test(temp.path(), &command).unwrap();
        assert!(!killed, "exit 0 means mutant survived");
    }

    #[test]
    fn test_run_custom_test_failure() {
        let temp = tempfile::TempDir::new().unwrap();
        let command = vec!["sh".to_string(), "-c".to_string(), "exit 1".to_string()];
        let killed = run_custom_test(temp.path(), &command).unwrap();
        assert!(killed, "exit 1 means mutant killed");
    }

    #[test]
    fn test_run_custom_test_baseline_success() {
        let temp = tempfile::TempDir::new().unwrap();
        let command = vec!["sh".to_string(), "-c".to_string(), "exit 0".to_string()];
        let result = run_custom_test_baseline(temp.path(), &command);
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_custom_test_baseline_failure() {
        let temp = tempfile::TempDir::new().unwrap();
        let command = vec![
            "sh".to_string(),
            "-c".to_string(),
            "echo 'test failed'; exit 1".to_string(),
        ];
        let result = run_custom_test_baseline(temp.path(), &command);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("test failed"));
    }

    #[test]
    fn test_copy_project_includes_node_modules() {
        use assert_fs::TempDir;
        use assert_fs::prelude::*;

        let temp_src = TempDir::new().unwrap();
        temp_src.child("src/Contract.sol").touch().unwrap();
        temp_src.child("node_modules/pkg/index.js").touch().unwrap();

        let temp_dst = TempDir::new().unwrap();
        copy_project_to_temp(temp_src.path(), temp_dst.path(), &[]).unwrap();

        // node_modules is copied, not symlinked
        assert!(temp_dst.path().join("node_modules/pkg/index.js").exists());
        assert!(
            !temp_dst
                .path()
                .join("node_modules")
                .symlink_metadata()
                .unwrap()
                .file_type()
                .is_symlink()
        );
    }

    #[test]
    fn test_copy_project_symlinks_specified_dirs() {
        use assert_fs::TempDir;
        use assert_fs::prelude::*;

        let temp_src = TempDir::new().unwrap();
        temp_src.child("src/Contract.sol").touch().unwrap();
        temp_src.child("node_modules/pkg/index.js").touch().unwrap();

        let temp_dst = TempDir::new().unwrap();
        let symlinks = vec!["node_modules".to_string()];
        copy_project_to_temp(temp_src.path(), temp_dst.path(), &symlinks).unwrap();

        // src is copied
        assert!(temp_dst.child("src/Contract.sol").exists());
        // node_modules is symlinked
        let nm = temp_dst.path().join("node_modules");
        assert!(nm.symlink_metadata().unwrap().file_type().is_symlink());
        assert!(nm.join("pkg/index.js").exists());
    }

    #[test]
    fn test_copy_project_symlink_missing_dir_fails() {
        use assert_fs::TempDir;
        use assert_fs::prelude::*;

        let temp_src = TempDir::new().unwrap();
        temp_src.child("src/Contract.sol").touch().unwrap();
        // No node_modules directory

        let temp_dst = TempDir::new().unwrap();
        let symlinks = vec!["node_modules".to_string()];
        let result = copy_project_to_temp(temp_src.path(), temp_dst.path(), &symlinks);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("not found in project")
        );
    }

    #[test]
    fn test_collect_symlinks() {
        use crate::config::TestCommand;
        let cmds = vec![
            TestCommand::Foundry { args: vec![] },
            TestCommand::Custom {
                command: vec!["yarn".to_string(), "test".to_string()],
                symlinks: vec!["node_modules".to_string()],
            },
            TestCommand::Custom {
                command: vec!["make".to_string(), "test".to_string()],
                symlinks: vec!["node_modules".to_string(), ".yarn".to_string()],
            },
        ];
        let result = collect_symlinks(&cmds);
        assert_eq!(
            result,
            vec!["node_modules".to_string(), ".yarn".to_string()]
        );
    }

    #[test]
    fn test_collect_symlinks_empty() {
        use crate::config::TestCommand;
        let cmds = vec![TestCommand::Foundry { args: vec![] }];
        assert!(collect_symlinks(&cmds).is_empty());
    }

    #[test]
    fn test_copy_project_preserves_top_level_symlinks() {
        use assert_fs::TempDir;
        use assert_fs::prelude::*;

        let temp_src = TempDir::new().unwrap();
        temp_src.child("src/Contract.sol").touch().unwrap();
        // Create a top-level symlink (e.g., lib -> some/path)
        let real_dir = TempDir::new().unwrap();
        real_dir.child("forge-std/src/Test.sol").touch().unwrap();
        std::os::unix::fs::symlink(real_dir.path(), temp_src.path().join("lib")).unwrap();

        let temp_dst = TempDir::new().unwrap();
        copy_project_to_temp(temp_src.path(), temp_dst.path(), &[]).unwrap();

        let link = temp_dst.path().join("lib");
        assert!(link.symlink_metadata().unwrap().file_type().is_symlink());
        assert!(link.join("forge-std/src/Test.sol").exists());
    }
}
