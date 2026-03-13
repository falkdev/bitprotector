use bitprotector_lib::db::repository::{create_memory_pool, Repository};
use bitprotector_lib::db::schema::initialize_schema;
use bitprotector_lib::cli::commands::sync::{SyncCommand, ListArgs, AddArgs, RunArgs, handle};
use bitprotector_lib::core::checksum;
use tempfile::TempDir;
use std::fs;

fn make_repo() -> Repository {
    let pool = create_memory_pool().unwrap();
    let conn = pool.get().unwrap();
    initialize_schema(&*conn).unwrap();
    drop(conn);
    Repository::new(pool)
}

fn setup_file(repo: &Repository, primary: &TempDir, secondary: &TempDir, name: &str) -> (bitprotector_lib::db::repository::DrivePair, bitprotector_lib::db::repository::TrackedFile) {
    let pair = repo.create_drive_pair("pair", primary.path().to_str().unwrap(), secondary.path().to_str().unwrap()).unwrap();
    let content = b"sync integration test content";
    fs::write(primary.path().join(name), content).unwrap();
    let hash = checksum::checksum_bytes(content);
    let file = repo.create_tracked_file(pair.id, name, &hash, content.len() as i64, None).unwrap();
    (pair, file)
}

#[test]
fn test_sync_list_empty_queue() {
    let repo = make_repo();
    handle(SyncCommand::List(ListArgs { status: None, page: 1, per_page: 50 }), &repo).unwrap();
}

#[test]
fn test_sync_add_to_queue_and_list() {
    let repo = make_repo();
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let (_, file) = setup_file(&repo, &primary, &secondary, "q.txt");

    handle(SyncCommand::Add(AddArgs { file_id: file.id, action: "verify".to_string() }), &repo).unwrap();

    let (items, total) = repo.list_sync_queue(Some("pending"), 1, 10).unwrap();
    assert_eq!(total, 1);
    assert_eq!(items[0].action, "verify");
}

#[test]
fn test_sync_process_mirrors_pending_file() {
    let repo = make_repo();
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let (_, file) = setup_file(&repo, &primary, &secondary, "mirror_me.txt");

    handle(SyncCommand::Add(AddArgs { file_id: file.id, action: "mirror".to_string() }), &repo).unwrap();
    handle(SyncCommand::Process, &repo).unwrap();

    assert!(secondary.path().join("mirror_me.txt").exists(), "File should be mirrored");
    let (items, _) = repo.list_sync_queue(Some("completed"), 1, 10).unwrap();
    assert_eq!(items.len(), 1);
}

#[test]
fn test_sync_run_integrity_check_queues_failures() {
    let repo = make_repo();
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let (_, _file) = setup_file(&repo, &primary, &secondary, "no_mirror.txt");
    // File is tracked but not mirrored — integrity check should detect MirrorMissing

    handle(SyncCommand::Run(RunArgs { task: "integrity-check".to_string() }), &repo).unwrap();

    let (items, _) = repo.list_sync_queue(Some("pending"), 1, 10).unwrap();
    assert!(!items.is_empty(), "Integrity check should create a queue item for the missing mirror");
}

#[test]
fn test_full_scheduled_sync_cycle() {
    // Full cycle: add file → queue mirror → process → verify file mirrored
    let repo = make_repo();
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let (_, file) = setup_file(&repo, &primary, &secondary, "full_cycle.txt");

    repo.create_sync_queue_item(file.id, "mirror").unwrap();
    handle(SyncCommand::Run(RunArgs { task: "sync".to_string() }), &repo).unwrap();

    assert!(secondary.path().join("full_cycle.txt").exists(), "File should be mirrored by sync task");
    let updated_file = repo.get_tracked_file(file.id).unwrap();
    let _ = updated_file; // Verify retrieval works
}
