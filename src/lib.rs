pub mod config;
pub mod generator;
pub mod manifest;
pub mod partition;
pub mod report;
pub mod runner;

#[cfg(test)]
pub(crate) mod test_utils {
    use std::path::PathBuf;

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
}
