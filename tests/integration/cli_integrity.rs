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

fn setup_tracked_file(
    db: &str,
    primary: &TempDir,
    secondary: &TempDir,
    name: &str,
    content: &[u8],
) {
    cmd(db)
        .args([
            "drives",
            "add",
            "--no-validate",
            "pair",
            primary.path().to_str().unwrap(),
            secondary.path().to_str().unwrap(),
        ])
        .assert()
        .success();
    fs::write(primary.path().join(name), content).unwrap();
    cmd(db)
        .args(["files", "track", "1", name])
        .assert()
        .success();
}

#[test]
fn test_integrity_check_ok() {
    let db = temp_db();
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let content = b"intact file";

    setup_tracked_file(
        db.path().to_str().unwrap(),
        &primary,
        &secondary,
        "ok.txt",
        content,
    );
    // Mirror it
    fs::write(secondary.path().join("ok.txt"), content).unwrap();

    cmd(db.path().to_str().unwrap())
        .args(["integrity", "check", "1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("OK"));
}

#[test]
fn test_integrity_check_mirror_missing() {
    let db = temp_db();
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    setup_tracked_file(
        db.path().to_str().unwrap(),
        &primary,
        &secondary,
        "nm.txt",
        b"data",
    );
    // No mirror file

    cmd(db.path().to_str().unwrap())
        .args(["integrity", "check", "1"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("MIRROR_MISSING"));
}

#[test]
fn test_integrity_check_with_recovery() {
    let db = temp_db();
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let content = b"good content";
    setup_tracked_file(
        db.path().to_str().unwrap(),
        &primary,
        &secondary,
        "rec.txt",
        content,
    );

    // Create a corrupt mirror
    fs::write(secondary.path().join("rec.txt"), b"corrupt").unwrap();

    cmd(db.path().to_str().unwrap())
        .args(["integrity", "check", "--recover", "1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Recovery: successful"));

    // Mirror should now match primary
    let restored = fs::read(secondary.path().join("rec.txt")).unwrap();
    assert_eq!(restored, content);

    // Pending mirror queue item should be reconciled by recovery.
    cmd(db.path().to_str().unwrap())
        .args(["sync", "list", "--status", "pending"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Total: 0"));

    cmd(db.path().to_str().unwrap())
        .args(["sync", "list", "--status", "completed"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Total: 1"));

    // Recovery and queue reconciliation must be visible in logs.
    cmd(db.path().to_str().unwrap())
        .args([
            "logs",
            "list",
            "--event-type",
            "recovery_success",
            "--file-id",
            "1",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Total: 1"));

    cmd(db.path().to_str().unwrap())
        .args([
            "logs",
            "list",
            "--event-type",
            "sync_completed",
            "--file-id",
            "1",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Total: 1"));
}

#[test]
fn test_integrity_check_all_clean() {
    let db = temp_db();
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();

    cmd(db.path().to_str().unwrap())
        .args([
            "drives",
            "add",
            "--no-validate",
            "p",
            primary.path().to_str().unwrap(),
            secondary.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    for name in &["x.txt", "y.txt"] {
        let content = format!("content of {}", name);
        fs::write(primary.path().join(name), content.as_bytes()).unwrap();
        fs::write(secondary.path().join(name), content.as_bytes()).unwrap();
        cmd(db.path().to_str().unwrap())
            .args(["files", "track", "1", name])
            .assert()
            .success();
    }

    cmd(db.path().to_str().unwrap())
        .args(["integrity", "check-all"])
        .assert()
        .success()
        .stdout(predicate::str::contains("2 checked"));
}
