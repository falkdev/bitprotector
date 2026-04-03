use bitprotector_lib::cli::commands::folders::{handle, AddArgs, FoldersCommand, ScanArgs};
use bitprotector_lib::core::checksum;
use bitprotector_lib::core::drive::{self, DriveRole};
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

fn setup_pair(
    repo: &Repository,
    primary: &TempDir,
    secondary: &TempDir,
) -> bitprotector_lib::db::repository::DrivePair {
    repo.create_drive_pair(
        "test_pair",
        primary.path().to_str().unwrap(),
        secondary.path().to_str().unwrap(),
    )
    .unwrap()
}

#[test]
fn test_folders_add_and_list() {
    let repo = make_repo();
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let pair = setup_pair(&repo, &primary, &secondary);
    fs::create_dir(primary.path().join("documents")).unwrap();

    handle(
        FoldersCommand::Add(AddArgs {
            drive_pair_id: pair.id,
            folder_path: "documents".to_string(),
            virtual_path: None,
        }),
        &repo,
    )
    .unwrap();

    handle(FoldersCommand::List, &repo).unwrap();
    let folders = repo.list_tracked_folders().unwrap();
    assert_eq!(folders.len(), 1);
    assert_eq!(folders[0].folder_path, "documents");
}

#[test]
fn test_folders_show() {
    let repo = make_repo();
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let pair = setup_pair(&repo, &primary, &secondary);
    fs::create_dir(primary.path().join("media")).unwrap();
    let folder = repo
        .create_tracked_folder(pair.id, "media", None)
        .unwrap();

    handle(FoldersCommand::Show { id: folder.id }, &repo).unwrap();
}

#[test]
fn test_folders_remove() {
    let repo = make_repo();
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let pair = setup_pair(&repo, &primary, &secondary);
    fs::create_dir(primary.path().join("temp")).unwrap();
    let folder = repo
        .create_tracked_folder(pair.id, "temp", None)
        .unwrap();

    handle(FoldersCommand::Remove { id: folder.id }, &repo).unwrap();
    assert!(repo.list_tracked_folders().unwrap().is_empty());
}

#[test]
fn test_folders_scan_auto_tracks_new_files() {
    let repo = make_repo();
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let pair = setup_pair(&repo, &primary, &secondary);
    fs::create_dir(primary.path().join("uploads")).unwrap();
    fs::write(primary.path().join("uploads/file1.txt"), b"content1").unwrap();
    fs::write(primary.path().join("uploads/file2.txt"), b"content2").unwrap();

    handle(
        FoldersCommand::Add(AddArgs {
            drive_pair_id: pair.id,
            folder_path: "uploads".to_string(),
            virtual_path: None,
        }),
        &repo,
    )
    .unwrap();

    let folders = repo.list_tracked_folders().unwrap();
    handle(FoldersCommand::Scan(ScanArgs { id: folders[0].id }), &repo).unwrap();

    let (files, _) = repo
        .list_tracked_files(Some(pair.id), None, None, 1, 100)
        .unwrap();
    assert_eq!(
        files.len(),
        2,
        "Both files should be auto-tracked during scan"
    );
    assert!(
        files.iter().all(|f| f.is_mirrored),
        "All files should be mirrored"
    );
}

#[test]
fn test_folders_add_nonexistent_folder_fails() {
    let repo = make_repo();
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let pair = setup_pair(&repo, &primary, &secondary);

    let result = handle(
        FoldersCommand::Add(AddArgs {
            drive_pair_id: pair.id,
            folder_path: "no_such_folder".to_string(),
            virtual_path: None,
        }),
        &repo,
    );

    assert!(result.is_err(), "Adding a non-existent folder should fail");
}

#[test]
fn test_folders_add_with_virtual_path() {
    let repo = make_repo();
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let virtual_root = TempDir::new().unwrap();
    let pair = setup_pair(&repo, &primary, &secondary);
    fs::create_dir(primary.path().join("photos")).unwrap();
    let virtual_path_on_disk = virtual_root.path().join("gallery");

    handle(
        FoldersCommand::Add(AddArgs {
            drive_pair_id: pair.id,
            folder_path: "photos".to_string(),
            virtual_path: Some(virtual_path_on_disk.to_str().unwrap().to_string()),
        }),
        &repo,
    )
    .unwrap();

    let folders = repo.list_tracked_folders().unwrap();
    assert_eq!(
        folders[0].virtual_path,
        Some(virtual_path_on_disk.to_string_lossy().to_string())
    );
    assert_eq!(fs::read_link(&virtual_path_on_disk).unwrap(), primary.path().join("photos"));
}

#[test]
fn test_folders_scan_uses_secondary_after_primary_failover() {
    let repo = make_repo();
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let pair = setup_pair(&repo, &primary, &secondary);

    fs::create_dir(primary.path().join("docs")).unwrap();
    fs::create_dir(secondary.path().join("docs")).unwrap();
    fs::write(primary.path().join("docs/existing.txt"), b"original").unwrap();
    fs::write(secondary.path().join("docs/existing.txt"), b"original").unwrap();

    handle(
        FoldersCommand::Add(AddArgs {
            drive_pair_id: pair.id,
            folder_path: "docs".to_string(),
            virtual_path: None,
        }),
        &repo,
    )
    .unwrap();
    let folder = repo.list_tracked_folders().unwrap().remove(0);

    let tracked =
        bitprotector_lib::core::tracker::track_file(&repo, &pair, "docs/existing.txt", None)
            .unwrap();
    repo.update_tracked_file_mirror_status(tracked.id, true)
        .unwrap();

    drive::mark_drive_quiescing(&repo, pair.id, DriveRole::Primary).unwrap();
    drive::confirm_drive_failure(&repo, pair.id, DriveRole::Primary).unwrap();

    fs::write(secondary.path().join("docs/existing.txt"), b"edited").unwrap();
    fs::write(secondary.path().join("docs/new.txt"), b"new").unwrap();

    handle(FoldersCommand::Scan(ScanArgs { id: folder.id }), &repo).unwrap();

    let updated = repo.get_tracked_file(tracked.id).unwrap();
    assert_eq!(updated.checksum, checksum::checksum_bytes(b"edited"));
    assert!(!updated.is_mirrored);

    let (files, _) = repo
        .list_tracked_files(Some(pair.id), None, None, 1, 100)
        .unwrap();
    assert!(files
        .iter()
        .any(|file| file.relative_path == "docs/new.txt"));
}
