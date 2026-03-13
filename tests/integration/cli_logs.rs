use bitprotector_lib::db::repository::{create_memory_pool, Repository};
use bitprotector_lib::db::schema::initialize_schema;
use bitprotector_lib::cli::commands::logs::{LogsCommand, ListArgs, handle};
use bitprotector_lib::core::tracker;
use bitprotector_lib::logging::event_logger;
use tempfile::TempDir;
use std::fs;

fn make_repo() -> Repository {
    let pool = create_memory_pool().unwrap();
    let conn = pool.get().unwrap();
    initialize_schema(&*conn).unwrap();
    drop(conn);
    Repository::new(pool)
}

#[test]
fn test_logs_list_empty() {
    let repo = make_repo();
    handle(LogsCommand::List(ListArgs {
        event_type: None, file_id: None, from: None, to: None, page: 1, per_page: 50,
    }), &repo).unwrap();
}

#[test]
fn test_file_track_creates_log_entry() {
    let repo = make_repo();
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let pair = repo.create_drive_pair("p", primary.path().to_str().unwrap(), secondary.path().to_str().unwrap()).unwrap();
    fs::write(primary.path().join("log_test.txt"), b"log content").unwrap();

    tracker::track_file(&repo, &pair, "log_test.txt", None).unwrap();

    // tracker::track_file logs a "file_created" event
    let (entries, total) = repo.list_event_logs(Some("file_created"), None, None, None, 1, 50).unwrap();
    assert_eq!(total, 1, "Tracking a file should create a file_created log entry");
    assert!(entries[0].message.contains("log_test.txt"));
}

#[test]
fn test_logs_list_filtered_by_event_type() {
    let repo = make_repo();
    // Use None for file_id to avoid FK constraint issues
    event_logger::log_event(&repo, "integrity_pass", None, "pass", None).unwrap();
    event_logger::log_event(&repo, "file_mirrored", None, "mirror", None).unwrap();
    handle(LogsCommand::List(ListArgs {
        event_type: Some("integrity_pass".to_string()),
        file_id: None, from: None, to: None, page: 1, per_page: 50,
    }), &repo).unwrap();

    let (entries, total) = repo.list_event_logs(Some("integrity_pass"), None, None, None, 1, 50).unwrap();
    assert_eq!(total, 1);
    assert_eq!(entries[0].event_type, "integrity_pass");
}

#[test]
fn test_logs_show_entry() {
    let repo = make_repo();
    let entry = repo.create_event_log("integrity_pass", None, "test message", Some("details here")).unwrap();
    handle(LogsCommand::Show { id: entry.id }, &repo).unwrap();
}

#[test]
fn test_sync_operation_creates_log_entry() {
    use bitprotector_lib::core::{sync_queue, checksum};

    let repo = make_repo();
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let pair = repo.create_drive_pair("p", primary.path().to_str().unwrap(), secondary.path().to_str().unwrap()).unwrap();
    let content = b"sync log content";
    fs::write(primary.path().join("synclog.txt"), content).unwrap();
    let hash = checksum::checksum_bytes(content);
    let file = repo.create_tracked_file(pair.id, "synclog.txt", &hash, content.len() as i64, None).unwrap();
    let item = repo.create_sync_queue_item(file.id, "mirror").unwrap();
    sync_queue::process_item(&repo, &item).unwrap();

    let (entries, _) = repo.list_event_logs(Some("sync_completed"), None, None, None, 1, 10).unwrap();
    assert!(!entries.is_empty(), "A sync_completed event should be logged after processing");

    handle(LogsCommand::List(ListArgs {
        event_type: None, file_id: Some(file.id), from: None, to: None, page: 1, per_page: 50,
    }), &repo).unwrap();
}
