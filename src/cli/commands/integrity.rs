use crate::core::drive;
use crate::core::integrity::{self, IntegrityStatus};
use crate::core::integrity_runs;
use crate::db::repository::Repository;
use clap::{Args, Subcommand};

#[derive(Subcommand, Debug)]
pub enum IntegrityCommand {
    /// Check and optionally recover a single tracked file
    Check(CheckArgs),
    /// Check all tracked files across all (or one) drive pair
    CheckAll(CheckAllArgs),
}

#[derive(Args, Debug)]
pub struct CheckArgs {
    /// Tracked file ID to check
    pub file_id: i64,
    /// Attempt automatic recovery if corruption is detected
    #[arg(long)]
    pub recover: bool,
}

#[derive(Args, Debug)]
pub struct CheckAllArgs {
    /// Limit checks to a specific drive pair ID
    #[arg(long)]
    pub drive_id: Option<i64>,
    /// Attempt automatic recovery for auto-recoverable issues
    #[arg(long)]
    pub recover: bool,
}

pub fn handle(cmd: IntegrityCommand, repo: &Repository) -> anyhow::Result<()> {
    match cmd {
        IntegrityCommand::Check(args) => check_single(repo, args.file_id, args.recover),
        IntegrityCommand::CheckAll(args) => check_all(repo, args.drive_id, args.recover),
    }
}

fn status_label(status: &IntegrityStatus) -> &'static str {
    match status {
        IntegrityStatus::Ok => "OK",
        IntegrityStatus::MasterCorrupted => "MASTER_CORRUPTED",
        IntegrityStatus::MirrorCorrupted => "MIRROR_CORRUPTED",
        IntegrityStatus::BothCorrupted => "BOTH_CORRUPTED",
        IntegrityStatus::MirrorMissing => "MIRROR_MISSING",
        IntegrityStatus::MasterMissing => "MASTER_MISSING",
        IntegrityStatus::PrimaryDriveUnavailable => "PRIMARY_DRIVE_UNAVAILABLE",
        IntegrityStatus::SecondaryDriveUnavailable => "SECONDARY_DRIVE_UNAVAILABLE",
    }
}

fn check_single(repo: &Repository, file_id: i64, recover: bool) -> anyhow::Result<()> {
    let file = repo.get_tracked_file(file_id)?;
    let pair = drive::load_operational_pair(repo, file.drive_pair_id)?;
    let result = integrity::check_file_integrity(&pair, &file)?;

    println!(
        "File #{}: {} — {}",
        file.id,
        file.relative_path,
        status_label(&result.status)
    );

    if result.status == IntegrityStatus::Ok {
        repo.update_tracked_file_last_integrity_check_at(file_id)?;
        return Ok(());
    }

    if recover {
        let recovered = integrity::attempt_recovery(&pair, &file, &result)?;
        if recovered {
            repo.update_tracked_file_last_integrity_check_at(file_id)?;
            println!("  Recovery: successful");
            return Ok(());
        } else {
            println!(
                "  Recovery: manual action required (both copies corrupted or master missing)"
            );
        }
    }

    anyhow::bail!("Integrity check failed: {}", status_label(&result.status))
}

fn check_all(repo: &Repository, drive_id: Option<i64>, recover: bool) -> anyhow::Result<()> {
    let run = integrity_runs::run_sync(repo, drive_id, recover, "cli")?;
    let clean = run.processed_files - run.attention_files;

    println!(
        "Integrity run complete (#{}): {} checked, {} clean, {} recovered, {} need attention",
        run.id, run.processed_files, clean, run.recovered_files, run.attention_files
    );
    if run.attention_files > 0 {
        let mut page = 1i64;
        let per_page = 200i64;
        loop {
            let (results, total) = repo.list_integrity_run_results(run.id, true, page, per_page)?;
            if results.is_empty() {
                break;
            }
            for result in &results {
                println!(
                    "  ISSUE #{}: {} — {}",
                    result.file_id, result.relative_path, result.status
                );
            }
            if (page * per_page) >= total {
                break;
            }
            page += 1;
        }
        anyhow::bail!(
            "{} files need attention in run #{}",
            run.attention_files,
            run.id
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::tracker;
    use crate::db::repository::{create_memory_pool, Repository};
    use crate::db::schema::initialize_schema;
    use std::fs;
    use tempfile::TempDir;

    fn make_repo() -> Repository {
        let pool = create_memory_pool().unwrap();
        initialize_schema(&pool.get().unwrap()).unwrap();
        Repository::new(pool)
    }

    fn setup(
        repo: &Repository,
        primary: &TempDir,
        secondary: &TempDir,
    ) -> crate::db::repository::DrivePair {
        repo.create_drive_pair(
            "t",
            primary.path().to_str().unwrap(),
            secondary.path().to_str().unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn test_check_ok_file() {
        let repo = make_repo();
        let primary = TempDir::new().unwrap();
        let secondary = TempDir::new().unwrap();
        let pair = setup(&repo, &primary, &secondary);

        let content = b"intact";
        fs::write(primary.path().join("f.txt"), content).unwrap();
        fs::write(secondary.path().join("f.txt"), content).unwrap();
        let tracked = tracker::track_file(&repo, &pair, "f.txt", None).unwrap();

        handle(
            IntegrityCommand::Check(CheckArgs {
                file_id: tracked.id,
                recover: false,
            }),
            &repo,
        )
        .unwrap();
    }

    #[test]
    fn test_check_missing_mirror_returns_err() {
        let repo = make_repo();
        let primary = TempDir::new().unwrap();
        let secondary = TempDir::new().unwrap();
        let pair = setup(&repo, &primary, &secondary);

        fs::write(primary.path().join("nm.txt"), b"data").unwrap();
        let tracked = tracker::track_file(&repo, &pair, "nm.txt", None).unwrap();

        let result = handle(
            IntegrityCommand::Check(CheckArgs {
                file_id: tracked.id,
                recover: false,
            }),
            &repo,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_check_mirror_corrupted_with_recovery() {
        let repo = make_repo();
        let primary = TempDir::new().unwrap();
        let secondary = TempDir::new().unwrap();
        let pair = setup(&repo, &primary, &secondary);

        let content = b"original";
        fs::write(primary.path().join("g.txt"), content).unwrap();
        let tracked = tracker::track_file(&repo, &pair, "g.txt", None).unwrap();

        // Corrupt the mirror
        fs::write(secondary.path().join("g.txt"), b"corrupted").unwrap();

        handle(
            IntegrityCommand::Check(CheckArgs {
                file_id: tracked.id,
                recover: true,
            }),
            &repo,
        )
        .unwrap();

        // Mirror should be restored
        let restored = fs::read(secondary.path().join("g.txt")).unwrap();
        assert_eq!(restored, content);
    }

    #[test]
    fn test_check_all_no_issues() {
        let repo = make_repo();
        let primary = TempDir::new().unwrap();
        let secondary = TempDir::new().unwrap();
        let pair = setup(&repo, &primary, &secondary);

        for name in &["a.txt", "b.txt"] {
            fs::write(primary.path().join(name), b"data").unwrap();
            fs::write(secondary.path().join(name), b"data").unwrap();
            tracker::track_file(&repo, &pair, name, None).unwrap();
        }

        handle(
            IntegrityCommand::CheckAll(CheckAllArgs {
                drive_id: Some(pair.id),
                recover: false,
            }),
            &repo,
        )
        .unwrap();
    }

    #[test]
    fn test_detect_master_corrupted() {
        let repo = make_repo();
        let primary = TempDir::new().unwrap();
        let secondary = TempDir::new().unwrap();
        let pair = setup(&repo, &primary, &secondary);

        let content = b"good content";
        fs::write(primary.path().join("mc.txt"), content).unwrap();
        fs::write(secondary.path().join("mc.txt"), content).unwrap();
        let tracked = tracker::track_file(&repo, &pair, "mc.txt", None).unwrap();

        // Corrupt master
        fs::write(primary.path().join("mc.txt"), b"corrupted master").unwrap();

        let file = repo.get_tracked_file(tracked.id).unwrap();
        let result = crate::core::integrity::check_file_integrity(&pair, &file).unwrap();
        assert_eq!(result.status, IntegrityStatus::MasterCorrupted);
    }

    #[test]
    fn test_detect_both_corrupted_no_recovery() {
        let repo = make_repo();
        let primary = TempDir::new().unwrap();
        let secondary = TempDir::new().unwrap();
        let pair = setup(&repo, &primary, &secondary);

        let content = b"original";
        fs::write(primary.path().join("bc.txt"), content).unwrap();
        fs::write(secondary.path().join("bc.txt"), content).unwrap();
        let tracked = tracker::track_file(&repo, &pair, "bc.txt", None).unwrap();

        fs::write(primary.path().join("bc.txt"), b"bad master").unwrap();
        fs::write(secondary.path().join("bc.txt"), b"bad mirror").unwrap();

        let file = repo.get_tracked_file(tracked.id).unwrap();
        let result = crate::core::integrity::check_file_integrity(&pair, &file).unwrap();
        assert_eq!(result.status, IntegrityStatus::BothCorrupted);

        // Attempt recovery returns false (cannot recover)
        let no_recovery = crate::core::integrity::attempt_recovery(&pair, &file, &result).unwrap();
        assert!(!no_recovery);
    }
}
