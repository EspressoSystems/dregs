mod cli;
mod config;
mod diff;
mod generator;
mod ignore;
mod manifest;
mod partition;
mod report;
mod runner;

pub use cli::{Cli, run};

#[cfg(test)]
mod test_utils {
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

    fn git(dir: &Path, args: &[&str]) -> std::process::Output {
        let output = Command::new("git")
            .args(args)
            .current_dir(dir)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .output()
            .expect("git command failed");
        assert!(output.status.success(), "git {:?} failed", args);
        output
    }

    pub fn init_git_repo(dir: &Path) {
        git(dir, &["init"]);
        git(dir, &["config", "user.email", "test@test.com"]);
        git(dir, &["config", "user.name", "Test"]);
    }

    pub fn git_add_commit(dir: &Path, msg: &str) {
        git(dir, &["add", "."]);
        git(dir, &["commit", "-m", msg]);
    }
}
