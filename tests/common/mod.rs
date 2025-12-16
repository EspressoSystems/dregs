use std::path::PathBuf;

pub fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
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
