use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
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
        .args(["drives", "add", "--no-validate", "showme", "/p", "/s"])
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

#[test]
fn test_primary_replacement_failover_retargets_virtual_path_and_rebuilds() {
    let db = temp_db();
    let db_path = db.path().to_str().unwrap();
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let replacement = TempDir::new().unwrap();
    let virtual_root = TempDir::new().unwrap();
    let virtual_path_on_disk = virtual_root.path().join("docs/report.txt");

    fs::write(primary.path().join("report.txt"), b"report").unwrap();

    cmd(db_path)
        .args([
            "drives",
            "add",
            "pair",
            primary.path().to_str().unwrap(),
            secondary.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    cmd(db_path)
        .args(["files", "track", "1", "report.txt"])
        .assert()
        .success();
    cmd(db_path)
        .args(["files", "mirror", "1"])
        .assert()
        .success();

    cmd(db_path)
        .args([
            "virtual-paths",
            "set",
            "1",
            virtual_path_on_disk.to_str().unwrap(),
        ])
        .assert()
        .success();

    assert_eq!(
        fs::read_link(&virtual_path_on_disk).unwrap(),
        primary.path().join("report.txt"),
    );

    cmd(db_path)
        .args(["drives", "replace", "mark", "1", "--role", "primary"])
        .assert()
        .success()
        .stdout(predicate::str::contains("quiescing"));

    cmd(db_path)
        .args(["drives", "replace", "confirm", "1", "--role", "primary"])
        .assert()
        .success()
        .stdout(predicate::str::contains("confirmed failed on primary"));

    assert_eq!(
        fs::read_link(&virtual_path_on_disk).unwrap(),
        secondary.path().join("report.txt"),
    );

    cmd(db_path)
        .args([
            "drives",
            "replace",
            "assign",
            "1",
            "--role",
            "primary",
            replacement.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("queued 1 rebuild item"));

    cmd(db_path).args(["sync", "process"]).assert().success();

    assert_eq!(
        fs::read(replacement.path().join("report.txt")).unwrap(),
        b"report"
    );
    assert_eq!(
        fs::read_link(&virtual_path_on_disk).unwrap(),
        replacement.path().join("report.txt"),
    );
}

#[test]
fn test_secondary_replacement_rebuilds_without_switching_active_role() {
    let db = temp_db();
    let db_path = db.path().to_str().unwrap();
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let replacement = TempDir::new().unwrap();

    fs::write(primary.path().join("data.bin"), b"payload").unwrap();

    cmd(db_path)
        .args([
            "drives",
            "add",
            "pair",
            primary.path().to_str().unwrap(),
            secondary.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    cmd(db_path)
        .args(["files", "track", "1", "data.bin"])
        .assert()
        .success();
    cmd(db_path)
        .args(["files", "mirror", "1"])
        .assert()
        .success();

    cmd(db_path)
        .args(["drives", "replace", "mark", "1", "--role", "secondary"])
        .assert()
        .success();
    cmd(db_path)
        .args(["drives", "replace", "confirm", "1", "--role", "secondary"])
        .assert()
        .success();
    cmd(db_path)
        .args([
            "drives",
            "replace",
            "assign",
            "1",
            "--role",
            "secondary",
            replacement.path().to_str().unwrap(),
        ])
        .assert()
        .success();
    cmd(db_path).args(["sync", "process"]).assert().success();

    assert_eq!(
        fs::read(replacement.path().join("data.bin")).unwrap(),
        b"payload"
    );

    cmd(db_path)
        .args(["drives", "show", "1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Active Role:     primary"))
        .stdout(predicate::str::contains("Secondary State: active"));
}
