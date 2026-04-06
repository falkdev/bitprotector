use bitprotector_lib::cli::commands::sync::{handle, AddArgs, ListArgs, RunArgs, SyncCommand};
use bitprotector_lib::core::checksum;
use bitprotector_lib::db::repository::{create_memory_pool, Repository};
use bitprotector_lib::db::schema::initialize_schema;
use std::fs;
use tempfile::TempDir;

fn make_repo() -> Repository {
    let pool = create_memory_pool().unwrap();
    let conn = pool.get().unwrap();
    initialize_schema(&*conn).unwrap();
    drop(conn);
    Repository::new(pool)
}

fn setup_file(
    repo: &Repository,
    primary: &TempDir,
    secondary: &TempDir,
    name: &str,
) -> (
    bitprotector_lib::db::repository::DrivePair,
    bitprotector_lib::db::repository::TrackedFile,
) {
    let pair = repo
        .create_drive_pair(
            "pair",
            primary.path().to_str().unwrap(),
            secondary.path().to_str().unwrap(),
        )
        .unwrap();
    let content = b"sync integration test content";
    fs::write(primary.path().join(name), content).unwrap();
    let hash = checksum::checksum_bytes(content);
    let file = repo
        .create_tracked_file(pair.id, name, &hash, content.len() as i64, None)
        .unwrap();
    (pair, file)
}

#[test]
fn test_sync_list_empty_queue() {
    let repo = make_repo();
    handle(
        SyncCommand::List(ListArgs {
            status: None,
            page: 1,
            per_page: 50,
        }),
        &repo,
    )
    .unwrap();
}

#[test]
fn test_sync_add_to_queue_and_list() {
    let repo = make_repo();
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let (_, file) = setup_file(&repo, &primary, &secondary, "q.txt");

    handle(
        SyncCommand::Add(AddArgs {
            file_id: file.id,
            action: "verify".to_string(),
        }),
        &repo,
    )
    .unwrap();

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

    handle(
        SyncCommand::Add(AddArgs {
            file_id: file.id,
            action: "mirror".to_string(),
        }),
        &repo,
    )
    .unwrap();
    handle(SyncCommand::Process, &repo).unwrap();

    assert!(
        secondary.path().join("mirror_me.txt").exists(),
        "File should be mirrored"
    );
    let (items, _) = repo.list_sync_queue(Some("completed"), 1, 10).unwrap();
    assert_eq!(items.len(), 1);
}

#[test]
fn test_sync_run_integrity_check_persists_run_results() {
    let repo = make_repo();
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let (_, _file) = setup_file(&repo, &primary, &secondary, "no_mirror.txt");
    // File is tracked but not mirrored — integrity check should detect MirrorMissing.

    handle(
        SyncCommand::Run(RunArgs {
            task: "integrity-check".to_string(),
        }),
        &repo,
    )
    .unwrap();

    let latest_run = repo
        .get_latest_integrity_run()
        .unwrap()
        .expect("Expected an integrity run to be persisted");
    assert_eq!(latest_run.attention_files, 1);

    let (items, _) = repo
        .list_integrity_run_results(latest_run.id, true, 1, 20)
        .unwrap();
    assert!(!items.is_empty(), "Integrity run should persist issue rows");
    assert_eq!(items[0].status, "mirror_missing");
}

#[test]
fn test_full_scheduled_sync_cycle() {
    // Full cycle: add file → queue mirror → process → verify file mirrored
    let repo = make_repo();
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let (_, file) = setup_file(&repo, &primary, &secondary, "full_cycle.txt");

    repo.create_sync_queue_item(file.id, "mirror").unwrap();
    handle(
        SyncCommand::Run(RunArgs {
            task: "sync".to_string(),
        }),
        &repo,
    )
    .unwrap();

    assert!(
        secondary.path().join("full_cycle.txt").exists(),
        "File should be mirrored by sync task"
    );
    let updated_file = repo.get_tracked_file(file.id).unwrap();
    let _ = updated_file; // Verify retrieval works
}

// ── resolve_queue_item conflict resolution ─────────────────────────────────

use bitprotector_lib::core::sync_queue::resolve_queue_item;

#[test]
fn test_resolve_keep_master_restores_mirror_from_primary() {
    let repo = make_repo();
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let (_, file) = setup_file(&repo, &primary, &secondary, "km.txt");

    // Create a user_action_required item
    let item = repo
        .create_sync_queue_item(file.id, "user_action_required")
        .unwrap();

    let resolved = resolve_queue_item(&repo, item.id, "keep_master", None).unwrap();
    assert_eq!(resolved.status, "completed");

    // Mirror should now contain the primary's content
    let content = fs::read(secondary.path().join("km.txt")).unwrap();
    assert_eq!(content, b"sync integration test content");
}

#[test]
fn test_resolve_keep_mirror_restores_primary_from_mirror() {
    let repo = make_repo();
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let (_, file) = setup_file(&repo, &primary, &secondary, "kmirr.txt");

    // Write matching content to secondary so restore_from_mirror will succeed
    fs::write(
        secondary.path().join("kmirr.txt"),
        b"sync integration test content",
    )
    .unwrap();

    let item = repo
        .create_sync_queue_item(file.id, "user_action_required")
        .unwrap();

    let resolved = resolve_queue_item(&repo, item.id, "keep_mirror", None).unwrap();
    assert_eq!(resolved.status, "completed");

    // Primary should now match the mirror content
    let restored = fs::read(primary.path().join("kmirr.txt")).unwrap();
    assert_eq!(restored, b"sync integration test content");
}

#[test]
fn test_resolve_provide_new_copies_to_both_sides() {
    let repo = make_repo();
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let new_file_dir = TempDir::new().unwrap();

    let (_, file) = setup_file(&repo, &primary, &secondary, "pn.txt");

    let new_content = b"brand new replacement content";
    let new_path = new_file_dir.path().join("replacement.txt");
    fs::write(&new_path, new_content).unwrap();

    let item = repo
        .create_sync_queue_item(file.id, "user_action_required")
        .unwrap();

    let resolved = resolve_queue_item(
        &repo,
        item.id,
        "provide_new",
        Some(new_path.to_str().unwrap()),
    )
    .unwrap();
    assert_eq!(resolved.status, "completed");

    assert_eq!(
        fs::read(primary.path().join("pn.txt")).unwrap(),
        new_content,
        "Primary should contain the new content"
    );
    assert_eq!(
        fs::read(secondary.path().join("pn.txt")).unwrap(),
        new_content,
        "Secondary should contain the new content"
    );
}

#[test]
fn test_resolve_provide_new_missing_path_returns_error() {
    let repo = make_repo();
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let (_, file) = setup_file(&repo, &primary, &secondary, "err.txt");

    let item = repo
        .create_sync_queue_item(file.id, "user_action_required")
        .unwrap();

    let result = resolve_queue_item(
        &repo,
        item.id,
        "provide_new",
        Some("/nonexistent/path/to/file.txt"),
    );
    assert!(
        result.is_err(),
        "Should fail when provided path does not exist"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("does not exist"),
        "Error should mention missing path, got: {}",
        msg
    );
}

#[test]
fn test_resolve_invalid_resolution_returns_error() {
    let repo = make_repo();
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let (_, file) = setup_file(&repo, &primary, &secondary, "inv.txt");

    let item = repo
        .create_sync_queue_item(file.id, "user_action_required")
        .unwrap();

    let result = resolve_queue_item(&repo, item.id, "destroy_everything", None);
    assert!(result.is_err(), "Unknown resolution should return an error");
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("Unknown resolution"),
        "Error should name the bad resolution, got: {}",
        msg
    );
}

#[test]
fn test_resolve_non_user_action_required_item_returns_error() {
    let repo = make_repo();
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let (_, file) = setup_file(&repo, &primary, &secondary, "nar.txt");

    // Create an item with a different action
    let item = repo.create_sync_queue_item(file.id, "mirror").unwrap();

    let result = resolve_queue_item(&repo, item.id, "keep_master", None);
    assert!(
        result.is_err(),
        "Should refuse to resolve a non-user_action_required item"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("user_action_required"),
        "Error should mention the required action type, got: {}",
        msg
    );
}

#[test]
fn test_process_all_pending_skips_user_action_required_items() {
    use bitprotector_lib::core::sync_queue::process_all_pending;

    let repo = make_repo();
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let (_, file) = setup_file(&repo, &primary, &secondary, "skip.txt");

    repo.create_sync_queue_item(file.id, "user_action_required")
        .unwrap();

    // process_all_pending should skip user_action_required and return 0 processed
    let processed = process_all_pending(&repo).unwrap();
    assert_eq!(
        processed, 0,
        "user_action_required items must be skipped by process_all_pending"
    );

    // The item should remain pending
    let (items, _) = repo.list_sync_queue(Some("pending"), 1, 10).unwrap();
    assert_eq!(
        items.len(),
        1,
        "user_action_required item should remain pending"
    );
    assert_eq!(items[0].action, "user_action_required");
}
