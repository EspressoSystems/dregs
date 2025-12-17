mod common;

use assert_cmd::Command;

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
