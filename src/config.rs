use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("failed to read foundry.toml: {0}")]
    ReadError(#[from] std::io::Error),
    #[error("failed to parse foundry.toml: {0}")]
    ParseError(#[from] toml::de::Error),
}

pub type Result<T> = std::result::Result<T, ConfigError>;

#[derive(Debug, Clone, Default)]
pub struct FoundryConfig {
    pub solc: Option<String>,
    pub optimizer: bool,
    pub evm_version: Option<String>,
    pub via_ir: bool,
    pub remappings: Vec<String>,
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

pub fn parse_foundry_toml(project_root: &Path) -> Result<Option<FoundryConfig>> {
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

pub fn find_project_root(file: &Path) -> Option<std::path::PathBuf> {
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
}
