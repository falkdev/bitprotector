use crate::core::checksum;
use crate::core::drive::{self, DriveRole, DriveState};
use crate::db::repository::DrivePair;
use anyhow::Context;
use std::fs;
use std::path::{Path, PathBuf};

/// Validates that a drive pair's paths exist and are directories.
pub fn validate_drive_pair(primary: &str, secondary: &str) -> anyhow::Result<()> {
    let primary_path = Path::new(primary);
    let secondary_path = Path::new(secondary);

    if !primary_path.exists() {
        anyhow::bail!("Primary path does not exist: {}", primary);
    }
    if !primary_path.is_dir() {
        anyhow::bail!("Primary path is not a directory: {}", primary);
    }
    if !secondary_path.exists() {
        anyhow::bail!("Secondary path does not exist: {}", secondary);
    }
    if !secondary_path.is_dir() {
        anyhow::bail!("Secondary path is not a directory: {}", secondary);
    }

    let primary_canon = primary_path
        .canonicalize()
        .context("Failed to canonicalize primary path")?;
    let secondary_canon = secondary_path
        .canonicalize()
        .context("Failed to canonicalize secondary path")?;
    if primary_canon == secondary_canon {
        anyhow::bail!("Primary and secondary paths must be different directories");
    }

    // Check write access by attempting to create a temp file
    let test_primary = primary_path.join(".bitprotector_write_test");
    fs::write(&test_primary, b"").context("Primary path is not writable")?;
    fs::remove_file(&test_primary).ok();

    let test_secondary = secondary_path.join(".bitprotector_write_test");
    fs::write(&test_secondary, b"").context("Secondary path is not writable")?;
    fs::remove_file(&test_secondary).ok();

    Ok(())
}

/// Mirror a file from primary to secondary drive.
pub fn mirror_file(drive_pair: &DrivePair, relative_path: &str) -> anyhow::Result<String> {
    if !drive_pair.standby_accepts_sync() {
        anyhow::bail!(
            "Standby {} path is not ready to receive sync data for drive pair #{}",
            drive_pair.standby_role().as_str(),
            drive_pair.id
        );
    }

    drive::ensure_drive_root_marker(drive_pair.active_path())?;
    drive::ensure_drive_root_marker(drive_pair.standby_path())?;

    let src = PathBuf::from(drive_pair.active_path()).join(relative_path);
    let dst = PathBuf::from(drive_pair.standby_path()).join(relative_path);

    if !src.exists() {
        anyhow::bail!("Source file does not exist: {}", src.display());
    }

    // Create destination parent directories if needed
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent).context("Failed to create mirror directory")?;
    }

    // Copy and compute source checksum in a single streaming pass, then verify
    // the destination matches. This halves the number of source-file reads compared
    // to fs::copy followed by two separate checksum_file calls.
    let src_checksum =
        checksum::copy_with_checksum(&src, &dst).context("Failed to copy file to mirror")?;
    let dst_checksum = checksum::checksum_file(&dst)?;

    if src_checksum != dst_checksum {
        anyhow::bail!(
            "Mirror verification failed: src={} dst={}",
            src_checksum,
            dst_checksum
        );
    }

    Ok(src_checksum)
}

/// Restore the primary file from the mirror.
pub fn restore_from_mirror(
    drive_pair: &DrivePair,
    relative_path: &str,
    expected_checksum: &str,
) -> anyhow::Result<()> {
    let src = PathBuf::from(&drive_pair.secondary_path).join(relative_path);
    let dst = PathBuf::from(&drive_pair.primary_path).join(relative_path);

    if drive_pair.role_state(DriveRole::Primary) == DriveState::Failed {
        anyhow::bail!(
            "Primary drive is marked failed for drive pair #{}",
            drive_pair.id
        );
    }

    if !src.exists() {
        anyhow::bail!("Mirror file does not exist: {}", src.display());
    }

    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    drive::ensure_drive_root_marker(&drive_pair.primary_path)?;

    // Copy and verify checksum in a single streaming pass to halve source reads.
    checksum::copy_and_verify_checksum(&src, &dst, expected_checksum).map_err(|e| {
        anyhow::anyhow!(
            "Mirror file checksum mismatch: stored={} ({})",
            expected_checksum,
            e
        )
    })?;
    Ok(())
}

/// Restore the mirror file from the primary (master).
pub fn restore_mirror_from_master(
    drive_pair: &DrivePair,
    relative_path: &str,
    expected_checksum: &str,
) -> anyhow::Result<()> {
    let src = PathBuf::from(&drive_pair.primary_path).join(relative_path);
    let dst = PathBuf::from(&drive_pair.secondary_path).join(relative_path);

    if drive_pair.role_state(DriveRole::Secondary) == DriveState::Failed {
        anyhow::bail!(
            "Secondary drive is marked failed for drive pair #{}",
            drive_pair.id
        );
    }

    if !src.exists() {
        anyhow::bail!("Primary file does not exist: {}", src.display());
    }

    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    drive::ensure_drive_root_marker(&drive_pair.secondary_path)?;

    // Copy and verify checksum in a single streaming pass to halve source reads.
    checksum::copy_and_verify_checksum(&src, &dst, expected_checksum).map_err(|e| {
        anyhow::anyhow!(
            "Master file checksum mismatch: stored={} ({})",
            expected_checksum,
            e
        )
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn setup_pair() -> (TempDir, TempDir) {
        (TempDir::new().unwrap(), TempDir::new().unwrap())
    }

    #[test]
    fn test_validate_valid_pair() {
        let (primary, secondary) = setup_pair();
        validate_drive_pair(
            primary.path().to_str().unwrap(),
            secondary.path().to_str().unwrap(),
        )
        .expect("Validation should pass for valid pair");
    }

    #[test]
    fn test_validate_nonexistent_primary() {
        let secondary = TempDir::new().unwrap();
        let result = validate_drive_pair(
            "/nonexistent/path/primary",
            secondary.path().to_str().unwrap(),
        );
        assert!(result.is_err(), "Should fail for nonexistent primary");
    }

    #[test]
    fn test_validate_nonexistent_secondary() {
        let primary = TempDir::new().unwrap();
        let result = validate_drive_pair(
            primary.path().to_str().unwrap(),
            "/nonexistent/path/secondary",
        );
        assert!(result.is_err(), "Should fail for nonexistent secondary");
    }

    #[test]
    fn test_validate_same_path() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().to_str().unwrap();
        let result = validate_drive_pair(path, path);
        assert!(result.is_err(), "Should fail when primary == secondary");
    }

    #[test]
    fn test_mirror_file_copies_correctly() {
        let (primary_dir, secondary_dir) = setup_pair();
        let content = b"test file content for mirror test";

        let file_path = primary_dir.path().join("test.txt");
        let mut f = fs::File::create(&file_path).unwrap();
        f.write_all(content).unwrap();

        let pair = DrivePair {
            id: 1,
            name: "test".to_string(),
            primary_path: primary_dir.path().to_str().unwrap().to_string(),
            secondary_path: secondary_dir.path().to_str().unwrap().to_string(),
            primary_state: "active".to_string(),
            secondary_state: "active".to_string(),
            active_role: "primary".to_string(),
            created_at: "".to_string(),
            updated_at: "".to_string(),
        };

        let checksum = mirror_file(&pair, "test.txt").unwrap();

        let dst = secondary_dir.path().join("test.txt");
        assert!(dst.exists(), "Mirror file should exist");
        let dst_content = fs::read(&dst).unwrap();
        assert_eq!(dst_content, content, "Mirror content should match source");

        let expected = crate::core::checksum::checksum_bytes(content);
        assert_eq!(checksum, expected, "Returned checksum should match content");
    }

    #[test]
    fn test_mirror_file_creates_subdirectories() {
        let (primary_dir, secondary_dir) = setup_pair();
        let sub = primary_dir.path().join("sub/dir");
        fs::create_dir_all(&sub).unwrap();
        let mut f = fs::File::create(sub.join("file.bin")).unwrap();
        f.write_all(b"nested").unwrap();

        let pair = DrivePair {
            id: 1,
            name: "test".to_string(),
            primary_path: primary_dir.path().to_str().unwrap().to_string(),
            secondary_path: secondary_dir.path().to_str().unwrap().to_string(),
            primary_state: "active".to_string(),
            secondary_state: "active".to_string(),
            active_role: "primary".to_string(),
            created_at: "".to_string(),
            updated_at: "".to_string(),
        };

        mirror_file(&pair, "sub/dir/file.bin").unwrap();
        assert!(secondary_dir.path().join("sub/dir/file.bin").exists());
    }

    #[test]
    fn test_mirror_file_nonexistent_source() {
        let (primary_dir, secondary_dir) = setup_pair();
        let pair = DrivePair {
            id: 1,
            name: "test".to_string(),
            primary_path: primary_dir.path().to_str().unwrap().to_string(),
            secondary_path: secondary_dir.path().to_str().unwrap().to_string(),
            primary_state: "active".to_string(),
            secondary_state: "active".to_string(),
            active_role: "primary".to_string(),
            created_at: "".to_string(),
            updated_at: "".to_string(),
        };
        let result = mirror_file(&pair, "nonexistent.txt");
        assert!(result.is_err(), "Should fail for nonexistent source");
    }

    #[test]
    fn test_restore_from_mirror() {
        let (primary_dir, secondary_dir) = setup_pair();
        let content = b"restore from mirror test";
        let expected_checksum = crate::core::checksum::checksum_bytes(content);

        // Write to mirror
        let mirror_file_path = secondary_dir.path().join("r.txt");
        fs::write(&mirror_file_path, content).unwrap();

        // Remove from primary
        let primary_file_path = primary_dir.path().join("r.txt");
        assert!(!primary_file_path.exists());

        let pair = DrivePair {
            id: 1,
            name: "test".to_string(),
            primary_path: primary_dir.path().to_str().unwrap().to_string(),
            secondary_path: secondary_dir.path().to_str().unwrap().to_string(),
            primary_state: "active".to_string(),
            secondary_state: "active".to_string(),
            active_role: "primary".to_string(),
            created_at: "".to_string(),
            updated_at: "".to_string(),
        };

        restore_from_mirror(&pair, "r.txt", &expected_checksum).unwrap();
        assert!(primary_file_path.exists());
        assert_eq!(fs::read(&primary_file_path).unwrap(), content);
    }
}
