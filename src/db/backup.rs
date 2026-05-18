use crate::db::repository::{DbBackupConfig, Repository};
use anyhow::{bail, Context};
use chrono::Utc;
use rusqlite::{Connection, DatabaseName, OpenFlags};
use serde::Serialize;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::thread;

const BACKUP_FILENAME: &str = "bitprotector.db";
const CHECKSUM_FILENAME: &str = "bitprotector.db.blake3";

/// Result of a single backup operation.
#[derive(Debug, Serialize)]
pub struct BackupResult {
    pub backup_config_id: i64,
    pub backup_path: String,
    pub status: String,
    pub error: Option<String>,
}

/// Result of a single backup integrity check.
#[derive(Debug, Serialize)]
pub struct BackupIntegrityResult {
    pub backup_config_id: i64,
    pub backup_path: String,
    pub status: String,
    pub checksum: Option<String>,
    pub repaired_from_id: Option<i64>,
    pub error: Option<String>,
}

/// Result of staging a database restore.
#[derive(Debug, Serialize)]
pub struct RestoreResult {
    pub status: String,
    pub restart_required: bool,
    pub safety_backup_path: String,
    pub staged_restore_path: String,
}

struct BackupHealth<'a> {
    config: &'a DbBackupConfig,
    path: PathBuf,
    checksum: Option<String>,
    error: Option<String>,
}

impl BackupHealth<'_> {
    fn is_healthy(&self) -> bool {
        self.error.is_none() && self.checksum.is_some()
    }
}

pub fn backup_file_path(config: &DbBackupConfig) -> PathBuf {
    Path::new(&config.backup_path).join(BACKUP_FILENAME)
}

fn checksum_file(path: &Path) -> anyhow::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0_u8; 8192];

    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    Ok(hasher.finalize().to_hex().to_string())
}

pub fn verify_sqlite_database(path: &Path) -> anyhow::Result<()> {
    if !path.is_file() {
        bail!("Database backup does not exist: {}", path.display());
    }

    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .with_context(|| format!("Failed to open SQLite database {}", path.display()))?;
    conn.busy_timeout(std::time::Duration::from_secs(5))
        .context("Failed to set busy_timeout on backup verification connection")?;
    let mut stmt = conn.prepare("PRAGMA integrity_check")?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
    for row in rows {
        let value = row?;
        if value != "ok" {
            bail!("SQLite integrity check failed: {}", value);
        }
    }
    Ok(())
}

fn create_sqlite_snapshot(db_path: &str, snapshot_path: &Path) -> anyhow::Result<()> {
    if let Some(parent) = snapshot_path.parent() {
        fs::create_dir_all(parent).context("Failed to create snapshot directory")?;
    }
    if snapshot_path.exists() {
        fs::remove_file(snapshot_path).context("Failed to remove old snapshot file")?;
    }

    let db_path = Path::new(db_path);
    if !db_path.is_file() {
        bail!("Live database does not exist: {}", db_path.display());
    }

    // Open read-write so we can issue a WAL checkpoint before snapshotting.
    // The checkpoint flushes WAL pages into the main file, making the WAL
    // smaller when the backup reader opens and reducing SQLITE_BUSY_SNAPSHOT
    // risk for concurrent writers during the backup.
    let source = Connection::open(db_path)
        .with_context(|| format!("Failed to open live database {}", db_path.display()))?;
    source
        .busy_timeout(std::time::Duration::from_secs(30))
        .context("Failed to set busy_timeout on snapshot connection")?;
    // Best-effort: errors here are non-fatal (backup proceeds regardless).
    let _ = source.execute_batch("PRAGMA wal_checkpoint(PASSIVE)");
    source
        .backup(DatabaseName::Main, snapshot_path, None)
        .context("Failed to create SQLite backup snapshot")?;
    verify_sqlite_database(snapshot_path)?;
    Ok(())
}

fn write_backup_file(snapshot_path: &Path, config: &DbBackupConfig) -> anyhow::Result<PathBuf> {
    let dest_dir = Path::new(&config.backup_path);
    fs::create_dir_all(dest_dir).context("Failed to create backup directory")?;

    let final_path = backup_file_path(config);
    let tmp_path = dest_dir.join(format!(
        ".{}.{}.tmp",
        BACKUP_FILENAME,
        Utc::now().format("%Y%m%d%H%M%S%f")
    ));

    fs::copy(snapshot_path, &tmp_path).context("Failed to copy database backup")?;
    verify_sqlite_database(&tmp_path)?;
    let checksum = checksum_file(&tmp_path)?;
    fs::rename(&tmp_path, &final_path).context("Failed to install database backup atomically")?;
    fs::write(dest_dir.join(CHECKSUM_FILENAME), checksum)
        .context("Failed to write database backup checksum")?;
    Ok(final_path)
}

fn copy_backup_atomically(source: &Path, dest: &Path) -> anyhow::Result<()> {
    let dest_dir = dest
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Backup destination has no parent directory"))?;
    fs::create_dir_all(dest_dir).context("Failed to create backup directory")?;
    let tmp_path = dest_dir.join(format!(
        ".{}.repair.{}.tmp",
        BACKUP_FILENAME,
        Utc::now().format("%Y%m%d%H%M%S%f")
    ));
    fs::copy(source, &tmp_path).context("Failed to copy healthy database backup")?;
    verify_sqlite_database(&tmp_path)?;
    fs::rename(&tmp_path, dest).context("Failed to replace corrupt database backup")?;
    let checksum = checksum_file(dest)?;
    fs::write(dest_dir.join(CHECKSUM_FILENAME), checksum)
        .context("Failed to write repaired backup checksum")?;
    Ok(())
}

/// Execute a backup of the database to a specific destination.
pub fn backup_to_destination(db_path: &str, config: &DbBackupConfig) -> anyhow::Result<String> {
    let snapshot_dir =
        tempfile::TempDir::new().context("Failed to create backup snapshot tempdir")?;
    let snapshot_path = snapshot_dir.path().join(BACKUP_FILENAME);
    create_sqlite_snapshot(db_path, &snapshot_path)?;
    let backup_path = write_backup_file(&snapshot_path, config)?;
    Ok(backup_path.to_string_lossy().to_string())
}

/// Run backups to all enabled destinations.
pub fn run_all_backups(repo: &Repository, db_path: &str) -> anyhow::Result<Vec<BackupResult>> {
    let configs = repo.list_db_backup_configs()?;
    let enabled: Vec<_> = configs.iter().filter(|config| config.enabled).collect();
    if enabled.is_empty() {
        return Ok(Vec::new());
    }

    let snapshot_dir =
        tempfile::TempDir::new().context("Failed to create backup snapshot tempdir")?;
    let snapshot_path = snapshot_dir.path().join(BACKUP_FILENAME);
    create_sqlite_snapshot(db_path, &snapshot_path)?;

    let mut results = Vec::new();
    for config in enabled {
        let result = match write_backup_file(&snapshot_path, config) {
            Ok(path) => {
                repo.update_db_backup_last_backup(config.id).ok();
                BackupResult {
                    backup_config_id: config.id,
                    backup_path: path.to_string_lossy().to_string(),
                    status: "success".to_string(),
                    error: None,
                }
            }
            Err(e) => {
                let message = e.to_string();
                repo.update_db_backup_error(config.id, Some(&message)).ok();
                BackupResult {
                    backup_config_id: config.id,
                    backup_path: backup_file_path(config).to_string_lossy().to_string(),
                    status: "failed".to_string(),
                    error: Some(message),
                }
            }
        };
        results.push(result);
    }
    repo.mark_db_backup_settings_backup_run().ok();

    Ok(results)
}

fn inspect_backup(config: &DbBackupConfig) -> BackupHealth<'_> {
    let path = backup_file_path(config);
    if !path.exists() {
        return BackupHealth {
            config,
            path,
            checksum: None,
            error: Some("missing".to_string()),
        };
    }

    match verify_sqlite_database(&path).and_then(|_| checksum_file(&path)) {
        Ok(checksum) => BackupHealth {
            config,
            path,
            checksum: Some(checksum),
            error: None,
        },
        Err(e) => BackupHealth {
            config,
            path,
            checksum: None,
            error: Some(e.to_string()),
        },
    }
}

/// Retry a status write up to `max_attempts` times with linear back-off.
/// Returns `(true, None)` on success, or `(false, Some(last_error_message))` after all attempts fail.
fn retry_status_write(
    repo: &Repository,
    config_id: i64,
    status: &str,
    error: Option<&str>,
    max_attempts: u32,
    label: &str,
) -> (bool, Option<String>) {
    let mut last_error: Option<String> = None;
    for attempt in 0..max_attempts {
        match repo.update_db_backup_integrity_status(config_id, status, error) {
            Ok(_) => return (true, None),
            Err(e) => {
                last_error = Some(e.to_string());
                if attempt < max_attempts - 1 {
                    tracing::warn!(
                        "Retrying {} write (attempt {}): {}",
                        label,
                        attempt + 1,
                        last_error.as_deref().unwrap_or("")
                    );
                    thread::sleep(std::time::Duration::from_millis(500 * (attempt as u64 + 1)));
                } else {
                    tracing::error!(
                        "Failed to write {} after {} attempts: {}",
                        label,
                        max_attempts,
                        last_error.as_deref().unwrap_or("")
                    );
                }
            }
        }
    }
    (false, last_error)
}

/// Verify configured backups and repair corrupt or missing copies from a healthy peer.
pub fn run_backup_integrity_check(repo: &Repository) -> anyhow::Result<Vec<BackupIntegrityResult>> {
    let configs = repo.list_db_backup_configs()?;
    let enabled: Vec<_> = configs.iter().filter(|config| config.enabled).collect();
    let health: Vec<_> = enabled
        .iter()
        .map(|config| inspect_backup(config))
        .collect();
    let primary_healthy = health.first().filter(|item| item.is_healthy());
    let repair_source = primary_healthy
        .or_else(|| health.iter().find(|item| item.is_healthy()))
        .map(|item| (item.config.id, item.path.clone()));

    let mut results = Vec::new();
    for item in health {
        if item.is_healthy() {
            let checksum = item.checksum.clone();
            // If the backup was previously in an error state (corrupt/missing/failed)
            // but is now healthy, it was repaired. This catches the case where a prior
            // repair run physically fixed the file but failed to persist the "repaired"
            // status to the database (e.g., due to transient DB contention).
            let status = match item.config.last_integrity_status.as_deref() {
                Some("corrupt") | Some("missing") | Some("failed") => "repaired",
                _ => "ok",
            };
            repo.update_db_backup_integrity_status(item.config.id, status, None)
                .ok();
            results.push(BackupIntegrityResult {
                backup_config_id: item.config.id,
                backup_path: item.path.to_string_lossy().to_string(),
                status: status.to_string(),
                checksum,
                repaired_from_id: None,
                error: None,
            });
            continue;
        }

        if let Some((source_id, source_path)) = &repair_source {
            if *source_id != item.config.id {
                // Persist the detected error state BEFORE attempting the physical repair.
                // If the post-repair "repaired" status write later fails under DB contention,
                // the next integrity run will see this "corrupt"/"missing" marker, find the
                // file healthy, and correctly write "repaired" via the is_healthy() branch above.
                let pre_status = if item.path.exists() {
                    "corrupt"
                } else {
                    "missing"
                };
                let pre_label = format!("pre-repair marker for backup #{}", item.config.id);
                let (pre_write_ok, pre_write_err) = retry_status_write(
                    repo,
                    item.config.id,
                    pre_status,
                    item.error.as_deref(),
                    3,
                    &pre_label,
                );
                if !pre_write_ok {
                    // Cannot safely proceed: without the pre-repair marker the recovery path
                    // in is_healthy() cannot fire if the post-repair write also fails.
                    // Defer to the next integrity cycle.
                    let err_msg = format!(
                        "pre-repair marker write failed; repair deferred to next integrity check: {}",
                        pre_write_err.as_deref().unwrap_or("unknown error")
                    );
                    results.push(BackupIntegrityResult {
                        backup_config_id: item.config.id,
                        backup_path: item.path.to_string_lossy().to_string(),
                        status: pre_status.to_string(),
                        checksum: None,
                        repaired_from_id: None,
                        error: Some(err_msg),
                    });
                    continue;
                }
                match copy_backup_atomically(source_path, &item.path)
                    .and_then(|_| checksum_file(&item.path))
                {
                    Ok(checksum) => {
                        // Retry the status write: a transient SQLITE_BUSY_SNAPSHOT on this
                        // call would otherwise silently drop the "repaired" record, causing
                        // the repair to be invisible to the API and tests.
                        let repair_label =
                            format!("repaired-status for backup #{}", item.config.id);
                        let (write_ok, _) = retry_status_write(
                            repo,
                            item.config.id,
                            "repaired",
                            None,
                            6,
                            &repair_label,
                        );
                        if !write_ok {
                            // Physical repair succeeded but status write failed.
                            // The pre-repair "corrupt"/"missing" marker written above ensures
                            // the next integrity check will see a healthy file with a prior
                            // error status and write "repaired" correctly.
                            tracing::error!(
                                "Could not persist repaired status for backup #{} after retries; \
                                 it will be reflected on the next integrity check",
                                item.config.id
                            );
                        }
                        results.push(BackupIntegrityResult {
                            backup_config_id: item.config.id,
                            backup_path: item.path.to_string_lossy().to_string(),
                            status: "repaired".to_string(),
                            checksum: Some(checksum),
                            repaired_from_id: Some(*source_id),
                            error: None,
                        });
                        continue;
                    }
                    Err(e) => {
                        let message = e.to_string();
                        repo.update_db_backup_integrity_status(
                            item.config.id,
                            "failed",
                            Some(&message),
                        )
                        .ok();
                        results.push(BackupIntegrityResult {
                            backup_config_id: item.config.id,
                            backup_path: item.path.to_string_lossy().to_string(),
                            status: "failed".to_string(),
                            checksum: None,
                            repaired_from_id: Some(*source_id),
                            error: Some(message),
                        });
                        continue;
                    }
                }
            }
        }

        let status = if item.path.exists() {
            "corrupt"
        } else {
            "missing"
        };
        let message = item
            .error
            .clone()
            .unwrap_or_else(|| "No healthy backup available for repair".to_string());
        repo.update_db_backup_integrity_status(item.config.id, status, Some(&message))
            .ok();
        results.push(BackupIntegrityResult {
            backup_config_id: item.config.id,
            backup_path: item.path.to_string_lossy().to_string(),
            status: status.to_string(),
            checksum: None,
            repaired_from_id: None,
            error: Some(message),
        });
    }
    repo.mark_db_backup_settings_integrity_run().ok();

    Ok(results)
}

pub fn pending_restore_path(db_path: &str) -> PathBuf {
    PathBuf::from(format!("{}.restore-pending", db_path))
}

fn safety_backup_path(db_path: &str) -> PathBuf {
    PathBuf::from(format!(
        "{}.safety-{}",
        db_path,
        Utc::now().format("%Y%m%d%H%M%S")
    ))
}

/// Stage a verified backup for restore. The pending file is applied on service startup.
pub fn stage_restore(db_path: &str, source_path: &str) -> anyhow::Result<RestoreResult> {
    let source = Path::new(source_path);
    verify_sqlite_database(source)?;

    let safety_path = safety_backup_path(db_path);
    fs::copy(db_path, &safety_path).context("Failed to create current database safety backup")?;
    verify_sqlite_database(&safety_path)?;

    let staged_path = pending_restore_path(db_path);
    fs::copy(source, &staged_path).context("Failed to stage database restore")?;
    verify_sqlite_database(&staged_path)?;

    Ok(RestoreResult {
        status: "staged".to_string(),
        restart_required: true,
        safety_backup_path: safety_path.to_string_lossy().to_string(),
        staged_restore_path: staged_path.to_string_lossy().to_string(),
    })
}

/// Apply a pending restore before the application opens its SQLite pool.
pub fn apply_pending_restore(db_path: &str) -> anyhow::Result<Option<PathBuf>> {
    let staged_path = pending_restore_path(db_path);
    if !staged_path.exists() {
        return Ok(None);
    }

    verify_sqlite_database(&staged_path)?;
    let applied_safety_path = safety_backup_path(db_path);
    if Path::new(db_path).exists() {
        fs::copy(db_path, &applied_safety_path)
            .context("Failed to create pre-restore safety backup")?;
    }
    fs::rename(&staged_path, db_path).context("Failed to apply staged database restore")?;
    Ok(Some(applied_safety_path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repository::{create_memory_pool, DbBackupConfig, Repository};
    use crate::db::schema::initialize_schema;
    use tempfile::{NamedTempFile, TempDir};

    fn make_repo() -> Repository {
        let pool = create_memory_pool().unwrap();
        {
            let conn = pool.get().unwrap();
            initialize_schema(&conn).unwrap();
        }
        Repository::new(pool)
    }

    fn make_config(id: i64, backup_path: &str, priority: i64) -> DbBackupConfig {
        DbBackupConfig {
            id,
            backup_path: backup_path.to_string(),
            drive_label: None,
            priority,
            enabled: true,
            last_backup: None,
            last_integrity_check: None,
            last_integrity_status: None,
            last_error: None,
            created_at: "".to_string(),
        }
    }

    fn make_sqlite_file() -> NamedTempFile {
        let file = NamedTempFile::new().unwrap();
        let conn = Connection::open(file.path()).unwrap();
        conn.execute(
            "CREATE TABLE sample (id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO sample (name) VALUES ('alpha')", [])
            .unwrap();
        drop(conn);
        file
    }

    #[test]
    fn test_backup_writes_canonical_database_file() {
        let backup_dir = TempDir::new().unwrap();
        let db_file = make_sqlite_file();

        let config = make_config(1, backup_dir.path().to_str().unwrap(), 0);
        let backup_path = backup_to_destination(db_file.path().to_str().unwrap(), &config).unwrap();

        assert_eq!(
            Path::new(&backup_path).file_name().unwrap(),
            BACKUP_FILENAME
        );
        verify_sqlite_database(Path::new(&backup_path)).unwrap();
    }

    #[test]
    fn test_repeated_backup_overwrites_single_canonical_file() {
        let backup_dir = TempDir::new().unwrap();
        let db_file = make_sqlite_file();
        let config = make_config(1, backup_dir.path().to_str().unwrap(), 0);

        backup_to_destination(db_file.path().to_str().unwrap(), &config).unwrap();
        backup_to_destination(db_file.path().to_str().unwrap(), &config).unwrap();

        let db_entries: Vec<_> = fs::read_dir(backup_dir.path())
            .unwrap()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_name() == BACKUP_FILENAME)
            .collect();
        assert_eq!(db_entries.len(), 1);
    }

    #[test]
    fn test_run_backups_skips_disabled_and_uses_priority_order() {
        let repo = make_repo();
        let first = TempDir::new().unwrap();
        let second = TempDir::new().unwrap();
        let disabled = TempDir::new().unwrap();
        let db_file = make_sqlite_file();

        let cfg1 = repo
            .create_db_backup_config(first.path().to_str().unwrap(), None, true)
            .unwrap();
        let cfg2 = repo
            .create_db_backup_config(second.path().to_str().unwrap(), None, true)
            .unwrap();
        repo.create_db_backup_config(disabled.path().to_str().unwrap(), None, false)
            .unwrap();

        let results = run_all_backups(&repo, db_file.path().to_str().unwrap()).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].backup_config_id, cfg1.id);
        assert_eq!(results[1].backup_config_id, cfg2.id);
        assert!(first.path().join(BACKUP_FILENAME).exists());
        assert!(second.path().join(BACKUP_FILENAME).exists());
        assert!(!disabled.path().join(BACKUP_FILENAME).exists());
    }

    #[test]
    fn test_integrity_repairs_corrupt_primary_from_secondary() {
        let repo = make_repo();
        let primary = TempDir::new().unwrap();
        let secondary = TempDir::new().unwrap();
        let db_file = make_sqlite_file();
        repo.create_db_backup_config(primary.path().to_str().unwrap(), None, true)
            .unwrap();
        repo.create_db_backup_config(secondary.path().to_str().unwrap(), None, true)
            .unwrap();
        run_all_backups(&repo, db_file.path().to_str().unwrap()).unwrap();

        fs::write(primary.path().join(BACKUP_FILENAME), b"not sqlite").unwrap();

        let results = run_backup_integrity_check(&repo).unwrap();
        assert_eq!(results[0].status, "repaired");
        verify_sqlite_database(&primary.path().join(BACKUP_FILENAME)).unwrap();
    }

    #[test]
    fn test_integrity_repairs_corrupt_secondary_from_primary() {
        let repo = make_repo();
        let primary = TempDir::new().unwrap();
        let secondary = TempDir::new().unwrap();
        let db_file = make_sqlite_file();
        repo.create_db_backup_config(primary.path().to_str().unwrap(), None, true)
            .unwrap();
        repo.create_db_backup_config(secondary.path().to_str().unwrap(), None, true)
            .unwrap();
        run_all_backups(&repo, db_file.path().to_str().unwrap()).unwrap();

        fs::write(secondary.path().join(BACKUP_FILENAME), b"not sqlite").unwrap();

        let results = run_backup_integrity_check(&repo).unwrap();
        assert_eq!(results[1].status, "repaired");
        verify_sqlite_database(&secondary.path().join(BACKUP_FILENAME)).unwrap();
    }

    #[test]
    fn test_integrity_reports_failure_when_no_healthy_backup_exists() {
        let repo = make_repo();
        let primary = TempDir::new().unwrap();
        let secondary = TempDir::new().unwrap();
        repo.create_db_backup_config(primary.path().to_str().unwrap(), None, true)
            .unwrap();
        repo.create_db_backup_config(secondary.path().to_str().unwrap(), None, true)
            .unwrap();
        fs::write(primary.path().join(BACKUP_FILENAME), b"not sqlite").unwrap();
        fs::write(secondary.path().join(BACKUP_FILENAME), b"also not sqlite").unwrap();

        let results = run_backup_integrity_check(&repo).unwrap();
        assert!(results.iter().all(|result| result.status == "corrupt"));
    }

    #[test]
    fn test_missing_backup_is_repaired_from_healthy_backup() {
        let repo = make_repo();
        let primary = TempDir::new().unwrap();
        let secondary = TempDir::new().unwrap();
        let db_file = make_sqlite_file();
        repo.create_db_backup_config(primary.path().to_str().unwrap(), None, true)
            .unwrap();
        repo.create_db_backup_config(secondary.path().to_str().unwrap(), None, true)
            .unwrap();
        run_all_backups(&repo, db_file.path().to_str().unwrap()).unwrap();

        fs::remove_file(secondary.path().join(BACKUP_FILENAME)).unwrap();

        let results = run_backup_integrity_check(&repo).unwrap();
        assert_eq!(results[1].status, "repaired");
        assert!(secondary.path().join(BACKUP_FILENAME).exists());
    }

    #[test]
    fn test_restore_stages_valid_backup_and_creates_safety_copy() {
        let current_db = make_sqlite_file();
        let restore_db = make_sqlite_file();

        let result = stage_restore(
            current_db.path().to_str().unwrap(),
            restore_db.path().to_str().unwrap(),
        )
        .unwrap();

        assert!(result.restart_required);
        assert!(Path::new(&result.safety_backup_path).exists());
        assert!(Path::new(&result.staged_restore_path).exists());
    }

    #[test]
    fn test_restore_rejects_corrupt_backup() {
        let current_db = make_sqlite_file();
        let corrupt = NamedTempFile::new().unwrap();
        fs::write(corrupt.path(), b"not sqlite").unwrap();

        let result = stage_restore(
            current_db.path().to_str().unwrap(),
            corrupt.path().to_str().unwrap(),
        );
        assert!(result.is_err());
    }

    /// When the DB is read-only every `update_db_backup_integrity_status` call fails with
    /// SQLITE_READONLY (returned immediately, no busy-wait).  The pre-repair marker write
    /// therefore exhausts its retries and the function must:
    ///  1. Return the deferred status for the corrupt backup (no panic / no error propagation).
    ///  2. NOT physically overwrite the corrupt file.
    ///  3. Include the underlying DB error in the result `error` field.
    #[test]
    fn test_integrity_defers_repair_when_status_write_fails() {
        use crate::db::repository::create_pool;
        use r2d2::Pool;
        use r2d2_sqlite::SqliteConnectionManager;

        // Use a file-based repo so we can reopen it in explicit read-only mode.
        let db_dir = TempDir::new().unwrap();
        let db_path = db_dir.path().join("test.db");
        let db_path_str = db_path.to_str().unwrap();
        let pool = create_pool(db_path_str).unwrap();
        {
            let conn = pool.get().unwrap();
            initialize_schema(&conn).unwrap();
        }
        let repo = Repository::new(pool);

        let primary = TempDir::new().unwrap();
        let secondary = TempDir::new().unwrap();
        let source_db = make_sqlite_file();
        repo.create_db_backup_config(primary.path().to_str().unwrap(), None, true)
            .unwrap();
        repo.create_db_backup_config(secondary.path().to_str().unwrap(), None, true)
            .unwrap();
        run_all_backups(&repo, source_db.path().to_str().unwrap()).unwrap();

        // Corrupt the primary so a repair will be attempted.
        fs::write(primary.path().join(BACKUP_FILENAME), b"not sqlite").unwrap();

        // Drop the original pool so all connections are closed before reopening.
        drop(repo);

        // Reopen in explicit read-only mode: SELECTs succeed, writes return SQLITE_READONLY
        // immediately (no busy-wait), so the retry loop exhausts quickly.
        let manager =
            SqliteConnectionManager::file(db_path_str).with_flags(OpenFlags::SQLITE_OPEN_READ_ONLY);
        let ro_pool = Pool::builder().max_size(1).build(manager).unwrap();
        let ro_repo = Repository::new(ro_pool);

        let results = run_backup_integrity_check(&ro_repo).unwrap();

        // Primary: pre-repair status write failed → repair deferred, status stays "corrupt".
        assert_eq!(results[0].status, "corrupt");
        let err = results[0].error.as_deref().unwrap_or("");
        assert!(
            err.contains("deferred"),
            "expected 'deferred' in error, got: {err}"
        );

        // The corrupt file must not have been physically replaced.
        assert_eq!(
            fs::read(primary.path().join(BACKUP_FILENAME)).unwrap(),
            b"not sqlite",
            "corrupt file must not be overwritten when the pre-repair status write fails"
        );
    }
}
