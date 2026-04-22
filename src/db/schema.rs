use rusqlite::{Connection, Result};

pub fn initialize_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA foreign_keys = ON;",
    )?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS drive_pairs (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            name            TEXT NOT NULL UNIQUE,
            primary_path    TEXT NOT NULL,
            secondary_path  TEXT NOT NULL,
            primary_state   TEXT NOT NULL DEFAULT 'active' CHECK(primary_state IN ('active', 'quiescing', 'failed', 'rebuilding')),
            secondary_state TEXT NOT NULL DEFAULT 'active' CHECK(secondary_state IN ('active', 'quiescing', 'failed', 'rebuilding')),
            active_role     TEXT NOT NULL DEFAULT 'primary' CHECK(active_role IN ('primary', 'secondary')),
            created_at      TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
        );",
    )?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS tracked_files (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            drive_pair_id   INTEGER NOT NULL REFERENCES drive_pairs(id),
            relative_path   TEXT NOT NULL,
            checksum        TEXT NOT NULL,
            file_size       INTEGER NOT NULL,
            virtual_path    TEXT,
            is_mirrored     INTEGER NOT NULL DEFAULT 0,
            tracked_direct  INTEGER NOT NULL DEFAULT 1,
            tracked_via_folder INTEGER NOT NULL DEFAULT 0,
            last_integrity_check_at TEXT,
            created_at      TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at      TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE(drive_pair_id, relative_path)
        );",
    )?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS tracked_folders (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            drive_pair_id   INTEGER NOT NULL REFERENCES drive_pairs(id),
            folder_path     TEXT NOT NULL,
            virtual_path    TEXT,
            last_scanned_at TEXT,
            created_at      TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE(drive_pair_id, folder_path)
        );",
    )?;

    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_tracked_files_virtual_path
         ON tracked_files(virtual_path);
         CREATE INDEX IF NOT EXISTS idx_tracked_files_drive_relative
         ON tracked_files(drive_pair_id, relative_path);
         CREATE INDEX IF NOT EXISTS idx_tracked_folders_drive_folder_path
         ON tracked_folders(drive_pair_id, folder_path);",
    )?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS integrity_runs (
            id                 INTEGER PRIMARY KEY AUTOINCREMENT,
            scope_drive_pair_id INTEGER REFERENCES drive_pairs(id) ON DELETE SET NULL,
            recover            INTEGER NOT NULL DEFAULT 0,
            trigger            TEXT NOT NULL,
            status             TEXT NOT NULL CHECK(status IN (
                                  'running', 'stopping', 'stopped', 'completed', 'failed'
                               )),
            total_files        INTEGER NOT NULL DEFAULT 0,
            processed_files    INTEGER NOT NULL DEFAULT 0,
            attention_files    INTEGER NOT NULL DEFAULT 0,
            recovered_files    INTEGER NOT NULL DEFAULT 0,
            stop_requested     INTEGER NOT NULL DEFAULT 0,
            started_at         TEXT NOT NULL DEFAULT (datetime('now')),
            ended_at           TEXT,
            error_message      TEXT
        );
         CREATE TABLE IF NOT EXISTS integrity_run_results (
            id               INTEGER PRIMARY KEY AUTOINCREMENT,
            run_id           INTEGER NOT NULL REFERENCES integrity_runs(id) ON DELETE CASCADE,
            file_id          INTEGER NOT NULL REFERENCES tracked_files(id) ON DELETE CASCADE,
            drive_pair_id    INTEGER NOT NULL REFERENCES drive_pairs(id),
            relative_path    TEXT NOT NULL,
            status           TEXT NOT NULL,
            recovered        INTEGER NOT NULL DEFAULT 0,
            needs_attention  INTEGER NOT NULL DEFAULT 0,
            checked_at       TEXT NOT NULL DEFAULT (datetime('now'))
        );
         CREATE INDEX IF NOT EXISTS idx_integrity_runs_active
            ON integrity_runs(status, started_at);
         CREATE INDEX IF NOT EXISTS idx_integrity_run_results_issue
            ON integrity_run_results(run_id, needs_attention, id);",
    )?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS sync_queue (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            tracked_file_id INTEGER NOT NULL REFERENCES tracked_files(id),
            action          TEXT NOT NULL CHECK(action IN (
                                'mirror', 'restore_master', 'restore_mirror',
                                'verify', 'user_action_required'
                            )),
            status          TEXT NOT NULL DEFAULT 'pending' CHECK(status IN (
                                'pending', 'in_progress', 'completed', 'failed'
                            )),
            error_message   TEXT,
            created_at      TEXT NOT NULL DEFAULT (datetime('now')),
            completed_at    TEXT
        );",
    )?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS event_log (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            event_type      TEXT NOT NULL CHECK(event_type IN (
                                'file_created', 'file_edited', 'file_mirrored',
                                'file_untracked',
                                'integrity_pass', 'integrity_fail',
                                'recovery_success', 'recovery_fail',
                                'both_corrupted', 'change_detected',
                                'sync_completed', 'sync_failed',
                                'folder_tracked', 'folder_untracked',
                                'integrity_run_started', 'integrity_run_completed',
                                'drive_created', 'drive_updated', 'drive_deleted',
                                'drive_failover', 'drive_quiescing', 'drive_quiesce_cancelled',
                                'drive_failure_confirmed', 'drive_replacement_assigned',
                                'drive_rebuild_completed'
                            )),
            tracked_file_id INTEGER REFERENCES tracked_files(id) ON DELETE SET NULL,
            message         TEXT NOT NULL,
            details         TEXT,
            created_at      TEXT NOT NULL DEFAULT (datetime('now'))
        );",
    )?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schedule_config (
            id               INTEGER PRIMARY KEY AUTOINCREMENT,
            task_type        TEXT NOT NULL CHECK(task_type IN ('sync', 'integrity_check')),
            cron_expr        TEXT,
            interval_seconds INTEGER,
            enabled          INTEGER NOT NULL DEFAULT 1,
            last_run         TEXT,
            next_run         TEXT,
            created_at       TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at       TEXT NOT NULL DEFAULT (datetime('now'))
        );",
    )?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS db_backup_config (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            backup_path TEXT NOT NULL,
            drive_label TEXT,
            max_copies  INTEGER NOT NULL DEFAULT 3,
            enabled     INTEGER NOT NULL DEFAULT 1,
            last_backup TEXT,
            created_at  TEXT NOT NULL DEFAULT (datetime('now'))
        );",
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_memory_db() -> Connection {
        Connection::open_in_memory().expect("Failed to open in-memory DB")
    }

    #[test]
    fn test_schema_creation_succeeds() {
        let conn = open_memory_db();
        initialize_schema(&conn).expect("Schema creation failed");
    }

    #[test]
    fn test_schema_idempotent() {
        let conn = open_memory_db();
        initialize_schema(&conn).expect("First schema creation failed");
        initialize_schema(&conn).expect("Second schema creation (idempotent) failed");
    }

    #[test]
    fn test_all_tables_created() {
        let conn = open_memory_db();
        initialize_schema(&conn).expect("Schema creation failed");

        let tables = [
            "drive_pairs",
            "tracked_files",
            "tracked_folders",
            "integrity_runs",
            "integrity_run_results",
            "sync_queue",
            "event_log",
            "schedule_config",
            "db_backup_config",
        ];

        for table in &tables {
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    rusqlite::params![table],
                    |row| row.get(0),
                )
                .expect("Query failed");
            assert_eq!(count, 1, "Table '{}' was not created", table);
        }
    }

    #[test]
    fn test_foreign_keys_enabled() {
        let conn = open_memory_db();
        initialize_schema(&conn).expect("Schema creation failed");

        // Insert a tracked_file referencing a non-existent drive_pair should fail
        let result = conn.execute(
            "INSERT INTO tracked_files (drive_pair_id, relative_path, checksum, file_size)
             VALUES (999, 'test.txt', 'abc', 100)",
            [],
        );
        assert!(result.is_err(), "Foreign key constraint should have failed");
    }

    #[test]
    fn test_drive_pair_unique_name_constraint() {
        let conn = open_memory_db();
        initialize_schema(&conn).expect("Schema creation failed");

        conn.execute(
            "INSERT INTO drive_pairs (name, primary_path, secondary_path)
             VALUES ('pair1', '/primary', '/secondary')",
            [],
        )
        .expect("First insert failed");

        let result = conn.execute(
            "INSERT INTO drive_pairs (name, primary_path, secondary_path)
             VALUES ('pair1', '/other_primary', '/other_secondary')",
            [],
        );
        assert!(result.is_err(), "Unique name constraint should have failed");
    }

    #[test]
    fn test_sync_queue_action_check_constraint() {
        let conn = open_memory_db();
        initialize_schema(&conn).expect("Schema creation failed");

        // Insert a valid drive pair and tracked file first
        conn.execute(
            "INSERT INTO drive_pairs (name, primary_path, secondary_path) VALUES ('p', '/a', '/b')",
            [],
        )
        .unwrap();
        let pair_id: i64 = conn
            .query_row("SELECT id FROM drive_pairs LIMIT 1", [], |r| r.get(0))
            .unwrap();
        conn.execute(
            "INSERT INTO tracked_files (drive_pair_id, relative_path, checksum, file_size)
             VALUES (?1, 'f.txt', 'abc', 1)",
            rusqlite::params![pair_id],
        )
        .unwrap();
        let file_id: i64 = conn
            .query_row("SELECT id FROM tracked_files LIMIT 1", [], |r| r.get(0))
            .unwrap();

        let result = conn.execute(
            "INSERT INTO sync_queue (tracked_file_id, action) VALUES (?1, 'invalid_action')",
            rusqlite::params![file_id],
        );
        assert!(
            result.is_err(),
            "Invalid action check constraint should fail"
        );
    }
}
