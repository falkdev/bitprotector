use std::path::PathBuf;
use crate::core::checksum;

/// Represents a detected file change.
#[derive(Debug, Clone)]
pub struct FileChange {
    pub relative_path: String,
    pub drive_pair_id: i64,
    pub old_checksum: String,
    pub new_checksum: String,
}

/// Detect changes in a tracked file by comparing current checksum to stored.
pub fn detect_change(
    primary_path: &str,
    relative_path: &str,
    stored_checksum: &str,
) -> anyhow::Result<Option<String>> {
    let full_path = PathBuf::from(primary_path).join(relative_path);
    if !full_path.exists() {
        return Ok(None);
    }
    let current = checksum::checksum_file(&full_path)?;
    if current != stored_checksum {
        Ok(Some(current))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    #[test]
    fn test_no_change_when_file_unchanged() {
        let dir = TempDir::new().unwrap();
        let content = b"unchanged content";
        fs::write(dir.path().join("f.txt"), content).unwrap();
        let stored = checksum::checksum_bytes(content);

        let change = detect_change(dir.path().to_str().unwrap(), "f.txt", &stored).unwrap();
        assert!(change.is_none());
    }

    #[test]
    fn test_changed_file_detected() {
        let dir = TempDir::new().unwrap();
        let original = b"original";
        let stored = checksum::checksum_bytes(original);
        // Write modified content
        fs::write(dir.path().join("f.txt"), b"modified content").unwrap();

        let change = detect_change(dir.path().to_str().unwrap(), "f.txt", &stored).unwrap();
        assert!(change.is_some());
        let new_hash = change.unwrap();
        assert_eq!(new_hash, checksum::checksum_bytes(b"modified content"));
    }

    #[test]
    fn test_missing_file_returns_none() {
        let dir = TempDir::new().unwrap();
        let change = detect_change(dir.path().to_str().unwrap(), "missing.txt", "anyhash").unwrap();
        assert!(change.is_none());
    }
}
