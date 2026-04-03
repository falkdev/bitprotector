use rusqlite::{Connection, Result};

fn column_exists(conn: &Connection, table: &str, column: &str) -> Result<bool> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let columns = stmt.query_map([], |row| row.get::<_, String>(1))?;
    for col in columns {
        if col? == column {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Create all database tables and apply migrations.
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

    if !column_exists(conn, "drive_pairs", "primary_state")? {
        conn.execute_batch(
            "ALTER TABLE drive_pairs
             ADD COLUMN primary_state TEXT NOT NULL DEFAULT 'active'
             CHECK(primary_state IN ('active', 'quiescing', 'failed', 'rebuilding'));",
        )?;
    }
    if !column_exists(conn, "drive_pairs", "secondary_state")? {
        conn.execute_batch(
            "ALTER TABLE drive_pairs
             ADD COLUMN secondary_state TEXT NOT NULL DEFAULT 'active'
             CHECK(secondary_state IN ('active', 'quiescing', 'failed', 'rebuilding'));",
        )?;
    }
    if !column_exists(conn, "drive_pairs", "active_role")? {
        conn.execute_batch(
            "ALTER TABLE drive_pairs
             ADD COLUMN active_role TEXT NOT NULL DEFAULT 'primary'
             CHECK(active_role IN ('primary', 'secondary'));",
        )?;
    }

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
            last_verified   TEXT,
            created_at      TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at      TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE(drive_pair_id, relative_path)
        );",
    )?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS tracked_folders (
            id                   INTEGER PRIMARY KEY AUTOINCREMENT,
            drive_pair_id        INTEGER NOT NULL REFERENCES drive_pairs(id),
            folder_path          TEXT NOT NULL,
            virtual_path         TEXT,
            auto_virtual_path    INTEGER NOT NULL DEFAULT 0,
            default_virtual_base TEXT,
            created_at           TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE(drive_pair_id, folder_path)
        );",
    )?;

    if !column_exists(conn, "tracked_folders", "virtual_path")? {
        conn.execute_batch(
            "ALTER TABLE tracked_folders
             ADD COLUMN virtual_path TEXT;",
        )?;
    }

    if !column_exists(conn, "tracked_files", "tracked_direct")? {
        conn.execute_batch(
            "ALTER TABLE tracked_files
             ADD COLUMN tracked_direct INTEGER NOT NULL DEFAULT 1;",
        )?;
    }
    if !column_exists(conn, "tracked_files", "tracked_via_folder")? {
        conn.execute_batch(
            "ALTER TABLE tracked_files
             ADD COLUMN tracked_via_folder INTEGER NOT NULL DEFAULT 0;",
        )?;
    }

    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_tracked_files_virtual_path
         ON tracked_files(virtual_path);
         CREATE INDEX IF NOT EXISTS idx_tracked_files_drive_relative
         ON tracked_files(drive_pair_id, relative_path);
         CREATE INDEX IF NOT EXISTS idx_tracked_folders_drive_folder_path
         ON tracked_folders(drive_pair_id, folder_path);",
    )?;

    // Backfill folder-derived provenance for pre-migration rows.
    conn.execute(
        "UPDATE tracked_files
         SET tracked_via_folder = 0",
        [],
    )?;
    conn.execute(
        "UPDATE tracked_files
         SET tracked_via_folder = 1
         WHERE EXISTS (
            SELECT 1
            FROM tracked_folders
            WHERE tracked_folders.drive_pair_id = tracked_files.drive_pair_id
              AND (
                    tracked_files.relative_path = rtrim(tracked_folders.folder_path, '/')
                    OR tracked_files.relative_path LIKE rtrim(tracked_folders.folder_path, '/') || '/%'
              )
         )",
        [],
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
                                'integrity_pass', 'integrity_fail',
                                'recovery_success', 'recovery_fail',
                                'both_corrupted', 'change_detected',
                                'sync_completed', 'sync_failed'
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

    fn index_exists(conn: &Connection, index: &str) -> bool {
        conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name=?1",
            rusqlite::params![index],
            |row| row.get::<_, i64>(0),
        )
        .map(|count| count == 1)
        .unwrap_or(false)
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

    #[test]
    fn test_drive_pair_state_columns_migrate_legacy_schema() {
        let conn = open_memory_db();
        conn.execute_batch(
            "CREATE TABLE drive_pairs (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                name            TEXT NOT NULL UNIQUE,
                primary_path    TEXT NOT NULL,
                secondary_path  TEXT NOT NULL,
                created_at      TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
            );",
        )
        .unwrap();

        initialize_schema(&conn).expect("Schema migration failed");

        for column in ["primary_state", "secondary_state", "active_role"] {
            assert!(
                column_exists(&conn, "drive_pairs", column).unwrap(),
                "Column '{}' should be added during migration",
                column
            );
        }
    }

    #[test]
    fn test_tracked_file_provenance_columns_indexes_and_backfill_migrate_legacy_schema() {
        let conn = open_memory_db();
        conn.execute_batch(
            "CREATE TABLE drive_pairs (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                name            TEXT NOT NULL UNIQUE,
                primary_path    TEXT NOT NULL,
                secondary_path  TEXT NOT NULL,
                created_at      TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE tracked_files (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                drive_pair_id   INTEGER NOT NULL REFERENCES drive_pairs(id),
                relative_path   TEXT NOT NULL,
                checksum        TEXT NOT NULL,
                file_size       INTEGER NOT NULL,
                virtual_path    TEXT,
                is_mirrored     INTEGER NOT NULL DEFAULT 0,
                last_verified   TEXT,
                created_at      TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at      TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(drive_pair_id, relative_path)
            );
            CREATE TABLE tracked_folders (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                drive_pair_id   INTEGER NOT NULL REFERENCES drive_pairs(id),
                folder_path     TEXT NOT NULL,
                virtual_path    TEXT,
                created_at      TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(drive_pair_id, folder_path)
            );",
        )
        .unwrap();

        conn.execute(
            "INSERT INTO drive_pairs (name, primary_path, secondary_path)
             VALUES ('pair', '/p', '/s')",
            [],
        )
        .unwrap();
        let pair_id: i64 = conn
            .query_row("SELECT id FROM drive_pairs LIMIT 1", [], |row| row.get(0))
            .unwrap();

        conn.execute(
            "INSERT INTO tracked_folders (drive_pair_id, folder_path, virtual_path)
             VALUES (?1, 'docs', '/virtual/docs')",
            rusqlite::params![pair_id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tracked_files (drive_pair_id, relative_path, checksum, file_size)
             VALUES (?1, 'docs/a.txt', 'h1', 10)",
            rusqlite::params![pair_id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tracked_files (drive_pair_id, relative_path, checksum, file_size)
             VALUES (?1, 'misc/b.txt', 'h2', 10)",
            rusqlite::params![pair_id],
        )
        .unwrap();

        initialize_schema(&conn).expect("Schema migration failed");

        for column in ["tracked_direct", "tracked_via_folder"] {
            assert!(
                column_exists(&conn, "tracked_files", column).unwrap(),
                "Column '{}' should be added during migration",
                column
            );
        }

        assert!(
            index_exists(&conn, "idx_tracked_files_virtual_path"),
            "tracked_files virtual_path index should be present"
        );
        assert!(
            index_exists(&conn, "idx_tracked_files_drive_relative"),
            "tracked_files drive_pair_id + relative_path index should be present"
        );
        assert!(
            index_exists(&conn, "idx_tracked_folders_drive_folder_path"),
            "tracked_folders drive_pair_id + folder_path index should be present"
        );

        let docs_flags: (i64, i64) = conn
            .query_row(
                "SELECT tracked_direct, tracked_via_folder
                 FROM tracked_files
                 WHERE relative_path='docs/a.txt'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            docs_flags,
            (1, 1),
            "File under tracked folder should be marked both"
        );

        let misc_flags: (i64, i64) = conn
            .query_row(
                "SELECT tracked_direct, tracked_via_folder
                 FROM tracked_files
                 WHERE relative_path='misc/b.txt'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            misc_flags,
            (1, 0),
            "File outside tracked folders should remain directly tracked only"
        );
    }
}
