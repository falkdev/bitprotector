use bitprotector_lib::cli::commands::virtual_paths::handle;
use bitprotector_lib::cli::commands::virtual_paths::{
    RefreshArgs, RemoveArgs, SetArgs, VirtualPathsCommand,
};
use bitprotector_lib::core::tracker;
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

fn setup_tracked_file(
    repo: &Repository,
    primary: &TempDir,
    secondary: &TempDir,
    filename: &str,
) -> (
    bitprotector_lib::db::repository::DrivePair,
    bitprotector_lib::db::repository::TrackedFile,
) {
    let pair = repo
        .create_drive_pair(
            "test_pair",
            primary.path().to_str().unwrap(),
            secondary.path().to_str().unwrap(),
        )
        .unwrap();
    fs::write(primary.path().join(filename), b"hello bitprotector").unwrap();
    let tracked = tracker::track_file(repo, &pair, filename, None).unwrap();
    (pair, tracked)
}

#[test]
fn test_set_virtual_path_creates_symlink() {
    let repo = make_repo();
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let symlink_dir = TempDir::new().unwrap();

    let (_, file) = setup_tracked_file(&repo, &primary, &secondary, "report.pdf");

    handle(
        VirtualPathsCommand::Set(SetArgs {
            file_id: file.id,
            virtual_path: "/docs/report.pdf".to_string(),
            symlink_base: symlink_dir.path().to_str().unwrap().to_string(),
        }),
        &repo,
    )
    .unwrap();

    let symlink = symlink_dir.path().join("docs/report.pdf");
    assert!(
        symlink.is_symlink(),
        "Symlink should be created at docs/report.pdf"
    );

    let updated = repo.get_tracked_file(file.id).unwrap();
    assert_eq!(updated.virtual_path, Some("/docs/report.pdf".to_string()));
}

#[test]
fn test_remove_virtual_path_clears_symlink() {
    let repo = make_repo();
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let symlink_dir = TempDir::new().unwrap();

    let (_, file) = setup_tracked_file(&repo, &primary, &secondary, "data.csv");

    handle(
        VirtualPathsCommand::Set(SetArgs {
            file_id: file.id,
            virtual_path: "/data/data.csv".to_string(),
            symlink_base: symlink_dir.path().to_str().unwrap().to_string(),
        }),
        &repo,
    )
    .unwrap();

    handle(
        VirtualPathsCommand::Remove(RemoveArgs {
            file_id: file.id,
            symlink_base: symlink_dir.path().to_str().unwrap().to_string(),
        }),
        &repo,
    )
    .unwrap();

    let symlink = symlink_dir.path().join("data/data.csv");
    assert!(
        !symlink.is_symlink(),
        "Symlink should be removed after remove command"
    );

    let updated = repo.get_tracked_file(file.id).unwrap();
    assert!(updated.virtual_path.is_none());
}

#[test]
fn test_list_virtual_paths() {
    let repo = make_repo();
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let symlink_dir = TempDir::new().unwrap();

    let (_, file) = setup_tracked_file(&repo, &primary, &secondary, "image.png");

    handle(
        VirtualPathsCommand::Set(SetArgs {
            file_id: file.id,
            virtual_path: "/images/image.png".to_string(),
            symlink_base: symlink_dir.path().to_str().unwrap().to_string(),
        }),
        &repo,
    )
    .unwrap();

    // Should not error — prints to stdout, not verifiable directly, just confirm no panic
    handle(VirtualPathsCommand::List, &repo).unwrap();
}

#[test]
fn test_refresh_rebuilds_symlink_from_db_state() {
    let repo = make_repo();
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let symlink_dir = TempDir::new().unwrap();

    let (_, file) = setup_tracked_file(&repo, &primary, &secondary, "notes.txt");

    // Assign virtual path in DB without going through set command (simulates lost symlink)
    repo.update_tracked_file_virtual_path(file.id, Some("/notes/notes.txt"))
        .unwrap();

    handle(
        VirtualPathsCommand::Refresh(RefreshArgs {
            symlink_base: symlink_dir.path().to_str().unwrap().to_string(),
        }),
        &repo,
    )
    .unwrap();

    let symlink = symlink_dir.path().join("notes/notes.txt");
    assert!(
        symlink.is_symlink(),
        "Refresh should recreate missing symlinks"
    );
}

#[test]
fn test_remove_on_file_without_virtual_path_returns_error() {
    let repo = make_repo();
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let symlink_dir = TempDir::new().unwrap();

    let (_, file) = setup_tracked_file(&repo, &primary, &secondary, "orphan.txt");

    let result = handle(
        VirtualPathsCommand::Remove(RemoveArgs {
            file_id: file.id,
            symlink_base: symlink_dir.path().to_str().unwrap().to_string(),
        }),
        &repo,
    );

    assert!(
        result.is_err(),
        "Remove on a file without a virtual path should return an error"
    );
}
