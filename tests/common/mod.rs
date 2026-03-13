use assert_cmd::Command;
use assert_fs::TempDir;
use std::path::{Path, PathBuf};
use std::process;

pub fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

pub struct TestRun {
    temp_dir: TempDir,
}

impl TestRun {
    pub fn from_fixture(name: &str) -> Self {
        let fixture_path = fixture(name);
        let temp_dir = TempDir::new().unwrap();

        copy_dir_recursive(&fixture_path, temp_dir.path()).unwrap();

        Self { temp_dir }
    }

    pub fn project_path(&self) -> PathBuf {
        self.temp_dir.path().to_path_buf()
    }

    #[allow(deprecated)]
    pub fn dregs_cmd(&self) -> Command {
        let mut cmd = Command::cargo_bin("dregs").unwrap();
        cmd.arg("run").arg("--project").arg(self.project_path());
        cmd
    }
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    if !dst.exists() {
        std::fs::create_dir_all(dst)?;
    }

    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name();
        let file_name_str = file_name.to_string_lossy();

        if file_name_str.starts_with('.')
            || file_name_str == "target"
            || file_name_str == "node_modules"
            || file_name_str == "cache"
            || file_name_str == "out"
            || file_name_str == "gambit_out"
            || file_name_str == "dregs.toml"
        {
            continue;
        }

        let dst_path = dst.join(&file_name);

        if path.is_dir() {
            copy_dir_recursive(&path, &dst_path)?;
        } else {
            std::fs::copy(&path, &dst_path)?;
        }
    }

    Ok(())
}

fn git(dir: &Path, args: &[&str]) -> process::Output {
    let output = process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_path_exists() {
        let path = fixture("simple");
        assert!(path.exists(), "simple fixture should exist at {:?}", path);
    }
}
