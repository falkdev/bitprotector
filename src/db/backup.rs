use crate::db::repository::{DbBackupConfig, Repository};
use anyhow::Context;
use chrono::Utc;
use std::fs;
use std::path::Path;

/// Result of a single backup operation.
#[derive(Debug)]
pub struct BackupResult {
    pub backup_config_id: i64,
    pub backup_path: String,
    pub status: String,
    pub error: Option<String>,
}

/// Execute a backup of the database to a specific destination.
pub fn backup_to_destination(db_path: &str, config: &DbBackupConfig) -> anyhow::Result<String> {
    let dest_dir = Path::new(&config.backup_path);
    if !dest_dir.exists() {
        fs::create_dir_all(dest_dir).context("Failed to create backup directory")?;
    }

    let timestamp = Utc::now().format("%Y-%m-%dT%H%M%S");
    let backup_filename = format!("bitprotector-{}.db", timestamp);
    let backup_path = dest_dir.join(&backup_filename);

    fs::copy(db_path, &backup_path).context("Failed to copy database")?;

    // Apply rotation (keep max_copies most recent)
    rotate_backups(dest_dir, config.max_copies)?;

    Ok(backup_path.to_string_lossy().to_string())
}

/// Remove oldest backup files keeping only max_copies.
fn rotate_backups(backup_dir: &Path, max_copies: i64) -> anyhow::Result<()> {
    let mut entries: Vec<_> = fs::read_dir(backup_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_str()
                .map(|n| n.starts_with("bitprotector-") && n.ends_with(".db"))
                .unwrap_or(false)
        })
        .collect();

    entries.sort_by_key(|e| e.file_name());

    while entries.len() as i64 > max_copies {
        let oldest = entries.remove(0);
        fs::remove_file(oldest.path()).context("Failed to remove old backup")?;
    }

    Ok(())
}

/// Run backups to all enabled destinations.
pub fn run_all_backups(repo: &Repository, db_path: &str) -> anyhow::Result<Vec<BackupResult>> {
    let configs = repo.list_db_backup_configs()?;
    let mut results = Vec::new();

    for config in &configs {
        if !config.enabled {
            continue;
        }

        let result = match backup_to_destination(db_path, config) {
            Ok(path) => {
                repo.update_db_backup_last_backup(config.id).ok();
                BackupResult {
                    backup_config_id: config.id,
                    backup_path: path,
                    status: "success".to_string(),
                    error: None,
                }
            }
            Err(e) => BackupResult {
                backup_config_id: config.id,
                backup_path: String::new(),
                status: "failed".to_string(),
                error: Some(e.to_string()),
            },
        };
        results.push(result);
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repository::{create_memory_pool, DbBackupConfig, Repository};
    use crate::db::schema::initialize_schema;
    use std::io::Write;
    use tempfile::{NamedTempFile, TempDir};

    fn make_config(id: i64, backup_path: &str, max_copies: i64) -> DbBackupConfig {
        DbBackupConfig {
            id,
            backup_path: backup_path.to_string(),
            drive_label: None,
            max_copies,
            enabled: true,
            last_backup: None,
            created_at: "".to_string(),
        }
    }

    #[test]
    fn test_backup_creates_exact_duplicate() {
        let backup_dir = TempDir::new().unwrap();
        let mut db_file = NamedTempFile::new().unwrap();
        let content = b"database content 12345";
        db_file.write_all(content).unwrap();
        db_file.flush().unwrap();

        let config = make_config(1, backup_dir.path().to_str().unwrap(), 5);
        let backup_path = backup_to_destination(db_file.path().to_str().unwrap(), &config).unwrap();

        let backup_content = fs::read(&backup_path).unwrap();
        assert_eq!(backup_content, content, "Backup should be byte-identical");
    }

    #[test]
    fn test_rotation_deletes_oldest_when_max_exceeded() {
        let backup_dir = TempDir::new().unwrap();
        let mut db_file = NamedTempFile::new().unwrap();
        db_file.write_all(b"db").unwrap();
        db_file.flush().unwrap();

        let config = make_config(1, backup_dir.path().to_str().unwrap(), 2);

        // Create 3 backups
        backup_to_destination(db_file.path().to_str().unwrap(), &config).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1100)); // ensure different timestamps
        backup_to_destination(db_file.path().to_str().unwrap(), &config).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1100));
        backup_to_destination(db_file.path().to_str().unwrap(), &config).unwrap();

        let entries: Vec<_> = fs::read_dir(backup_dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_str()
                    .map(|n| n.ends_with(".db"))
                    .unwrap_or(false)
            })
            .collect();

        assert_eq!(entries.len(), 2, "Should keep only max_copies=2 backups");
    }

    #[test]
    fn test_backup_to_multiple_destinations() {
        let pool = create_memory_pool().unwrap();
        {
            let conn = pool.get().unwrap();
            initialize_schema(&conn).unwrap();
        }
        let repo = Repository::new(pool);

        let dest1 = TempDir::new().unwrap();
        let dest2 = TempDir::new().unwrap();
        repo.create_db_backup_config(dest1.path().to_str().unwrap(), None, 3, true)
            .unwrap();
        repo.create_db_backup_config(dest2.path().to_str().unwrap(), None, 3, true)
            .unwrap();

        let mut db_file = NamedTempFile::new().unwrap();
        db_file.write_all(b"db").unwrap();
        db_file.flush().unwrap();

        let results = run_all_backups(&repo, db_file.path().to_str().unwrap()).unwrap();
        assert_eq!(results.len(), 2);
        for r in &results {
            assert_eq!(r.status, "success");
        }
    }
}
