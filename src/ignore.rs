use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::generator::Mutant;

#[derive(Error, Debug)]
pub(crate) enum IgnoreError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{path}:{line}: unclosed dregs:ignore-start block")]
    UnclosedBlock { path: String, line: u32 },
    #[error("{path}:{line}: dregs:ignore-end without matching dregs:ignore-start")]
    UnmatchedEnd { path: String, line: u32 },
    #[error(
        "{path}:{line}: nested dregs:ignore-start (previous block started at line {start_line})"
    )]
    NestedStart {
        path: String,
        line: u32,
        start_line: u32,
    },
}

pub(crate) type Result<T> = std::result::Result<T, IgnoreError>;

/// Read a file and return the set of line numbers that should be ignored.
pub(crate) fn ignored_lines(path: &Path) -> Result<HashSet<u32>> {
    let content = fs::read_to_string(path)?;
    let path_str = path.display().to_string();
    let mut ignored = HashSet::new();
    let mut block_start: Option<u32> = None;

    for (idx, line) in content.lines().enumerate() {
        let line_num = idx as u32 + 1;

        if line.contains("dregs:ignore-start") {
            if let Some(start_line) = block_start {
                return Err(IgnoreError::NestedStart {
                    path: path_str,
                    line: line_num,
                    start_line,
                });
            }
            block_start = Some(line_num);
            ignored.insert(line_num);
        } else if line.contains("dregs:ignore-end") {
            if block_start.is_none() {
                return Err(IgnoreError::UnmatchedEnd {
                    path: path_str,
                    line: line_num,
                });
            }
            block_start = None;
            ignored.insert(line_num);
        } else if block_start.is_some() || line.contains("dregs:ignore") {
            // Branch order matters: "dregs:ignore-start"/"dregs:ignore-end" checked above,
            // so this only matches standalone "dregs:ignore" or lines inside a block.
            ignored.insert(line_num);
        }
    }

    if let Some(start_line) = block_start {
        return Err(IgnoreError::UnclosedBlock {
            path: path_str,
            line: start_line,
        });
    }

    Ok(ignored)
}

/// Split mutants into (active, ignored) based on dregs:ignore comments in source files.
pub(crate) fn filter_ignored_mutants(mutants: Vec<Mutant>) -> Result<(Vec<Mutant>, Vec<Mutant>)> {
    let mut cache: HashMap<PathBuf, HashSet<u32>> = HashMap::new();

    // Build cache of ignored lines per unique source file.
    for mutant in &mutants {
        if !cache.contains_key(&mutant.source_path) {
            let lines = ignored_lines(&mutant.source_path)?;
            cache.insert(mutant.source_path.clone(), lines);
        }
    }

    let mut active = Vec::new();
    let mut ignored = Vec::new();

    for mutant in mutants {
        let lines = &cache[&mutant.source_path];
        if lines.contains(&mutant.line) {
            ignored.push(mutant);
        } else {
            active.push(mutant);
        }
    }

    Ok((active, ignored))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use assert_fs::TempDir;
    use assert_fs::prelude::*;

    use super::*;

    fn make_mutant(source_path: PathBuf, line: u32, id: u32) -> Mutant {
        Mutant {
            id,
            source_path: source_path.clone(),
            relative_source_path: source_path,
            mutant_path: PathBuf::from(format!("gambit_out/mutants/{id}/Foo.sol")),
            operator: "binary-op-mutation".to_string(),
            original: "+".to_string(),
            replacement: "-".to_string(),
            line,
            forge_args: vec![],
        }
    }

    #[test]
    fn test_ignored_lines_single_line() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.child("Foo.sol");
        file.write_str(
            "// SPDX-License-Identifier: MIT\n\
             pragma solidity ^0.8.0;\n\
             contract Foo {\n\
             function bar() public { // dregs:ignore\n\
                 x = x + 1;\n\
             }\n\
             }\n",
        )
        .unwrap();

        let lines = ignored_lines(file.path()).unwrap();
        assert_eq!(lines, HashSet::from([4]));
    }

    #[test]
    fn test_ignored_lines_block() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.child("Foo.sol");
        file.write_str(
            "// SPDX-License-Identifier: MIT\n\
             pragma solidity ^0.8.0;\n\
             contract Foo {\n\
             // dregs:ignore-start\n\
             function baz() public {\n\
                 y = y - 1;\n\
             }\n\
             // dregs:ignore-end\n\
             }\n",
        )
        .unwrap();

        let lines = ignored_lines(file.path()).unwrap();
        assert_eq!(lines, HashSet::from([4, 5, 6, 7, 8]));
    }

    #[test]
    fn test_ignored_lines_unclosed_fails() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.child("Foo.sol");
        file.write_str(
            "contract Foo {\n\
             // dregs:ignore-start\n\
             function baz() public {}\n\
             }\n",
        )
        .unwrap();

        let err = ignored_lines(file.path()).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains(":2:"), "expected line 2, got: {msg}");
        assert!(msg.contains("unclosed"));
    }

    #[test]
    fn test_ignored_lines_nested_start_fails() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.child("Foo.sol");
        file.write_str(
            "// dregs:ignore-start\n\
             x = 1;\n\
             // dregs:ignore-start\n\
             y = 2;\n\
             // dregs:ignore-end\n",
        )
        .unwrap();

        let err = ignored_lines(file.path()).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains(":3:"), "expected line 3, got: {msg}");
        assert!(msg.contains("nested"));
        assert!(msg.contains("line 1"));
    }

    #[test]
    fn test_ignored_lines_unmatched_end_fails() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.child("Foo.sol");
        file.write_str(
            "x = 1;\n\
             // dregs:ignore-end\n",
        )
        .unwrap();

        let err = ignored_lines(file.path()).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains(":2:"), "expected line 2, got: {msg}");
        assert!(msg.contains("without matching"));
    }

    #[test]
    fn test_filter_ignored_mutants_ok() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.child("Foo.sol");
        file.write_str(
            "// SPDX-License-Identifier: MIT\n\
             pragma solidity ^0.8.0;\n\
             contract Foo {\n\
             function bar() public { // dregs:ignore\n\
                 x = x + 1;\n\
             }\n\
             // dregs:ignore-start\n\
             function baz() public {\n\
                 y = y - 1;\n\
             }\n\
             // dregs:ignore-end\n\
             }\n",
        )
        .unwrap();

        let path = file.path().to_path_buf();
        let mutants = vec![
            make_mutant(path.clone(), 4, 1),  // ignored (single-line)
            make_mutant(path.clone(), 5, 2),  // active
            make_mutant(path.clone(), 9, 3),  // ignored (block)
            make_mutant(path.clone(), 12, 4), // active
        ];

        let (active, ignored) = filter_ignored_mutants(mutants).unwrap();
        let active_ids: Vec<u32> = active.iter().map(|m| m.id).collect();
        let ignored_ids: Vec<u32> = ignored.iter().map(|m| m.id).collect();
        assert_eq!(active_ids, vec![2, 4]);
        assert_eq!(ignored_ids, vec![1, 3]);
    }

    #[test]
    fn test_filter_ignored_mutants_all_ignored() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.child("Foo.sol");
        file.write_str(
            "// dregs:ignore-start\n\
             x = x + 1;\n\
             y = y - 1;\n\
             // dregs:ignore-end\n",
        )
        .unwrap();

        let path = file.path().to_path_buf();
        let mutants = vec![
            make_mutant(path.clone(), 2, 1),
            make_mutant(path.clone(), 3, 2),
        ];

        let (active, ignored) = filter_ignored_mutants(mutants).unwrap();
        assert!(active.is_empty());
        assert_eq!(ignored.len(), 2);
    }

    #[test]
    fn test_filter_ignored_mutants_multiple_files() {
        let tmp = TempDir::new().unwrap();

        let file_a = tmp.child("A.sol");
        file_a
            .write_str(
                "x = x + 1; // dregs:ignore\n\
                 y = y - 1;\n",
            )
            .unwrap();

        let file_b = tmp.child("B.sol");
        file_b
            .write_str(
                "a = a * 2;\n\
                 b = b / 3;\n",
            )
            .unwrap();

        let mutants = vec![
            make_mutant(file_a.path().to_path_buf(), 1, 1), // ignored
            make_mutant(file_a.path().to_path_buf(), 2, 2), // active
            make_mutant(file_b.path().to_path_buf(), 1, 3), // active
            make_mutant(file_b.path().to_path_buf(), 2, 4), // active
        ];

        let (active, ignored) = filter_ignored_mutants(mutants).unwrap();
        let active_ids: Vec<u32> = active.iter().map(|m| m.id).collect();
        let ignored_ids: Vec<u32> = ignored.iter().map(|m| m.id).collect();
        assert_eq!(active_ids, vec![2, 3, 4]);
        assert_eq!(ignored_ids, vec![1]);
    }
}
