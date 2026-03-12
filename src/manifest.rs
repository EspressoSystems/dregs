use crate::generator::Mutant;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ManifestError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("missing mutant file: {0}")]
    MissingMutantFile(PathBuf),
}

pub type Result<T> = std::result::Result<T, ManifestError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub version: u32,
    pub mutants: Vec<Mutant>,
}

impl Manifest {
    /// Write manifest and copy mutant files to output directory.
    /// Paths in manifest are stored relative to the output directory.
    pub fn write(output_dir: &Path, mutants: Vec<Mutant>) -> Result<Self> {
        fs::create_dir_all(output_dir)?;

        let mut manifest_mutants = Vec::new();
        for mutant in mutants {
            let rel_mutant_dir = PathBuf::from("mutants").join(mutant.id.to_string());
            let rel_mutant_path = rel_mutant_dir.join(
                mutant
                    .relative_source_path
                    .file_name()
                    .expect("mutant source has no filename"),
            );

            let dest = output_dir.join(&rel_mutant_path);
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&mutant.mutant_path, &dest)?;

            manifest_mutants.push(Mutant {
                mutant_path: rel_mutant_path,
                ..mutant
            });
        }

        let manifest = Manifest {
            version: 1,
            mutants: manifest_mutants,
        };

        let json = serde_json::to_string_pretty(&manifest)?;
        fs::write(output_dir.join("manifest.json"), json)?;

        Ok(manifest)
    }

    /// Read manifest from file and resolve mutant_path relative to manifest's parent dir.
    pub fn read(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let mut manifest: Manifest = serde_json::from_str(&content)?;

        let manifest_dir = path.parent().unwrap_or(Path::new("."));

        for mutant in &mut manifest.mutants {
            let resolved = manifest_dir.join(&mutant.mutant_path);
            if !resolved.exists() {
                return Err(ManifestError::MissingMutantFile(resolved));
            }
            mutant.mutant_path = resolved;
        }

        Ok(manifest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::TempDir;
    use assert_fs::prelude::*;

    fn sample_mutants(mutant_dir: &Path) -> Vec<Mutant> {
        vec![
            Mutant {
                id: 1,
                source_path: PathBuf::from("/project/src/Counter.sol"),
                relative_source_path: PathBuf::from("src/Counter.sol"),
                mutant_path: mutant_dir.join("1.sol"),
                operator: "binary-op-mutation".to_string(),
                original: "+".to_string(),
                replacement: "-".to_string(),
                line: 12,
                forge_args: vec![],
            },
            Mutant {
                id: 2,
                source_path: PathBuf::from("/project/src/Counter.sol"),
                relative_source_path: PathBuf::from("src/Counter.sol"),
                mutant_path: mutant_dir.join("2.sol"),
                operator: "require-mutation".to_string(),
                original: "require(true)".to_string(),
                replacement: "require(false)".to_string(),
                line: 15,
                forge_args: vec![],
            },
        ]
    }

    fn setup_mutant_files(dir: &TempDir) {
        dir.child("1.sol").write_str("mutant 1 content").unwrap();
        dir.child("2.sol").write_str("mutant 2 content").unwrap();
    }

    #[test]
    fn write_ok() {
        let mutant_dir = TempDir::new().unwrap();
        setup_mutant_files(&mutant_dir);
        let output_dir = TempDir::new().unwrap();

        let mutants = sample_mutants(mutant_dir.path());
        let manifest = Manifest::write(output_dir.path(), mutants).unwrap();

        assert!(output_dir.child("manifest.json").exists());
        assert!(output_dir.child("mutants/1/Counter.sol").exists());
        assert!(output_dir.child("mutants/2/Counter.sol").exists());
        assert_eq!(manifest.version, 1);
        assert_eq!(manifest.mutants.len(), 2);
    }

    #[test]
    fn read_ok() {
        let mutant_dir = TempDir::new().unwrap();
        setup_mutant_files(&mutant_dir);
        let output_dir = TempDir::new().unwrap();

        let mutants = sample_mutants(mutant_dir.path());
        Manifest::write(output_dir.path(), mutants).unwrap();

        let manifest_path = output_dir.path().join("manifest.json");
        let loaded = Manifest::read(&manifest_path).unwrap();

        // mutant_path should be resolved to absolute
        for mutant in &loaded.mutants {
            assert!(mutant.mutant_path.is_absolute());
            assert!(mutant.mutant_path.exists());
        }
    }

    #[test]
    fn roundtrip_ok() {
        let mutant_dir = TempDir::new().unwrap();
        setup_mutant_files(&mutant_dir);
        let output_dir = TempDir::new().unwrap();

        let mutants = sample_mutants(mutant_dir.path());
        let written = Manifest::write(output_dir.path(), mutants).unwrap();

        let manifest_path = output_dir.path().join("manifest.json");
        let loaded = Manifest::read(&manifest_path).unwrap();

        assert_eq!(loaded.mutants.len(), written.mutants.len());
        for (w, l) in written.mutants.iter().zip(loaded.mutants.iter()) {
            assert_eq!(w.id, l.id);
            assert_eq!(w.operator, l.operator);
            assert_eq!(w.original, l.original);
            assert_eq!(w.replacement, l.replacement);
            assert_eq!(w.line, l.line);
        }
    }

    #[test]
    fn missing_file_fails() {
        let output_dir = TempDir::new().unwrap();

        let manifest_json = serde_json::json!({
            "version": 1,
            "mutants": [{
                "id": 1,
                "source_path": "/project/src/Counter.sol",
                "relative_source_path": "src/Counter.sol",
                "mutant_path": "mutants/99/Counter.sol",
                "operator": "op",
                "original": "a",
                "replacement": "b",
                "line": 1
            }]
        });

        let manifest_path = output_dir.path().join("manifest.json");
        fs::write(&manifest_path, manifest_json.to_string()).unwrap();

        let result = Manifest::read(&manifest_path);
        pretty_assertions::assert_matches!(result, Err(ManifestError::MissingMutantFile(_)));
    }
}
