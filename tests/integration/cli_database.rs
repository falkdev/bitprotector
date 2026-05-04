use bitprotector_lib::cli::commands::database::{
    handle, handle_with_db_path, AddArgs, DatabaseCommand, RestoreArgs, SettingsArgs,
};
use bitprotector_lib::db::repository::{create_memory_pool, Repository};
use bitprotector_lib::db::schema::initialize_schema;
use rusqlite::Connection;
use tempfile::TempDir;

fn make_repo() -> Repository {
    let pool = create_memory_pool().unwrap();
    let conn = pool.get().unwrap();
    initialize_schema(&*conn).unwrap();
    drop(conn);
    Repository::new(pool)
}

fn make_sqlite_db(dir: &TempDir) -> String {
    let path = dir.path().join("bitprotector.db");
    let conn = Connection::open(&path).unwrap();
    conn.execute("CREATE TABLE sample (id INTEGER PRIMARY KEY)", [])
        .unwrap();
    drop(conn);
    path.to_string_lossy().to_string()
}

#[test]
fn test_backup_add_and_list() {
    let repo = make_repo();
    let dir = TempDir::new().unwrap();

    handle(
        DatabaseCommand::Add(AddArgs {
            path: dir.path().to_str().unwrap().to_string(),
            drive_label: None,
            disabled: false,
        }),
        &repo,
    )
    .unwrap();

    handle(DatabaseCommand::List, &repo).unwrap();
    let configs = repo.list_db_backup_configs().unwrap();
    assert_eq!(configs.len(), 1);
    assert_eq!(configs[0].priority, 0);
    assert!(configs[0].enabled);
}

#[test]
fn test_backup_show() {
    let repo = make_repo();
    let dir = TempDir::new().unwrap();
    let cfg = repo
        .create_db_backup_config(dir.path().to_str().unwrap(), None, true)
        .unwrap();
    handle(DatabaseCommand::Show { id: cfg.id }, &repo).unwrap();
}

#[test]
fn test_backup_remove() {
    let repo = make_repo();
    let dir = TempDir::new().unwrap();
    let cfg = repo
        .create_db_backup_config(dir.path().to_str().unwrap(), None, true)
        .unwrap();
    handle(DatabaseCommand::Remove { id: cfg.id }, &repo).unwrap();
    assert!(repo.get_db_backup_config(cfg.id).is_err());
}

#[test]
fn test_backup_run_creates_canonical_file() {
    let repo = make_repo();
    let backup_dir = TempDir::new().unwrap();
    let db_dir = TempDir::new().unwrap();
    let db_path = make_sqlite_db(&db_dir);

    repo.create_db_backup_config(backup_dir.path().to_str().unwrap(), None, true)
        .unwrap();

    handle_with_db_path(DatabaseCommand::Run, &repo, &db_path).unwrap();

    assert!(backup_dir.path().join("bitprotector.db").exists());
}

#[test]
fn test_repeated_backup_run_keeps_single_canonical_file() {
    let repo = make_repo();
    let backup_dir = TempDir::new().unwrap();
    let db_dir = TempDir::new().unwrap();
    let db_path = make_sqlite_db(&db_dir);

    repo.create_db_backup_config(backup_dir.path().to_str().unwrap(), None, true)
        .unwrap();

    handle_with_db_path(DatabaseCommand::Run, &repo, &db_path).unwrap();
    handle_with_db_path(DatabaseCommand::Run, &repo, &db_path).unwrap();

    let db_entries: Vec<_> = std::fs::read_dir(backup_dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|entry| entry.file_name() == "bitprotector.db")
        .collect();
    assert_eq!(db_entries.len(), 1);
}

#[test]
fn test_backup_integrity_repairs_corrupt_secondary() {
    let repo = make_repo();
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let db_dir = TempDir::new().unwrap();
    let db_path = make_sqlite_db(&db_dir);

    repo.create_db_backup_config(primary.path().to_str().unwrap(), None, true)
        .unwrap();
    repo.create_db_backup_config(secondary.path().to_str().unwrap(), None, true)
        .unwrap();
    handle_with_db_path(DatabaseCommand::Run, &repo, &db_path).unwrap();
    std::fs::write(secondary.path().join("bitprotector.db"), b"not sqlite").unwrap();

    handle(DatabaseCommand::CheckIntegrity, &repo).unwrap();

    let conn = Connection::open(secondary.path().join("bitprotector.db")).unwrap();
    let status: String = conn
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .unwrap();
    assert_eq!(status, "ok");
}

#[test]
fn test_restore_rejects_corrupt_backup() {
    let repo = make_repo();
    let db_dir = TempDir::new().unwrap();
    let db_path = make_sqlite_db(&db_dir);
    let corrupt_dir = TempDir::new().unwrap();
    let corrupt_path = corrupt_dir.path().join("bad.db");
    std::fs::write(&corrupt_path, b"not sqlite").unwrap();

    let result = handle_with_db_path(
        DatabaseCommand::Restore(RestoreArgs {
            source_path: corrupt_path.to_string_lossy().to_string(),
        }),
        &repo,
        &db_path,
    );
    assert!(result.is_err());
}

#[test]
fn test_backup_settings_updates() {
    let repo = make_repo();
    handle(
        DatabaseCommand::Settings(SettingsArgs {
            backup_enabled: true,
            backup_disabled: false,
            backup_interval_seconds: Some(3600),
            integrity_enabled: true,
            integrity_disabled: false,
            integrity_interval_seconds: Some(7200),
        }),
        &repo,
    )
    .unwrap();

    let settings = repo.get_db_backup_settings().unwrap();
    assert!(settings.backup_enabled);
    assert_eq!(settings.backup_interval_seconds, 3600);
    assert!(settings.integrity_enabled);
    assert_eq!(settings.integrity_interval_seconds, 7200);
}
