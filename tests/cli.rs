mod common;

use assert_cmd::Command;
use assert_fs::TempDir;
use predicates::prelude::*;

// cargo_bin is deprecated for custom build-dir support we don't use
#[allow(deprecated)]
fn mutr_cmd() -> Command {
    Command::cargo_bin("mutr").unwrap()
}

#[test]
fn test_help() {
    mutr_cmd().arg("--help").assert().success();
}

#[test]
fn test_run_help() {
    mutr_cmd().arg("run").arg("--help").assert().success();
}

#[test]
fn test_fixture_exists() {
    let path = common::fixture("simple");
    assert!(path.exists());
    assert!(path.join("foundry.toml").exists());
    assert!(path.join("src/Counter.sol").exists());
    assert!(path.join("test/Counter.t.sol").exists());
}

#[test]
fn test_run_simple_project() {
    let test_run = common::TestRun::from_fixture("simple");
    test_run
        .mutr_cmd()
        .assert()
        .success()
        .stdout(predicate::str::contains("Mutation score"));
}

#[test]
fn test_run_with_explicit_files() {
    let test_run = common::TestRun::from_fixture("simple");
    let file_path = test_run.project_path().join("src/Counter.sol");

    test_run
        .mutr_cmd()
        .arg(file_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("Mutation score"));
}

#[test]
fn test_run_with_json_output() {
    let test_run = common::TestRun::from_fixture("simple");
    let temp = TempDir::new().unwrap();
    let output_path = temp.path().join("report.json");

    test_run
        .mutr_cmd()
        .arg("--output")
        .arg(&output_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("Report written to"));

    assert!(output_path.exists());
    let content = std::fs::read_to_string(&output_path).unwrap();
    assert!(content.contains("total_mutants"));
    assert!(content.contains("mutation_score"));
}

#[test]
fn test_run_with_fail_under_passing() {
    let test_run = common::TestRun::from_fixture("simple");

    test_run
        .mutr_cmd()
        .arg("--fail-under")
        .arg("0.0")
        .assert()
        .success();
}

#[test]
fn test_run_with_fail_under_failing() {
    let test_run = common::TestRun::from_fixture("simple");

    test_run
        .mutr_cmd()
        .arg("--fail-under")
        .arg("1.0")
        .assert()
        .failure()
        .stderr(predicate::str::contains("is below threshold"));
}

#[test]
fn test_run_with_invalid_project_path() {
    mutr_cmd()
        .arg("run")
        .arg("--project")
        .arg("/nonexistent/path")
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid project path"));
}

#[test]
fn test_run_with_no_solidity_files() {
    let temp = TempDir::new().unwrap();

    mutr_cmd()
        .arg("run")
        .arg("--project")
        .arg(temp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("no Solidity files found"));
}

#[test]
fn test_run_with_specific_mutations() {
    let test_run = common::TestRun::from_fixture("simple");

    test_run
        .mutr_cmd()
        .arg("--mutations")
        .arg("binary-op-mutation")
        .assert()
        .success()
        .stdout(predicate::str::contains("Mutation score"));
}

#[test]
fn test_run_with_multiple_mutations() {
    let test_run = common::TestRun::from_fixture("simple");

    test_run
        .mutr_cmd()
        .arg("--mutations")
        .arg("binary-op-mutation,require-mutation")
        .assert()
        .success()
        .stdout(predicate::str::contains("Mutation score"));
}

#[test]
fn test_run_with_invalid_file_path() {
    let test_run = common::TestRun::from_fixture("simple");

    test_run
        .mutr_cmd()
        .arg("nonexistent.sol")
        .assert()
        .failure()
        .stderr(predicate::str::contains("failed to resolve file path"));
}

#[test]
fn test_discover_files_in_src_directory() {
    let test_run = common::TestRun::from_fixture("simple");

    test_run
        .mutr_cmd()
        .assert()
        .success()
        .stdout(predicate::str::contains("Counter.sol"));
}

#[test]
fn test_run_with_no_mutants_generated() {
    use assert_fs::prelude::*;

    let temp = TempDir::new().unwrap();
    temp.child("foundry.toml")
        .write_str("[profile.default]\nsrc = \"src\"\ntest = \"test\"\n")
        .unwrap();
    temp.child("src/Empty.sol")
        .write_str("// SPDX-License-Identifier: MIT\npragma solidity ^0.8.0;\n")
        .unwrap();
    temp.child("test").create_dir_all().unwrap();

    mutr_cmd()
        .arg("run")
        .arg("--project")
        .arg(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("No mutants generated"));
}
