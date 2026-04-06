use crate::core::{change_detection, drive, mirror, tracker};
use crate::db::repository::Repository;
use clap::{Args, Subcommand};

#[derive(Subcommand, Debug)]
pub enum FoldersCommand {
    /// Start tracking a folder on a drive pair
    Add(AddArgs),
    /// List all tracked folders
    List,
    /// Show details of a tracked folder
    Show { id: i64 },
    /// Remove a tracked folder (does not delete files)
    Remove { id: i64 },
    /// Scan a tracked folder for new or changed files
    Scan(ScanArgs),
    /// Mirror all unmirrored tracked files under a tracked folder
    Mirror { id: i64 },
    /// Watch a tracked folder for filesystem changes (runs until Ctrl+C)
    Watch(WatchArgs),
}

#[derive(Args, Debug)]
pub struct AddArgs {
    /// Drive pair ID to associate with this folder
    pub drive_pair_id: i64,
    /// Folder path relative to the primary drive root
    pub folder_path: String,
    /// Exact virtual path for the tracked folder
    #[arg(long)]
    pub virtual_path: Option<String>,
}

#[derive(Args, Debug)]
pub struct ScanArgs {
    /// Tracked folder ID to scan
    pub id: i64,
}

#[derive(Args, Debug)]
pub struct WatchArgs {
    /// Tracked folder ID to watch
    pub id: i64,
}

pub fn handle(cmd: FoldersCommand, repo: &Repository) -> anyhow::Result<()> {
    match cmd {
        FoldersCommand::Add(args) => {
            let pair = drive::load_operational_pair(repo, args.drive_pair_id)?;
            let folder = tracker::track_folder(
                repo,
                &pair,
                &args.folder_path,
                args.virtual_path.as_deref(),
            )?;
            println!(
                "Registered folder #{}: {} (drive pair #{})",
                folder.id, folder.folder_path, folder.drive_pair_id
            );
        }
        FoldersCommand::List => {
            let folders = repo.list_tracked_folders()?;
            if folders.is_empty() {
                println!("No tracked folders.");
            } else {
                println!(
                    "{:<6} {:<8} {:<40} {}",
                    "ID", "Drive", "Folder Path", "Virtual Path"
                );
                println!("{}", "-".repeat(96));
                for f in folders {
                    println!(
                        "{:<6} {:<8} {:<40} {}",
                        f.id,
                        f.drive_pair_id,
                        f.folder_path,
                        f.virtual_path.as_deref().unwrap_or("-"),
                    );
                }
            }
        }
        FoldersCommand::Show { id } => {
            let folder = repo.get_tracked_folder(id)?;
            println!("Folder #{}", folder.id);
            println!("  Drive Pair:        #{}", folder.drive_pair_id);
            println!("  Path:              {}", folder.folder_path);
            println!(
                "  Virtual Path:      {}",
                folder.virtual_path.as_deref().unwrap_or("(none)")
            );
            println!("  Created:           {}", folder.created_at);
        }
        FoldersCommand::Remove { id } => {
            crate::core::virtual_path::remove_folder_virtual_path(repo, id)?;
            repo.delete_tracked_folder(id)?;
            println!("Removed tracked folder #{}", id);
        }
        FoldersCommand::Scan(args) => {
            let folder = repo.get_tracked_folder(args.id)?;
            let pair = drive::load_operational_pair(repo, folder.drive_pair_id)?;

            // Auto-track new files
            let new_files = tracker::auto_track_folder_files(repo, &pair, &folder)?;
            println!("New files tracked: {}", new_files.len());
            for f in &new_files {
                println!("  + {} (id=#{})", f.relative_path, f.id);
            }

            // Detect changes in existing files
            let changes = change_detection::scan_and_record_changes(repo, &pair)?;
            let folder_changes: Vec<_> = changes
                .iter()
                .filter(|(f, _)| {
                    f.relative_path
                        .starts_with(&format!("{}/", folder.folder_path))
                })
                .collect();

            if folder_changes.is_empty() {
                println!("No changes detected in folder.");
            } else {
                println!("Changed files: {}", folder_changes.len());
                for (f, new_hash) in &folder_changes {
                    println!(
                        "  ~ {} (stored: {}..., current: {}...)",
                        f.relative_path,
                        &f.checksum[..8],
                        &new_hash[..8]
                    );
                }
            }
        }
        FoldersCommand::Mirror { id } => {
            let folder = repo.get_tracked_folder(id)?;
            let pair = drive::load_operational_pair(repo, folder.drive_pair_id)?;
            anyhow::ensure!(
                pair.standby_accepts_sync(),
                "Standby drive is not currently available for mirroring"
            );

            let files = repo.list_tracked_files_under_folder(pair.id, &folder.folder_path)?;
            let mut mirrored = 0usize;
            for file in files.into_iter().filter(|file| !file.is_mirrored) {
                mirror::mirror_file(&pair, &file.relative_path)?;
                repo.update_tracked_file_mirror_status(file.id, true)?;
                let _ = repo.complete_pending_mirror_queue_for_file(file.id);
                mirrored += 1;
            }
            println!(
                "Mirrored {} file(s) under folder {}",
                mirrored, folder.folder_path
            );
        }
        FoldersCommand::Watch(args) => {
            let folder = repo.get_tracked_folder(args.id)?;
            let pair = drive::load_operational_pair(repo, folder.drive_pair_id)?;
            let full_path = std::path::PathBuf::from(pair.active_path()).join(&folder.folder_path);

            println!("Watching {} (press Ctrl+C to stop)...", full_path.display());

            let (tx, rx) = std::sync::mpsc::channel();
            let _watcher = change_detection::watch_folder(
                full_path
                    .to_str()
                    .ok_or_else(|| anyhow::anyhow!("Invalid path"))?,
                move |event| {
                    let _ = tx.send(event);
                },
            )?;

            for event in rx {
                println!("  Event: {:?}", event.kind);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repository::{create_memory_pool, Repository};
    use crate::db::schema::initialize_schema;
    use std::fs;
    use tempfile::TempDir;

    fn make_repo() -> Repository {
        let pool = create_memory_pool().unwrap();
        initialize_schema(&pool.get().unwrap()).unwrap();
        Repository::new(pool)
    }

    fn setup_pair(
        repo: &Repository,
        primary: &TempDir,
        secondary: &TempDir,
    ) -> crate::db::repository::DrivePair {
        repo.create_drive_pair(
            "test",
            primary.path().to_str().unwrap(),
            secondary.path().to_str().unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn test_add_folder() {
        let repo = make_repo();
        let primary = TempDir::new().unwrap();
        let secondary = TempDir::new().unwrap();
        let pair = setup_pair(&repo, &primary, &secondary);
        fs::create_dir(primary.path().join("docs")).unwrap();

        handle(
            FoldersCommand::Add(AddArgs {
                drive_pair_id: pair.id,
                folder_path: "docs".to_string(),
                virtual_path: None,
            }),
            &repo,
        )
        .unwrap();

        let folders = repo.list_tracked_folders().unwrap();
        assert_eq!(folders.len(), 1);
        assert_eq!(folders[0].folder_path, "docs");
    }

    #[test]
    fn test_list_folders_empty() {
        let repo = make_repo();
        handle(FoldersCommand::List, &repo).unwrap();
    }

    #[test]
    fn test_show_folder() {
        let repo = make_repo();
        let primary = TempDir::new().unwrap();
        let secondary = TempDir::new().unwrap();
        let pair = setup_pair(&repo, &primary, &secondary);
        fs::create_dir(primary.path().join("proj")).unwrap();
        let folder = repo.create_tracked_folder(pair.id, "proj", None).unwrap();
        handle(FoldersCommand::Show { id: folder.id }, &repo).unwrap();
    }

    #[test]
    fn test_remove_folder() {
        let repo = make_repo();
        let primary = TempDir::new().unwrap();
        let secondary = TempDir::new().unwrap();
        let pair = setup_pair(&repo, &primary, &secondary);
        fs::create_dir(primary.path().join("tmp")).unwrap();
        let folder = repo.create_tracked_folder(pair.id, "tmp", None).unwrap();
        handle(FoldersCommand::Remove { id: folder.id }, &repo).unwrap();
        assert!(repo.list_tracked_folders().unwrap().is_empty());
    }

    #[test]
    fn test_scan_finds_new_files() {
        let repo = make_repo();
        let primary = TempDir::new().unwrap();
        let secondary = TempDir::new().unwrap();
        let pair = setup_pair(&repo, &primary, &secondary);
        fs::create_dir(primary.path().join("incoming")).unwrap();
        fs::write(primary.path().join("incoming/new.txt"), b"new file").unwrap();
        let folder = repo
            .create_tracked_folder(pair.id, "incoming", None)
            .unwrap();

        handle(FoldersCommand::Scan(ScanArgs { id: folder.id }), &repo).unwrap();

        let (files, _) = repo
            .list_tracked_files(Some(pair.id), None, None, 1, 100)
            .unwrap();
        assert_eq!(files.len(), 1, "New file should be auto-tracked by scan");
    }
    #[test]
    fn test_add_folder_with_virtual_path() {
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
            folders[0].virtual_path.as_deref(),
            Some(virtual_path_on_disk.to_str().unwrap())
        );
        assert_eq!(
            fs::read_link(&virtual_path_on_disk).unwrap(),
            primary.path().join("photos")
        );
    }
}
