mod common;

use assert_cmd::Command;
use assert_fs::TempDir;
use predicates::prelude::*;

// cargo_bin is deprecated for custom build-dir support we don't use
#[allow(deprecated)]
fn dregs_cmd() -> Command {
    Command::cargo_bin("dregs").unwrap()
}

#[test]
fn test_help() {
    dregs_cmd().arg("--help").assert().success();
}

#[test]
fn test_run_help() {
    dregs_cmd().arg("run").arg("--help").assert().success();
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
        .dregs_cmd()
        .assert()
        .success()
        .stdout(predicate::str::contains("Mutation score"));
}

#[test]
fn test_run_with_explicit_files() {
    let test_run = common::TestRun::from_fixture("simple");
    let file_path = test_run.project_path().join("src/Counter.sol");

    test_run
        .dregs_cmd()
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
        .dregs_cmd()
        .arg("--output")
        .arg(&output_path)
        .assert()
        .success()
        .stderr(predicate::str::contains("Report written to"));

    assert!(output_path.exists());
    let content = std::fs::read_to_string(&output_path).unwrap();
    assert!(content.contains("total_mutants"));
    assert!(content.contains("mutation_score"));
}

#[test]
fn test_run_with_fail_under_passing() {
    let test_run = common::TestRun::from_fixture("simple");

    test_run
        .dregs_cmd()
        .arg("--fail-under")
        .arg("0.0")
        .assert()
        .success();
}

#[test]
fn test_run_with_fail_under_failing() {
    let test_run = common::TestRun::from_fixture("simple");

    test_run
        .dregs_cmd()
        .arg("--fail-under")
        .arg("1.0")
        .assert()
        .failure()
        .stderr(predicate::str::contains("is below threshold"));
}

#[test]
fn test_run_with_invalid_project_path() {
    dregs_cmd()
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

    dregs_cmd()
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
        .dregs_cmd()
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
        .dregs_cmd()
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
        .dregs_cmd()
        .arg("nonexistent.sol")
        .assert()
        .failure()
        .stderr(predicate::str::contains("failed to resolve file path"));
}

#[test]
fn test_discover_files_in_src_directory() {
    let test_run = common::TestRun::from_fixture("simple");

    test_run
        .dregs_cmd()
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
    dregs_cmd()
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

    dregs_cmd()
        .arg("run")
        .arg(&file_a)
        .arg(&file_b)
        .assert()
        .failure()
        .stderr(predicate::str::contains("different project roots"));
}

#[test]
fn test_run_auto_detect_same_root_multiple_files() {
    let test_run = common::TestRun::from_fixture("simple");
    // Create a second sol file in the same project
    std::fs::write(
        test_run.project_path().join("src/Extra.sol"),
        "// SPDX-License-Identifier: MIT\npragma solidity ^0.8.30;\ncontract Extra { function f() public { uint x = 1 + 2; } }\n",
    )
    .unwrap();

    let file1 = test_run.project_path().join("src/Counter.sol");
    let file2 = test_run.project_path().join("src/Extra.sol");

    // Pass two files without --project to exercise the multi-file same-root path
    dregs_cmd()
        .arg("run")
        .arg(&file1)
        .arg(&file2)
        .assert()
        .success()
        .stdout(predicate::str::contains("Mutation score"));
}

#[test]
fn test_run_auto_detect_project_root_from_relative_files() {
    let test_run = common::TestRun::from_fixture("simple");

    // Use relative path with current_dir set to the project
    dregs_cmd()
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
    dregs_cmd()
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
    dregs_cmd()
        .current_dir(temp.path())
        .arg("run")
        .arg("src/Contract.sol")
        .assert()
        .success()
        .stdout(predicate::str::contains("No mutants generated"));
}

#[test]
fn test_run_with_forge_args_shows_matched_tests() {
    let test_run = common::TestRun::from_fixture("simple");

    test_run
        .dregs_cmd()
        .arg("--")
        .arg("--match-test")
        .arg("Increment")
        .assert()
        .success()
        .stderr(predicate::str::contains("Matched"))
        .stderr(predicate::str::contains("CounterTest::test_Increment"));
}

#[test]
fn test_run_with_forge_args_no_match_fails() {
    let test_run = common::TestRun::from_fixture("simple");

    test_run
        .dregs_cmd()
        .arg("--")
        .arg("--match-test")
        .arg("NonexistentTest")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "no tests matched the provided filters",
        ));
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

    dregs_cmd()
        .arg("run")
        .arg("--project")
        .arg(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("No mutants generated"));
}

// --- Help tests for subcommands ---

#[test]
fn test_generate_help() {
    dregs_cmd().arg("generate").arg("--help").assert().success();
}

#[test]
fn test_test_help() {
    dregs_cmd().arg("test").arg("--help").assert().success();
}

#[test]
fn test_report_help() {
    dregs_cmd().arg("report").arg("--help").assert().success();
}

// --- Generate subcommand ---

#[test]
fn test_generate_simple_project() {
    let test_run = common::TestRun::from_fixture("simple");
    let output_dir = test_run.project_path().join("mutants_out");

    dregs_cmd()
        .arg("generate")
        .arg("--project")
        .arg(test_run.project_path())
        .arg("--output")
        .arg(&output_dir)
        .assert()
        .success()
        .stderr(predicate::str::contains("Generated"));

    assert!(output_dir.join("manifest.json").exists());
    let content = std::fs::read_to_string(output_dir.join("manifest.json")).unwrap();
    assert!(content.contains("\"version\": 1"));
    assert!(content.contains("mutants"));
}

#[test]
fn test_generate_with_no_solidity_files() {
    let temp = TempDir::new().unwrap();
    dregs_cmd()
        .arg("generate")
        .arg("--project")
        .arg(temp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("no Solidity files found"));
}

#[test]
fn test_generate_with_invalid_project_path() {
    dregs_cmd()
        .arg("generate")
        .arg("--project")
        .arg("/nonexistent/path")
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid project path"));
}

#[test]
fn test_generate_with_explicit_files() {
    let test_run = common::TestRun::from_fixture("simple");
    let output_dir = test_run.project_path().join("mutants_out");
    let file_path = test_run.project_path().join("src/Counter.sol");

    dregs_cmd()
        .arg("generate")
        .arg("--project")
        .arg(test_run.project_path())
        .arg("--output")
        .arg(&output_dir)
        .arg(file_path)
        .assert()
        .success()
        .stderr(predicate::str::contains("Generated"));
}

#[test]
fn test_generate_no_mutants() {
    use assert_fs::prelude::*;

    let temp = TempDir::new().unwrap();
    temp.child("foundry.toml")
        .write_str("[profile.default]\nsrc = \"src\"\ntest = \"test\"\n")
        .unwrap();
    temp.child("src/Empty.sol")
        .write_str("// SPDX-License-Identifier: MIT\npragma solidity ^0.8.0;\n")
        .unwrap();
    temp.child("test").create_dir_all().unwrap();

    dregs_cmd()
        .arg("generate")
        .arg("--project")
        .arg(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("No mutants generated"));
}

// --- Full generate -> test -> report pipeline ---

#[test]
fn test_generate_test_report_pipeline() {
    let test_run = common::TestRun::from_fixture("simple");
    let mutants_dir = test_run.project_path().join("mutants_out");
    let results_path = test_run.project_path().join("results.json");

    // Generate
    dregs_cmd()
        .arg("generate")
        .arg("--project")
        .arg(test_run.project_path())
        .arg("--output")
        .arg(&mutants_dir)
        .assert()
        .success();

    let manifest_path = mutants_dir.join("manifest.json");
    assert!(manifest_path.exists());

    // Test
    dregs_cmd()
        .arg("test")
        .arg("--manifest")
        .arg(&manifest_path)
        .arg("--project")
        .arg(test_run.project_path())
        .arg("--output")
        .arg(&results_path)
        .assert()
        .success();

    assert!(results_path.exists());
    let results_content = std::fs::read_to_string(&results_path).unwrap();
    assert!(results_content.starts_with('['));

    // Report
    dregs_cmd()
        .arg("report")
        .arg(&manifest_path)
        .arg(&results_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("Mutation score"));
}

// --- Partition tests ---

#[test]
fn test_generate_test_with_partition() {
    let test_run = common::TestRun::from_fixture("simple");
    let mutants_dir = test_run.project_path().join("mutants_out");
    let results1_path = test_run.project_path().join("results1.json");
    let results2_path = test_run.project_path().join("results2.json");

    // Generate
    dregs_cmd()
        .arg("generate")
        .arg("--project")
        .arg(test_run.project_path())
        .arg("--output")
        .arg(&mutants_dir)
        .assert()
        .success();

    let manifest_path = mutants_dir.join("manifest.json");

    // Test partition 1/2
    dregs_cmd()
        .arg("test")
        .arg("--manifest")
        .arg(&manifest_path)
        .arg("--project")
        .arg(test_run.project_path())
        .arg("--partition")
        .arg("slice:1/2")
        .arg("--output")
        .arg(&results1_path)
        .assert()
        .success();

    // Test partition 2/2
    dregs_cmd()
        .arg("test")
        .arg("--manifest")
        .arg(&manifest_path)
        .arg("--project")
        .arg(test_run.project_path())
        .arg("--partition")
        .arg("slice:2/2")
        .arg("--output")
        .arg(&results2_path)
        .assert()
        .success();

    // Report merging both
    dregs_cmd()
        .arg("report")
        .arg(&manifest_path)
        .arg(&results1_path)
        .arg(&results2_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("Mutation score"));
}

// --- Diff-based filtering tests ---

#[test]
fn test_diff_base_generate_test_report_pipeline() {
    let test_run = common::TestRun::from_fixture("simple");
    let project = test_run.project_path();

    // Set up git history: initial commit, then modify one line
    common::init_git_repo(&project);
    common::git_add_commit(&project, "initial");

    // Add a comment on the increment line (changes line but doesn't break tests)
    let counter_path = project.join("src/Counter.sol");
    let content = std::fs::read_to_string(&counter_path).unwrap();
    let modified = content.replace("number = number + 1;", "number = number + 1; // updated");
    std::fs::write(&counter_path, modified).unwrap();
    common::git_add_commit(&project, "add comment to increment");

    let mutants_dir = project.join("mutants_out");
    let results1_path = project.join("results1.json");
    let results2_path = project.join("results2.json");

    // Generate with --diff-base
    dregs_cmd()
        .arg("generate")
        .arg("--project")
        .arg(&project)
        .arg("--diff-base")
        .arg("HEAD~1")
        .arg("--output")
        .arg(&mutants_dir)
        .assert()
        .success()
        .stderr(predicate::str::contains("Diff filter"));

    let manifest_path = mutants_dir.join("manifest.json");
    assert!(manifest_path.exists());

    // Read manifest to verify mutants are only on changed line
    let manifest_content = std::fs::read_to_string(&manifest_path).unwrap();
    let manifest: serde_json::Value = serde_json::from_str(&manifest_content).unwrap();
    let mutants = manifest["mutants"].as_array().unwrap();
    assert!(!mutants.is_empty(), "should have mutants on changed line");
    for m in mutants {
        assert_eq!(
            m["line"].as_u64().unwrap(),
            12,
            "all mutants should be on line 12 (the changed line)"
        );
    }

    // Test with partitions
    dregs_cmd()
        .arg("test")
        .arg("--manifest")
        .arg(&manifest_path)
        .arg("--project")
        .arg(&project)
        .arg("--partition")
        .arg("slice:1/2")
        .arg("--output")
        .arg(&results1_path)
        .assert()
        .success();

    dregs_cmd()
        .arg("test")
        .arg("--manifest")
        .arg(&manifest_path)
        .arg("--project")
        .arg(&project)
        .arg("--partition")
        .arg("slice:2/2")
        .arg("--output")
        .arg(&results2_path)
        .assert()
        .success();

    // Report
    dregs_cmd()
        .arg("report")
        .arg(&manifest_path)
        .arg(&results1_path)
        .arg(&results2_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("Mutation score"));
}

#[test]
fn test_diff_base_no_changes() {
    let test_run = common::TestRun::from_fixture("simple");
    let project = test_run.project_path();

    common::init_git_repo(&project);
    common::git_add_commit(&project, "initial");

    // No changes since HEAD -> should exit cleanly
    dregs_cmd()
        .arg("run")
        .arg("--project")
        .arg(&project)
        .arg("--diff-base")
        .arg("HEAD")
        .assert()
        .success()
        .stdout(predicate::str::contains("Mutation score: 100.0%"));
}

#[test]
fn test_diff_base_run_simple() {
    let test_run = common::TestRun::from_fixture("simple");
    let project = test_run.project_path();

    common::init_git_repo(&project);
    common::git_add_commit(&project, "initial");

    // Add comment to decrement line (changes line 17 without breaking tests,
    // gambit will still mutate the `-` operator)
    let counter_path = project.join("src/Counter.sol");
    let content = std::fs::read_to_string(&counter_path).unwrap();
    let modified = content.replace("number = number - 1;", "number = number - 1; // updated");
    std::fs::write(&counter_path, modified).unwrap();
    common::git_add_commit(&project, "add comment to decrement");

    dregs_cmd()
        .arg("run")
        .arg("--project")
        .arg(&project)
        .arg("--diff-base")
        .arg("HEAD~1")
        .assert()
        .success()
        .stderr(predicate::str::contains("Diff filter"))
        .stdout(predicate::str::contains("Mutation score"));
}

// --- Test subcommand errors ---

#[test]
fn test_test_invalid_manifest() {
    dregs_cmd()
        .arg("test")
        .arg("--manifest")
        .arg("/nonexistent/manifest.json")
        .assert()
        .failure()
        .stderr(predicate::str::contains("failed to read manifest"));
}

#[test]
fn test_test_invalid_partition() {
    let test_run = common::TestRun::from_fixture("simple");
    let mutants_dir = test_run.project_path().join("mutants_out");

    // Generate first
    dregs_cmd()
        .arg("generate")
        .arg("--project")
        .arg(test_run.project_path())
        .arg("--output")
        .arg(&mutants_dir)
        .assert()
        .success();

    dregs_cmd()
        .arg("test")
        .arg("--manifest")
        .arg(mutants_dir.join("manifest.json"))
        .arg("--project")
        .arg(test_run.project_path())
        .arg("--partition")
        .arg("bad_format")
        .assert()
        .failure()
        .stderr(predicate::str::contains("failed to parse partition"));
}

#[test]
fn test_test_with_workers() {
    let test_run = common::TestRun::from_fixture("simple");
    let mutants_dir = test_run.project_path().join("mutants_out");

    dregs_cmd()
        .arg("generate")
        .arg("--project")
        .arg(test_run.project_path())
        .arg("--output")
        .arg(&mutants_dir)
        .assert()
        .success();

    dregs_cmd()
        .arg("test")
        .arg("--manifest")
        .arg(mutants_dir.join("manifest.json"))
        .arg("--project")
        .arg(test_run.project_path())
        .arg("--workers")
        .arg("2")
        .assert()
        .success();
}

#[test]
fn test_test_no_output_prints_to_stdout() {
    let test_run = common::TestRun::from_fixture("simple");
    let mutants_dir = test_run.project_path().join("mutants_out");

    dregs_cmd()
        .arg("generate")
        .arg("--project")
        .arg(test_run.project_path())
        .arg("--output")
        .arg(&mutants_dir)
        .assert()
        .success();

    // Without --output, results go to stdout as JSON
    dregs_cmd()
        .arg("test")
        .arg("--manifest")
        .arg(mutants_dir.join("manifest.json"))
        .arg("--project")
        .arg(test_run.project_path())
        .assert()
        .success()
        .stdout(predicate::str::contains("mutant_id"));
}

#[test]
fn test_test_with_forge_args() {
    let test_run = common::TestRun::from_fixture("simple");
    let mutants_dir = test_run.project_path().join("mutants_out");

    dregs_cmd()
        .arg("generate")
        .arg("--project")
        .arg(test_run.project_path())
        .arg("--output")
        .arg(&mutants_dir)
        .assert()
        .success();

    dregs_cmd()
        .arg("test")
        .arg("--manifest")
        .arg(mutants_dir.join("manifest.json"))
        .arg("--project")
        .arg(test_run.project_path())
        .arg("--")
        .arg("--match-test")
        .arg("Increment")
        .assert()
        .success()
        .stderr(predicate::str::contains("Matched"));
}

// --- Report subcommand errors ---

#[test]
fn test_report_no_result_files() {
    let test_run = common::TestRun::from_fixture("simple");
    let mutants_dir = test_run.project_path().join("mutants_out");

    dregs_cmd()
        .arg("generate")
        .arg("--project")
        .arg(test_run.project_path())
        .arg("--output")
        .arg(&mutants_dir)
        .assert()
        .success();

    dregs_cmd()
        .arg("report")
        .arg(mutants_dir.join("manifest.json"))
        .assert()
        .failure()
        .stderr(predicate::str::contains("failed to merge results"));
}

#[test]
fn test_report_with_fail_under() {
    let test_run = common::TestRun::from_fixture("simple");
    let mutants_dir = test_run.project_path().join("mutants_out");
    let results_path = test_run.project_path().join("results.json");

    // Generate
    dregs_cmd()
        .arg("generate")
        .arg("--project")
        .arg(test_run.project_path())
        .arg("--output")
        .arg(&mutants_dir)
        .assert()
        .success();

    // Test
    dregs_cmd()
        .arg("test")
        .arg("--manifest")
        .arg(mutants_dir.join("manifest.json"))
        .arg("--project")
        .arg(test_run.project_path())
        .arg("--output")
        .arg(&results_path)
        .assert()
        .success();

    // Report with impossible threshold
    dregs_cmd()
        .arg("report")
        .arg(mutants_dir.join("manifest.json"))
        .arg(&results_path)
        .arg("--fail-under")
        .arg("1.0")
        .assert()
        .failure()
        .stderr(predicate::str::contains("is below threshold"));
}

#[test]
fn test_report_with_json_output() {
    let test_run = common::TestRun::from_fixture("simple");
    let mutants_dir = test_run.project_path().join("mutants_out");
    let results_path = test_run.project_path().join("results.json");
    let report_path = test_run.project_path().join("report.json");

    // Generate
    dregs_cmd()
        .arg("generate")
        .arg("--project")
        .arg(test_run.project_path())
        .arg("--output")
        .arg(&mutants_dir)
        .assert()
        .success();

    // Test
    dregs_cmd()
        .arg("test")
        .arg("--manifest")
        .arg(mutants_dir.join("manifest.json"))
        .arg("--project")
        .arg(test_run.project_path())
        .arg("--output")
        .arg(&results_path)
        .assert()
        .success();

    // Report with JSON output
    dregs_cmd()
        .arg("report")
        .arg(mutants_dir.join("manifest.json"))
        .arg(&results_path)
        .arg("--output")
        .arg(&report_path)
        .assert()
        .success()
        .stderr(predicate::str::contains("Report written to"));

    assert!(report_path.exists());
    let content = std::fs::read_to_string(&report_path).unwrap();
    assert!(content.contains("mutation_score"));
}

#[test]
fn test_report_with_markdown_format() {
    let test_run = common::TestRun::from_fixture("simple");
    let mutants_dir = test_run.project_path().join("mutants_out");
    let results_path = test_run.project_path().join("results.json");

    // Generate
    dregs_cmd()
        .arg("generate")
        .arg("--project")
        .arg(test_run.project_path())
        .arg("--output")
        .arg(&mutants_dir)
        .assert()
        .success();

    // Test
    dregs_cmd()
        .arg("test")
        .arg("--manifest")
        .arg(mutants_dir.join("manifest.json"))
        .arg("--project")
        .arg(test_run.project_path())
        .arg("--output")
        .arg(&results_path)
        .assert()
        .success();

    // Report with markdown format
    dregs_cmd()
        .arg("report")
        .arg(mutants_dir.join("manifest.json"))
        .arg(&results_path)
        .arg("--format")
        .arg("markdown")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "| ID | File:Line | Operator | Status | Killed By |",
        ))
        .stdout(predicate::str::contains("**Mutation score:"));
}

#[test]
fn test_report_partial_coverage_warning() {
    let test_run = common::TestRun::from_fixture("simple");
    let mutants_dir = test_run.project_path().join("mutants_out");

    // Generate
    dregs_cmd()
        .arg("generate")
        .arg("--project")
        .arg(test_run.project_path())
        .arg("--output")
        .arg(&mutants_dir)
        .assert()
        .success();

    // Write a partial results file (only one result when there are multiple mutants)
    let results_path = test_run.project_path().join("partial_results.json");
    let partial = serde_json::json!([{
        "mutant_id": 1,
        "killed": true,
        "killed_by": "SomeTest",
        "duration": { "secs": 1, "nanos": 0 }
    }]);
    std::fs::write(&results_path, partial.to_string()).unwrap();

    // Report should warn about partial coverage
    dregs_cmd()
        .arg("report")
        .arg(mutants_dir.join("manifest.json"))
        .arg(&results_path)
        .assert()
        .success()
        .stderr(predicate::str::contains("Warning: results cover"));
}

#[test]
fn test_test_empty_partition_with_output() {
    let test_run = common::TestRun::from_fixture("simple");
    let mutants_dir = test_run.project_path().join("mutants_out");
    let results_path = test_run.project_path().join("empty_results.json");

    // Generate (simple fixture produces a small number of mutants)
    dregs_cmd()
        .arg("generate")
        .arg("--project")
        .arg(test_run.project_path())
        .arg("--output")
        .arg(&mutants_dir)
        .assert()
        .success();

    // Use a very high partition total so this partition index has no mutants
    dregs_cmd()
        .arg("test")
        .arg("--manifest")
        .arg(mutants_dir.join("manifest.json"))
        .arg("--project")
        .arg(test_run.project_path())
        .arg("--partition")
        .arg("slice:100/100")
        .arg("--output")
        .arg(&results_path)
        .assert()
        .success()
        .stderr(predicate::str::contains("No mutants in this partition"));

    // Should write empty JSON array
    assert!(results_path.exists());
    let content = std::fs::read_to_string(&results_path).unwrap();
    assert_eq!(content.trim(), "[]");
}

#[test]
fn test_test_empty_partition_without_output() {
    let test_run = common::TestRun::from_fixture("simple");
    let mutants_dir = test_run.project_path().join("mutants_out");

    dregs_cmd()
        .arg("generate")
        .arg("--project")
        .arg(test_run.project_path())
        .arg("--output")
        .arg(&mutants_dir)
        .assert()
        .success();

    // Empty partition without --output
    dregs_cmd()
        .arg("test")
        .arg("--manifest")
        .arg(mutants_dir.join("manifest.json"))
        .arg("--project")
        .arg(test_run.project_path())
        .arg("--partition")
        .arg("slice:100/100")
        .assert()
        .success()
        .stderr(predicate::str::contains("No mutants in this partition"));
}

#[test]
fn test_run_baseline_failure() {
    // Use the simple fixture, but add a test that always fails
    let test_run = common::TestRun::from_fixture("simple");
    std::fs::write(
        test_run.project_path().join("test/Failing.t.sol"),
        r#"// SPDX-License-Identifier: MIT
pragma solidity ^0.8.30;
contract FailingTest {
    function test_AlwaysFails() public pure {
        assert(false);
    }
}
"#,
    )
    .unwrap();

    dregs_cmd()
        .arg("run")
        .arg("--project")
        .arg(test_run.project_path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("baseline tests failed"));
}

// --- Workers validation ---

#[test]
fn test_run_workers_zero_fails() {
    dregs_cmd()
        .arg("run")
        .arg("--workers")
        .arg("0")
        .assert()
        .failure();
}

#[test]
fn test_test_workers_zero_fails() {
    dregs_cmd()
        .arg("test")
        .arg("--manifest")
        .arg("dummy")
        .arg("--workers")
        .arg("0")
        .assert()
        .failure();
}

#[test]
fn test_run_with_workers() {
    let test_run = common::TestRun::from_fixture("simple");
    test_run
        .dregs_cmd()
        .arg("--workers")
        .arg("2")
        .assert()
        .success()
        .stdout(predicate::str::contains("Mutation score"));
}
