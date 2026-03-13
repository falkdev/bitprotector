use bitprotector_lib::db::repository::{create_memory_pool, Repository};
use bitprotector_lib::db::schema::initialize_schema;
use bitprotector_lib::cli::commands::folders::{FoldersCommand, AddArgs, ScanArgs, handle};
use tempfile::TempDir;
use std::fs;

fn make_repo() -> Repository {
    let pool = create_memory_pool().unwrap();
    let conn = pool.get().unwrap();
    initialize_schema(&*conn).unwrap();
    drop(conn);
    Repository::new(pool)
}

fn setup_pair(repo: &Repository, primary: &TempDir, secondary: &TempDir) -> bitprotector_lib::db::repository::DrivePair {
    repo.create_drive_pair("test_pair", primary.path().to_str().unwrap(), secondary.path().to_str().unwrap()).unwrap()
}

#[test]
fn test_folders_add_and_list() {
    let repo = make_repo();
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let pair = setup_pair(&repo, &primary, &secondary);
    fs::create_dir(primary.path().join("documents")).unwrap();

    handle(FoldersCommand::Add(AddArgs {
        drive_pair_id: pair.id,
        folder_path: "documents".to_string(),
        auto_virtual_path: false,
        virtual_base: None,
    }), &repo).unwrap();

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
    let folder = repo.create_tracked_folder(pair.id, "media", false, None).unwrap();

    handle(FoldersCommand::Show { id: folder.id }, &repo).unwrap();
}

#[test]
fn test_folders_remove() {
    let repo = make_repo();
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let pair = setup_pair(&repo, &primary, &secondary);
    fs::create_dir(primary.path().join("temp")).unwrap();
    let folder = repo.create_tracked_folder(pair.id, "temp", false, None).unwrap();

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

    handle(FoldersCommand::Add(AddArgs {
        drive_pair_id: pair.id,
        folder_path: "uploads".to_string(),
        auto_virtual_path: false,
        virtual_base: None,
    }), &repo).unwrap();

    let folders = repo.list_tracked_folders().unwrap();
    handle(FoldersCommand::Scan(ScanArgs { id: folders[0].id }), &repo).unwrap();

    let (files, _) = repo.list_tracked_files(Some(pair.id), None, None, 1, 100).unwrap();
    assert_eq!(files.len(), 2, "Both files should be auto-tracked during scan");
    assert!(files.iter().all(|f| f.is_mirrored), "All files should be mirrored");
}

#[test]
fn test_folders_add_nonexistent_folder_fails() {
    let repo = make_repo();
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let pair = setup_pair(&repo, &primary, &secondary);

    let result = handle(FoldersCommand::Add(AddArgs {
        drive_pair_id: pair.id,
        folder_path: "no_such_folder".to_string(),
        auto_virtual_path: false,
        virtual_base: None,
    }), &repo);

    assert!(result.is_err(), "Adding a non-existent folder should fail");
}

#[test]
fn test_folders_scan_with_auto_virtual_path() {
    let repo = make_repo();
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let pair = setup_pair(&repo, &primary, &secondary);
    fs::create_dir(primary.path().join("photos")).unwrap();
    fs::write(primary.path().join("photos/sunset.jpg"), b"image data").unwrap();

    handle(FoldersCommand::Add(AddArgs {
        drive_pair_id: pair.id,
        folder_path: "photos".to_string(),
        auto_virtual_path: true,
        virtual_base: Some("/gallery".to_string()),
    }), &repo).unwrap();

    let folders = repo.list_tracked_folders().unwrap();
    handle(FoldersCommand::Scan(ScanArgs { id: folders[0].id }), &repo).unwrap();

    let (files, _) = repo.list_tracked_files(Some(pair.id), None, None, 1, 100).unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].virtual_path, Some("/gallery/sunset.jpg".to_string()));
}
