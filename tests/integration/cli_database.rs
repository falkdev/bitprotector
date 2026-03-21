use bitprotector_lib::cli::commands::database::{handle, AddArgs, DatabaseCommand, RunArgs};
use bitprotector_lib::db::repository::{create_memory_pool, Repository};
use bitprotector_lib::db::schema::initialize_schema;
use std::io::Write;
use tempfile::TempDir;

fn make_repo() -> Repository {
    let pool = create_memory_pool().unwrap();
    let conn = pool.get().unwrap();
    initialize_schema(&*conn).unwrap();
    drop(conn);
    Repository::new(pool)
}

#[test]
fn test_backup_add_and_list() {
    let repo = make_repo();
    let dir = TempDir::new().unwrap();

    handle(
        DatabaseCommand::Add(AddArgs {
            path: dir.path().to_str().unwrap().to_string(),
            max_copies: 3,
            drive_label: None,
            disabled: false,
        }),
        &repo,
    )
    .unwrap();

    handle(DatabaseCommand::List, &repo).unwrap();
    let configs = repo.list_db_backup_configs().unwrap();
    assert_eq!(configs.len(), 1);
    assert_eq!(configs[0].max_copies, 3);
    assert!(configs[0].enabled);
}

#[test]
fn test_backup_show() {
    let repo = make_repo();
    let dir = TempDir::new().unwrap();
    let cfg = repo
        .create_db_backup_config(dir.path().to_str().unwrap(), None, 5, true)
        .unwrap();
    handle(DatabaseCommand::Show { id: cfg.id }, &repo).unwrap();
}

#[test]
fn test_backup_remove() {
    let repo = make_repo();
    let dir = TempDir::new().unwrap();
    let cfg = repo
        .create_db_backup_config(dir.path().to_str().unwrap(), None, 5, true)
        .unwrap();
    handle(DatabaseCommand::Remove { id: cfg.id }, &repo).unwrap();
    assert!(repo.get_db_backup_config(cfg.id).is_err());
}

#[test]
fn test_backup_run_creates_file() {
    let repo = make_repo();
    let backup_dir = TempDir::new().unwrap();
    let mut db_file = tempfile::NamedTempFile::new().unwrap();
    db_file.write_all(b"fake database content").unwrap();
    db_file.flush().unwrap();

    repo.create_db_backup_config(backup_dir.path().to_str().unwrap(), None, 5, true)
        .unwrap();

    handle(
        DatabaseCommand::Run(RunArgs {
            db_path: db_file.path().to_str().unwrap().to_string(),
        }),
        &repo,
    )
    .unwrap();

    let entries: Vec<_> = std::fs::read_dir(backup_dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert_eq!(entries.len(), 1, "Backup run should create one backup file");
}

#[test]
fn test_backup_rotation() {
    use bitprotector_lib::db::backup;

    let repo = make_repo();
    let backup_dir = TempDir::new().unwrap();
    let mut db_file = tempfile::NamedTempFile::new().unwrap();
    db_file.write_all(b"db").unwrap();
    db_file.flush().unwrap();

    let cfg = repo
        .create_db_backup_config(backup_dir.path().to_str().unwrap(), None, 2, true)
        .unwrap();

    // Run 3 backups with max_copies=2; only 2 should remain
    for _ in 0..3 {
        let db_cfg = repo.get_db_backup_config(cfg.id).unwrap();
        backup::backup_to_destination(db_file.path().to_str().unwrap(), &db_cfg).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1100));
    }

    let remaining: Vec<_> = std::fs::read_dir(backup_dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_str()
                .map(|n| n.ends_with(".db"))
                .unwrap_or(false)
        })
        .collect();
    assert_eq!(
        remaining.len(),
        2,
        "Only max_copies=2 backups should remain"
    );
}
