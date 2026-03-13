use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;
use thiserror::Error;

#[derive(Error, Debug)]
pub(crate) enum ConfigError {
    #[error("failed to read foundry.toml: {0}")]
    Read(#[from] std::io::Error),
    #[error("failed to parse foundry.toml: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("invalid dregs.toml: {0}")]
    DregsConfig(String),
}

pub(crate) type Result<T> = std::result::Result<T, ConfigError>;

#[derive(Debug, Clone, Default)]
pub(crate) struct FoundryConfig {
    pub(crate) solc: Option<String>,
    pub(crate) optimizer: bool,
    pub(crate) evm_version: Option<String>,
    pub(crate) via_ir: bool,
    pub(crate) remappings: Vec<String>,
}

#[derive(Deserialize)]
struct FoundryToml {
    profile: Option<HashMap<String, ProfileConfig>>,
}

#[derive(Deserialize, Default, Clone)]
struct ProfileConfig {
    solc: Option<String>,
    optimizer: Option<bool>,
    evm_version: Option<String>,
    via_ir: Option<bool>,
    remappings: Option<Vec<String>>,
}

pub(crate) fn parse_foundry_toml(project_root: &Path) -> Result<Option<FoundryConfig>> {
    let toml_path = project_root.join("foundry.toml");
    if !toml_path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(&toml_path)?;
    let parsed: FoundryToml = toml::from_str(&content)?;

    let profile = parsed
        .profile
        .and_then(|p| p.get("default").cloned())
        .unwrap_or_default();

    Ok(Some(FoundryConfig {
        solc: profile.solc,
        optimizer: profile.optimizer.unwrap_or(false),
        evm_version: profile.evm_version,
        via_ir: profile.via_ir.unwrap_or(false),
        remappings: profile.remappings.unwrap_or_default(),
    }))
}

pub(crate) fn resolve_remappings(project_root: &Path) -> Vec<String> {
    let output = Command::new("forge")
        .arg("remappings")
        .arg("--root")
        .arg(project_root)
        .output();

    match output {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter(|l| !l.is_empty())
            .map(String::from)
            .collect(),
        _ => {
            eprintln!("Warning: failed to resolve remappings via `forge remappings`");
            Vec::new()
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct DregsConfig {
    #[serde(rename = "target")]
    pub(crate) targets: Vec<TargetConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct TargetConfig {
    pub(crate) files: Vec<String>,
    pub(crate) contracts: Option<Vec<String>>,
    pub(crate) functions: Option<Vec<String>>,
    pub(crate) forge_args: Option<Vec<String>>,
}

pub(crate) fn parse_dregs_toml(
    project_root: &Path,
    config_path: Option<&Path>,
) -> Result<Option<DregsConfig>> {
    let toml_path = config_path
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| project_root.join("dregs.toml"));
    if !toml_path.exists() {
        if config_path.is_some() {
            return Err(ConfigError::DregsConfig(format!(
                "config file not found: {}",
                toml_path.display()
            )));
        }
        return Ok(None);
    }

    let content = fs::read_to_string(&toml_path)?;
    let config: DregsConfig =
        toml::from_str(&content).map_err(|e| ConfigError::DregsConfig(e.to_string()))?;

    if config.targets.is_empty() {
        return Err(ConfigError::DregsConfig("no targets defined".to_string()));
    }

    for (i, t) in config.targets.iter().enumerate() {
        if t.files.is_empty() {
            return Err(ConfigError::DregsConfig(format!(
                "target {} has no files",
                i + 1
            )));
        }
    }

    Ok(Some(config))
}

pub(crate) fn find_project_root(file: &Path) -> Option<std::path::PathBuf> {
    let mut dir = if file.is_file() { file.parent()? } else { file };
    loop {
        if dir.join("foundry.toml").exists() {
            return Some(dir.to_path_buf());
        }
        dir = dir.parent()?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::TempDir;
    use assert_fs::prelude::*;

    #[test]
    fn test_parse_foundry_toml_not_found() {
        let temp = TempDir::new().unwrap();
        let result = parse_foundry_toml(temp.path()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_foundry_toml_minimal() {
        let temp = TempDir::new().unwrap();
        temp.child("foundry.toml")
            .write_str(
                r#"[profile.default]
src = "src"
"#,
            )
            .unwrap();

        let config = parse_foundry_toml(temp.path()).unwrap().unwrap();
        assert!(config.solc.is_none());
        assert!(!config.optimizer);
        assert!(!config.via_ir);
        assert!(config.remappings.is_empty());
    }

    #[test]
    fn test_parse_foundry_toml_full() {
        let temp = TempDir::new().unwrap();
        temp.child("foundry.toml")
            .write_str(
                r#"[profile.default]
solc = "0.8.30"
optimizer = true
optimizer_runs = 200
evm_version = "cancun"
via_ir = true
remappings = ["@openzeppelin/=lib/openzeppelin/"]
"#,
            )
            .unwrap();

        let config = parse_foundry_toml(temp.path()).unwrap().unwrap();
        assert_eq!(config.solc, Some("0.8.30".to_string()));
        assert!(config.optimizer);
        assert_eq!(config.evm_version, Some("cancun".to_string()));
        assert!(config.via_ir);
        assert_eq!(config.remappings, vec!["@openzeppelin/=lib/openzeppelin/"]);
    }

    #[test]
    fn test_parse_foundry_toml_no_default_profile() {
        let temp = TempDir::new().unwrap();
        temp.child("foundry.toml")
            .write_str(
                r#"[profile.ci]
optimizer = true
"#,
            )
            .unwrap();

        let config = parse_foundry_toml(temp.path()).unwrap().unwrap();
        assert!(!config.optimizer);
    }

    #[test]
    fn test_find_project_root_from_file() {
        let temp = TempDir::new().unwrap();
        temp.child("foundry.toml")
            .write_str("[profile.default]")
            .unwrap();
        temp.child("src/Contract.sol").touch().unwrap();

        let file = temp.path().join("src/Contract.sol");
        let root = find_project_root(&file).unwrap();
        assert_eq!(root, temp.path());
    }

    #[test]
    fn test_find_project_root_from_nested() {
        let temp = TempDir::new().unwrap();
        temp.child("foundry.toml")
            .write_str("[profile.default]")
            .unwrap();
        temp.child("src/deep/nested/Contract.sol").touch().unwrap();

        let file = temp.path().join("src/deep/nested/Contract.sol");
        let root = find_project_root(&file).unwrap();
        assert_eq!(root, temp.path());
    }

    #[test]
    fn test_find_project_root_not_found() {
        let temp = TempDir::new().unwrap();
        temp.child("src/Contract.sol").touch().unwrap();

        let file = temp.path().join("src/Contract.sol");
        let root = find_project_root(&file);
        assert!(root.is_none());
    }

    #[test]
    fn test_find_project_root_from_directory() {
        let temp = TempDir::new().unwrap();
        temp.child("foundry.toml")
            .write_str("[profile.default]")
            .unwrap();
        temp.child("src").create_dir_all().unwrap();

        let dir = temp.path().join("src");
        let root = find_project_root(&dir).unwrap();
        assert_eq!(root, temp.path());
    }

    #[test]
    fn test_resolve_remappings_failure() {
        let temp = TempDir::new().unwrap();
        // No foundry project -> forge remappings will fail
        let remappings = resolve_remappings(temp.path());
        assert!(remappings.is_empty());
    }

    #[test]
    fn test_resolve_remappings_success() {
        let (_temp, project_root) = crate::test_utils::fixture_to_temp("simple");
        // Verify forge actually succeeds on this fixture (not silently falling through to failure path)
        let output = std::process::Command::new("forge")
            .arg("remappings")
            .arg("--root")
            .arg(&project_root)
            .output()
            .expect("forge must be in PATH");
        assert!(output.status.success(), "forge remappings must succeed");
        // Now test our function takes the success path
        let remappings = resolve_remappings(&project_root);
        // Simple fixture has no lib deps, so result is empty
        assert!(remappings.is_empty());
    }

    #[test]
    fn test_parse_foundry_toml_invalid_toml() {
        let temp = TempDir::new().unwrap();
        temp.child("foundry.toml")
            .write_str("this is not valid { toml }")
            .unwrap();
        let result = parse_foundry_toml(temp.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_dregs_toml_not_found() {
        let temp = TempDir::new().unwrap();
        let result = parse_dregs_toml(temp.path(), None).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_dregs_toml_ok() {
        let temp = TempDir::new().unwrap();
        temp.child("dregs.toml")
            .write_str(
                r#"
[[target]]
files = ["src/A.sol"]
contracts = ["A"]
functions = ["transfer"]
forge_args = ["--match-contract", "ATest"]

[[target]]
files = ["src/B.sol", "src/C.sol"]
"#,
            )
            .unwrap();

        let config = parse_dregs_toml(temp.path(), None).unwrap().unwrap();
        assert_eq!(config.targets.len(), 2);

        let t0 = &config.targets[0];
        assert_eq!(t0.files, vec!["src/A.sol"]);
        assert_eq!(t0.contracts, Some(vec!["A".to_string()]));
        assert_eq!(t0.functions, Some(vec!["transfer".to_string()]));
        assert_eq!(
            t0.forge_args,
            Some(vec!["--match-contract".to_string(), "ATest".to_string()])
        );

        let t1 = &config.targets[1];
        assert_eq!(t1.files, vec!["src/B.sol", "src/C.sol"]);
        assert!(t1.contracts.is_none());
        assert!(t1.functions.is_none());
        assert!(t1.forge_args.is_none());
    }

    #[test]
    fn test_parse_dregs_toml_invalid_toml() {
        let temp = TempDir::new().unwrap();
        temp.child("dregs.toml")
            .write_str("not valid { toml }")
            .unwrap();
        let result = parse_dregs_toml(temp.path(), None);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("invalid dregs.toml"),
            "error should mention invalid dregs.toml"
        );
    }

    #[test]
    fn test_parse_dregs_toml_empty_targets() {
        let temp = TempDir::new().unwrap();
        temp.child("dregs.toml").write_str("target = []\n").unwrap();
        let result = parse_dregs_toml(temp.path(), None);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("no targets defined"),
            "error should mention no targets defined"
        );
    }

    #[test]
    fn test_parse_dregs_toml_minimal_target() {
        let temp = TempDir::new().unwrap();
        temp.child("dregs.toml")
            .write_str(
                r#"
[[target]]
files = ["src/**/*.sol"]
"#,
            )
            .unwrap();

        let config = parse_dregs_toml(temp.path(), None).unwrap().unwrap();
        assert_eq!(config.targets.len(), 1);
        assert_eq!(config.targets[0].files, vec!["src/**/*.sol"]);
        assert!(config.targets[0].contracts.is_none());
        assert!(config.targets[0].functions.is_none());
        assert!(config.targets[0].forge_args.is_none());
    }

    #[test]
    fn test_parse_dregs_toml_explicit_path() {
        let temp = TempDir::new().unwrap();
        let custom = temp.path().join("custom.toml");
        std::fs::write(&custom, "[[target]]\nfiles = [\"src/A.sol\"]\n").unwrap();

        let config = parse_dregs_toml(temp.path(), Some(&custom))
            .unwrap()
            .unwrap();
        assert_eq!(config.targets.len(), 1);
    }

    #[test]
    fn test_parse_dregs_toml_explicit_path_not_found() {
        let temp = TempDir::new().unwrap();
        let missing = temp.path().join("missing.toml");
        let result = parse_dregs_toml(temp.path(), Some(&missing));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("config file not found")
        );
    }

    #[test]
    fn test_parse_dregs_toml_empty_files_in_target() {
        let temp = TempDir::new().unwrap();
        temp.child("dregs.toml")
            .write_str("[[target]]\nfiles = []\n")
            .unwrap();
        let result = parse_dregs_toml(temp.path(), None);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("has no files"));
    }
}
