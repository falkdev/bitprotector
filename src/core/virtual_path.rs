use crate::core::drive;
use crate::db::repository::{DrivePair, Repository, TrackedFile, TrackedFolder};
use anyhow::Context;
use std::collections::HashMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VirtualOwner {
    File(i64),
    Folder(i64),
}

#[derive(Debug, Clone)]
struct VirtualReservation {
    owner: VirtualOwner,
    virtual_path: String,
}

#[derive(Debug, Clone)]
struct VirtualMapping {
    owner: VirtualOwner,
    virtual_path: String,
    real_path: String,
}

#[derive(Debug)]
pub struct SymlinkRefreshResult {
    pub created: u32,
    pub removed: u32,
    pub errors: Vec<String>,
}

pub fn normalize_virtual_path(virtual_path: &str) -> anyhow::Result<String> {
    let trimmed = virtual_path.trim();
    if trimmed.is_empty() {
        anyhow::bail!("Virtual path is required");
    }

    let raw_path = Path::new(trimmed);
    if !raw_path.is_absolute() {
        anyhow::bail!(
            "Virtual path must be absolute (start with /): {}",
            virtual_path
        );
    }

    let mut segments = Vec::new();
    for component in raw_path.components() {
        match component {
            Component::RootDir => {}
            Component::Normal(segment) => segments.push(segment.to_string_lossy().to_string()),
            Component::CurDir => {}
            Component::ParentDir => {
                anyhow::bail!("Parent-directory traversal is not allowed in virtual paths")
            }
            Component::Prefix(_) => anyhow::bail!("Unsupported virtual path: {}", virtual_path),
        }
    }

    if segments.is_empty() {
        anyhow::bail!("Virtual paths may not use the filesystem root");
    }

    Ok(format!("/{}", segments.join("/")))
}

/// Set the virtual path for a tracked file and create/update its virtual-path symlink.
pub fn set_virtual_path(repo: &Repository, file_id: i64, virtual_path: &str) -> anyhow::Result<()> {
    let file = repo.get_tracked_file(file_id)?;
    let pair = drive::load_operational_pair(repo, file.drive_pair_id)?;
    let normalized = validate_virtual_path(repo, VirtualOwner::File(file_id), virtual_path)?;
    let real_path = file_real_path(&pair, &file);

    let previous = file
        .virtual_path
        .as_deref()
        .map(normalize_virtual_path)
        .transpose()?;
    let replacing_same_path = previous.as_deref() == Some(normalized.as_str());

    create_symlink(&normalized, &real_path, replacing_same_path)?;

    if let Err(error) = repo.update_tracked_file_virtual_path(file_id, Some(&normalized)) {
        if !replacing_same_path {
            let _ = remove_symlink(&normalized);
        }
        return Err(error);
    }

    if let Some(previous) = previous {
        if previous != normalized {
            remove_symlink(&previous)?;
        }
    }

    Ok(())
}

/// Remove the virtual path for a tracked file and delete the virtual-path symlink.
pub fn remove_virtual_path(repo: &Repository, file_id: i64) -> anyhow::Result<()> {
    let file = repo.get_tracked_file(file_id)?;
    let Some(virtual_path) = file.virtual_path.as_deref() else {
        anyhow::bail!("File #{} has no virtual path assigned", file_id);
    };

    let normalized = normalize_virtual_path(virtual_path)?;
    repo.update_tracked_file_virtual_path(file_id, None)?;
    remove_symlink(&normalized)?;
    Ok(())
}

pub fn set_folder_virtual_path(
    repo: &Repository,
    folder_id: i64,
    virtual_path: &str,
) -> anyhow::Result<()> {
    let folder = repo.get_tracked_folder(folder_id)?;
    let pair = drive::load_operational_pair(repo, folder.drive_pair_id)?;
    let normalized = validate_virtual_path(repo, VirtualOwner::Folder(folder_id), virtual_path)?;
    let real_path = folder_real_path(&pair, &folder);

    let previous = folder
        .virtual_path
        .as_deref()
        .map(normalize_virtual_path)
        .transpose()?;
    let replacing_same_path = previous.as_deref() == Some(normalized.as_str());

    create_symlink(&normalized, &real_path, replacing_same_path)?;

    if let Err(error) = repo.update_tracked_folder(folder_id, Some(Some(&normalized))) {
        if !replacing_same_path {
            let _ = remove_symlink(&normalized);
        }
        return Err(error);
    }

    if let Some(previous) = previous {
        if previous != normalized {
            remove_symlink(&previous)?;
        }
    }

    Ok(())
}

pub fn remove_folder_virtual_path(repo: &Repository, folder_id: i64) -> anyhow::Result<()> {
    let folder = repo.get_tracked_folder(folder_id)?;
    let Some(virtual_path) = folder.virtual_path.as_deref() else {
        return Ok(());
    };

    let normalized = normalize_virtual_path(virtual_path)?;
    repo.update_tracked_folder(folder_id, Some(None))?;
    remove_symlink(&normalized)?;
    Ok(())
}

/// Create a symlink at `virtual_path` -> `real_path`.
pub fn create_symlink(
    virtual_path: &str,
    real_path: &str,
    allow_replace_existing_symlink: bool,
) -> anyhow::Result<()> {
    let link_path = PathBuf::from(virtual_path);

    if let Some(parent) = link_path.parent() {
        fs::create_dir_all(parent).context("Failed to create virtual path directories")?;
    }

    match fs::symlink_metadata(&link_path) {
        Ok(metadata) => {
            if !metadata.file_type().is_symlink() {
                anyhow::bail!(
                    "Virtual path already exists and is not a BitProtector-managed symlink: {}",
                    virtual_path
                );
            }
            if !allow_replace_existing_symlink {
                anyhow::bail!("Virtual path is already in use: {}", virtual_path);
            }
            fs::remove_file(&link_path).context("Failed to remove old symlink")?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error).context("Failed to inspect virtual path"),
    }

    std::os::unix::fs::symlink(real_path, &link_path).context("Failed to create symlink")?;
    Ok(())
}

/// Remove the symlink at `virtual_path`.
pub fn remove_symlink(virtual_path: &str) -> anyhow::Result<()> {
    let link_path = PathBuf::from(virtual_path);
    match fs::symlink_metadata(&link_path) {
        Ok(metadata) => {
            if !metadata.file_type().is_symlink() {
                anyhow::bail!(
                    "Virtual path is not a BitProtector-managed symlink: {}",
                    virtual_path
                );
            }
            fs::remove_file(&link_path)?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error).context("Failed to inspect virtual path"),
    }
    Ok(())
}

/// Regenerate all virtual-path symlinks from the database.
pub fn refresh_all_virtual_paths(
    repo: &Repository,
    drive_pairs: &HashMap<i64, DrivePair>,
) -> anyhow::Result<SymlinkRefreshResult> {
    let mut created = 0u32;
    let removed = 0u32;
    let mut errors = Vec::new();

    let mappings = collect_virtual_mappings(repo, drive_pairs)?;
    for mapping in mappings {
        if let Err(error) = validate_virtual_path(repo, mapping.owner, &mapping.virtual_path) {
            errors.push(owner_error(mapping.owner, &error.to_string()));
            continue;
        }

        match create_symlink(&mapping.virtual_path, &mapping.real_path, true) {
            Ok(()) => created += 1,
            Err(error) => errors.push(owner_error(mapping.owner, &error.to_string())),
        }
    }

    Ok(SymlinkRefreshResult {
        created,
        removed,
        errors,
    })
}

fn validate_virtual_path(
    repo: &Repository,
    owner: VirtualOwner,
    virtual_path: &str,
) -> anyhow::Result<String> {
    let normalized = normalize_virtual_path(virtual_path)?;
    let reservations = collect_virtual_reservations(repo)?;

    for reservation in reservations {
        if reservation.owner == owner {
            continue;
        }

        if reservation.virtual_path == normalized {
            anyhow::bail!("Virtual path is already assigned: {}", normalized);
        }

        if paths_overlap(&reservation.virtual_path, &normalized) {
            anyhow::bail!(
                "Virtual path overlaps an existing virtual path: {}",
                reservation.virtual_path
            );
        }
    }

    match fs::symlink_metadata(&normalized) {
        Ok(metadata) => {
            if !metadata.file_type().is_symlink() {
                anyhow::bail!(
                    "Virtual path already exists and is not a BitProtector-managed symlink: {}",
                    normalized
                );
            }

            let owned_by_current =
                collect_virtual_reservations(repo)?
                    .into_iter()
                    .any(|reservation| {
                        reservation.owner == owner && reservation.virtual_path == normalized
                    });

            if !owned_by_current {
                anyhow::bail!("Virtual path is already in use: {}", normalized);
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error).context("Failed to inspect virtual path"),
    }

    Ok(normalized)
}

fn collect_virtual_reservations(repo: &Repository) -> anyhow::Result<Vec<VirtualReservation>> {
    let (files, _) = repo.list_tracked_files(None, None, None, 1, i64::MAX)?;
    let mut reservations = Vec::new();

    for file in files {
        if let Some(virtual_path) = file.virtual_path.as_deref() {
            reservations.push(VirtualReservation {
                owner: VirtualOwner::File(file.id),
                virtual_path: normalize_virtual_path(virtual_path)?,
            });
        }
    }

    for folder in repo.list_tracked_folders()? {
        if let Some(virtual_path) = folder.virtual_path.as_deref() {
            reservations.push(VirtualReservation {
                owner: VirtualOwner::Folder(folder.id),
                virtual_path: normalize_virtual_path(virtual_path)?,
            });
        }
    }

    Ok(reservations)
}

fn collect_virtual_mappings(
    repo: &Repository,
    drive_pairs: &HashMap<i64, DrivePair>,
) -> anyhow::Result<Vec<VirtualMapping>> {
    let (files, _) = repo.list_tracked_files(None, None, None, 1, i64::MAX)?;
    let mut mappings = Vec::new();

    for file in files {
        let Some(virtual_path) = file.virtual_path.as_deref() else {
            continue;
        };
        let pair = drive_pairs
            .get(&file.drive_pair_id)
            .ok_or_else(|| anyhow::anyhow!("Drive pair {} not found", file.drive_pair_id))?;
        mappings.push(VirtualMapping {
            owner: VirtualOwner::File(file.id),
            virtual_path: normalize_virtual_path(virtual_path)?,
            real_path: file_real_path(pair, &file),
        });
    }

    for folder in repo.list_tracked_folders()? {
        let Some(virtual_path) = folder.virtual_path.as_deref() else {
            continue;
        };
        let pair = drive_pairs
            .get(&folder.drive_pair_id)
            .ok_or_else(|| anyhow::anyhow!("Drive pair {} not found", folder.drive_pair_id))?;
        mappings.push(VirtualMapping {
            owner: VirtualOwner::Folder(folder.id),
            virtual_path: normalize_virtual_path(virtual_path)?,
            real_path: folder_real_path(pair, &folder),
        });
    }

    Ok(mappings)
}

fn file_real_path(pair: &DrivePair, file: &TrackedFile) -> String {
    PathBuf::from(pair.active_path())
        .join(&file.relative_path)
        .to_string_lossy()
        .to_string()
}

fn folder_real_path(pair: &DrivePair, folder: &TrackedFolder) -> String {
    PathBuf::from(pair.active_path())
        .join(&folder.folder_path)
        .to_string_lossy()
        .to_string()
}

fn paths_overlap(left: &str, right: &str) -> bool {
    left == right
        || left
            .strip_prefix(right)
            .is_some_and(|suffix| suffix.starts_with('/'))
        || right
            .strip_prefix(left)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

fn owner_error(owner: VirtualOwner, message: &str) -> String {
    match owner {
        VirtualOwner::File(id) => format!("File {}: {}", id, message),
        VirtualOwner::Folder(id) => format!("Folder {}: {}", id, message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::tracker;
    use crate::db::repository::{create_memory_pool, Repository};
    use crate::db::schema::initialize_schema;
    use proptest::prelude::*;
    use tempfile::TempDir;

    fn setup_repo() -> Repository {
        let pool = create_memory_pool().unwrap();
        {
            let conn = pool.get().unwrap();
            initialize_schema(&conn).unwrap();
        }
        Repository::new(pool)
    }

    fn setup_file(
        repo: &Repository,
        primary: &TempDir,
        secondary: &TempDir,
        name: &str,
    ) -> (
        crate::db::repository::DrivePair,
        crate::db::repository::TrackedFile,
    ) {
        let pair = repo
            .create_drive_pair(
                "pair",
                primary.path().to_str().unwrap(),
                secondary.path().to_str().unwrap(),
            )
            .unwrap();
        fs::write(primary.path().join(name), b"content").unwrap();
        let file = tracker::track_file(repo, &pair, name, None).unwrap();
        (pair, file)
    }

    #[test]
    fn test_normalize_virtual_path() {
        assert_eq!(
            normalize_virtual_path(" /docs/../bad ")
                .unwrap_err()
                .to_string(),
            "Parent-directory traversal is not allowed in virtual paths"
        );
        assert_eq!(
            normalize_virtual_path("/docs//report.txt").unwrap(),
            "/docs/report.txt"
        );
    }

    #[test]
    fn test_set_virtual_path_creates_literal_symlink() {
        let repo = setup_repo();
        let primary = TempDir::new().unwrap();
        let secondary = TempDir::new().unwrap();
        let virtual_root = TempDir::new().unwrap();
        let (_, file) = setup_file(&repo, &primary, &secondary, "doc.txt");
        let virtual_path_on_disk = virtual_root.path().join("docs/report.txt");

        set_virtual_path(&repo, file.id, virtual_path_on_disk.to_str().unwrap()).unwrap();

        assert!(virtual_path_on_disk.is_symlink());
        assert_eq!(
            fs::read_link(&virtual_path_on_disk).unwrap(),
            primary.path().join("doc.txt")
        );
    }

    #[test]
    fn test_set_virtual_path_refuses_existing_regular_file() {
        let repo = setup_repo();
        let primary = TempDir::new().unwrap();
        let secondary = TempDir::new().unwrap();
        let virtual_root = TempDir::new().unwrap();
        let (_, file) = setup_file(&repo, &primary, &secondary, "doc.txt");
        let virtual_path_on_disk = virtual_root.path().join("docs/report.txt");
        fs::create_dir_all(virtual_path_on_disk.parent().unwrap()).unwrap();
        fs::write(&virtual_path_on_disk, b"foreign").unwrap();

        let error =
            set_virtual_path(&repo, file.id, virtual_path_on_disk.to_str().unwrap()).unwrap_err();
        assert!(error
            .to_string()
            .contains("not a BitProtector-managed symlink"));
    }

    #[test]
    fn test_remove_virtual_path_deletes_symlink() {
        let repo = setup_repo();
        let primary = TempDir::new().unwrap();
        let secondary = TempDir::new().unwrap();
        let virtual_root = TempDir::new().unwrap();
        let (_, file) = setup_file(&repo, &primary, &secondary, "doc.txt");
        let virtual_path_on_disk = virtual_root.path().join("docs/report.txt");

        set_virtual_path(&repo, file.id, virtual_path_on_disk.to_str().unwrap()).unwrap();
        remove_virtual_path(&repo, file.id).unwrap();

        assert!(!virtual_path_on_disk.exists());
        assert!(repo
            .get_tracked_file(file.id)
            .unwrap()
            .virtual_path
            .is_none());
    }

    #[test]
    fn test_folder_virtual_path_creates_directory_symlink() {
        let repo = setup_repo();
        let primary = TempDir::new().unwrap();
        let secondary = TempDir::new().unwrap();
        let virtual_root = TempDir::new().unwrap();
        fs::create_dir(primary.path().join("reports")).unwrap();
        let pair = repo
            .create_drive_pair(
                "pair",
                primary.path().to_str().unwrap(),
                secondary.path().to_str().unwrap(),
            )
            .unwrap();
        let folder = tracker::track_folder(&repo, &pair, "reports", None).unwrap();
        let virtual_path_on_disk = virtual_root.path().join("docs");

        set_folder_virtual_path(&repo, folder.id, virtual_path_on_disk.to_str().unwrap()).unwrap();

        assert!(virtual_path_on_disk.is_symlink());
        assert_eq!(
            fs::read_link(&virtual_path_on_disk).unwrap(),
            primary.path().join("reports")
        );
    }

    #[test]
    fn test_overlap_validation_rejects_nested_paths() {
        let repo = setup_repo();
        let primary = TempDir::new().unwrap();
        let secondary = TempDir::new().unwrap();
        let virtual_root = TempDir::new().unwrap();

        fs::create_dir(primary.path().join("reports")).unwrap();
        fs::write(primary.path().join("reports/doc.txt"), b"content").unwrap();
        let pair = repo
            .create_drive_pair(
                "pair",
                primary.path().to_str().unwrap(),
                secondary.path().to_str().unwrap(),
            )
            .unwrap();
        let folder = tracker::track_folder(&repo, &pair, "reports", None).unwrap();
        let file = tracker::track_file(&repo, &pair, "reports/doc.txt", None).unwrap();
        let folder_virtual_path = virtual_root.path().join("docs");
        let file_virtual_path = virtual_root.path().join("docs/doc.txt");

        set_folder_virtual_path(&repo, folder.id, folder_virtual_path.to_str().unwrap()).unwrap();
        let error =
            set_virtual_path(&repo, file.id, file_virtual_path.to_str().unwrap()).unwrap_err();

        assert!(error.to_string().contains("overlaps"));
    }

    #[test]
    fn test_refresh_retargets_file_and_folder_symlinks() {
        let repo = setup_repo();
        let primary = TempDir::new().unwrap();
        let secondary = TempDir::new().unwrap();
        let virtual_root = TempDir::new().unwrap();

        fs::create_dir(primary.path().join("docs")).unwrap();
        fs::create_dir(secondary.path().join("docs")).unwrap();
        fs::write(primary.path().join("docs/report.txt"), b"content").unwrap();
        fs::write(secondary.path().join("docs/report.txt"), b"content").unwrap();

        let pair = repo
            .create_drive_pair(
                "pair",
                primary.path().to_str().unwrap(),
                secondary.path().to_str().unwrap(),
            )
            .unwrap();
        let tracked = tracker::track_file(&repo, &pair, "docs/report.txt", None).unwrap();
        let folder = tracker::track_folder(&repo, &pair, "docs", None).unwrap();

        let file_virtual_path = virtual_root.path().join("virtual/report.txt");
        let folder_virtual_path = virtual_root.path().join("virtual/docs");
        set_virtual_path(&repo, tracked.id, file_virtual_path.to_str().unwrap()).unwrap();
        set_folder_virtual_path(&repo, folder.id, folder_virtual_path.to_str().unwrap()).unwrap();

        drive::ensure_drive_root_marker(primary.path().to_str().unwrap()).unwrap();
        drive::ensure_drive_root_marker(secondary.path().to_str().unwrap()).unwrap();
        fs::remove_file(drive::drive_root_marker_path(
            primary.path().to_str().unwrap(),
        ))
        .unwrap();

        let failed_over = drive::load_operational_pair(&repo, pair.id).unwrap();
        assert_eq!(failed_over.active_role, "secondary");
        assert_eq!(
            fs::read_link(&file_virtual_path).unwrap(),
            secondary.path().join("docs/report.txt")
        );
        assert_eq!(
            fs::read_link(&folder_virtual_path).unwrap(),
            secondary.path().join("docs")
        );
    }

    proptest! {
        #[test]
        fn proptest_normalize_virtual_path_is_idempotent(input in ".{0,64}") {
            if let Ok(normalized) = normalize_virtual_path(&input) {
                let second = normalize_virtual_path(&normalized).unwrap();
                prop_assert_eq!(second, normalized);
            }
        }

        #[test]
        fn proptest_normalized_virtual_paths_have_expected_shape(input in ".{0,64}") {
            if let Ok(normalized) = normalize_virtual_path(&input) {
                prop_assert!(normalized.starts_with('/'));
                prop_assert!(!normalized.contains("/../"));
                prop_assert!(!normalized.ends_with('/'));
            }
        }
    }
}
