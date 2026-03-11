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
fn test_run_auto_detect_project_root_from_files() {
    let test_run = common::TestRun::from_fixture("simple");
    let file_path = test_run.project_path().join("src/Counter.sol");

    // Pass files without --project; the default "." won't match since we don't cd there,
    // so resolve_project_root should auto-detect from the file's foundry.toml
    mutr_cmd()
        .arg("run")
        .arg(file_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("Mutation score"));
}

#[test]
fn test_run_different_project_roots_fails() {
    use assert_fs::prelude::*;

    // Create two separate foundry projects
    let temp_a = TempDir::new().unwrap();
    temp_a
        .child("foundry.toml")
        .write_str("[profile.default]\nsrc = \"src\"\n")
        .unwrap();
    temp_a.child("src/A.sol").touch().unwrap();

    let temp_b = TempDir::new().unwrap();
    temp_b
        .child("foundry.toml")
        .write_str("[profile.default]\nsrc = \"src\"\n")
        .unwrap();
    temp_b.child("src/B.sol").touch().unwrap();

    let file_a = temp_a.path().join("src/A.sol");
    let file_b = temp_b.path().join("src/B.sol");

    mutr_cmd()
        .arg("run")
        .arg(&file_a)
        .arg(&file_b)
        .assert()
        .failure()
        .stderr(predicate::str::contains("different project roots"));
}

#[test]
fn test_run_auto_detect_project_root_from_relative_files() {
    let test_run = common::TestRun::from_fixture("simple");

    // Use relative path with current_dir set to the project
    mutr_cmd()
        .current_dir(test_run.project_path())
        .arg("run")
        .arg("src/Counter.sol")
        .assert()
        .success()
        .stdout(predicate::str::contains("Mutation score"));
}

#[test]
fn test_run_different_project_roots_relative_files_fails() {
    use assert_fs::prelude::*;

    // Create two projects in separate subdirectories
    let temp = TempDir::new().unwrap();

    let proj_a = temp.child("proj_a");
    proj_a.create_dir_all().unwrap();
    proj_a
        .child("foundry.toml")
        .write_str("[profile.default]\nsrc = \"src\"\n")
        .unwrap();
    proj_a.child("src/A.sol").touch().unwrap();

    let proj_b = temp.child("proj_b");
    proj_b.create_dir_all().unwrap();
    proj_b
        .child("foundry.toml")
        .write_str("[profile.default]\nsrc = \"src\"\n")
        .unwrap();
    proj_b.child("src/B.sol").touch().unwrap();

    // Use relative paths from the parent temp dir
    mutr_cmd()
        .current_dir(temp.path())
        .arg("run")
        .arg("proj_a/src/A.sol")
        .arg("proj_b/src/B.sol")
        .assert()
        .failure()
        .stderr(predicate::str::contains("different project roots"));
}

#[test]
fn test_run_file_without_project_root_falls_back() {
    use assert_fs::prelude::*;

    // Create a sol file with no foundry.toml anywhere above it
    let temp = TempDir::new().unwrap();
    temp.child("src/Contract.sol")
        .write_str(
            "// SPDX-License-Identifier: MIT\npragma solidity ^0.8.0;\ncontract C { function f() public {} }\n",
        )
        .unwrap();

    // No foundry.toml exists -> find_project_root returns None -> falls back to canonicalize(".")
    // The contract has no mutable operations so gambit generates no mutants.
    mutr_cmd()
        .current_dir(temp.path())
        .arg("run")
        .arg("src/Contract.sol")
        .assert()
        .success()
        .stdout(predicate::str::contains("No mutants generated"));
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
