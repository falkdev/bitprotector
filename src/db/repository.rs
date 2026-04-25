use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::Result;

pub type DbPool = Pool<SqliteConnectionManager>;

/// Create a connection pool for the given database path.
pub fn create_pool(db_path: &str) -> anyhow::Result<DbPool> {
    let manager = SqliteConnectionManager::file(db_path).with_init(|conn| {
        conn.execute_batch(
            "PRAGMA busy_timeout = 5000; PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;",
        )?;
        Ok(())
    });
    let pool = Pool::builder().max_size(10).build(manager)?;
    Ok(pool)
}

/// Create a single-connection pool for CLI commands that share the DB with a running service.
pub fn create_cli_pool(db_path: &str) -> anyhow::Result<DbPool> {
    let manager = SqliteConnectionManager::file(db_path).with_init(|conn| {
        conn.execute_batch(
            "PRAGMA busy_timeout = 5000; PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;",
        )?;
        Ok(())
    });
    let pool = Pool::builder().max_size(1).build(manager)?;
    Ok(pool)
}

/// Create an in-memory pool for testing.
pub fn create_memory_pool() -> anyhow::Result<DbPool> {
    let manager = SqliteConnectionManager::memory().with_init(|conn| {
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")?;
        Ok(())
    });
    let pool = Pool::builder().max_size(1).build(manager)?;
    Ok(pool)
}

/// Represents a drive pair record.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DrivePair {
    pub id: i64,
    pub name: String,
    pub primary_path: String,
    pub secondary_path: String,
    pub primary_state: String,
    pub secondary_state: String,
    pub active_role: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Represents a tracked file record.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TrackedFile {
    pub id: i64,
    pub drive_pair_id: i64,
    pub relative_path: String,
    pub checksum: String,
    pub file_size: i64,
    pub virtual_path: Option<String>,
    pub is_mirrored: bool,
    pub tracked_direct: bool,
    pub tracked_via_folder: bool,
    pub last_integrity_check_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Represents a tracked folder record.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TrackedFolder {
    pub id: i64,
    pub drive_pair_id: i64,
    pub folder_path: String,
    pub virtual_path: Option<String>,
    pub last_scanned_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrackingItemKind {
    File,
    Folder,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrackingSource {
    Direct,
    Folder,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrackingFolderStatus {
    NotScanned,
    Empty,
    Tracked,
    Mirrored,
    Partial,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TrackingItem {
    pub kind: TrackingItemKind,
    pub id: i64,
    pub drive_pair_id: i64,
    pub path: String,
    pub virtual_path: Option<String>,
    pub is_mirrored: Option<bool>,
    pub tracked_direct: Option<bool>,
    pub tracked_via_folder: Option<bool>,
    pub source: TrackingSource,
    pub folder_status: Option<TrackingFolderStatus>,
    pub folder_total_files: Option<i64>,
    pub folder_mirrored_files: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VirtualPathTreeNode {
    pub name: String,
    pub path: String,
    pub item_count: i64,
    pub has_children: bool,
}

/// Represents a sync queue item.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SyncQueueItem {
    pub id: i64,
    pub tracked_file_id: i64,
    pub action: String,
    pub status: String,
    pub error_message: Option<String>,
    pub created_at: String,
    pub completed_at: Option<String>,
}

/// Represents an event log entry.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EventLogEntry {
    pub id: i64,
    pub event_type: String,
    pub tracked_file_id: Option<i64>,
    pub file_path: Option<String>,
    pub message: String,
    pub details: Option<String>,
    pub created_at: String,
}

/// Represents a schedule configuration.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ScheduleConfig {
    pub id: i64,
    pub task_type: String,
    pub cron_expr: Option<String>,
    pub interval_seconds: Option<i64>,
    pub enabled: bool,
    pub last_run: Option<String>,
    pub next_run: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Represents a database backup configuration.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DbBackupConfig {
    pub id: i64,
    pub backup_path: String,
    pub drive_label: Option<String>,
    pub max_copies: i64,
    pub enabled: bool,
    pub last_backup: Option<String>,
    pub created_at: String,
}

/// Represents an integrity run summary.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IntegrityRun {
    pub id: i64,
    pub scope_drive_pair_id: Option<i64>,
    pub recover: bool,
    pub trigger: String,
    pub status: String,
    pub total_files: i64,
    pub processed_files: i64,
    pub attention_files: i64,
    pub recovered_files: i64,
    pub stop_requested: bool,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub error_message: Option<String>,
}

/// Represents a single integrity run file result.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IntegrityRunResult {
    pub id: i64,
    pub run_id: i64,
    pub file_id: i64,
    pub drive_pair_id: i64,
    pub relative_path: String,
    pub status: String,
    pub recovered: bool,
    pub needs_attention: bool,
    pub checked_at: String,
}

/// Data access repository providing CRUD operations on all entities.
#[derive(Clone)]
pub struct Repository {
    pool: DbPool,
}

impl Repository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    fn conn(&self) -> anyhow::Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        Ok(self.pool.get()?)
    }

    // ─── Drive Pairs ────────────────────────────────────────────────────────────

    pub fn create_drive_pair(
        &self,
        name: &str,
        primary: &str,
        secondary: &str,
    ) -> anyhow::Result<DrivePair> {
        let id = {
            let conn = self.conn()?;
            conn.execute(
                "INSERT INTO drive_pairs (
                    name, primary_path, secondary_path, primary_state, secondary_state, active_role
                 ) VALUES (?1, ?2, ?3, 'active', 'active', 'primary')",
                rusqlite::params![name, primary, secondary],
            )?;
            conn.last_insert_rowid()
        };
        self.get_drive_pair(id)
    }

    pub fn get_drive_pair(&self, id: i64) -> anyhow::Result<DrivePair> {
        let conn = self.conn()?;
        let pair = conn.query_row(
            "SELECT id, name, primary_path, secondary_path, primary_state, secondary_state,
                    active_role, created_at, updated_at
             FROM drive_pairs WHERE id = ?1",
            rusqlite::params![id],
            |row| {
                Ok(DrivePair {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    primary_path: row.get(2)?,
                    secondary_path: row.get(3)?,
                    primary_state: row.get(4)?,
                    secondary_state: row.get(5)?,
                    active_role: row.get(6)?,
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
                })
            },
        )?;
        Ok(pair)
    }

    pub fn list_drive_pairs(&self) -> anyhow::Result<Vec<DrivePair>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, primary_path, secondary_path, primary_state, secondary_state,
                    active_role, created_at, updated_at
             FROM drive_pairs ORDER BY id",
        )?;
        let pairs = stmt
            .query_map([], |row| {
                Ok(DrivePair {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    primary_path: row.get(2)?,
                    secondary_path: row.get(3)?,
                    primary_state: row.get(4)?,
                    secondary_state: row.get(5)?,
                    active_role: row.get(6)?,
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(pairs)
    }

    pub fn update_drive_pair(
        &self,
        id: i64,
        name: Option<&str>,
        primary: Option<&str>,
        secondary: Option<&str>,
    ) -> anyhow::Result<DrivePair> {
        {
            let conn = self.conn()?;
            if let Some(n) = name {
                conn.execute(
                    "UPDATE drive_pairs SET name=?1, updated_at=datetime('now') WHERE id=?2",
                    rusqlite::params![n, id],
                )?;
            }
            if let Some(p) = primary {
                conn.execute("UPDATE drive_pairs SET primary_path=?1, updated_at=datetime('now') WHERE id=?2", rusqlite::params![p, id])?;
            }
            if let Some(s) = secondary {
                conn.execute("UPDATE drive_pairs SET secondary_path=?1, updated_at=datetime('now') WHERE id=?2", rusqlite::params![s, id])?;
            }
        }
        self.get_drive_pair(id)
    }

    pub fn update_drive_pair_runtime(
        &self,
        id: i64,
        primary_path: Option<&str>,
        secondary_path: Option<&str>,
        primary_state: Option<&str>,
        secondary_state: Option<&str>,
        active_role: Option<&str>,
    ) -> anyhow::Result<DrivePair> {
        {
            let conn = self.conn()?;
            if let Some(path) = primary_path {
                conn.execute(
                    "UPDATE drive_pairs SET primary_path=?1, updated_at=datetime('now') WHERE id=?2",
                    rusqlite::params![path, id],
                )?;
            }
            if let Some(path) = secondary_path {
                conn.execute(
                    "UPDATE drive_pairs SET secondary_path=?1, updated_at=datetime('now') WHERE id=?2",
                    rusqlite::params![path, id],
                )?;
            }
            if let Some(state) = primary_state {
                conn.execute(
                    "UPDATE drive_pairs SET primary_state=?1, updated_at=datetime('now') WHERE id=?2",
                    rusqlite::params![state, id],
                )?;
            }
            if let Some(state) = secondary_state {
                conn.execute(
                    "UPDATE drive_pairs SET secondary_state=?1, updated_at=datetime('now') WHERE id=?2",
                    rusqlite::params![state, id],
                )?;
            }
            if let Some(role) = active_role {
                conn.execute(
                    "UPDATE drive_pairs SET active_role=?1, updated_at=datetime('now') WHERE id=?2",
                    rusqlite::params![role, id],
                )?;
            }
        }
        self.get_drive_pair(id)
    }

    pub fn delete_drive_pair(&self, id: i64) -> anyhow::Result<()> {
        let conn = self.conn()?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM tracked_files WHERE drive_pair_id=?1",
            rusqlite::params![id],
            |r| r.get(0),
        )?;
        if count > 0 {
            anyhow::bail!(
                "Cannot delete drive pair: {} files are still tracked",
                count
            );
        }
        conn.execute("DELETE FROM drive_pairs WHERE id=?1", rusqlite::params![id])?;
        Ok(())
    }

    pub fn mark_drive_pair_unmirrored(&self, drive_pair_id: i64) -> anyhow::Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE tracked_files SET is_mirrored=0, updated_at=datetime('now') WHERE drive_pair_id=?1",
            rusqlite::params![drive_pair_id],
        )?;
        Ok(())
    }

    pub fn count_tracked_files_for_drive_pair(&self, drive_pair_id: i64) -> anyhow::Result<i64> {
        let conn = self.conn()?;
        let count = conn.query_row(
            "SELECT COUNT(*) FROM tracked_files WHERE drive_pair_id=?1",
            rusqlite::params![drive_pair_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    // ─── Tracked Files ──────────────────────────────────────────────────────────

    pub fn create_tracked_file(
        &self,
        drive_pair_id: i64,
        relative_path: &str,
        checksum: &str,
        file_size: i64,
        virtual_path: Option<&str>,
    ) -> anyhow::Result<TrackedFile> {
        self.create_tracked_file_with_source(
            drive_pair_id,
            relative_path,
            checksum,
            file_size,
            virtual_path,
            true,
            false,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_tracked_file_with_source(
        &self,
        drive_pair_id: i64,
        relative_path: &str,
        checksum: &str,
        file_size: i64,
        virtual_path: Option<&str>,
        tracked_direct: bool,
        tracked_via_folder: bool,
    ) -> anyhow::Result<TrackedFile> {
        let id = {
            let conn = self.conn()?;
            conn.execute(
                "INSERT INTO tracked_files (
                    drive_pair_id, relative_path, checksum, file_size, virtual_path,
                    tracked_direct, tracked_via_folder
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![
                    drive_pair_id,
                    relative_path,
                    checksum,
                    file_size,
                    virtual_path,
                    tracked_direct as i64,
                    tracked_via_folder as i64
                ],
            )?;
            conn.last_insert_rowid()
        };
        self.get_tracked_file(id)
    }

    pub fn get_tracked_file(&self, id: i64) -> anyhow::Result<TrackedFile> {
        let conn = self.conn()?;
        let file = conn.query_row(
            "SELECT id, drive_pair_id, relative_path, checksum, file_size, virtual_path,
                    is_mirrored, tracked_direct, tracked_via_folder,
                    last_integrity_check_at, created_at, updated_at
             FROM tracked_files WHERE id=?1",
            rusqlite::params![id],
            |row| {
                Ok(TrackedFile {
                    id: row.get(0)?,
                    drive_pair_id: row.get(1)?,
                    relative_path: row.get(2)?,
                    checksum: row.get(3)?,
                    file_size: row.get(4)?,
                    virtual_path: row.get(5)?,
                    is_mirrored: row.get::<_, i64>(6)? != 0,
                    tracked_direct: row.get::<_, i64>(7)? != 0,
                    tracked_via_folder: row.get::<_, i64>(8)? != 0,
                    last_integrity_check_at: row.get(9)?,
                    created_at: row.get(10)?,
                    updated_at: row.get(11)?,
                })
            },
        )?;
        Ok(file)
    }

    pub fn get_tracked_file_by_path(
        &self,
        drive_pair_id: i64,
        relative_path: &str,
    ) -> anyhow::Result<TrackedFile> {
        let conn = self.conn()?;
        let file = conn.query_row(
            "SELECT id, drive_pair_id, relative_path, checksum, file_size, virtual_path,
                    is_mirrored, tracked_direct, tracked_via_folder,
                    last_integrity_check_at, created_at, updated_at
             FROM tracked_files WHERE drive_pair_id=?1 AND relative_path=?2",
            rusqlite::params![drive_pair_id, relative_path],
            |row| {
                Ok(TrackedFile {
                    id: row.get(0)?,
                    drive_pair_id: row.get(1)?,
                    relative_path: row.get(2)?,
                    checksum: row.get(3)?,
                    file_size: row.get(4)?,
                    virtual_path: row.get(5)?,
                    is_mirrored: row.get::<_, i64>(6)? != 0,
                    tracked_direct: row.get::<_, i64>(7)? != 0,
                    tracked_via_folder: row.get::<_, i64>(8)? != 0,
                    last_integrity_check_at: row.get(9)?,
                    created_at: row.get(10)?,
                    updated_at: row.get(11)?,
                })
            },
        )?;
        Ok(file)
    }

    pub fn list_tracked_files(
        &self,
        drive_pair_id: Option<i64>,
        virtual_path_prefix: Option<&str>,
        is_mirrored: Option<bool>,
        page: i64,
        per_page: i64,
    ) -> anyhow::Result<(Vec<TrackedFile>, i64)> {
        use rusqlite::types::Value;
        let conn = self.conn()?;
        let offset = (page - 1) * per_page;

        let mut params: Vec<Value> = Vec::new();
        let mut conditions = Vec::new();
        if let Some(id) = drive_pair_id {
            conditions.push(format!("drive_pair_id = ?{}", params.len() + 1));
            params.push(Value::Integer(id));
        }
        if let Some(prefix) = virtual_path_prefix {
            conditions.push(format!("virtual_path LIKE ?{} || '%'", params.len() + 1));
            params.push(Value::Text(prefix.to_string()));
        }
        if let Some(mirrored) = is_mirrored {
            conditions.push(format!("is_mirrored = ?{}", params.len() + 1));
            params.push(Value::Integer(mirrored as i64));
        }
        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        let query = format!(
            "SELECT id, drive_pair_id, relative_path, checksum, file_size, virtual_path,
                    is_mirrored, tracked_direct, tracked_via_folder,
                    last_integrity_check_at, created_at, updated_at
             FROM tracked_files {where_clause} ORDER BY id LIMIT {per_page} OFFSET {offset}"
        );
        let count_query = format!("SELECT COUNT(*) FROM tracked_files {where_clause}");

        let files = conn
            .prepare(&query)?
            .query_map(rusqlite::params_from_iter(params.iter()), |row| {
                Ok(TrackedFile {
                    id: row.get(0)?,
                    drive_pair_id: row.get(1)?,
                    relative_path: row.get(2)?,
                    checksum: row.get(3)?,
                    file_size: row.get(4)?,
                    virtual_path: row.get(5)?,
                    is_mirrored: row.get::<_, i64>(6)? != 0,
                    tracked_direct: row.get::<_, i64>(7)? != 0,
                    tracked_via_folder: row.get::<_, i64>(8)? != 0,
                    last_integrity_check_at: row.get(9)?,
                    created_at: row.get(10)?,
                    updated_at: row.get(11)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let total: i64 = conn.query_row(
            &count_query,
            rusqlite::params_from_iter(params.iter()),
            |r| r.get(0),
        )?;
        Ok((files, total))
    }

    pub fn update_tracked_file_checksum(
        &self,
        id: i64,
        checksum: &str,
        file_size: i64,
    ) -> anyhow::Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE tracked_files SET checksum=?1, file_size=?2, updated_at=datetime('now') WHERE id=?3",
            rusqlite::params![checksum, file_size, id],
        )?;
        Ok(())
    }

    pub fn update_tracked_file_mirror_status(
        &self,
        id: i64,
        is_mirrored: bool,
    ) -> anyhow::Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE tracked_files SET is_mirrored=?1, updated_at=datetime('now') WHERE id=?2",
            rusqlite::params![is_mirrored as i64, id],
        )?;
        Ok(())
    }

    pub fn update_tracked_file_last_integrity_check_at(&self, id: i64) -> anyhow::Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE tracked_files SET last_integrity_check_at=datetime('now'), updated_at=datetime('now') WHERE id=?1",
            rusqlite::params![id],
        )?;
        Ok(())
    }

    pub fn update_tracked_file_virtual_path(
        &self,
        id: i64,
        virtual_path: Option<&str>,
    ) -> anyhow::Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE tracked_files SET virtual_path=?1, updated_at=datetime('now') WHERE id=?2",
            rusqlite::params![virtual_path, id],
        )?;
        Ok(())
    }

    pub fn update_tracked_file_sources(
        &self,
        id: i64,
        tracked_direct: bool,
        tracked_via_folder: bool,
    ) -> anyhow::Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE tracked_files
             SET tracked_direct=?1, tracked_via_folder=?2, updated_at=datetime('now')
             WHERE id=?3",
            rusqlite::params![tracked_direct as i64, tracked_via_folder as i64, id],
        )?;
        Ok(())
    }

    pub fn recompute_folder_provenance_for_drive(&self, drive_pair_id: i64) -> anyhow::Result<()> {
        let conn = self.conn()?;
        // Direct tracking wins over folder provenance. Keep tracked_via_folder only for rows
        // that are currently not directly tracked.
        conn.execute(
            "UPDATE tracked_files
             SET tracked_via_folder=0, updated_at=datetime('now')
             WHERE drive_pair_id=?1",
            rusqlite::params![drive_pair_id],
        )?;
        conn.execute(
            "UPDATE tracked_files
             SET tracked_via_folder=1, updated_at=datetime('now')
             WHERE drive_pair_id=?1
               AND tracked_direct=0
               AND EXISTS (
                    SELECT 1
                    FROM tracked_folders
                    WHERE tracked_folders.drive_pair_id = tracked_files.drive_pair_id
                      AND (
                            tracked_files.relative_path = rtrim(tracked_folders.folder_path, '/')
                            OR tracked_files.relative_path LIKE rtrim(tracked_folders.folder_path, '/') || '/%'
                      )
               )",
            rusqlite::params![drive_pair_id],
        )?;
        conn.execute(
            "UPDATE tracked_files
             SET tracked_direct=1, updated_at=datetime('now')
             WHERE drive_pair_id=?1
               AND tracked_direct=0
               AND tracked_via_folder=0",
            rusqlite::params![drive_pair_id],
        )?;
        Ok(())
    }

    pub fn delete_tracked_file(&self, id: i64) -> anyhow::Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "DELETE FROM sync_queue WHERE tracked_file_id=?1",
            rusqlite::params![id],
        )?;
        conn.execute(
            "DELETE FROM tracked_files WHERE id=?1",
            rusqlite::params![id],
        )?;
        Ok(())
    }

    pub fn count_tracked_files(&self, drive_pair_id: Option<i64>) -> anyhow::Result<i64> {
        let conn = self.conn()?;
        let count = if let Some(id) = drive_pair_id {
            conn.query_row(
                "SELECT COUNT(*) FROM tracked_files WHERE drive_pair_id=?1",
                rusqlite::params![id],
                |row| row.get(0),
            )?
        } else {
            conn.query_row("SELECT COUNT(*) FROM tracked_files", [], |row| row.get(0))?
        };
        Ok(count)
    }

    // ─── Integrity Runs ──────────────────────────────────────────────────────────

    pub fn create_integrity_run(
        &self,
        scope_drive_pair_id: Option<i64>,
        recover: bool,
        trigger: &str,
        total_files: i64,
    ) -> anyhow::Result<IntegrityRun> {
        let id = {
            let conn = self.conn()?;
            conn.execute(
                "INSERT INTO integrity_runs (
                    scope_drive_pair_id, recover, trigger, status, total_files
                 ) VALUES (?1, ?2, ?3, 'running', ?4)",
                rusqlite::params![scope_drive_pair_id, recover as i64, trigger, total_files],
            )?;
            conn.last_insert_rowid()
        };
        self.get_integrity_run(id)
    }

    pub fn get_integrity_run(&self, id: i64) -> anyhow::Result<IntegrityRun> {
        let conn = self.conn()?;
        let run = conn.query_row(
            "SELECT id, scope_drive_pair_id, recover, trigger, status,
                    total_files, processed_files, attention_files, recovered_files,
                    stop_requested, started_at, ended_at, error_message
             FROM integrity_runs
             WHERE id=?1",
            rusqlite::params![id],
            |row| {
                Ok(IntegrityRun {
                    id: row.get(0)?,
                    scope_drive_pair_id: row.get(1)?,
                    recover: row.get::<_, i64>(2)? != 0,
                    trigger: row.get(3)?,
                    status: row.get(4)?,
                    total_files: row.get(5)?,
                    processed_files: row.get(6)?,
                    attention_files: row.get(7)?,
                    recovered_files: row.get(8)?,
                    stop_requested: row.get::<_, i64>(9)? != 0,
                    started_at: row.get(10)?,
                    ended_at: row.get(11)?,
                    error_message: row.get(12)?,
                })
            },
        )?;
        Ok(run)
    }

    pub fn get_active_integrity_run(&self) -> anyhow::Result<Option<IntegrityRun>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, scope_drive_pair_id, recover, trigger, status,
                    total_files, processed_files, attention_files, recovered_files,
                    stop_requested, started_at, ended_at, error_message
             FROM integrity_runs
             WHERE status IN ('running', 'stopping')
             ORDER BY started_at DESC, id DESC
             LIMIT 1",
        )?;
        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? {
            return Ok(Some(IntegrityRun {
                id: row.get(0)?,
                scope_drive_pair_id: row.get(1)?,
                recover: row.get::<_, i64>(2)? != 0,
                trigger: row.get(3)?,
                status: row.get(4)?,
                total_files: row.get(5)?,
                processed_files: row.get(6)?,
                attention_files: row.get(7)?,
                recovered_files: row.get(8)?,
                stop_requested: row.get::<_, i64>(9)? != 0,
                started_at: row.get(10)?,
                ended_at: row.get(11)?,
                error_message: row.get(12)?,
            }));
        }
        Ok(None)
    }

    pub fn get_latest_integrity_run(&self) -> anyhow::Result<Option<IntegrityRun>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, scope_drive_pair_id, recover, trigger, status,
                    total_files, processed_files, attention_files, recovered_files,
                    stop_requested, started_at, ended_at, error_message
             FROM integrity_runs
             ORDER BY started_at DESC, id DESC
             LIMIT 1",
        )?;
        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? {
            return Ok(Some(IntegrityRun {
                id: row.get(0)?,
                scope_drive_pair_id: row.get(1)?,
                recover: row.get::<_, i64>(2)? != 0,
                trigger: row.get(3)?,
                status: row.get(4)?,
                total_files: row.get(5)?,
                processed_files: row.get(6)?,
                attention_files: row.get(7)?,
                recovered_files: row.get(8)?,
                stop_requested: row.get::<_, i64>(9)? != 0,
                started_at: row.get(10)?,
                ended_at: row.get(11)?,
                error_message: row.get(12)?,
            }));
        }
        Ok(None)
    }

    pub fn request_integrity_run_stop(&self, id: i64) -> anyhow::Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE integrity_runs
             SET stop_requested=1,
                 status=CASE WHEN status='running' THEN 'stopping' ELSE status END
             WHERE id=?1",
            rusqlite::params![id],
        )?;
        Ok(())
    }

    pub fn finish_integrity_run(
        &self,
        id: i64,
        status: &str,
        error_message: Option<&str>,
    ) -> anyhow::Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE integrity_runs
             SET status=?1, ended_at=datetime('now'), error_message=?2
             WHERE id=?3",
            rusqlite::params![status, error_message, id],
        )?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn append_integrity_run_result(
        &self,
        run_id: i64,
        file_id: i64,
        drive_pair_id: i64,
        relative_path: &str,
        status: &str,
        recovered: bool,
        needs_attention: bool,
    ) -> anyhow::Result<IntegrityRunResult> {
        let id = {
            let conn = self.conn()?;
            conn.execute(
                "INSERT INTO integrity_run_results (
                    run_id, file_id, drive_pair_id, relative_path, status,
                    recovered, needs_attention
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![
                    run_id,
                    file_id,
                    drive_pair_id,
                    relative_path,
                    status,
                    recovered as i64,
                    needs_attention as i64
                ],
            )?;
            conn.last_insert_rowid()
        };

        let conn = self.conn()?;
        let result = conn.query_row(
            "SELECT id, run_id, file_id, drive_pair_id, relative_path, status,
                    recovered, needs_attention, checked_at
             FROM integrity_run_results
             WHERE id=?1",
            rusqlite::params![id],
            |row| {
                Ok(IntegrityRunResult {
                    id: row.get(0)?,
                    run_id: row.get(1)?,
                    file_id: row.get(2)?,
                    drive_pair_id: row.get(3)?,
                    relative_path: row.get(4)?,
                    status: row.get(5)?,
                    recovered: row.get::<_, i64>(6)? != 0,
                    needs_attention: row.get::<_, i64>(7)? != 0,
                    checked_at: row.get(8)?,
                })
            },
        )?;
        Ok(result)
    }

    pub fn increment_integrity_run_progress(
        &self,
        run_id: i64,
        attention_delta: i64,
        recovered_delta: i64,
    ) -> anyhow::Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE integrity_runs
             SET processed_files=processed_files + 1,
                 attention_files=attention_files + ?1,
                 recovered_files=recovered_files + ?2
             WHERE id=?3",
            rusqlite::params![attention_delta, recovered_delta, run_id],
        )?;
        Ok(())
    }

    pub fn list_integrity_run_results(
        &self,
        run_id: i64,
        issues_only: bool,
        page: i64,
        per_page: i64,
    ) -> anyhow::Result<(Vec<IntegrityRunResult>, i64)> {
        let conn = self.conn()?;
        let page = page.max(1);
        let per_page = per_page.clamp(1, 200);
        let offset = (page - 1) * per_page;

        let where_clause = if issues_only {
            "WHERE run_id=?1 AND needs_attention=1"
        } else {
            "WHERE run_id=?1"
        };

        let query = format!(
            "SELECT id, run_id, file_id, drive_pair_id, relative_path, status,
                    recovered, needs_attention, checked_at
             FROM integrity_run_results
             {where_clause}
             ORDER BY id
             LIMIT ?2 OFFSET ?3"
        );
        let count_query = format!("SELECT COUNT(*) FROM integrity_run_results {where_clause}");

        let results = conn
            .prepare(&query)?
            .query_map(rusqlite::params![run_id, per_page, offset], |row| {
                Ok(IntegrityRunResult {
                    id: row.get(0)?,
                    run_id: row.get(1)?,
                    file_id: row.get(2)?,
                    drive_pair_id: row.get(3)?,
                    relative_path: row.get(4)?,
                    status: row.get(5)?,
                    recovered: row.get::<_, i64>(6)? != 0,
                    needs_attention: row.get::<_, i64>(7)? != 0,
                    checked_at: row.get(8)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let total = conn.query_row(&count_query, rusqlite::params![run_id], |row| row.get(0))?;
        Ok((results, total))
    }

    // ─── Tracked Folders ─────────────────────────────────────────────────────────

    pub fn create_tracked_folder(
        &self,
        drive_pair_id: i64,
        folder_path: &str,
        virtual_path: Option<&str>,
    ) -> anyhow::Result<TrackedFolder> {
        let id = {
            let conn = self.conn()?;
            conn.execute(
                "INSERT INTO tracked_folders (drive_pair_id, folder_path, virtual_path)
                 VALUES (?1, ?2, ?3)",
                rusqlite::params![drive_pair_id, folder_path, virtual_path],
            )?;
            conn.last_insert_rowid()
        };
        self.get_tracked_folder(id)
    }

    pub fn get_tracked_folder(&self, id: i64) -> anyhow::Result<TrackedFolder> {
        let conn = self.conn()?;
        let folder = conn.query_row(
            "SELECT id, drive_pair_id, folder_path, virtual_path, last_scanned_at, created_at
             FROM tracked_folders WHERE id=?1",
            rusqlite::params![id],
            |row| {
                Ok(TrackedFolder {
                    id: row.get(0)?,
                    drive_pair_id: row.get(1)?,
                    folder_path: row.get(2)?,
                    virtual_path: row.get(3)?,
                    last_scanned_at: row.get(4)?,
                    created_at: row.get(5)?,
                })
            },
        )?;
        Ok(folder)
    }

    pub fn list_tracked_folders(&self) -> anyhow::Result<Vec<TrackedFolder>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, drive_pair_id, folder_path, virtual_path, last_scanned_at, created_at
             FROM tracked_folders ORDER BY id",
        )?;
        let folders = stmt
            .query_map([], |row| {
                Ok(TrackedFolder {
                    id: row.get(0)?,
                    drive_pair_id: row.get(1)?,
                    folder_path: row.get(2)?,
                    virtual_path: row.get(3)?,
                    last_scanned_at: row.get(4)?,
                    created_at: row.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(folders)
    }

    pub fn mark_tracked_folder_scanned(&self, id: i64) -> anyhow::Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE tracked_folders SET last_scanned_at=datetime('now') WHERE id=?1",
            rusqlite::params![id],
        )?;
        Ok(())
    }

    pub fn list_tracked_files_under_folder(
        &self,
        drive_pair_id: i64,
        folder_path: &str,
    ) -> anyhow::Result<Vec<TrackedFile>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, drive_pair_id, relative_path, checksum, file_size, virtual_path,
                    is_mirrored, tracked_direct, tracked_via_folder,
                    last_integrity_check_at, created_at, updated_at
             FROM tracked_files
             WHERE drive_pair_id = ?1
               AND (
                    relative_path = rtrim(?2, '/')
                    OR relative_path LIKE rtrim(?2, '/') || '/%'
               )
             ORDER BY id",
        )?;
        let files = stmt
            .query_map(rusqlite::params![drive_pair_id, folder_path], |row| {
                Ok(TrackedFile {
                    id: row.get(0)?,
                    drive_pair_id: row.get(1)?,
                    relative_path: row.get(2)?,
                    checksum: row.get(3)?,
                    file_size: row.get(4)?,
                    virtual_path: row.get(5)?,
                    is_mirrored: row.get::<_, i64>(6)? != 0,
                    tracked_direct: row.get::<_, i64>(7)? != 0,
                    tracked_via_folder: row.get::<_, i64>(8)? != 0,
                    last_integrity_check_at: row.get(9)?,
                    created_at: row.get(10)?,
                    updated_at: row.get(11)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(files)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn list_tracking_items(
        &self,
        drive_pair_id: Option<i64>,
        query: Option<&str>,
        virtual_prefix: Option<&str>,
        has_virtual_path: Option<bool>,
        item_kind: Option<&str>,
        source: Option<&str>,
        page: i64,
        per_page: i64,
    ) -> anyhow::Result<(Vec<TrackingItem>, i64)> {
        use rusqlite::types::Value;

        let conn = self.conn()?;
        let page = page.max(1);
        let per_page = per_page.clamp(1, 200);
        let offset = (page - 1) * per_page;

        let item_kind = item_kind.unwrap_or("all");
        let source = source.unwrap_or("all");

        let include_files = item_kind != "folder";
        let include_folders = item_kind != "file" && matches!(source, "all" | "folder");

        if !include_files && !include_folders {
            return Ok((Vec::new(), 0));
        }

        let mut params: Vec<Value> = Vec::new();
        let mut branches = Vec::new();

        if include_files {
            let effective_virtual_expr = "COALESCE(
                tf.virtual_path,
                (
                    SELECT CASE
                        WHEN tf.relative_path = rtrim(f.folder_path, '/') THEN f.virtual_path
                        ELSE f.virtual_path || '/' || substr(
                            tf.relative_path,
                            length(rtrim(f.folder_path, '/')) + 2
                        )
                    END
                    FROM tracked_folders f
                    WHERE f.drive_pair_id = tf.drive_pair_id
                      AND f.virtual_path IS NOT NULL
                      AND (
                            tf.relative_path = rtrim(f.folder_path, '/')
                            OR tf.relative_path LIKE rtrim(f.folder_path, '/') || '/%'
                      )
                    ORDER BY length(rtrim(f.folder_path, '/')) DESC
                    LIMIT 1
                )
            )";
            let mut conditions = Vec::new();
            if let Some(id) = drive_pair_id {
                conditions.push(format!("tf.drive_pair_id = ?{}", params.len() + 1));
                params.push(Value::Integer(id));
            }
            if let Some(raw_q) = query.map(str::trim).filter(|q| !q.is_empty()) {
                conditions.push(format!(
                    "(tf.relative_path LIKE ?{0} OR COALESCE({1}, '') LIKE ?{0})",
                    params.len() + 1,
                    effective_virtual_expr
                ));
                params.push(Value::Text(format!("%{}%", raw_q)));
            }
            if let Some(prefix) = virtual_prefix.map(str::trim).filter(|p| !p.is_empty()) {
                conditions.push(format!(
                    "{effective_virtual_expr} LIKE ?{} || '%'",
                    params.len() + 1
                ));
                params.push(Value::Text(prefix.to_string()));
            }
            if let Some(has_path) = has_virtual_path {
                conditions.push(if has_path {
                    format!("{effective_virtual_expr} IS NOT NULL")
                } else {
                    format!("{effective_virtual_expr} IS NULL")
                });
            }
            match source {
                "direct" => conditions.push("tf.tracked_direct = 1".to_string()),
                "folder" => conditions
                    .push("tf.tracked_direct = 0 AND tf.tracked_via_folder = 1".to_string()),
                _ => {}
            }

            let where_clause = if conditions.is_empty() {
                String::new()
            } else {
                format!("WHERE {}", conditions.join(" AND "))
            };

            branches.push(format!(
                "SELECT
                    'file' AS kind,
                    tf.id,
                    tf.drive_pair_id,
                    tf.relative_path AS path,
                    {effective_virtual_expr} AS virtual_path,
                    tf.is_mirrored,
                    tf.tracked_direct,
                    tf.tracked_via_folder,
                    CASE
                        WHEN tf.tracked_via_folder = 1 THEN 'folder'
                        ELSE 'direct'
                    END AS source,
                    NULL AS folder_status,
                    NULL AS folder_total_files,
                    NULL AS folder_mirrored_files,
                    tf.created_at,
                    tf.updated_at
                 FROM tracked_files tf {where_clause}"
            ));
        }

        if include_folders {
            let mut conditions = Vec::new();
            if let Some(id) = drive_pair_id {
                conditions.push(format!("tfolder.drive_pair_id = ?{}", params.len() + 1));
                params.push(Value::Integer(id));
            }
            if let Some(raw_q) = query.map(str::trim).filter(|q| !q.is_empty()) {
                conditions.push(format!(
                    "(tfolder.folder_path LIKE ?{0} OR COALESCE(tfolder.virtual_path, '') LIKE ?{0})",
                    params.len() + 1
                ));
                params.push(Value::Text(format!("%{}%", raw_q)));
            }
            if let Some(prefix) = virtual_prefix.map(str::trim).filter(|p| !p.is_empty()) {
                conditions.push(format!(
                    "tfolder.virtual_path LIKE ?{} || '%'",
                    params.len() + 1
                ));
                params.push(Value::Text(prefix.to_string()));
            }
            if let Some(has_path) = has_virtual_path {
                conditions.push(if has_path {
                    "tfolder.virtual_path IS NOT NULL".to_string()
                } else {
                    "tfolder.virtual_path IS NULL".to_string()
                });
            }

            let where_clause = if conditions.is_empty() {
                String::new()
            } else {
                format!("WHERE {}", conditions.join(" AND "))
            };

            branches.push(format!(
                "SELECT
                    'folder' AS kind,
                    tfolder.id,
                    tfolder.drive_pair_id,
                    tfolder.folder_path AS path,
                    tfolder.virtual_path,
                    NULL AS is_mirrored,
                    NULL AS tracked_direct,
                    NULL AS tracked_via_folder,
                    'folder' AS source,
                    CASE
                        WHEN COALESCE(stats.total_files, 0) = 0 AND tfolder.last_scanned_at IS NULL THEN 'not_scanned'
                        WHEN COALESCE(stats.total_files, 0) = 0 THEN 'empty'
                        WHEN COALESCE(stats.mirrored_files, 0) = COALESCE(stats.total_files, 0) THEN 'mirrored'
                        WHEN COALESCE(stats.mirrored_files, 0) = 0 THEN 'tracked'
                        ELSE 'partial'
                    END AS folder_status,
                    COALESCE(stats.total_files, 0) AS folder_total_files,
                    COALESCE(stats.mirrored_files, 0) AS folder_mirrored_files,
                    tfolder.created_at,
                    tfolder.created_at AS updated_at
                 FROM tracked_folders tfolder
                 LEFT JOIN (
                    SELECT
                        f.id AS folder_id,
                        COUNT(t.id) AS total_files,
                        COALESCE(SUM(CASE WHEN t.is_mirrored = 1 THEN 1 ELSE 0 END), 0) AS mirrored_files
                    FROM tracked_folders f
                    LEFT JOIN tracked_files t
                      ON t.drive_pair_id = f.drive_pair_id
                     AND (
                            t.relative_path = rtrim(f.folder_path, '/')
                            OR t.relative_path LIKE rtrim(f.folder_path, '/') || '/%'
                         )
                    GROUP BY f.id
                 ) stats ON stats.folder_id = tfolder.id
                 {where_clause}"
            ));
        }

        let union = branches.join(" UNION ALL ");
        let query_sql = format!(
            "SELECT kind, id, drive_pair_id, path, virtual_path, is_mirrored,
                    tracked_direct, tracked_via_folder, source,
                    folder_status, folder_total_files, folder_mirrored_files,
                    created_at, updated_at
             FROM ({union})
             ORDER BY kind, id
             LIMIT {per_page} OFFSET {offset}"
        );
        let count_sql = format!("SELECT COUNT(*) FROM ({union})");

        let items = conn
            .prepare(&query_sql)?
            .query_map(rusqlite::params_from_iter(params.iter()), |row| {
                let kind_text: String = row.get(0)?;
                let source_text: String = row.get(8)?;

                let kind = match kind_text.as_str() {
                    "folder" => TrackingItemKind::Folder,
                    _ => TrackingItemKind::File,
                };
                let source = match source_text.as_str() {
                    "folder" => TrackingSource::Folder,
                    _ => TrackingSource::Direct,
                };
                let folder_status =
                    row.get::<_, Option<String>>(9)?
                        .map(|status| match status.as_str() {
                            "not_scanned" => TrackingFolderStatus::NotScanned,
                            "empty" => TrackingFolderStatus::Empty,
                            "mirrored" => TrackingFolderStatus::Mirrored,
                            "partial" => TrackingFolderStatus::Partial,
                            _ => TrackingFolderStatus::Tracked,
                        });

                Ok(TrackingItem {
                    kind,
                    id: row.get(1)?,
                    drive_pair_id: row.get(2)?,
                    path: row.get(3)?,
                    virtual_path: row.get(4)?,
                    is_mirrored: row.get::<_, Option<i64>>(5)?.map(|value| value != 0),
                    tracked_direct: row.get::<_, Option<i64>>(6)?.map(|value| value != 0),
                    tracked_via_folder: row.get::<_, Option<i64>>(7)?.map(|value| value != 0),
                    source,
                    folder_status,
                    folder_total_files: row.get(10)?,
                    folder_mirrored_files: row.get(11)?,
                    created_at: row.get(12)?,
                    updated_at: row.get(13)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let total: i64 = conn.query_row(
            &count_sql,
            rusqlite::params_from_iter(params.iter()),
            |row| row.get(0),
        )?;

        Ok((items, total))
    }

    pub fn list_virtual_path_tree_nodes(
        &self,
        parent: &str,
    ) -> anyhow::Result<Vec<VirtualPathTreeNode>> {
        let conn = self.conn()?;
        let parent = if parent.trim().is_empty() {
            "/".to_string()
        } else {
            let mut normalized = parent.trim().to_string();
            if !normalized.starts_with('/') {
                normalized = format!("/{normalized}");
            }
            if normalized.len() > 1 {
                normalized = normalized.trim_end_matches('/').to_string();
            }
            normalized
        };

        let search_prefix = if parent == "/" {
            "/".to_string()
        } else {
            format!("{}/", parent)
        };
        let pattern = format!("{}%", search_prefix);
        let segment_start = search_prefix.len() as i64 + 1;

        let mut stmt = conn.prepare(
            "WITH file_paths AS (
                SELECT COALESCE(
                    tf.virtual_path,
                    (
                        SELECT CASE
                            WHEN tf.relative_path = rtrim(f.folder_path, '/') THEN f.virtual_path
                            ELSE f.virtual_path || '/' || substr(
                                tf.relative_path,
                                length(rtrim(f.folder_path, '/')) + 2
                            )
                        END
                        FROM tracked_folders f
                        WHERE f.drive_pair_id = tf.drive_pair_id
                          AND f.virtual_path IS NOT NULL
                          AND (
                                tf.relative_path = rtrim(f.folder_path, '/')
                                OR tf.relative_path LIKE rtrim(f.folder_path, '/') || '/%'
                          )
                        ORDER BY length(rtrim(f.folder_path, '/')) DESC
                        LIMIT 1
                    )
                ) AS virtual_path
                FROM tracked_files tf
            ),
            paths AS (
                SELECT virtual_path
                FROM file_paths
                WHERE virtual_path IS NOT NULL
                  AND virtual_path LIKE ?1
                UNION ALL
                SELECT virtual_path
                FROM tracked_folders
                WHERE virtual_path IS NOT NULL
                  AND virtual_path LIKE ?1
            ),
            suffixes AS (
                SELECT substr(virtual_path, ?2) AS suffix
                FROM paths
                WHERE length(virtual_path) >= (?2 - 1)
            ),
            segments AS (
                SELECT
                    CASE
                        WHEN instr(suffix, '/') = 0 THEN suffix
                        ELSE substr(suffix, 1, instr(suffix, '/') - 1)
                    END AS name,
                    CASE WHEN instr(suffix, '/') > 0 THEN 1 ELSE 0 END AS has_children
                FROM suffixes
                WHERE suffix <> ''
            )
            SELECT
                name,
                CASE WHEN ?3 = '/' THEN '/' || name ELSE ?3 || '/' || name END AS path,
                COUNT(*) AS item_count,
                MAX(has_children) AS has_children
            FROM segments
            WHERE name <> ''
            GROUP BY name
            ORDER BY name",
        )?;

        let nodes = stmt
            .query_map(rusqlite::params![&pattern, segment_start, &parent], |row| {
                Ok(VirtualPathTreeNode {
                    name: row.get(0)?,
                    path: row.get(1)?,
                    item_count: row.get(2)?,
                    has_children: row.get::<_, i64>(3)? != 0,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(nodes)
    }

    pub fn update_tracked_folder(
        &self,
        id: i64,
        virtual_path: Option<Option<&str>>,
    ) -> anyhow::Result<TrackedFolder> {
        {
            let conn = self.conn()?;
            if let Some(dvb) = virtual_path {
                conn.execute(
                    "UPDATE tracked_folders SET virtual_path=?1 WHERE id=?2",
                    rusqlite::params![dvb, id],
                )?;
            }
        }
        self.get_tracked_folder(id)
    }

    pub fn delete_tracked_folder(&self, id: i64) -> anyhow::Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "DELETE FROM tracked_folders WHERE id=?1",
            rusqlite::params![id],
        )?;
        Ok(())
    }

    // ─── Sync Queue ───────────────────────────────────────────────────────────────

    pub fn create_sync_queue_item(
        &self,
        tracked_file_id: i64,
        action: &str,
    ) -> anyhow::Result<SyncQueueItem> {
        let id = {
            let conn = self.conn()?;
            conn.execute(
                "INSERT INTO sync_queue (tracked_file_id, action) VALUES (?1, ?2)",
                rusqlite::params![tracked_file_id, action],
            )?;
            conn.last_insert_rowid()
        };
        self.get_sync_queue_item(id)
    }

    pub fn create_sync_queue_item_dedup(
        &self,
        tracked_file_id: i64,
        action: &str,
    ) -> anyhow::Result<Option<SyncQueueItem>> {
        self.create_sync_queue_item_dedup_with_created(tracked_file_id, action)
            .map(|(item, _created)| Some(item))
    }

    pub fn create_sync_queue_item_dedup_with_created(
        &self,
        tracked_file_id: i64,
        action: &str,
    ) -> anyhow::Result<(SyncQueueItem, bool)> {
        let conn = self.conn()?;
        let existing: Result<i64, _> = conn.query_row(
            "SELECT id FROM sync_queue
             WHERE tracked_file_id=?1 AND action=?2 AND status IN ('pending', 'in_progress')
             ORDER BY id LIMIT 1",
            rusqlite::params![tracked_file_id, action],
            |row| row.get(0),
        );
        match existing {
            Ok(id) => Ok((self.get_sync_queue_item(id)?, false)),
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                drop(conn);
                self.create_sync_queue_item(tracked_file_id, action)
                    .map(|item| (item, true))
            }
            Err(e) => Err(e.into()),
        }
    }

    pub fn get_sync_queue_item(&self, id: i64) -> anyhow::Result<SyncQueueItem> {
        let conn = self.conn()?;
        let item = conn.query_row(
            "SELECT id, tracked_file_id, action, status, error_message, created_at, completed_at
             FROM sync_queue WHERE id=?1",
            rusqlite::params![id],
            |row| {
                Ok(SyncQueueItem {
                    id: row.get(0)?,
                    tracked_file_id: row.get(1)?,
                    action: row.get(2)?,
                    status: row.get(3)?,
                    error_message: row.get(4)?,
                    created_at: row.get(5)?,
                    completed_at: row.get(6)?,
                })
            },
        )?;
        Ok(item)
    }

    pub fn list_sync_queue(
        &self,
        status: Option<&str>,
        page: i64,
        per_page: i64,
    ) -> anyhow::Result<(Vec<SyncQueueItem>, i64)> {
        let conn = self.conn()?;
        let offset = (page - 1) * per_page;
        let (where_clause, params_status) = if let Some(s) = status {
            (format!("WHERE status='{}'", s.replace('\'', "''")), true)
        } else {
            (String::new(), false)
        };
        let query = format!(
            "SELECT id, tracked_file_id, action, status, error_message, created_at, completed_at
             FROM sync_queue {where_clause} ORDER BY id LIMIT {per_page} OFFSET {offset}"
        );
        let count_query = format!("SELECT COUNT(*) FROM sync_queue {where_clause}");

        let items = conn
            .prepare(&query)?
            .query_map([], |row| {
                Ok(SyncQueueItem {
                    id: row.get(0)?,
                    tracked_file_id: row.get(1)?,
                    action: row.get(2)?,
                    status: row.get(3)?,
                    error_message: row.get(4)?,
                    created_at: row.get(5)?,
                    completed_at: row.get(6)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let _ = params_status; // suppress unused warning

        let total: i64 = conn.query_row(&count_query, [], |r| r.get(0))?;
        Ok((items, total))
    }

    pub fn clear_completed_sync_queue(&self) -> anyhow::Result<u64> {
        let conn = self.conn()?;
        let deleted = conn.execute("DELETE FROM sync_queue WHERE status='completed'", [])?;
        Ok(deleted as u64)
    }

    pub fn requeue_in_progress_sync_queue(&self) -> anyhow::Result<u64> {
        let conn = self.conn()?;
        let affected = conn.execute(
            "UPDATE sync_queue
             SET status='pending',
                 error_message=NULL,
                 completed_at=NULL
             WHERE status='in_progress'",
            [],
        )?;
        Ok(affected as u64)
    }

    pub fn update_sync_queue_status(
        &self,
        id: i64,
        status: &str,
        error_message: Option<&str>,
    ) -> anyhow::Result<()> {
        let conn = self.conn()?;
        let completed_at = if status == "completed" || status == "failed" {
            "datetime('now')"
        } else {
            "NULL"
        };
        conn.execute(
            &format!(
                "UPDATE sync_queue SET status=?1, error_message=?2, completed_at={} WHERE id=?3",
                completed_at
            ),
            rusqlite::params![status, error_message, id],
        )?;
        Ok(())
    }

    pub fn complete_pending_mirror_queue_for_file(
        &self,
        tracked_file_id: i64,
    ) -> anyhow::Result<u64> {
        let conn = self.conn()?;
        let affected = conn.execute(
            "UPDATE sync_queue
             SET status='completed',
                 error_message=NULL,
                 completed_at=datetime('now')
             WHERE tracked_file_id=?1
               AND action='mirror'
               AND status IN ('pending', 'in_progress')",
            rusqlite::params![tracked_file_id],
        )?;
        Ok(affected as u64)
    }

    pub fn fail_pending_sync_queue_for_drive_pair(
        &self,
        drive_pair_id: i64,
        error_message: &str,
    ) -> anyhow::Result<u64> {
        let conn = self.conn()?;
        let affected = conn.execute(
            "UPDATE sync_queue
             SET status='failed', error_message=?1, completed_at=datetime('now')
             WHERE status IN ('pending', 'in_progress')
               AND tracked_file_id IN (
                   SELECT id FROM tracked_files WHERE drive_pair_id=?2
               )",
            rusqlite::params![error_message, drive_pair_id],
        )?;
        Ok(affected as u64)
    }

    pub fn count_sync_queue_items_for_drive_pair_action(
        &self,
        drive_pair_id: i64,
        action: &str,
        statuses: &[&str],
    ) -> anyhow::Result<i64> {
        let conn = self.conn()?;
        let mut sql = format!(
            "SELECT COUNT(*) FROM sync_queue
             WHERE action='{}'
               AND tracked_file_id IN (
                   SELECT id FROM tracked_files WHERE drive_pair_id={}
               )",
            action.replace('\'', "''"),
            drive_pair_id
        );
        if !statuses.is_empty() {
            let joined = statuses
                .iter()
                .map(|status| format!("'{}'", status.replace('\'', "''")))
                .collect::<Vec<_>>()
                .join(", ");
            sql.push_str(&format!(" AND status IN ({joined})"));
        }
        let count = conn.query_row(&sql, [], |row| row.get(0))?;
        Ok(count)
    }

    // ─── Event Log ────────────────────────────────────────────────────────────────

    pub fn create_event_log(
        &self,
        event_type: &str,
        tracked_file_id: Option<i64>,
        message: &str,
        details: Option<&str>,
    ) -> anyhow::Result<EventLogEntry> {
        let id = {
            let conn = self.conn()?;
            conn.execute(
                "INSERT INTO event_log (event_type, tracked_file_id, message, details)
                 VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![event_type, tracked_file_id, message, details],
            )?;
            conn.last_insert_rowid()
        };
        self.get_event_log(id)
    }

    pub fn get_event_log(&self, id: i64) -> anyhow::Result<EventLogEntry> {
        let conn = self.conn()?;
        let entry = conn.query_row(
            "SELECT e.id, e.event_type, e.tracked_file_id,
                    CASE WHEN tf.id IS NOT NULL THEN dp.primary_path || '/' || tf.relative_path ELSE NULL END,
                    e.message, e.details, e.created_at
             FROM event_log e
             LEFT JOIN tracked_files tf ON e.tracked_file_id = tf.id
             LEFT JOIN drive_pairs dp ON tf.drive_pair_id = dp.id
             WHERE e.id=?1",
            rusqlite::params![id],
            |row| {
                Ok(EventLogEntry {
                    id: row.get(0)?,
                    event_type: row.get(1)?,
                    tracked_file_id: row.get(2)?,
                    file_path: row.get(3)?,
                    message: row.get(4)?,
                    details: row.get(5)?,
                    created_at: row.get(6)?,
                })
            },
        )?;
        Ok(entry)
    }

    pub fn list_event_logs(
        &self,
        event_type: Option<&str>,
        tracked_file_id: Option<i64>,
        from: Option<&str>,
        to: Option<&str>,
        page: i64,
        per_page: i64,
    ) -> anyhow::Result<(Vec<EventLogEntry>, i64)> {
        let conn = self.conn()?;
        let offset = (page - 1) * per_page;
        let mut conditions = Vec::new();
        if let Some(et) = event_type {
            conditions.push(format!("e.event_type='{}'", et.replace('\'', "''")));
        }
        if let Some(fid) = tracked_file_id {
            conditions.push(format!("e.tracked_file_id={}", fid));
        }
        if let Some(f) = from {
            conditions.push(format!("e.created_at>='{}'", f.replace('\'', "''")));
        }
        if let Some(t) = to {
            conditions.push(format!("e.created_at<='{}'", t.replace('\'', "''")));
        }
        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        let query = format!(
            "SELECT e.id, e.event_type, e.tracked_file_id,
                    CASE WHEN tf.id IS NOT NULL THEN dp.primary_path || '/' || tf.relative_path ELSE NULL END,
                    e.message, e.details, e.created_at
             FROM event_log e
             LEFT JOIN tracked_files tf ON e.tracked_file_id = tf.id
             LEFT JOIN drive_pairs dp ON tf.drive_pair_id = dp.id
             {where_clause} ORDER BY e.id DESC LIMIT {per_page} OFFSET {offset}"
        );
        let count_query = format!("SELECT COUNT(*) FROM event_log e {where_clause}");

        let entries = conn
            .prepare(&query)?
            .query_map([], |row| {
                Ok(EventLogEntry {
                    id: row.get(0)?,
                    event_type: row.get(1)?,
                    tracked_file_id: row.get(2)?,
                    file_path: row.get(3)?,
                    message: row.get(4)?,
                    details: row.get(5)?,
                    created_at: row.get(6)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let total: i64 = conn.query_row(&count_query, [], |r| r.get(0))?;
        Ok((entries, total))
    }

    // ─── Schedule Config ──────────────────────────────────────────────────────────

    pub fn create_schedule_config(
        &self,
        task_type: &str,
        cron_expr: Option<&str>,
        interval_seconds: Option<i64>,
        enabled: bool,
    ) -> anyhow::Result<ScheduleConfig> {
        let id = {
            let conn = self.conn()?;
            conn.execute(
                "INSERT INTO schedule_config (task_type, cron_expr, interval_seconds, enabled)
                 VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![task_type, cron_expr, interval_seconds, enabled as i64],
            )?;
            conn.last_insert_rowid()
        };
        self.get_schedule_config(id)
    }

    pub fn get_schedule_config(&self, id: i64) -> anyhow::Result<ScheduleConfig> {
        let conn = self.conn()?;
        let cfg = conn.query_row(
            "SELECT id, task_type, cron_expr, interval_seconds, enabled, last_run, next_run, created_at, updated_at
             FROM schedule_config WHERE id=?1",
            rusqlite::params![id],
            |row| {
                Ok(ScheduleConfig {
                    id: row.get(0)?,
                    task_type: row.get(1)?,
                    cron_expr: row.get(2)?,
                    interval_seconds: row.get(3)?,
                    enabled: row.get::<_, i64>(4)? != 0,
                    last_run: row.get(5)?,
                    next_run: row.get(6)?,
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
                })
            },
        )?;
        Ok(cfg)
    }

    pub fn list_schedule_configs(&self) -> anyhow::Result<Vec<ScheduleConfig>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, task_type, cron_expr, interval_seconds, enabled, last_run, next_run, created_at, updated_at
             FROM schedule_config ORDER BY id",
        )?;
        let cfgs = stmt
            .query_map([], |row| {
                Ok(ScheduleConfig {
                    id: row.get(0)?,
                    task_type: row.get(1)?,
                    cron_expr: row.get(2)?,
                    interval_seconds: row.get(3)?,
                    enabled: row.get::<_, i64>(4)? != 0,
                    last_run: row.get(5)?,
                    next_run: row.get(6)?,
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(cfgs)
    }

    pub fn update_schedule_config(
        &self,
        id: i64,
        cron_expr: Option<Option<&str>>,
        interval_seconds: Option<Option<i64>>,
        enabled: Option<bool>,
    ) -> anyhow::Result<ScheduleConfig> {
        {
            let conn = self.conn()?;
            if let Some(ce) = cron_expr {
                conn.execute("UPDATE schedule_config SET cron_expr=?1, updated_at=datetime('now') WHERE id=?2", rusqlite::params![ce, id])?;
            }
            if let Some(is) = interval_seconds {
                conn.execute("UPDATE schedule_config SET interval_seconds=?1, updated_at=datetime('now') WHERE id=?2", rusqlite::params![is, id])?;
            }
            if let Some(en) = enabled {
                conn.execute(
                    "UPDATE schedule_config SET enabled=?1, updated_at=datetime('now') WHERE id=?2",
                    rusqlite::params![en as i64, id],
                )?;
            }
        }
        self.get_schedule_config(id)
    }

    pub fn delete_schedule_config(&self, id: i64) -> anyhow::Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "DELETE FROM schedule_config WHERE id=?1",
            rusqlite::params![id],
        )?;
        Ok(())
    }

    // ─── DB Backup Config ─────────────────────────────────────────────────────────

    pub fn create_db_backup_config(
        &self,
        backup_path: &str,
        drive_label: Option<&str>,
        max_copies: i64,
        enabled: bool,
    ) -> anyhow::Result<DbBackupConfig> {
        let id = {
            let conn = self.conn()?;
            conn.execute(
                "INSERT INTO db_backup_config (backup_path, drive_label, max_copies, enabled)
                 VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![backup_path, drive_label, max_copies, enabled as i64],
            )?;
            conn.last_insert_rowid()
        };
        self.get_db_backup_config(id)
    }

    pub fn get_db_backup_config(&self, id: i64) -> anyhow::Result<DbBackupConfig> {
        let conn = self.conn()?;
        let cfg = conn.query_row(
            "SELECT id, backup_path, drive_label, max_copies, enabled, last_backup, created_at
             FROM db_backup_config WHERE id=?1",
            rusqlite::params![id],
            |row| {
                Ok(DbBackupConfig {
                    id: row.get(0)?,
                    backup_path: row.get(1)?,
                    drive_label: row.get(2)?,
                    max_copies: row.get(3)?,
                    enabled: row.get::<_, i64>(4)? != 0,
                    last_backup: row.get(5)?,
                    created_at: row.get(6)?,
                })
            },
        )?;
        Ok(cfg)
    }

    pub fn list_db_backup_configs(&self) -> anyhow::Result<Vec<DbBackupConfig>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, backup_path, drive_label, max_copies, enabled, last_backup, created_at
             FROM db_backup_config ORDER BY id",
        )?;
        let cfgs = stmt
            .query_map([], |row| {
                Ok(DbBackupConfig {
                    id: row.get(0)?,
                    backup_path: row.get(1)?,
                    drive_label: row.get(2)?,
                    max_copies: row.get(3)?,
                    enabled: row.get::<_, i64>(4)? != 0,
                    last_backup: row.get(5)?,
                    created_at: row.get(6)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(cfgs)
    }

    pub fn update_db_backup_config(
        &self,
        id: i64,
        max_copies: Option<i64>,
        enabled: Option<bool>,
    ) -> anyhow::Result<DbBackupConfig> {
        {
            let conn = self.conn()?;
            if let Some(mc) = max_copies {
                conn.execute(
                    "UPDATE db_backup_config SET max_copies=?1 WHERE id=?2",
                    rusqlite::params![mc, id],
                )?;
            }
            if let Some(en) = enabled {
                conn.execute(
                    "UPDATE db_backup_config SET enabled=?1 WHERE id=?2",
                    rusqlite::params![en as i64, id],
                )?;
            }
        }
        self.get_db_backup_config(id)
    }

    pub fn delete_db_backup_config(&self, id: i64) -> anyhow::Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "DELETE FROM db_backup_config WHERE id=?1",
            rusqlite::params![id],
        )?;
        Ok(())
    }

    pub fn update_db_backup_last_backup(&self, id: i64) -> anyhow::Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE db_backup_config SET last_backup=datetime('now') WHERE id=?1",
            rusqlite::params![id],
        )?;
        Ok(())
    }

    // ─── System Status ────────────────────────────────────────────────────────────

    pub fn get_system_status(&self) -> anyhow::Result<SystemStatus> {
        let conn = self.conn()?;
        let files_tracked: i64 =
            conn.query_row("SELECT COUNT(*) FROM tracked_files", [], |r| r.get(0))?;
        let files_mirrored: i64 = conn.query_row(
            "SELECT COUNT(*) FROM tracked_files WHERE is_mirrored=1",
            [],
            |r| r.get(0),
        )?;
        let pending_sync: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sync_queue WHERE status='pending'",
            [],
            |r| r.get(0),
        )?;
        let integrity_issues: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sync_queue WHERE action IN ('user_action_required') AND status='pending'",
            [], |r| r.get(0)
        )?;
        let drive_pairs: i64 =
            conn.query_row("SELECT COUNT(*) FROM drive_pairs", [], |r| r.get(0))?;
        Ok(SystemStatus {
            files_tracked,
            files_mirrored,
            pending_sync,
            integrity_issues,
            drive_pairs,
        })
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct SystemStatus {
    pub files_tracked: i64,
    pub files_mirrored: i64,
    pub pending_sync: i64,
    pub integrity_issues: i64,
    pub drive_pairs: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::schema::initialize_schema;
    use std::thread;
    use tempfile::NamedTempFile;

    fn make_repo() -> Repository {
        let pool = create_memory_pool().expect("Failed to create pool");
        {
            let conn = pool.get().expect("Failed to get connection");
            initialize_schema(&conn).expect("Schema init failed");
        }
        Repository::new(pool)
    }

    // ─── Drive Pairs ────────────────────────────────────────────────────────────

    #[test]
    fn test_drive_pair_crud() {
        let repo = make_repo();

        // Create
        let pair = repo
            .create_drive_pair("test-pair", "/primary", "/secondary")
            .unwrap();
        assert_eq!(pair.name, "test-pair");
        assert_eq!(pair.primary_path, "/primary");
        assert_eq!(pair.secondary_path, "/secondary");

        // List
        let pairs = repo.list_drive_pairs().unwrap();
        assert_eq!(pairs.len(), 1);

        // Get
        let fetched = repo.get_drive_pair(pair.id).unwrap();
        assert_eq!(fetched.id, pair.id);

        // Update
        let updated = repo
            .update_drive_pair(pair.id, Some("new-name"), None, None)
            .unwrap();
        assert_eq!(updated.name, "new-name");

        // Delete (no files tracked)
        repo.delete_drive_pair(pair.id).unwrap();
        let pairs_after = repo.list_drive_pairs().unwrap();
        assert!(pairs_after.is_empty());
    }

    #[test]
    fn test_delete_drive_pair_with_files_fails() {
        let repo = make_repo();
        let pair = repo.create_drive_pair("p", "/a", "/b").unwrap();
        repo.create_tracked_file(pair.id, "file.txt", "abc123", 100, None)
            .unwrap();
        let result = repo.delete_drive_pair(pair.id);
        assert!(result.is_err(), "Delete should fail when files are tracked");
    }

    // ─── Tracked Files ──────────────────────────────────────────────────────────

    #[test]
    fn test_tracked_file_crud() {
        let repo = make_repo();
        let pair = repo.create_drive_pair("p", "/a", "/b").unwrap();

        let file = repo
            .create_tracked_file(
                pair.id,
                "docs/report.pdf",
                "blake3hex",
                1024,
                Some("/docs/report.pdf"),
            )
            .unwrap();
        assert_eq!(file.relative_path, "docs/report.pdf");
        assert_eq!(file.checksum, "blake3hex");
        assert_eq!(file.file_size, 1024);
        assert!(file.virtual_path.is_some());
        assert!(!file.is_mirrored);

        repo.update_tracked_file_mirror_status(file.id, true)
            .unwrap();
        let updated = repo.get_tracked_file(file.id).unwrap();
        assert!(updated.is_mirrored);

        repo.update_tracked_file_checksum(file.id, "newhash", 2048)
            .unwrap();
        let updated2 = repo.get_tracked_file(file.id).unwrap();
        assert_eq!(updated2.checksum, "newhash");
        assert_eq!(updated2.file_size, 2048);

        repo.delete_tracked_file(file.id).unwrap();
        assert!(repo.get_tracked_file(file.id).is_err());
    }

    #[test]
    fn test_tracked_file_list_pagination() {
        let repo = make_repo();
        let pair = repo.create_drive_pair("p", "/a", "/b").unwrap();
        for i in 0..5 {
            repo.create_tracked_file(pair.id, &format!("file{}.txt", i), "hash", 100, None)
                .unwrap();
        }
        let (files, total) = repo.list_tracked_files(None, None, None, 1, 3).unwrap();
        assert_eq!(files.len(), 3);
        assert_eq!(total, 5);

        let (files2, _) = repo.list_tracked_files(None, None, None, 2, 3).unwrap();
        assert_eq!(files2.len(), 2);
    }

    // ─── Tracked Folders ─────────────────────────────────────────────────────────

    #[test]
    fn test_tracked_folder_crud() {
        let repo = make_repo();
        let pair = repo.create_drive_pair("p", "/a", "/b").unwrap();

        let folder = repo
            .create_tracked_folder(pair.id, "documents/", Some("/docs"))
            .unwrap();
        assert_eq!(folder.folder_path, "documents/");
        assert_eq!(folder.virtual_path.as_deref(), Some("/docs"));
        assert!(folder.last_scanned_at.is_none());

        repo.mark_tracked_folder_scanned(folder.id).unwrap();
        let scanned = repo.get_tracked_folder(folder.id).unwrap();
        assert!(scanned.last_scanned_at.is_some());

        let updated = repo.update_tracked_folder(folder.id, Some(None)).unwrap();
        assert!(updated.virtual_path.is_none());

        repo.delete_tracked_folder(folder.id).unwrap();
        assert!(repo.get_tracked_folder(folder.id).is_err());
    }

    // ─── Sync Queue ───────────────────────────────────────────────────────────────

    #[test]
    fn test_sync_queue_crud() {
        let repo = make_repo();
        let pair = repo.create_drive_pair("p", "/a", "/b").unwrap();
        let file = repo
            .create_tracked_file(pair.id, "f.txt", "h", 1, None)
            .unwrap();

        let item = repo.create_sync_queue_item(file.id, "mirror").unwrap();
        assert_eq!(item.action, "mirror");
        assert_eq!(item.status, "pending");

        repo.update_sync_queue_status(item.id, "completed", None)
            .unwrap();
        let updated = repo.get_sync_queue_item(item.id).unwrap();
        assert_eq!(updated.status, "completed");
        assert!(updated.completed_at.is_some());
    }

    #[test]
    fn test_clear_completed_sync_queue_only_removes_completed() {
        let repo = make_repo();
        let pair = repo.create_drive_pair("p", "/a", "/b").unwrap();
        let file1 = repo
            .create_tracked_file(pair.id, "f1.txt", "h1", 1, None)
            .unwrap();
        let file2 = repo
            .create_tracked_file(pair.id, "f2.txt", "h2", 1, None)
            .unwrap();
        let file3 = repo
            .create_tracked_file(pair.id, "f3.txt", "h3", 1, None)
            .unwrap();

        let completed = repo.create_sync_queue_item(file1.id, "mirror").unwrap();
        repo.update_sync_queue_status(completed.id, "completed", None)
            .unwrap();

        let pending = repo.create_sync_queue_item(file2.id, "verify").unwrap();

        let failed = repo
            .create_sync_queue_item(file3.id, "restore_master")
            .unwrap();
        repo.update_sync_queue_status(failed.id, "failed", Some("forced failure"))
            .unwrap();

        let deleted = repo.clear_completed_sync_queue().unwrap();
        assert_eq!(deleted, 1);

        let (items, total) = repo.list_sync_queue(None, 1, 50).unwrap();
        assert_eq!(total, 2);
        assert!(items.iter().all(|item| item.status != "completed"));
        assert!(items.iter().any(|item| item.id == pending.id));
        assert!(items.iter().any(|item| item.id == failed.id));
    }

    #[test]
    fn test_clear_completed_sync_queue_preserves_in_flight() {
        let repo = make_repo();
        let pair = repo.create_drive_pair("p", "/a", "/b").unwrap();
        let file1 = repo
            .create_tracked_file(pair.id, "f1.txt", "h1", 1, None)
            .unwrap();
        let file2 = repo
            .create_tracked_file(pair.id, "f2.txt", "h2", 1, None)
            .unwrap();
        let file3 = repo
            .create_tracked_file(pair.id, "f3.txt", "h3", 1, None)
            .unwrap();

        let completed = repo.create_sync_queue_item(file1.id, "mirror").unwrap();
        repo.update_sync_queue_status(completed.id, "completed", None)
            .unwrap();
        let in_progress = repo.create_sync_queue_item(file2.id, "verify").unwrap();
        repo.update_sync_queue_status(in_progress.id, "in_progress", None)
            .unwrap();
        let pending = repo
            .create_sync_queue_item(file3.id, "restore_master")
            .unwrap();

        let deleted = repo.clear_completed_sync_queue().unwrap();
        assert_eq!(deleted, 1);

        let (remaining, total) = repo.list_sync_queue(None, 1, 50).unwrap();
        assert_eq!(total, 2);
        assert!(remaining.iter().any(|item| item.id == in_progress.id));
        assert!(remaining.iter().any(|item| item.id == pending.id));
        assert!(remaining.iter().all(|item| item.status != "completed"));
    }

    #[test]
    fn test_list_tracking_items_filter_combinations() {
        let repo = make_repo();
        let pair_a = repo.create_drive_pair("pair-a", "/pa", "/sa").unwrap();
        let pair_b = repo.create_drive_pair("pair-b", "/pb", "/sb").unwrap();

        let direct_with_virtual = repo
            .create_tracked_file_with_source(
                pair_a.id,
                "docs/direct.txt",
                "hash-direct",
                10,
                Some("/virtual/docs/direct.txt"),
                true,
                false,
            )
            .unwrap();
        repo.update_tracked_file_mirror_status(direct_with_virtual.id, true)
            .unwrap();

        let folder_based_no_virtual = repo
            .create_tracked_file_with_source(
                pair_a.id,
                "docs/folder.txt",
                "hash-folder",
                20,
                None,
                false,
                true,
            )
            .unwrap();
        let _folder = repo
            .create_tracked_folder(pair_a.id, "docs", Some("/virtual/docs"))
            .unwrap();

        let _pair_b_file = repo
            .create_tracked_file_with_source(
                pair_b.id,
                "other/ignore.txt",
                "hash-b",
                30,
                Some("/virtual/other/ignore.txt"),
                true,
                false,
            )
            .unwrap();

        let (filtered, total) = repo
            .list_tracking_items(
                Some(pair_a.id),
                None,
                Some("/virtual/docs"),
                Some(true),
                Some("file"),
                Some("direct"),
                1,
                50,
            )
            .unwrap();
        assert_eq!(total, 1);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, direct_with_virtual.id);

        let (folder_source_items, total_folder_source) = repo
            .list_tracking_items(
                Some(pair_a.id),
                None,
                None,
                Some(true),
                Some("file"),
                Some("folder"),
                1,
                50,
            )
            .unwrap();
        assert_eq!(total_folder_source, 1);
        assert_eq!(folder_source_items[0].id, folder_based_no_virtual.id);

        let (paged, total_paged) = repo
            .list_tracking_items(
                Some(pair_a.id),
                None,
                None,
                None,
                Some("all"),
                Some("all"),
                2,
                1,
            )
            .unwrap();
        assert_eq!(total_paged, 3);
        assert_eq!(paged.len(), 1);
    }

    #[test]
    fn test_transaction_rollback_on_fk_violation() {
        let repo = make_repo();
        let mut conn = repo.conn().unwrap();
        let tx = conn.transaction().unwrap();
        let result = tx.execute(
            "INSERT INTO tracked_files (drive_pair_id, relative_path, checksum, file_size)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![9999i64, "ghost.txt", "hash", 1i64],
        );
        assert!(result.is_err());
        tx.rollback().unwrap();
        drop(conn);
        assert_eq!(repo.count_tracked_files(None).unwrap(), 0);
    }

    #[test]
    fn test_connection_pool_behaviour_under_concurrent_use() {
        let db_file = NamedTempFile::new().unwrap();
        let pool = create_pool(db_file.path().to_str().unwrap()).unwrap();
        {
            let conn = pool.get().unwrap();
            initialize_schema(&conn).unwrap();
        }
        let repo = Repository::new(pool);
        repo.create_drive_pair("concurrency", "/p", "/s").unwrap();

        let mut handles = Vec::new();
        for _ in 0..8 {
            let repo_clone = repo.clone();
            handles.push(thread::spawn(move || {
                for _ in 0..50 {
                    let _ = repo_clone.list_drive_pairs().unwrap();
                    let _ = repo_clone.get_system_status().unwrap();
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }

    // ─── Event Log ────────────────────────────────────────────────────────────────

    #[test]
    fn test_event_log_crud() {
        let repo = make_repo();
        let entry = repo
            .create_event_log("integrity_pass", None, "Check passed", Some("details"))
            .unwrap();
        assert_eq!(entry.event_type, "integrity_pass");
        assert_eq!(entry.message, "Check passed");

        let (logs, total) = repo.list_event_logs(None, None, None, None, 1, 50).unwrap();
        assert_eq!(total, 1);
        assert_eq!(logs[0].id, entry.id);
    }

    // ─── Schedule Config ──────────────────────────────────────────────────────────

    #[test]
    fn test_schedule_config_crud() {
        let repo = make_repo();
        let cfg = repo
            .create_schedule_config("sync", Some("0 2 * * *"), None, true)
            .unwrap();
        assert_eq!(cfg.task_type, "sync");
        assert_eq!(cfg.cron_expr, Some("0 2 * * *".to_string()));

        let updated = repo
            .update_schedule_config(cfg.id, None, Some(Some(3600)), Some(false))
            .unwrap();
        assert!(!updated.enabled);
        assert_eq!(updated.interval_seconds, Some(3600));

        repo.delete_schedule_config(cfg.id).unwrap();
        assert!(repo.get_schedule_config(cfg.id).is_err());
    }

    // ─── DB Backup Config ─────────────────────────────────────────────────────────

    #[test]
    fn test_db_backup_config_crud() {
        let repo = make_repo();
        let cfg = repo
            .create_db_backup_config("/mnt/backup/", Some("backup-1"), 3, true)
            .unwrap();
        assert_eq!(cfg.backup_path, "/mnt/backup/");
        assert_eq!(cfg.max_copies, 3);

        let updated = repo.update_db_backup_config(cfg.id, Some(5), None).unwrap();
        assert_eq!(updated.max_copies, 5);

        repo.delete_db_backup_config(cfg.id).unwrap();
        assert!(repo.get_db_backup_config(cfg.id).is_err());
    }

    // ─── System Status ────────────────────────────────────────────────────────────

    #[test]
    fn test_system_status() {
        let repo = make_repo();
        let pair = repo.create_drive_pair("p", "/a", "/b").unwrap();
        let file = repo
            .create_tracked_file(pair.id, "f.txt", "h", 1, None)
            .unwrap();
        repo.update_tracked_file_mirror_status(file.id, true)
            .unwrap();

        let status = repo.get_system_status().unwrap();
        assert_eq!(status.files_tracked, 1);
        assert_eq!(status.files_mirrored, 1);
        assert_eq!(status.drive_pairs, 1);
    }
}
