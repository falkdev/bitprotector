use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::Result;

pub type DbPool = Pool<SqliteConnectionManager>;

/// Create a connection pool for the given database path.
pub fn create_pool(db_path: &str) -> anyhow::Result<DbPool> {
    let manager = SqliteConnectionManager::file(db_path).with_init(|conn| {
        conn.execute_batch("PRAGMA busy_timeout = 5000; PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")?;
        Ok(())
    });
    let pool = Pool::builder().max_size(10).build(manager)?;
    Ok(pool)
}

/// Create a single-connection pool for CLI commands that share the DB with a running service.
pub fn create_cli_pool(db_path: &str) -> anyhow::Result<DbPool> {
    let manager = SqliteConnectionManager::file(db_path).with_init(|conn| {
        conn.execute_batch("PRAGMA busy_timeout = 5000; PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")?;
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
    pub last_verified: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Represents a tracked folder record.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TrackedFolder {
    pub id: i64,
    pub drive_pair_id: i64,
    pub folder_path: String,
    pub auto_virtual_path: bool,
    pub default_virtual_base: Option<String>,
    pub created_at: String,
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

    pub fn create_drive_pair(&self, name: &str, primary: &str, secondary: &str) -> anyhow::Result<DrivePair> {
        let id = {
            let conn = self.conn()?;
            conn.execute(
                "INSERT INTO drive_pairs (name, primary_path, secondary_path) VALUES (?1, ?2, ?3)",
                rusqlite::params![name, primary, secondary],
            )?;
            conn.last_insert_rowid()
        };
        self.get_drive_pair(id)
    }

    pub fn get_drive_pair(&self, id: i64) -> anyhow::Result<DrivePair> {
        let conn = self.conn()?;
        let pair = conn.query_row(
            "SELECT id, name, primary_path, secondary_path, created_at, updated_at
             FROM drive_pairs WHERE id = ?1",
            rusqlite::params![id],
            |row| {
                Ok(DrivePair {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    primary_path: row.get(2)?,
                    secondary_path: row.get(3)?,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                })
            },
        )?;
        Ok(pair)
    }

    pub fn list_drive_pairs(&self) -> anyhow::Result<Vec<DrivePair>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, primary_path, secondary_path, created_at, updated_at
             FROM drive_pairs ORDER BY id",
        )?;
        let pairs = stmt
            .query_map([], |row| {
                Ok(DrivePair {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    primary_path: row.get(2)?,
                    secondary_path: row.get(3)?,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
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
                conn.execute("UPDATE drive_pairs SET name=?1, updated_at=datetime('now') WHERE id=?2", rusqlite::params![n, id])?;
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

    pub fn delete_drive_pair(&self, id: i64) -> anyhow::Result<()> {
        let conn = self.conn()?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM tracked_files WHERE drive_pair_id=?1",
            rusqlite::params![id],
            |r| r.get(0),
        )?;
        if count > 0 {
            anyhow::bail!("Cannot delete drive pair: {} files are still tracked", count);
        }
        conn.execute("DELETE FROM drive_pairs WHERE id=?1", rusqlite::params![id])?;
        Ok(())
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
        let id = {
            let conn = self.conn()?;
            conn.execute(
                "INSERT INTO tracked_files (drive_pair_id, relative_path, checksum, file_size, virtual_path)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![drive_pair_id, relative_path, checksum, file_size, virtual_path],
            )?;
            conn.last_insert_rowid()
        };
        self.get_tracked_file(id)
    }

    pub fn get_tracked_file(&self, id: i64) -> anyhow::Result<TrackedFile> {
        let conn = self.conn()?;
        let file = conn.query_row(
            "SELECT id, drive_pair_id, relative_path, checksum, file_size, virtual_path,
                    is_mirrored, last_verified, created_at, updated_at
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
                    last_verified: row.get(7)?,
                    created_at: row.get(8)?,
                    updated_at: row.get(9)?,
                })
            },
        )?;
        Ok(file)
    }

    pub fn get_tracked_file_by_path(&self, drive_pair_id: i64, relative_path: &str) -> anyhow::Result<TrackedFile> {
        let conn = self.conn()?;
        let file = conn.query_row(
            "SELECT id, drive_pair_id, relative_path, checksum, file_size, virtual_path,
                    is_mirrored, last_verified, created_at, updated_at
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
                    last_verified: row.get(7)?,
                    created_at: row.get(8)?,
                    updated_at: row.get(9)?,
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
        let where_clause = if conditions.is_empty() { String::new() } else { format!("WHERE {}", conditions.join(" AND ")) };

        let query = format!(
            "SELECT id, drive_pair_id, relative_path, checksum, file_size, virtual_path,
                    is_mirrored, last_verified, created_at, updated_at
             FROM tracked_files {where_clause} ORDER BY id LIMIT {per_page} OFFSET {offset}"
        );
        let count_query = format!("SELECT COUNT(*) FROM tracked_files {where_clause}");

        let files = conn.prepare(&query)?.query_map(
            rusqlite::params_from_iter(params.iter()),
            |row| {
                Ok(TrackedFile {
                    id: row.get(0)?,
                    drive_pair_id: row.get(1)?,
                    relative_path: row.get(2)?,
                    checksum: row.get(3)?,
                    file_size: row.get(4)?,
                    virtual_path: row.get(5)?,
                    is_mirrored: row.get::<_, i64>(6)? != 0,
                    last_verified: row.get(7)?,
                    created_at: row.get(8)?,
                    updated_at: row.get(9)?,
                })
            },
        )?.collect::<Result<Vec<_>, _>>()?;

        let total: i64 = conn.query_row(&count_query, rusqlite::params_from_iter(params.iter()), |r| r.get(0))?;
        Ok((files, total))
    }

    pub fn update_tracked_file_checksum(&self, id: i64, checksum: &str, file_size: i64) -> anyhow::Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE tracked_files SET checksum=?1, file_size=?2, updated_at=datetime('now') WHERE id=?3",
            rusqlite::params![checksum, file_size, id],
        )?;
        Ok(())
    }

    pub fn update_tracked_file_mirror_status(&self, id: i64, is_mirrored: bool) -> anyhow::Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE tracked_files SET is_mirrored=?1, updated_at=datetime('now') WHERE id=?2",
            rusqlite::params![is_mirrored as i64, id],
        )?;
        Ok(())
    }

    pub fn update_tracked_file_last_verified(&self, id: i64) -> anyhow::Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE tracked_files SET last_verified=datetime('now'), updated_at=datetime('now') WHERE id=?1",
            rusqlite::params![id],
        )?;
        Ok(())
    }

    pub fn update_tracked_file_virtual_path(&self, id: i64, virtual_path: Option<&str>) -> anyhow::Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE tracked_files SET virtual_path=?1, updated_at=datetime('now') WHERE id=?2",
            rusqlite::params![virtual_path, id],
        )?;
        Ok(())
    }

    pub fn delete_tracked_file(&self, id: i64) -> anyhow::Result<()> {
        let conn = self.conn()?;
        conn.execute("DELETE FROM tracked_files WHERE id=?1", rusqlite::params![id])?;
        Ok(())
    }

    // ─── Tracked Folders ─────────────────────────────────────────────────────────

    pub fn create_tracked_folder(
        &self,
        drive_pair_id: i64,
        folder_path: &str,
        auto_virtual_path: bool,
        default_virtual_base: Option<&str>,
    ) -> anyhow::Result<TrackedFolder> {
        let id = {
            let conn = self.conn()?;
            conn.execute(
                "INSERT INTO tracked_folders (drive_pair_id, folder_path, auto_virtual_path, default_virtual_base)
                 VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![drive_pair_id, folder_path, auto_virtual_path as i64, default_virtual_base],
            )?;
            conn.last_insert_rowid()
        };
        self.get_tracked_folder(id)
    }

    pub fn get_tracked_folder(&self, id: i64) -> anyhow::Result<TrackedFolder> {
        let conn = self.conn()?;
        let folder = conn.query_row(
            "SELECT id, drive_pair_id, folder_path, auto_virtual_path, default_virtual_base, created_at
             FROM tracked_folders WHERE id=?1",
            rusqlite::params![id],
            |row| {
                Ok(TrackedFolder {
                    id: row.get(0)?,
                    drive_pair_id: row.get(1)?,
                    folder_path: row.get(2)?,
                    auto_virtual_path: row.get::<_, i64>(3)? != 0,
                    default_virtual_base: row.get(4)?,
                    created_at: row.get(5)?,
                })
            },
        )?;
        Ok(folder)
    }

    pub fn list_tracked_folders(&self) -> anyhow::Result<Vec<TrackedFolder>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, drive_pair_id, folder_path, auto_virtual_path, default_virtual_base, created_at
             FROM tracked_folders ORDER BY id",
        )?;
        let folders = stmt.query_map([], |row| {
            Ok(TrackedFolder {
                id: row.get(0)?,
                drive_pair_id: row.get(1)?,
                folder_path: row.get(2)?,
                auto_virtual_path: row.get::<_, i64>(3)? != 0,
                default_virtual_base: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?.collect::<Result<Vec<_>, _>>()?;
        Ok(folders)
    }

    pub fn update_tracked_folder(
        &self,
        id: i64,
        auto_virtual_path: Option<bool>,
        default_virtual_base: Option<Option<&str>>,
    ) -> anyhow::Result<TrackedFolder> {
        {
            let conn = self.conn()?;
            if let Some(avp) = auto_virtual_path {
                conn.execute(
                    "UPDATE tracked_folders SET auto_virtual_path=?1 WHERE id=?2",
                    rusqlite::params![avp as i64, id],
                )?;
            }
            if let Some(dvb) = default_virtual_base {
                conn.execute(
                    "UPDATE tracked_folders SET default_virtual_base=?1 WHERE id=?2",
                    rusqlite::params![dvb, id],
                )?;
            }
        }
        self.get_tracked_folder(id)
    }

    pub fn delete_tracked_folder(&self, id: i64) -> anyhow::Result<()> {
        let conn = self.conn()?;
        conn.execute("DELETE FROM tracked_folders WHERE id=?1", rusqlite::params![id])?;
        Ok(())
    }

    // ─── Sync Queue ───────────────────────────────────────────────────────────────

    pub fn create_sync_queue_item(&self, tracked_file_id: i64, action: &str) -> anyhow::Result<SyncQueueItem> {
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

        let items = conn.prepare(&query)?.query_map([], |row| {
            Ok(SyncQueueItem {
                id: row.get(0)?,
                tracked_file_id: row.get(1)?,
                action: row.get(2)?,
                status: row.get(3)?,
                error_message: row.get(4)?,
                created_at: row.get(5)?,
                completed_at: row.get(6)?,
            })
        })?.collect::<Result<Vec<_>, _>>()?;
        let _ = params_status; // suppress unused warning

        let total: i64 = conn.query_row(&count_query, [], |r| r.get(0))?;
        Ok((items, total))
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
            "SELECT id, event_type, tracked_file_id, message, details, created_at
             FROM event_log WHERE id=?1",
            rusqlite::params![id],
            |row| {
                Ok(EventLogEntry {
                    id: row.get(0)?,
                    event_type: row.get(1)?,
                    tracked_file_id: row.get(2)?,
                    message: row.get(3)?,
                    details: row.get(4)?,
                    created_at: row.get(5)?,
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
        if let Some(et) = event_type { conditions.push(format!("event_type='{}'", et.replace('\'', "''"))); }
        if let Some(fid) = tracked_file_id { conditions.push(format!("tracked_file_id={}", fid)); }
        if let Some(f) = from { conditions.push(format!("created_at>='{}'", f.replace('\'', "''"))); }
        if let Some(t) = to { conditions.push(format!("created_at<='{}'", t.replace('\'', "''"))); }
        let where_clause = if conditions.is_empty() { String::new() } else { format!("WHERE {}", conditions.join(" AND ")) };

        let query = format!(
            "SELECT id, event_type, tracked_file_id, message, details, created_at
             FROM event_log {where_clause} ORDER BY id DESC LIMIT {per_page} OFFSET {offset}"
        );
        let count_query = format!("SELECT COUNT(*) FROM event_log {where_clause}");

        let entries = conn.prepare(&query)?.query_map([], |row| {
            Ok(EventLogEntry {
                id: row.get(0)?,
                event_type: row.get(1)?,
                tracked_file_id: row.get(2)?,
                message: row.get(3)?,
                details: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?.collect::<Result<Vec<_>, _>>()?;

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
        let cfgs = stmt.query_map([], |row| {
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
        })?.collect::<Result<Vec<_>, _>>()?;
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
                conn.execute("UPDATE schedule_config SET enabled=?1, updated_at=datetime('now') WHERE id=?2", rusqlite::params![en as i64, id])?;
            }
        }
        self.get_schedule_config(id)
    }

    pub fn delete_schedule_config(&self, id: i64) -> anyhow::Result<()> {
        let conn = self.conn()?;
        conn.execute("DELETE FROM schedule_config WHERE id=?1", rusqlite::params![id])?;
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
        let cfgs = stmt.query_map([], |row| {
            Ok(DbBackupConfig {
                id: row.get(0)?,
                backup_path: row.get(1)?,
                drive_label: row.get(2)?,
                max_copies: row.get(3)?,
                enabled: row.get::<_, i64>(4)? != 0,
                last_backup: row.get(5)?,
                created_at: row.get(6)?,
            })
        })?.collect::<Result<Vec<_>, _>>()?;
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
                conn.execute("UPDATE db_backup_config SET max_copies=?1 WHERE id=?2", rusqlite::params![mc, id])?;
            }
            if let Some(en) = enabled {
                conn.execute("UPDATE db_backup_config SET enabled=?1 WHERE id=?2", rusqlite::params![en as i64, id])?;
            }
        }
        self.get_db_backup_config(id)
    }

    pub fn delete_db_backup_config(&self, id: i64) -> anyhow::Result<()> {
        let conn = self.conn()?;
        conn.execute("DELETE FROM db_backup_config WHERE id=?1", rusqlite::params![id])?;
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
        let files_tracked: i64 = conn.query_row("SELECT COUNT(*) FROM tracked_files", [], |r| r.get(0))?;
        let files_mirrored: i64 = conn.query_row("SELECT COUNT(*) FROM tracked_files WHERE is_mirrored=1", [], |r| r.get(0))?;
        let pending_sync: i64 = conn.query_row("SELECT COUNT(*) FROM sync_queue WHERE status='pending'", [], |r| r.get(0))?;
        let integrity_issues: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sync_queue WHERE action IN ('user_action_required') AND status='pending'",
            [], |r| r.get(0)
        )?;
        let drive_pairs: i64 = conn.query_row("SELECT COUNT(*) FROM drive_pairs", [], |r| r.get(0))?;
        Ok(SystemStatus { files_tracked, files_mirrored, pending_sync, integrity_issues, drive_pairs })
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
        let pair = repo.create_drive_pair("test-pair", "/primary", "/secondary").unwrap();
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
        let updated = repo.update_drive_pair(pair.id, Some("new-name"), None, None).unwrap();
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
        repo.create_tracked_file(pair.id, "file.txt", "abc123", 100, None).unwrap();
        let result = repo.delete_drive_pair(pair.id);
        assert!(result.is_err(), "Delete should fail when files are tracked");
    }

    // ─── Tracked Files ──────────────────────────────────────────────────────────

    #[test]
    fn test_tracked_file_crud() {
        let repo = make_repo();
        let pair = repo.create_drive_pair("p", "/a", "/b").unwrap();

        let file = repo.create_tracked_file(pair.id, "docs/report.pdf", "blake3hex", 1024, Some("/docs/report.pdf")).unwrap();
        assert_eq!(file.relative_path, "docs/report.pdf");
        assert_eq!(file.checksum, "blake3hex");
        assert_eq!(file.file_size, 1024);
        assert!(file.virtual_path.is_some());
        assert!(!file.is_mirrored);

        repo.update_tracked_file_mirror_status(file.id, true).unwrap();
        let updated = repo.get_tracked_file(file.id).unwrap();
        assert!(updated.is_mirrored);

        repo.update_tracked_file_checksum(file.id, "newhash", 2048).unwrap();
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
            repo.create_tracked_file(pair.id, &format!("file{}.txt", i), "hash", 100, None).unwrap();
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

        let folder = repo.create_tracked_folder(pair.id, "documents/", true, Some("/docs")).unwrap();
        assert_eq!(folder.folder_path, "documents/");
        assert!(folder.auto_virtual_path);

        let updated = repo.update_tracked_folder(folder.id, Some(false), None).unwrap();
        assert!(!updated.auto_virtual_path);

        repo.delete_tracked_folder(folder.id).unwrap();
        assert!(repo.get_tracked_folder(folder.id).is_err());
    }

    // ─── Sync Queue ───────────────────────────────────────────────────────────────

    #[test]
    fn test_sync_queue_crud() {
        let repo = make_repo();
        let pair = repo.create_drive_pair("p", "/a", "/b").unwrap();
        let file = repo.create_tracked_file(pair.id, "f.txt", "h", 1, None).unwrap();

        let item = repo.create_sync_queue_item(file.id, "mirror").unwrap();
        assert_eq!(item.action, "mirror");
        assert_eq!(item.status, "pending");

        repo.update_sync_queue_status(item.id, "completed", None).unwrap();
        let updated = repo.get_sync_queue_item(item.id).unwrap();
        assert_eq!(updated.status, "completed");
        assert!(updated.completed_at.is_some());
    }

    // ─── Event Log ────────────────────────────────────────────────────────────────

    #[test]
    fn test_event_log_crud() {
        let repo = make_repo();
        let entry = repo.create_event_log("integrity_pass", None, "Check passed", Some("details")).unwrap();
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
        let cfg = repo.create_schedule_config("sync", Some("0 2 * * *"), None, true).unwrap();
        assert_eq!(cfg.task_type, "sync");
        assert_eq!(cfg.cron_expr, Some("0 2 * * *".to_string()));

        let updated = repo.update_schedule_config(cfg.id, None, Some(Some(3600)), Some(false)).unwrap();
        assert!(!updated.enabled);
        assert_eq!(updated.interval_seconds, Some(3600));

        repo.delete_schedule_config(cfg.id).unwrap();
        assert!(repo.get_schedule_config(cfg.id).is_err());
    }

    // ─── DB Backup Config ─────────────────────────────────────────────────────────

    #[test]
    fn test_db_backup_config_crud() {
        let repo = make_repo();
        let cfg = repo.create_db_backup_config("/mnt/backup/", Some("backup-1"), 3, true).unwrap();
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
        let file = repo.create_tracked_file(pair.id, "f.txt", "h", 1, None).unwrap();
        repo.update_tracked_file_mirror_status(file.id, true).unwrap();

        let status = repo.get_system_status().unwrap();
        assert_eq!(status.files_tracked, 1);
        assert_eq!(status.files_mirrored, 1);
        assert_eq!(status.drive_pairs, 1);
    }
}
