//! Shared test utilities for pve-mcp crates.

/// Writes the given text to a temp file and returns its path. Caller drops
/// the TempDir to clean up.
pub fn write_temp_config(dir: &tempfile::TempDir, name: &str, content: &str) -> std::path::PathBuf {
    let path = dir.path().join(name);
    std::fs::write(&path, content).expect("write temp config");
    path
}

#[cfg(test)]
mod tests {
    #[test]
    fn writes_and_reads_back() {
        let dir = tempfile::tempdir().unwrap();
        let path = super::write_temp_config(&dir, "c.toml", "x = 1");
        assert_eq!(std::fs::read_to_string(path).unwrap(), "x = 1");
    }
}
