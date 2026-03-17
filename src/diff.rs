use std::io::Read;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::process::Command;

use thiserror::Error;

use crate::generator::{FileTarget, Mutant};

#[derive(Error, Debug)]
pub(crate) enum DiffError {
    #[error("failed to run git diff: {0}")]
    GitCommand(String),
    #[error("failed to parse diff output: {0}")]
    Parse(String),
    #[error("failed to read diff input: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DiffRange {
    pub(crate) file: PathBuf,
    pub(crate) lines: Vec<Range<u32>>,
}

pub(crate) fn parse_git_diff(
    project_root: &Path,
    base_ref: &str,
) -> Result<Vec<DiffRange>, DiffError> {
    let output = Command::new("git")
        .args([
            "diff",
            &format!("{base_ref}...HEAD"),
            "--unified=0",
            "--",
            "*.sol",
        ])
        .current_dir(project_root)
        .output()
        .map_err(|e| DiffError::GitCommand(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(DiffError::GitCommand(stderr.into_owned()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_diff_output(&stdout)
}

pub(crate) fn parse_diff_from_reader(mut reader: impl Read) -> Result<Vec<DiffRange>, DiffError> {
    let mut buf = String::new();
    reader.read_to_string(&mut buf)?;
    parse_diff_output(&buf)
}

pub(crate) fn parse_diff_output(output: &str) -> Result<Vec<DiffRange>, DiffError> {
    let mut ranges: Vec<DiffRange> = Vec::new();
    let mut current_file: Option<PathBuf> = None;

    for line in output.lines() {
        if let Some(path) = line.strip_prefix("+++ b/") {
            current_file = Some(PathBuf::from(path));
        } else if line == "+++ /dev/null" {
            // Deleted file, no additions possible.
            current_file = None;
        } else if line.starts_with("@@ ") {
            let Some(ref file) = current_file else {
                continue;
            };

            let hunk = parse_hunk_header(line)?;
            if hunk.count == 0 {
                continue;
            }

            let range = hunk.start..(hunk.start + hunk.count);

            // Append to existing DiffRange for this file or create a new one.
            if let Some(existing) = ranges.iter_mut().find(|r| r.file == *file) {
                existing.lines.push(range);
            } else {
                ranges.push(DiffRange {
                    file: file.clone(),
                    lines: vec![range],
                });
            }
        }
    }

    Ok(ranges)
}

struct HunkAdd {
    start: u32,
    count: u32,
}

fn parse_hunk_header(line: &str) -> Result<HunkAdd, DiffError> {
    // Format: @@ -a[,b] +c[,d] @@
    let header = line
        .strip_prefix("@@ ")
        .and_then(|s| s.split(" @@").next())
        .ok_or_else(|| DiffError::Parse(format!("invalid hunk header: {line}")))?;

    let plus_part = header
        .split_whitespace()
        .find(|s| s.starts_with('+'))
        .ok_or_else(|| DiffError::Parse(format!("no + range in hunk: {line}")))?;

    let nums = &plus_part[1..]; // strip leading '+'
    let (start, count) = if let Some((s, c)) = nums.split_once(',') {
        (parse_u32(s, line)?, parse_u32(c, line)?)
    } else {
        (parse_u32(nums, line)?, 1)
    };

    Ok(HunkAdd { start, count })
}

fn parse_u32(s: &str, context: &str) -> Result<u32, DiffError> {
    s.parse()
        .map_err(|_| DiffError::Parse(format!("invalid number '{s}' in: {context}")))
}

pub(crate) fn filter_mutants(mutants: Vec<Mutant>, diff_ranges: &[DiffRange]) -> Vec<Mutant> {
    mutants
        .into_iter()
        .filter(|m| {
            diff_ranges.iter().any(|dr| {
                dr.file == m.relative_source_path && dr.lines.iter().any(|r| r.contains(&m.line))
            })
        })
        .collect()
}

pub(crate) fn filter_targets_by_diff(
    targets: Vec<FileTarget>,
    diff_ranges: &[DiffRange],
    project_root: &Path,
) -> Vec<FileTarget> {
    targets
        .into_iter()
        .filter(|t| {
            let relative = t.file.strip_prefix(project_root).unwrap_or(&t.file);
            diff_ranges.iter().any(|dr| dr.file == relative)
        })
        .collect()
}

#[cfg(test)]
#[allow(clippy::single_range_in_vec_init)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn make_mutant(id: u32, path: &str, line: u32) -> Mutant {
        Mutant {
            id,
            source_path: PathBuf::from(path),
            relative_source_path: PathBuf::from(path),
            mutant_path: PathBuf::from(format!("gambit_out/mutants/{id}/file.sol")),
            operator: "binary-op-mutation".to_string(),
            original: "+".to_string(),
            replacement: "-".to_string(),
            line,
            test_commands: vec![],
        }
    }

    fn make_target(path: &str) -> FileTarget {
        FileTarget::new(PathBuf::from(path))
    }

    // --- parse_diff_output tests ---

    #[test]
    fn test_parse_multi_hunk_multi_file() {
        let diff = "\
diff --git a/src/Counter.sol b/src/Counter.sol
index abc1234..def5678 100644
--- a/src/Counter.sol
+++ b/src/Counter.sol
@@ -10,3 +10,5 @@ contract Counter {
@@ -20,0 +22,3 @@ contract Counter {
diff --git a/src/Token.sol b/src/Token.sol
index 1111111..2222222 100644
--- a/src/Token.sol
+++ b/src/Token.sol
@@ -5,2 +5,4 @@ contract Token {
";
        let ranges = parse_diff_output(diff).unwrap();
        assert_eq!(ranges.len(), 2);

        assert_eq!(ranges[0].file, PathBuf::from("src/Counter.sol"));
        assert_eq!(ranges[0].lines, vec![10..15, 22..25]);

        assert_eq!(ranges[1].file, PathBuf::from("src/Token.sol"));
        assert_eq!(ranges[1].lines, vec![5..9]);
    }

    #[test]
    fn test_parse_single_line_change() {
        let diff = "\
diff --git a/src/Counter.sol b/src/Counter.sol
--- a/src/Counter.sol
+++ b/src/Counter.sol
@@ -5 +5 @@ contract Counter {
";
        let ranges = parse_diff_output(diff).unwrap();
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].lines, vec![5..6]);
    }

    #[test]
    fn test_parse_new_file() {
        let diff = "\
diff --git a/src/New.sol b/src/New.sol
new file mode 100644
--- /dev/null
+++ b/src/New.sol
@@ -0,0 +1,20 @@
";
        let ranges = parse_diff_output(diff).unwrap();
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].file, PathBuf::from("src/New.sol"));
        assert_eq!(ranges[0].lines, vec![1..21]);
    }

    #[test]
    fn test_parse_deleted_file() {
        let diff = "\
diff --git a/src/Old.sol b/src/Old.sol
deleted file mode 100644
--- a/src/Old.sol
+++ /dev/null
@@ -1,10 +0,0 @@
";
        let ranges = parse_diff_output(diff).unwrap();
        assert_eq!(ranges.len(), 0);
    }

    #[test]
    fn test_parse_empty_diff() {
        let ranges = parse_diff_output("").unwrap();
        assert_eq!(ranges.len(), 0);
    }

    #[test]
    fn test_parse_pure_deletion_hunk() {
        let diff = "\
diff --git a/src/Counter.sol b/src/Counter.sol
--- a/src/Counter.sol
+++ b/src/Counter.sol
@@ -5,3 +5,0 @@ contract Counter {
@@ -15,2 +12,4 @@ contract Counter {
";
        let ranges = parse_diff_output(diff).unwrap();
        assert_eq!(ranges.len(), 1);
        // First hunk (+5,0) is skipped, only second hunk remains.
        assert_eq!(ranges[0].lines, vec![12..16]);
    }

    #[test]
    fn test_parse_rename_header() {
        let diff = "\
diff --git a/src/OldName.sol b/src/NewName.sol
similarity index 90%
rename from src/OldName.sol
rename to src/NewName.sol
--- a/src/OldName.sol
+++ b/src/NewName.sol
@@ -3,2 +3,4 @@ contract Foo {
";
        let ranges = parse_diff_output(diff).unwrap();
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].file, PathBuf::from("src/NewName.sol"));
        assert_eq!(ranges[0].lines, vec![3..7]);
    }

    // --- filter_mutants tests ---

    #[test]
    fn test_filter_mutants_on_changed_lines() {
        let mutants = vec![
            make_mutant(1, "src/Counter.sol", 10),
            make_mutant(2, "src/Counter.sol", 50),
            make_mutant(3, "src/Token.sol", 5),
        ];
        let diff_ranges = vec![
            DiffRange {
                file: PathBuf::from("src/Counter.sol"),
                lines: vec![8..15],
            },
            DiffRange {
                file: PathBuf::from("src/Token.sol"),
                lines: vec![3..6],
            },
        ];

        let filtered = filter_mutants(mutants, &diff_ranges);
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].id, 1);
        assert_eq!(filtered[1].id, 3);
    }

    #[test]
    fn test_filter_mutants_all_filtered() {
        let mutants = vec![
            make_mutant(1, "src/Counter.sol", 100),
            make_mutant(2, "src/Other.sol", 5),
        ];
        let diff_ranges = vec![DiffRange {
            file: PathBuf::from("src/Counter.sol"),
            lines: vec![1..10],
        }];

        let filtered = filter_mutants(mutants, &diff_ranges);
        assert!(filtered.is_empty());
    }

    #[test]
    fn test_filter_mutants_boundary() {
        let mutants = vec![
            make_mutant(1, "src/A.sol", 10), // start of range, inclusive
            make_mutant(2, "src/A.sol", 14), // last in range
            make_mutant(3, "src/A.sol", 15), // end of range, exclusive
        ];
        let diff_ranges = vec![DiffRange {
            file: PathBuf::from("src/A.sol"),
            lines: vec![10..15],
        }];

        let filtered = filter_mutants(mutants, &diff_ranges);
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].id, 1);
        assert_eq!(filtered[1].id, 2);
    }

    // --- filter_targets_by_diff tests ---

    #[test]
    fn test_filter_targets_reduces_to_diff() {
        let targets = vec![
            make_target("src/Counter.sol"),
            make_target("src/Token.sol"),
            make_target("src/Vault.sol"),
        ];
        let diff_ranges = vec![
            DiffRange {
                file: PathBuf::from("src/Counter.sol"),
                lines: vec![1..10],
            },
            DiffRange {
                file: PathBuf::from("src/Vault.sol"),
                lines: vec![5..8],
            },
        ];

        let filtered = filter_targets_by_diff(targets, &diff_ranges, Path::new(""));
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].file, PathBuf::from("src/Counter.sol"));
        assert_eq!(filtered[1].file, PathBuf::from("src/Vault.sol"));
    }

    #[test]
    fn test_filter_targets_with_project_root() {
        let targets = vec![
            make_target("/home/user/project/src/Counter.sol"),
            make_target("/home/user/project/src/Token.sol"),
        ];
        let diff_ranges = vec![DiffRange {
            file: PathBuf::from("src/Token.sol"),
            lines: vec![1..5],
        }];

        let filtered =
            filter_targets_by_diff(targets, &diff_ranges, Path::new("/home/user/project"));
        assert_eq!(filtered.len(), 1);
        assert_eq!(
            filtered[0].file,
            PathBuf::from("/home/user/project/src/Token.sol")
        );
    }

    #[test]
    fn test_filter_targets_no_match() {
        let targets = vec![make_target("src/Counter.sol")];
        let diff_ranges = vec![DiffRange {
            file: PathBuf::from("src/Other.sol"),
            lines: vec![1..10],
        }];

        let filtered = filter_targets_by_diff(targets, &diff_ranges, Path::new(""));
        assert!(filtered.is_empty());
    }

    use crate::test_utils::{git_add_commit, init_git_repo};

    #[test]
    fn test_parse_git_diff_with_changes() {
        let temp = tempfile::TempDir::new().unwrap();
        let dir = temp.path();
        init_git_repo(dir);

        let src = dir.join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("Counter.sol"), "line1\nline2\nline3\n").unwrap();
        git_add_commit(dir, "initial");

        std::fs::write(src.join("Counter.sol"), "line1\nchanged\nline3\n").unwrap();
        git_add_commit(dir, "modify");

        let ranges = parse_git_diff(dir, "HEAD~1").unwrap();
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].file, PathBuf::from("src/Counter.sol"));
        assert!(ranges[0].lines.iter().any(|r| r.contains(&2)));
    }

    #[test]
    fn test_parse_git_diff_invalid_ref() {
        let temp = tempfile::TempDir::new().unwrap();
        let dir = temp.path();
        init_git_repo(dir);

        std::fs::write(dir.join("a.sol"), "x\n").unwrap();
        git_add_commit(dir, "initial");

        let result = parse_git_diff(dir, "nonexistent-ref");
        assert!(result.is_err());
        match result.unwrap_err() {
            DiffError::GitCommand(_) => {}
            other => panic!("expected GitCommand error, got: {other}"),
        }
    }

    #[test]
    fn test_parse_diff_from_reader() {
        let diff = "\
diff --git a/src/Counter.sol b/src/Counter.sol
--- a/src/Counter.sol
+++ b/src/Counter.sol
@@ -10,3 +10,5 @@ contract Counter {
";
        let ranges = parse_diff_from_reader(diff.as_bytes()).unwrap();
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].file, PathBuf::from("src/Counter.sol"));
        assert_eq!(ranges[0].lines, vec![10..15]);
    }

    #[test]
    fn test_parse_hunk_with_at_signs_in_context() {
        let diff = "\
diff --git a/src/Asm.sol b/src/Asm.sol
--- a/src/Asm.sol
+++ b/src/Asm.sol
@@ -10,3 +10,5 @@ assembly { // @@ edge case
";
        let ranges = parse_diff_output(diff).unwrap();
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].lines, vec![10..15]);
    }
}
