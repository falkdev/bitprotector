use crate::core::{drive, mirror, tracker};
use crate::db::repository::Repository;
use clap::{Args, Subcommand};

#[derive(Subcommand, Debug)]
pub enum FilesCommand {
    /// Track a file on a drive pair
    Track(TrackArgs),
    /// List tracked files
    List(ListArgs),
    /// Show details of a tracked file
    Show {
        /// Tracked file ID
        id: i64,
    },
    /// Mirror a tracked file to the secondary drive
    Mirror {
        /// Tracked file ID
        id: i64,
    },
    /// Remove a file from tracking
    Untrack {
        /// Tracked file ID
        id: i64,
    },
}

#[derive(Args, Debug)]
pub struct TrackArgs {
    /// Drive pair ID to associate the file with
    pub drive_pair_id: i64,
    /// Relative path of the file within the primary drive
    pub relative_path: String,
    /// Optional virtual path to assign
    #[arg(long)]
    pub virtual_path: Option<String>,
    /// Mirror the file immediately after tracking
    #[arg(long)]
    pub mirror: bool,
}

#[derive(Args, Debug)]
pub struct ListArgs {
    /// Filter by drive pair ID
    #[arg(long)]
    pub drive_id: Option<i64>,
    /// Filter by virtual path prefix
    #[arg(long)]
    pub virtual_prefix: Option<String>,
    /// Filter by mirror status (true/false)
    #[arg(long)]
    pub mirrored: Option<bool>,
    /// Page number (1-based)
    #[arg(long, default_value_t = 1)]
    pub page: i64,
    /// Results per page
    #[arg(long, default_value_t = 50)]
    pub per_page: i64,
}

pub fn handle(cmd: FilesCommand, repo: &Repository) -> anyhow::Result<()> {
    match cmd {
        FilesCommand::Track(args) => {
            let pair = drive::load_operational_pair(repo, args.drive_pair_id)?;
            let tracked = tracker::track_file(
                repo,
                &pair,
                &args.relative_path,
                args.virtual_path.as_deref(),
            )?;
            println!("Tracked file #{}: {}", tracked.id, tracked.relative_path);
            println!("  Checksum:  {}", tracked.checksum);
            println!("  Size:      {} bytes", tracked.file_size);

            if args.mirror {
                let pair = drive::load_operational_pair(repo, args.drive_pair_id)?;
                if pair.standby_accepts_sync() {
                    mirror::mirror_file(&pair, &tracked.relative_path)?;
                    repo.update_tracked_file_mirror_status(tracked.id, true)?;
                    println!("  Mirrored:  yes");
                } else {
                    println!("  Mirrored:  no (standby drive unavailable)");
                }
            }
        }
        FilesCommand::List(args) => {
            let (files, total) = repo.list_tracked_files(
                args.drive_id,
                args.virtual_prefix.as_deref(),
                args.mirrored,
                args.page,
                args.per_page,
            )?;
            println!(
                "Showing {} of {} tracked files (page {})",
                files.len(),
                total,
                args.page
            );
            println!(
                "{:<6} {:<8} {:<40} {:<8} {}",
                "ID", "Drive", "Path", "Mirrored", "Checksum"
            );
            println!("{}", "-".repeat(90));
            for f in files {
                println!(
                    "{:<6} {:<8} {:<40} {:<8} {}",
                    f.id,
                    f.drive_pair_id,
                    f.relative_path,
                    if f.is_mirrored { "yes" } else { "no" },
                    &f.checksum[..16]
                );
            }
        }
        FilesCommand::Show { id } => {
            let file = repo.get_tracked_file(id)?;
            println!("Tracked File #{}", file.id);
            println!("  Drive Pair:    {}", file.drive_pair_id);
            println!("  Path:          {}", file.relative_path);
            println!("  Checksum:      {}", file.checksum);
            println!("  Size:          {} bytes", file.file_size);
            println!(
                "  Virtual Path:  {}",
                file.virtual_path.as_deref().unwrap_or("-")
            );
            println!(
                "  Mirrored:      {}",
                if file.is_mirrored { "yes" } else { "no" }
            );
            println!(
                "  Last Verified: {}",
                file.last_verified.as_deref().unwrap_or("never")
            );
            println!("  Created:       {}", file.created_at);
        }
        FilesCommand::Mirror { id } => {
            let file = repo.get_tracked_file(id)?;
            let pair = drive::load_operational_pair(repo, file.drive_pair_id)?;
            mirror::mirror_file(&pair, &file.relative_path)?;
            repo.update_tracked_file_mirror_status(id, true)?;
            println!("Mirrored file #{}: {}", id, file.relative_path);
        }
        FilesCommand::Untrack { id } => {
            repo.delete_tracked_file(id)?;
            println!("Untracked file #{}", id);
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
    fn test_track_and_list() {
        let repo = make_repo();
        let primary = TempDir::new().unwrap();
        let secondary = TempDir::new().unwrap();
        let pair = setup_pair(&repo, &primary, &secondary);

        fs::write(primary.path().join("a.txt"), b"data").unwrap();
        handle(
            FilesCommand::Track(TrackArgs {
                drive_pair_id: pair.id,
                relative_path: "a.txt".to_string(),
                virtual_path: None,
                mirror: false,
            }),
            &repo,
        )
        .unwrap();

        let (files, total) = repo.list_tracked_files(None, None, None, 1, 50).unwrap();
        assert_eq!(total, 1);
        assert_eq!(files[0].relative_path, "a.txt");
    }

    #[test]
    fn test_track_and_mirror() {
        let repo = make_repo();
        let primary = TempDir::new().unwrap();
        let secondary = TempDir::new().unwrap();
        let pair = setup_pair(&repo, &primary, &secondary);

        fs::write(primary.path().join("b.txt"), b"mirror content").unwrap();
        handle(
            FilesCommand::Track(TrackArgs {
                drive_pair_id: pair.id,
                relative_path: "b.txt".to_string(),
                virtual_path: None,
                mirror: true,
            }),
            &repo,
        )
        .unwrap();

        assert!(secondary.path().join("b.txt").exists());
        let (files, _) = repo.list_tracked_files(None, None, None, 1, 50).unwrap();
        assert!(files[0].is_mirrored);
    }

    #[test]
    fn test_mirror_command() {
        let repo = make_repo();
        let primary = TempDir::new().unwrap();
        let secondary = TempDir::new().unwrap();
        let pair = setup_pair(&repo, &primary, &secondary);

        fs::write(primary.path().join("c.txt"), b"content").unwrap();
        let tracked = repo
            .create_tracked_file(pair.id, "c.txt", "hash", 7, None)
            .unwrap();
        // update checksum properly
        let checksum = crate::core::checksum::checksum_bytes(b"content");
        repo.update_tracked_file_checksum(tracked.id, &checksum, 7)
            .unwrap();

        handle(FilesCommand::Mirror { id: tracked.id }, &repo).unwrap();

        assert!(secondary.path().join("c.txt").exists());
        let file = repo.get_tracked_file(tracked.id).unwrap();
        assert!(file.is_mirrored);
    }

    #[test]
    fn test_untrack_file() {
        let repo = make_repo();
        let pair = repo.create_drive_pair("p", "/a", "/b").unwrap();
        let file = repo
            .create_tracked_file(pair.id, "f.txt", "hash", 1, None)
            .unwrap();
        handle(FilesCommand::Untrack { id: file.id }, &repo).unwrap();
        assert!(repo.get_tracked_file(file.id).is_err());
    }

    #[test]
    fn test_track_mirror_verify_roundtrip() {
        // Module test: Track -> Mirror -> Verify
        use crate::core::{checksum, integrity};
        let repo = make_repo();
        let primary = TempDir::new().unwrap();
        let secondary = TempDir::new().unwrap();
        let pair = setup_pair(&repo, &primary, &secondary);

        let content = b"roundtrip test content";
        fs::write(primary.path().join("rt.txt"), content).unwrap();

        // Track
        let tracked = tracker::track_file(&repo, &pair, "rt.txt", None).unwrap();
        assert_eq!(tracked.checksum, checksum::checksum_bytes(content));

        // Mirror
        mirror::mirror_file(&pair, "rt.txt").unwrap();
        repo.update_tracked_file_mirror_status(tracked.id, true)
            .unwrap();

        // Verify mirror is byte-identical
        let mirror_checksum =
            checksum::checksum_file(&std::path::PathBuf::from(&pair.secondary_path).join("rt.txt"))
                .unwrap();
        assert_eq!(mirror_checksum, tracked.checksum);

        // Integrity check passes
        let result = integrity::check_file_integrity(&pair, &tracked).unwrap();
        assert_eq!(result.status, integrity::IntegrityStatus::Ok);
    }
}
