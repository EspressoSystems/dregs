pub mod cli;
pub mod config;
pub mod diff;
pub mod generator;
pub mod manifest;
pub mod partition;
pub mod report;
pub mod runner;

#[cfg(test)]
pub(crate) mod test_utils {
    use std::path::{Path, PathBuf};
    use std::process::Command;

    /// Copy a fixture directory to a temp dir, returning both (to keep TempDir alive).
    pub fn fixture_to_temp(name: &str) -> (tempfile::TempDir, PathBuf) {
        let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name);
        let temp_dir = tempfile::TempDir::new().unwrap();
        crate::runner::copy_dir_recursive(&fixture_path, temp_dir.path()).unwrap();
        let path = temp_dir.path().to_path_buf();
        (temp_dir, path)
    }

    pub fn init_git_repo(dir: &Path) {
        for args in [
            vec!["init"],
            vec!["config", "user.email", "test@test.com"],
            vec!["config", "user.name", "Test"],
        ] {
            Command::new("git")
                .args(&args)
                .current_dir(dir)
                .output()
                .unwrap();
        }
    }

    pub fn git_add_commit(dir: &Path, msg: &str) {
        Command::new("git")
            .args(["add", "."])
            .current_dir(dir)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", msg])
            .current_dir(dir)
            .output()
            .unwrap();
    }
}
