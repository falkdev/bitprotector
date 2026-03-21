use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::{NamedTempFile, TempDir};

fn cmd(db: &str) -> Command {
    let mut c = Command::cargo_bin("bitprotector").unwrap();
    c.arg("--db").arg(db);
    c
}

fn temp_db() -> NamedTempFile {
    NamedTempFile::new().unwrap()
}

#[test]
fn test_status_empty_database() {
    let db = temp_db();
    cmd(db.path().to_str().unwrap())
        .args(["status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("BitProtector Status"))
        .stdout(predicate::str::contains("Drives: 0"))
        .stdout(predicate::str::contains("Files: 0"));
}

#[test]
fn test_status_shows_drive_and_file_counts() {
    let db = temp_db();
    let db_path = db.path().to_str().unwrap();
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();

    // Add a drive pair using real directories
    cmd(db_path)
        .args([
            "drives",
            "add",
            "test",
            primary.path().to_str().unwrap(),
            secondary.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    // Status should show 1 drive
    cmd(db_path)
        .args(["status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Drives: 1"));
}

#[test]
fn test_status_shows_sync_queue_empty() {
    let db = temp_db();
    cmd(db.path().to_str().unwrap())
        .args(["status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Sync queue empty"));
}

#[test]
fn test_status_no_integrity_failures() {
    let db = temp_db();
    cmd(db.path().to_str().unwrap())
        .args(["status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No integrity failures"));
}

#[test]
fn test_status_no_backups_configured() {
    let db = temp_db();
    cmd(db.path().to_str().unwrap())
        .args(["status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No backups configured"));
}
