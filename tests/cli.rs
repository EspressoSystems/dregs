mod common;

use assert_cmd::Command;

#[test]
fn test_help() {
    let mut cmd = Command::cargo_bin("mutr").unwrap();
    cmd.arg("--help").assert().success();
}

#[test]
fn test_run_help() {
    let mut cmd = Command::cargo_bin("mutr").unwrap();
    cmd.arg("run").arg("--help").assert().success();
}

#[test]
fn test_fixture_exists() {
    let path = common::fixture("simple");
    assert!(path.exists());
    assert!(path.join("foundry.toml").exists());
    assert!(path.join("src/Counter.sol").exists());
    assert!(path.join("test/Counter.t.sol").exists());
}
