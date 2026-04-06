use crate::core::checksum;
use crate::core::drive::{self, DriveRole, DriveState};
use crate::db::repository::{DrivePair, TrackedFile};
use std::path::PathBuf;

/// The result of verifying a tracked file's integrity.
#[derive(Debug, Clone, PartialEq)]
pub enum IntegrityStatus {
    /// Both master and mirror match the stored checksum.
    Ok,
    /// Master is corrupted; mirror is intact — auto-recoverable.
    MasterCorrupted,
    /// Mirror is corrupted; master is intact — auto-recoverable.
    MirrorCorrupted,
    /// Both master and mirror are corrupted — user action required.
    BothCorrupted,
    /// Mirror file does not exist.
    MirrorMissing,
    /// Master file does not exist.
    MasterMissing,
    /// The primary drive root is unavailable while the pair still expects it to be active.
    PrimaryDriveUnavailable,
    /// The secondary drive root is unavailable while the pair still expects it to be active.
    SecondaryDriveUnavailable,
}

/// Detailed result of an integrity check on a single file.
#[derive(Debug)]
pub struct IntegrityCheckResult {
    pub file_id: i64,
    pub stored_checksum: String,
    pub master_checksum: Option<String>,
    pub mirror_checksum: Option<String>,
    pub master_valid: bool,
    pub mirror_valid: bool,
    pub status: IntegrityStatus,
}

/// Check the integrity of a single tracked file.
pub fn check_file_integrity(
    drive_pair: &DrivePair,
    file: &TrackedFile,
) -> anyhow::Result<IntegrityCheckResult> {
    let master_path = PathBuf::from(&drive_pair.primary_path).join(&file.relative_path);
    let mirror_path = PathBuf::from(&drive_pair.secondary_path).join(&file.relative_path);
    let primary_root_available = drive::path_is_available(&drive_pair.primary_path);
    let secondary_root_available = drive::path_is_available(&drive_pair.secondary_path);

    if drive_pair.role_state(DriveRole::Primary) == DriveState::Active && !primary_root_available {
        return Ok(IntegrityCheckResult {
            file_id: file.id,
            stored_checksum: file.checksum.clone(),
            master_checksum: None,
            mirror_checksum: None,
            master_valid: false,
            mirror_valid: false,
            status: IntegrityStatus::PrimaryDriveUnavailable,
        });
    }

    if drive_pair.role_state(DriveRole::Secondary) == DriveState::Active
        && !secondary_root_available
    {
        return Ok(IntegrityCheckResult {
            file_id: file.id,
            stored_checksum: file.checksum.clone(),
            master_checksum: None,
            mirror_checksum: None,
            master_valid: false,
            mirror_valid: false,
            status: IntegrityStatus::SecondaryDriveUnavailable,
        });
    }

    let master_checksum = if primary_root_available && master_path.exists() {
        Some(checksum::checksum_file(&master_path)?)
    } else {
        None
    };

    let mirror_checksum = if secondary_root_available && mirror_path.exists() {
        Some(checksum::checksum_file(&mirror_path)?)
    } else {
        None
    };

    let master_valid = master_checksum.as_deref() == Some(&file.checksum);
    let mirror_valid = mirror_checksum.as_deref() == Some(&file.checksum);

    let status = if drive_pair.is_degraded() {
        match drive_pair.active_role_enum() {
            DriveRole::Primary => {
                if master_valid {
                    IntegrityStatus::Ok
                } else {
                    IntegrityStatus::BothCorrupted
                }
            }
            DriveRole::Secondary => {
                if mirror_valid {
                    IntegrityStatus::Ok
                } else {
                    IntegrityStatus::BothCorrupted
                }
            }
        }
    } else {
        match (
            master_checksum.is_some(),
            mirror_checksum.is_some(),
            master_valid,
            mirror_valid,
        ) {
            (false, _, _, _) => IntegrityStatus::MasterMissing,
            (_, false, _, _) => IntegrityStatus::MirrorMissing,
            (true, true, true, true) => IntegrityStatus::Ok,
            (true, true, false, true) => IntegrityStatus::MasterCorrupted,
            (true, true, true, false) => IntegrityStatus::MirrorCorrupted,
            (true, true, false, false) => IntegrityStatus::BothCorrupted,
        }
    };

    Ok(IntegrityCheckResult {
        file_id: file.id,
        stored_checksum: file.checksum.clone(),
        master_checksum,
        mirror_checksum,
        master_valid,
        mirror_valid,
        status,
    })
}

/// Attempt automatic recovery based on the integrity check result.
/// Returns true if recovery was performed, false if manual action is required.
pub fn attempt_recovery(
    drive_pair: &DrivePair,
    file: &TrackedFile,
    result: &IntegrityCheckResult,
) -> anyhow::Result<bool> {
    if drive_pair.is_degraded() {
        return Ok(false);
    }

    match &result.status {
        IntegrityStatus::MasterCorrupted => {
            crate::core::mirror::restore_from_mirror(
                drive_pair,
                &file.relative_path,
                &file.checksum,
            )?;
            Ok(true)
        }
        IntegrityStatus::MirrorCorrupted | IntegrityStatus::MirrorMissing => {
            crate::core::mirror::restore_mirror_from_master(
                drive_pair,
                &file.relative_path,
                &file.checksum,
            )?;
            Ok(true)
        }
        IntegrityStatus::Ok => Ok(false),
        IntegrityStatus::BothCorrupted
        | IntegrityStatus::MasterMissing
        | IntegrityStatus::PrimaryDriveUnavailable
        | IntegrityStatus::SecondaryDriveUnavailable => Ok(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn make_pair(primary: &TempDir, secondary: &TempDir) -> DrivePair {
        DrivePair {
            id: 1,
            name: "test".to_string(),
            primary_path: primary.path().to_str().unwrap().to_string(),
            secondary_path: secondary.path().to_str().unwrap().to_string(),
            primary_state: "active".to_string(),
            secondary_state: "active".to_string(),
            active_role: "primary".to_string(),
            created_at: "".to_string(),
            updated_at: "".to_string(),
        }
    }

    fn write_file(dir: &TempDir, name: &str, content: &[u8]) -> String {
        let path = dir.path().join(name);
        fs::write(&path, content).unwrap();
        checksum::checksum_bytes(content)
    }

    fn make_tracked_file(id: i64, relative_path: &str, checksum: &str) -> TrackedFile {
        TrackedFile {
            id,
            drive_pair_id: 1,
            relative_path: relative_path.to_string(),
            checksum: checksum.to_string(),
            file_size: 100,
            virtual_path: None,
            is_mirrored: true,
            tracked_direct: true,
            tracked_via_folder: false,
            last_integrity_check_at: None,
            created_at: "".to_string(),
            updated_at: "".to_string(),
        }
    }

    #[test]
    fn test_integrity_ok() {
        let primary = TempDir::new().unwrap();
        let secondary = TempDir::new().unwrap();
        let content = b"intact content";
        let hash = write_file(&primary, "f.txt", content);
        write_file(&secondary, "f.txt", content);

        let pair = make_pair(&primary, &secondary);
        let file = make_tracked_file(1, "f.txt", &hash);

        let result = check_file_integrity(&pair, &file).unwrap();
        assert_eq!(result.status, IntegrityStatus::Ok);
        assert!(result.master_valid);
        assert!(result.mirror_valid);
    }

    #[test]
    fn test_integrity_master_corrupted() {
        let primary = TempDir::new().unwrap();
        let secondary = TempDir::new().unwrap();
        let original = b"original content";
        let hash = checksum::checksum_bytes(original);
        write_file(&secondary, "f.txt", original);
        // Write corrupted content to primary
        fs::write(primary.path().join("f.txt"), b"corrupted content").unwrap();

        let pair = make_pair(&primary, &secondary);
        let file = make_tracked_file(1, "f.txt", &hash);

        let result = check_file_integrity(&pair, &file).unwrap();
        assert_eq!(result.status, IntegrityStatus::MasterCorrupted);
        assert!(!result.master_valid);
        assert!(result.mirror_valid);
    }

    #[test]
    fn test_integrity_mirror_corrupted() {
        let primary = TempDir::new().unwrap();
        let secondary = TempDir::new().unwrap();
        let original = b"original content";
        let hash = write_file(&primary, "f.txt", original);
        // Write corrupted content to secondary
        fs::write(secondary.path().join("f.txt"), b"corrupted mirror").unwrap();

        let pair = make_pair(&primary, &secondary);
        let file = make_tracked_file(1, "f.txt", &hash);

        let result = check_file_integrity(&pair, &file).unwrap();
        assert_eq!(result.status, IntegrityStatus::MirrorCorrupted);
        assert!(result.master_valid);
        assert!(!result.mirror_valid);
    }

    #[test]
    fn test_integrity_both_corrupted() {
        let primary = TempDir::new().unwrap();
        let secondary = TempDir::new().unwrap();
        let stored_hash = checksum::checksum_bytes(b"original");
        fs::write(primary.path().join("f.txt"), b"bad1").unwrap();
        fs::write(secondary.path().join("f.txt"), b"bad2").unwrap();

        let pair = make_pair(&primary, &secondary);
        let file = make_tracked_file(1, "f.txt", &stored_hash);

        let result = check_file_integrity(&pair, &file).unwrap();
        assert_eq!(result.status, IntegrityStatus::BothCorrupted);
        assert!(!result.master_valid);
        assert!(!result.mirror_valid);
    }

    #[test]
    fn test_integrity_mirror_missing() {
        let primary = TempDir::new().unwrap();
        let secondary = TempDir::new().unwrap();
        let hash = write_file(&primary, "f.txt", b"content");

        let pair = make_pair(&primary, &secondary);
        let file = make_tracked_file(1, "f.txt", &hash);

        let result = check_file_integrity(&pair, &file).unwrap();
        assert_eq!(result.status, IntegrityStatus::MirrorMissing);
    }

    #[test]
    fn test_integrity_master_missing() {
        let primary = TempDir::new().unwrap();
        let secondary = TempDir::new().unwrap();
        let hash = write_file(&secondary, "f.txt", b"content");

        let pair = make_pair(&primary, &secondary);
        let file = make_tracked_file(1, "f.txt", &hash);

        let result = check_file_integrity(&pair, &file).unwrap();
        assert_eq!(result.status, IntegrityStatus::MasterMissing);
    }

    #[test]
    fn test_integrity_primary_drive_unavailable() {
        let primary = TempDir::new().unwrap();
        let secondary = TempDir::new().unwrap();
        let hash = write_file(&secondary, "f.txt", b"content");
        std::fs::remove_dir_all(primary.path()).unwrap();

        let pair = make_pair(&primary, &secondary);
        let file = make_tracked_file(1, "f.txt", &hash);

        let result = check_file_integrity(&pair, &file).unwrap();
        assert_eq!(result.status, IntegrityStatus::PrimaryDriveUnavailable);
    }

    #[test]
    fn test_integrity_degraded_pair_ignores_failed_primary_root() {
        let primary = TempDir::new().unwrap();
        let secondary = TempDir::new().unwrap();
        let hash = write_file(&secondary, "f.txt", b"content");
        std::fs::remove_dir_all(primary.path()).unwrap();

        let mut pair = make_pair(&primary, &secondary);
        pair.primary_state = "failed".to_string();
        pair.active_role = "secondary".to_string();
        let file = make_tracked_file(1, "f.txt", &hash);

        let result = check_file_integrity(&pair, &file).unwrap();
        assert_eq!(result.status, IntegrityStatus::Ok);
        assert!(result.mirror_valid);
    }

    #[test]
    fn test_recovery_from_master_corruption() {
        let primary = TempDir::new().unwrap();
        let secondary = TempDir::new().unwrap();
        let original = b"original content";
        let hash = checksum::checksum_bytes(original);
        write_file(&secondary, "f.txt", original);
        fs::write(primary.path().join("f.txt"), b"corrupted").unwrap();

        let pair = make_pair(&primary, &secondary);
        let file = make_tracked_file(1, "f.txt", &hash);

        let result = check_file_integrity(&pair, &file).unwrap();
        let recovered = attempt_recovery(&pair, &file, &result).unwrap();
        assert!(recovered);

        // Verify primary is now restored
        let restored = fs::read(primary.path().join("f.txt")).unwrap();
        assert_eq!(restored, original);
    }

    #[test]
    fn test_recovery_from_mirror_corruption() {
        let primary = TempDir::new().unwrap();
        let secondary = TempDir::new().unwrap();
        let original = b"intact primary";
        let hash = write_file(&primary, "f.txt", original);
        fs::write(secondary.path().join("f.txt"), b"bad mirror").unwrap();

        let pair = make_pair(&primary, &secondary);
        let file = make_tracked_file(1, "f.txt", &hash);

        let result = check_file_integrity(&pair, &file).unwrap();
        let recovered = attempt_recovery(&pair, &file, &result).unwrap();
        assert!(recovered);

        let restored = fs::read(secondary.path().join("f.txt")).unwrap();
        assert_eq!(restored, original);
    }

    #[test]
    fn test_no_recovery_needed_for_ok() {
        let primary = TempDir::new().unwrap();
        let secondary = TempDir::new().unwrap();
        let original = b"good content";
        let hash = write_file(&primary, "f.txt", original);
        write_file(&secondary, "f.txt", original);

        let pair = make_pair(&primary, &secondary);
        let file = make_tracked_file(1, "f.txt", &hash);

        let result = check_file_integrity(&pair, &file).unwrap();
        let recovered = attempt_recovery(&pair, &file, &result).unwrap();
        assert!(!recovered);
    }

    #[test]
    fn test_no_recovery_for_both_corrupted() {
        let primary = TempDir::new().unwrap();
        let secondary = TempDir::new().unwrap();
        let stored_hash = checksum::checksum_bytes(b"original");
        fs::write(primary.path().join("f.txt"), b"bad1").unwrap();
        fs::write(secondary.path().join("f.txt"), b"bad2").unwrap();

        let pair = make_pair(&primary, &secondary);
        let file = make_tracked_file(1, "f.txt", &stored_hash);

        let result = check_file_integrity(&pair, &file).unwrap();
        let recovered = attempt_recovery(&pair, &file, &result).unwrap();
        assert!(!recovered);
    }
}
