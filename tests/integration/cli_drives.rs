use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::{NamedTempFile, TempDir};

fn cmd(db: &str) -> Command {
    let mut c = Command::cargo_bin("bitprotector").unwrap();
    c.arg("--db").arg(db);
    c
}

/// Return a temp db path (the file is kept alive by the returned NamedTempFile).
fn temp_db() -> NamedTempFile {
    NamedTempFile::new().unwrap()
}

#[test]
fn test_drives_list_empty() {
    let db = temp_db();
    cmd(db.path().to_str().unwrap())
        .args(["drives", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No drive pairs registered"));
}

#[test]
fn test_drives_add_and_list() {
    let db = temp_db();
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();

    cmd(db.path().to_str().unwrap())
        .args([
            "drives",
            "add",
            "backup",
            primary.path().to_str().unwrap(),
            secondary.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created drive pair #1: backup"));

    cmd(db.path().to_str().unwrap())
        .args(["drives", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("backup"));
}

#[test]
fn test_drives_add_no_validate() {
    let db = temp_db();

    cmd(db.path().to_str().unwrap())
        .args([
            "drives",
            "add",
            "--no-validate",
            "raw",
            "/nonexistent/primary",
            "/nonexistent/secondary",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created drive pair #1: raw"));
}

#[test]
fn test_drives_add_invalid_path_fails() {
    let db = temp_db();

    cmd(db.path().to_str().unwrap())
        .args([
            "drives",
            "add",
            "bad",
            "/nonexistent/primary",
            "/nonexistent/secondary",
        ])
        .assert()
        .failure();
}

#[test]
fn test_drives_add_same_path_fails() {
    let db = temp_db();
    let dir = TempDir::new().unwrap();
    let path = dir.path().to_str().unwrap();

    cmd(db.path().to_str().unwrap())
        .args(["drives", "add", "same", path, path])
        .assert()
        .failure();
}

#[test]
fn test_drives_show() {
    let db = temp_db();
    let p = TempDir::new().unwrap();
    let s = TempDir::new().unwrap();

    cmd(db.path().to_str().unwrap())
        .args([
            "drives", "add", "--no-validate", "showme", "/p", "/s",
        ])
        .assert()
        .success();

    cmd(db.path().to_str().unwrap())
        .args(["drives", "show", "1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Drive Pair #1"))
        .stdout(predicate::str::contains("showme"));

    // suppress unused variable warnings
    drop(p);
    drop(s);
}

#[test]
fn test_drives_update_name() {
    let db = temp_db();

    cmd(db.path().to_str().unwrap())
        .args(["drives", "add", "--no-validate", "original", "/p", "/s"])
        .assert()
        .success();

    cmd(db.path().to_str().unwrap())
        .args(["drives", "update", "1", "--name", "renamed"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Updated drive pair #1: renamed"));
}

#[test]
fn test_drives_remove() {
    let db = temp_db();

    cmd(db.path().to_str().unwrap())
        .args(["drives", "add", "--no-validate", "removeme", "/p", "/s"])
        .assert()
        .success();

    cmd(db.path().to_str().unwrap())
        .args(["drives", "remove", "1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Removed drive pair #1"));

    cmd(db.path().to_str().unwrap())
        .args(["drives", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No drive pairs registered"));
}

#[test]
fn test_drives_show_nonexistent_fails() {
    let db = temp_db();

    cmd(db.path().to_str().unwrap())
        .args(["drives", "show", "999"])
        .assert()
        .failure();
}
