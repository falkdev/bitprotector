use bitprotector_lib::core::{checksum, drive, mirror};
use bitprotector_lib::db::repository::{create_memory_pool, DrivePair, Repository, TrackedFile};
use bitprotector_lib::db::schema::initialize_schema;
use std::fs;
use tempfile::TempDir;

fn make_repo() -> Repository {
    let pool = create_memory_pool().unwrap();
    initialize_schema(&pool.get().unwrap()).unwrap();
    Repository::new(pool)
}

/// Create a repo + drive pair + tracked file with `content` written to primary.
fn setup(
    primary: &TempDir,
    secondary: &TempDir,
    name: &str,
    content: &[u8],
) -> (Repository, DrivePair, TrackedFile) {
    let repo = make_repo();
    fs::write(primary.path().join(name), content).unwrap();
    let hash = checksum::checksum_bytes(content);
    let pair = repo
        .create_drive_pair(
            "pair",
            primary.path().to_str().unwrap(),
            secondary.path().to_str().unwrap(),
        )
        .unwrap();
    let file = repo
        .create_tracked_file(pair.id, name, &hash, content.len() as i64, None)
        .unwrap();
    (repo, pair, file)
}

// ── restore_mirror_from_master ─────────────────────────────────────────────

#[test]
fn test_restore_mirror_from_master_happy_path() {
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let content = b"master content";
    let (_, pair, file) = setup(&primary, &secondary, "doc.txt", content);

    // Secondary is empty; mirror should be created from master
    mirror::restore_mirror_from_master(&pair, &file.relative_path, &file.checksum).unwrap();

    let mirror_content = fs::read(secondary.path().join("doc.txt")).unwrap();
    assert_eq!(mirror_content, content, "Mirror must match master content");
}

#[test]
fn test_restore_mirror_from_master_primary_file_missing() {
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let content = b"content";
    let (_, pair, file) = setup(&primary, &secondary, "missing.txt", content);

    // Remove the primary file after tracking
    fs::remove_file(primary.path().join("missing.txt")).unwrap();

    let result = mirror::restore_mirror_from_master(&pair, &file.relative_path, &file.checksum);
    assert!(result.is_err(), "Should fail when primary file is missing");
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("does not exist") || msg.contains("not exist"),
        "Error should mention missing file, got: {}",
        msg
    );
}

#[test]
fn test_restore_mirror_from_master_checksum_mismatch() {
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let content = b"original";
    let (_, pair, file) = setup(&primary, &secondary, "corrupt.txt", content);

    // Tamper with the primary file after tracking
    fs::write(primary.path().join("corrupt.txt"), b"tampered content").unwrap();

    let result = mirror::restore_mirror_from_master(&pair, &file.relative_path, &file.checksum);
    assert!(
        result.is_err(),
        "Should fail when primary checksum mismatches stored"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("checksum mismatch"),
        "Error should mention checksum mismatch, got: {}",
        msg
    );
}

#[test]
fn test_restore_mirror_from_master_creates_subdirectories() {
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let content = b"nested content";
    // Create nested directory on primary
    fs::create_dir_all(primary.path().join("a/b")).unwrap();
    fs::write(primary.path().join("a/b/nested.txt"), content).unwrap();

    let repo = make_repo();
    let hash = checksum::checksum_bytes(content);
    let pair = repo
        .create_drive_pair(
            "pair",
            primary.path().to_str().unwrap(),
            secondary.path().to_str().unwrap(),
        )
        .unwrap();
    let file = repo
        .create_tracked_file(pair.id, "a/b/nested.txt", &hash, content.len() as i64, None)
        .unwrap();

    // Secondary has no a/b/ directory yet
    mirror::restore_mirror_from_master(&pair, &file.relative_path, &file.checksum).unwrap();

    assert!(secondary.path().join("a/b/nested.txt").exists());
    assert_eq!(
        fs::read(secondary.path().join("a/b/nested.txt")).unwrap(),
        content
    );
}

#[test]
fn test_restore_mirror_from_master_secondary_failed_returns_error() {
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let content = b"data";
    let (repo, pair, file) = setup(&primary, &secondary, "f.txt", content);

    // Mark secondary as quiescing then failed
    drive::mark_drive_quiescing(&repo, pair.id, drive::DriveRole::Secondary).unwrap();
    let failed_pair =
        drive::confirm_drive_failure(&repo, pair.id, drive::DriveRole::Secondary).unwrap();

    let result =
        mirror::restore_mirror_from_master(&failed_pair, &file.relative_path, &file.checksum);
    assert!(
        result.is_err(),
        "Should fail when secondary drive is marked failed"
    );
}

// ── restore_from_mirror ────────────────────────────────────────────────────

#[test]
fn test_restore_from_mirror_happy_path() {
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let content = b"mirror content";
    let (_, pair, file) = setup(&primary, &secondary, "restore.txt", content);

    // Write mirror file with matching content
    fs::write(secondary.path().join("restore.txt"), content).unwrap();
    // Remove primary to simulate master loss
    fs::remove_file(primary.path().join("restore.txt")).unwrap();

    mirror::restore_from_mirror(&pair, &file.relative_path, &file.checksum).unwrap();

    let restored = fs::read(primary.path().join("restore.txt")).unwrap();
    assert_eq!(restored, content, "Primary should be restored from mirror");
}

#[test]
fn test_restore_from_mirror_mirror_missing() {
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let content = b"content";
    let (_, pair, file) = setup(&primary, &secondary, "no_mirror.txt", content);

    // No file in secondary
    let result = mirror::restore_from_mirror(&pair, &file.relative_path, &file.checksum);
    assert!(result.is_err(), "Should fail when mirror file is absent");
}

#[test]
fn test_restore_from_mirror_checksum_mismatch() {
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let content = b"original";
    let (_, pair, file) = setup(&primary, &secondary, "chk.txt", content);

    // Write a corrupted mirror
    fs::write(secondary.path().join("chk.txt"), b"corrupted mirror").unwrap();

    let result = mirror::restore_from_mirror(&pair, &file.relative_path, &file.checksum);
    assert!(
        result.is_err(),
        "Should fail when mirror checksum mismatches stored"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("checksum mismatch"),
        "Error should mention checksum mismatch, got: {}",
        msg
    );
}

#[test]
fn test_restore_from_mirror_primary_failed_returns_error() {
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let content = b"data";
    let (repo, pair, file) = setup(&primary, &secondary, "pf.txt", content);
    fs::write(secondary.path().join("pf.txt"), content).unwrap();

    // Mark primary as quiescing then failed (drive pair now has secondary as active)
    drive::mark_drive_quiescing(&repo, pair.id, drive::DriveRole::Primary).unwrap();
    let failed_pair =
        drive::confirm_drive_failure(&repo, pair.id, drive::DriveRole::Primary).unwrap();

    let result = mirror::restore_from_mirror(&failed_pair, &file.relative_path, &file.checksum);
    assert!(
        result.is_err(),
        "Should fail when primary drive is marked failed"
    );
}

// ── mirror_file ────────────────────────────────────────────────────────────

#[test]
fn test_mirror_file_standby_not_ready_returns_error() {
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let content = b"data";
    let (repo, pair, file) = setup(&primary, &secondary, "q.txt", content);

    // Make secondary quiescing — standby_accepts_sync() will return false
    drive::mark_drive_quiescing(&repo, pair.id, drive::DriveRole::Secondary).unwrap();
    let quiescing_pair = repo.get_drive_pair(pair.id).unwrap();

    let result = mirror::mirror_file(&quiescing_pair, &file.relative_path);
    assert!(
        result.is_err(),
        "mirror_file should refuse when standby is quiescing"
    );
}
