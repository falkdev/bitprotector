use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::{NamedTempFile, TempDir};

fn cmd(db: &str) -> Command {
    let mut c = Command::cargo_bin("bitprotector").unwrap();
    c.arg("--db").arg(db);
    c
}

fn temp_db() -> NamedTempFile {
    NamedTempFile::new().unwrap()
}

/// Create a drive pair and return its ID (always 1 for a fresh db)
fn create_pair(db: &str, primary: &TempDir, secondary: &TempDir) {
    cmd(db)
        .args([
            "drives",
            "add",
            "--no-validate",
            "test_pair",
            primary.path().to_str().unwrap(),
            secondary.path().to_str().unwrap(),
        ])
        .assert()
        .success();
}

#[test]
fn test_files_track_and_list() {
    let db = temp_db();
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();

    create_pair(db.path().to_str().unwrap(), &primary, &secondary);
    fs::write(primary.path().join("hello.txt"), b"hello").unwrap();

    cmd(db.path().to_str().unwrap())
        .args(["files", "track", "1", "hello.txt"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Tracked file #1"));

    cmd(db.path().to_str().unwrap())
        .args(["files", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("hello.txt"));
}

#[test]
fn test_files_track_with_mirror() {
    let db = temp_db();
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();

    create_pair(db.path().to_str().unwrap(), &primary, &secondary);
    fs::write(primary.path().join("to_mirror.txt"), b"mirror me").unwrap();

    cmd(db.path().to_str().unwrap())
        .args(["files", "track", "--mirror", "1", "to_mirror.txt"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Mirrored:  yes"));

    assert!(secondary.path().join("to_mirror.txt").exists());
}

#[test]
fn test_files_show() {
    let db = temp_db();
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();

    create_pair(db.path().to_str().unwrap(), &primary, &secondary);
    fs::write(primary.path().join("show.txt"), b"content").unwrap();
    cmd(db.path().to_str().unwrap())
        .args(["files", "track", "1", "show.txt"])
        .assert()
        .success();

    cmd(db.path().to_str().unwrap())
        .args(["files", "show", "1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Tracked File #1"))
        .stdout(predicate::str::contains("show.txt"));
}

#[test]
fn test_files_mirror_separately() {
    let db = temp_db();
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();

    create_pair(db.path().to_str().unwrap(), &primary, &secondary);
    fs::write(primary.path().join("later.txt"), b"data").unwrap();
    cmd(db.path().to_str().unwrap())
        .args(["files", "track", "1", "later.txt"])
        .assert()
        .success();

    cmd(db.path().to_str().unwrap())
        .args(["files", "mirror", "1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Mirrored file #1"));

    assert!(secondary.path().join("later.txt").exists());
}

#[test]
fn test_files_untrack() {
    let db = temp_db();
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();

    create_pair(db.path().to_str().unwrap(), &primary, &secondary);
    fs::write(primary.path().join("bye.txt"), b"data").unwrap();
    cmd(db.path().to_str().unwrap())
        .args(["files", "track", "1", "bye.txt"])
        .assert()
        .success();

    cmd(db.path().to_str().unwrap())
        .args(["files", "untrack", "1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Untracked file #1"));
}

#[test]
fn test_files_track_missing_file_fails() {
    let db = temp_db();
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();

    create_pair(db.path().to_str().unwrap(), &primary, &secondary);

    cmd(db.path().to_str().unwrap())
        .args(["files", "track", "1", "nonexistent.txt"])
        .assert()
        .failure();
}
