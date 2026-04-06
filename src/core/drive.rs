use std::collections::HashMap;
use std::path::Path;

use crate::db::repository::{DrivePair, Repository};
use crate::logging::event_logger;

pub const DRIVE_ROOT_MARKER: &str = ".bitprotector-drive-root";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriveRole {
    Primary,
    Secondary,
}

impl DriveRole {
    pub fn as_str(self) -> &'static str {
        match self {
            DriveRole::Primary => "primary",
            DriveRole::Secondary => "secondary",
        }
    }

    pub fn opposite(self) -> DriveRole {
        match self {
            DriveRole::Primary => DriveRole::Secondary,
            DriveRole::Secondary => DriveRole::Primary,
        }
    }

    pub fn from_str(value: &str) -> DriveRole {
        match value {
            "secondary" => DriveRole::Secondary,
            _ => DriveRole::Primary,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriveState {
    Active,
    Quiescing,
    Failed,
    Rebuilding,
}

impl DriveState {
    pub fn as_str(self) -> &'static str {
        match self {
            DriveState::Active => "active",
            DriveState::Quiescing => "quiescing",
            DriveState::Failed => "failed",
            DriveState::Rebuilding => "rebuilding",
        }
    }

    pub fn from_str(value: &str) -> DriveState {
        match value {
            "quiescing" => DriveState::Quiescing,
            "failed" => DriveState::Failed,
            "rebuilding" => DriveState::Rebuilding,
            _ => DriveState::Active,
        }
    }
}

impl DrivePair {
    pub fn role_state(&self, role: DriveRole) -> DriveState {
        match role {
            DriveRole::Primary => DriveState::from_str(&self.primary_state),
            DriveRole::Secondary => DriveState::from_str(&self.secondary_state),
        }
    }

    pub fn active_role_enum(&self) -> DriveRole {
        DriveRole::from_str(&self.active_role)
    }

    pub fn standby_role(&self) -> DriveRole {
        self.active_role_enum().opposite()
    }

    pub fn path_for_role(&self, role: DriveRole) -> &str {
        match role {
            DriveRole::Primary => &self.primary_path,
            DriveRole::Secondary => &self.secondary_path,
        }
    }

    pub fn active_path(&self) -> &str {
        self.path_for_role(self.active_role_enum())
    }

    pub fn standby_path(&self) -> &str {
        self.path_for_role(self.standby_role())
    }

    pub fn is_quiescing(&self) -> bool {
        self.role_state(DriveRole::Primary) == DriveState::Quiescing
            || self.role_state(DriveRole::Secondary) == DriveState::Quiescing
    }

    pub fn is_degraded(&self) -> bool {
        self.role_state(DriveRole::Primary) != DriveState::Active
            || self.role_state(DriveRole::Secondary) != DriveState::Active
    }

    pub fn standby_accepts_sync(&self) -> bool {
        matches!(
            self.role_state(self.standby_role()),
            DriveState::Active | DriveState::Rebuilding
        ) && path_is_available(self.standby_path())
    }
}

pub fn path_is_available(path: &str) -> bool {
    let meta = match std::fs::metadata(path) {
        Ok(meta) => meta,
        Err(_) => return false,
    };
    if !meta.is_dir() {
        return false;
    }

    // A dead or forcibly removed mount can still leave the mountpoint path behind.
    // Touch the directory itself so future operations can fail over before opening files.
    std::fs::read_dir(Path::new(path)).is_ok()
}

pub fn drive_root_marker_path(path: &str) -> std::path::PathBuf {
    Path::new(path).join(DRIVE_ROOT_MARKER)
}

pub fn ensure_drive_root_marker(path: &str) -> anyhow::Result<()> {
    if !path_is_available(path) {
        anyhow::bail!("Drive root unavailable: {}", path);
    }

    let marker = drive_root_marker_path(path);
    if !marker.exists() {
        std::fs::write(&marker, b"bitprotector managed root\n")?;
    }
    Ok(())
}

pub fn missing_expected_root_marker(pair: &DrivePair, role: DriveRole) -> bool {
    let path = pair.path_for_role(role);
    if !path_is_available(path) {
        return false;
    }

    let marker = drive_root_marker_path(path);
    if marker.exists() {
        return false;
    }

    let opposite_marker = drive_root_marker_path(pair.path_for_role(role.opposite()));
    opposite_marker.exists()
}

pub fn rebuild_action_for_role(role: DriveRole) -> &'static str {
    match role {
        DriveRole::Primary => "restore_master",
        DriveRole::Secondary => "restore_mirror",
    }
}

pub fn require_pair_mutation_allowed(pair: &DrivePair) -> anyhow::Result<()> {
    if pair.is_quiescing() {
        anyhow::bail!(
            "Drive pair #{} is quiescing; confirm or cancel replacement first",
            pair.id
        );
    }
    Ok(())
}

pub fn refresh_pair_virtual_paths(repo: &Repository, pair: &DrivePair) -> anyhow::Result<()> {
    let mut pairs = HashMap::new();
    pairs.insert(pair.id, pair.clone());
    let _ = crate::core::virtual_path::refresh_all_virtual_paths(repo, &pairs)?;
    Ok(())
}

pub fn load_operational_pair(repo: &Repository, pair_id: i64) -> anyhow::Result<DrivePair> {
    let pair = repo.get_drive_pair(pair_id)?;
    maybe_emergency_failover(repo, &pair)
}

pub fn maybe_emergency_failover(repo: &Repository, pair: &DrivePair) -> anyhow::Result<DrivePair> {
    let active_role = pair.active_role_enum();
    let standby_role = active_role.opposite();
    if path_is_available(pair.active_path()) && !missing_expected_root_marker(pair, active_role) {
        return Ok(pair.clone());
    }

    if pair.role_state(active_role) != DriveState::Active {
        return Ok(pair.clone());
    }

    if pair.role_state(standby_role) != DriveState::Active
        || !path_is_available(pair.path_for_role(standby_role))
    {
        return Ok(pair.clone());
    }

    let updated = repo.update_drive_pair_runtime(
        pair.id,
        None,
        None,
        if active_role == DriveRole::Primary {
            Some(DriveState::Failed.as_str())
        } else {
            None
        },
        if active_role == DriveRole::Secondary {
            Some(DriveState::Failed.as_str())
        } else {
            None
        },
        Some(standby_role.as_str()),
    )?;

    repo.mark_drive_pair_unmirrored(pair.id)?;
    let _ = repo.fail_pending_sync_queue_for_drive_pair(
        pair.id,
        "drive failover required after fatal root error",
    );
    let _ = refresh_pair_virtual_paths(repo, &updated);
    let _ = event_logger::log_event(
        repo,
        "sync_failed",
        None,
        &format!(
            "Emergency failover for drive pair #{}: {} root unavailable, switched active role to {}",
            pair.id,
            active_role.as_str(),
            standby_role.as_str()
        ),
        Some(pair.path_for_role(active_role)),
    );

    repo.get_drive_pair(pair.id)
}

pub fn mark_drive_quiescing(
    repo: &Repository,
    pair_id: i64,
    role: DriveRole,
) -> anyhow::Result<DrivePair> {
    let pair = repo.get_drive_pair(pair_id)?;
    if pair.role_state(role) != DriveState::Active {
        anyhow::bail!(
            "Cannot quiesce {} drive for pair #{} while it is {}",
            role.as_str(),
            pair_id,
            pair.role_state(role).as_str()
        );
    }
    let updated = repo.update_drive_pair_runtime(
        pair_id,
        None,
        None,
        if role == DriveRole::Primary {
            Some(DriveState::Quiescing.as_str())
        } else {
            None
        },
        if role == DriveRole::Secondary {
            Some(DriveState::Quiescing.as_str())
        } else {
            None
        },
        None,
    )?;
    let _ = event_logger::log_event(
        repo,
        "sync_completed",
        None,
        &format!(
            "Drive pair #{} entered quiescing state for {} replacement",
            pair_id,
            role.as_str()
        ),
        None,
    );
    Ok(updated)
}

pub fn cancel_drive_quiescing(
    repo: &Repository,
    pair_id: i64,
    role: DriveRole,
) -> anyhow::Result<DrivePair> {
    let pair = repo.get_drive_pair(pair_id)?;
    if pair.role_state(role) != DriveState::Quiescing {
        anyhow::bail!(
            "Cannot cancel {} quiesce for pair #{} while it is {}",
            role.as_str(),
            pair_id,
            pair.role_state(role).as_str()
        );
    }
    let updated = repo.update_drive_pair_runtime(
        pair_id,
        None,
        None,
        if role == DriveRole::Primary {
            Some(DriveState::Active.as_str())
        } else {
            None
        },
        if role == DriveRole::Secondary {
            Some(DriveState::Active.as_str())
        } else {
            None
        },
        None,
    )?;
    let _ = event_logger::log_event(
        repo,
        "sync_completed",
        None,
        &format!(
            "Drive pair #{} cancelled {} replacement quiesce",
            pair_id,
            role.as_str()
        ),
        None,
    );
    Ok(updated)
}

pub fn confirm_drive_failure(
    repo: &Repository,
    pair_id: i64,
    role: DriveRole,
) -> anyhow::Result<DrivePair> {
    let pair = repo.get_drive_pair(pair_id)?;
    if pair.role_state(role) != DriveState::Quiescing {
        anyhow::bail!(
            "Cannot confirm {} failure for pair #{} while it is {}",
            role.as_str(),
            pair_id,
            pair.role_state(role).as_str()
        );
    }

    let next_active = if pair.active_role_enum() == role {
        role.opposite()
    } else {
        pair.active_role_enum()
    };

    if role == pair.active_role_enum() && !path_is_available(pair.path_for_role(next_active)) {
        anyhow::bail!(
            "Cannot fail over drive pair #{} because the {} path is unavailable",
            pair_id,
            next_active.as_str()
        );
    }

    let updated = repo.update_drive_pair_runtime(
        pair_id,
        None,
        None,
        if role == DriveRole::Primary {
            Some(DriveState::Failed.as_str())
        } else {
            None
        },
        if role == DriveRole::Secondary {
            Some(DriveState::Failed.as_str())
        } else {
            None
        },
        Some(next_active.as_str()),
    )?;
    repo.mark_drive_pair_unmirrored(pair_id)?;
    let _ = repo.fail_pending_sync_queue_for_drive_pair(
        pair_id,
        "drive replacement confirmed; rebuild required",
    );
    let _ = refresh_pair_virtual_paths(repo, &updated);
    let _ = event_logger::log_event(
        repo,
        "sync_failed",
        None,
        &format!(
            "Drive pair #{} confirmed {} failure; active role is now {}",
            pair_id,
            role.as_str(),
            updated.active_role
        ),
        Some(updated.path_for_role(role)),
    );
    Ok(updated)
}

pub fn assign_replacement_drive(
    repo: &Repository,
    pair_id: i64,
    role: DriveRole,
    new_path: &str,
) -> anyhow::Result<(DrivePair, usize)> {
    let pair = repo.get_drive_pair(pair_id)?;
    if pair.role_state(role) != DriveState::Failed {
        anyhow::bail!(
            "Cannot assign replacement {} drive for pair #{} while it is {}",
            role.as_str(),
            pair_id,
            pair.role_state(role).as_str()
        );
    }

    ensure_drive_root_marker(new_path)?;

    let updated = repo.update_drive_pair_runtime(
        pair_id,
        if role == DriveRole::Primary {
            Some(new_path)
        } else {
            None
        },
        if role == DriveRole::Secondary {
            Some(new_path)
        } else {
            None
        },
        if role == DriveRole::Primary {
            Some(DriveState::Rebuilding.as_str())
        } else {
            None
        },
        if role == DriveRole::Secondary {
            Some(DriveState::Rebuilding.as_str())
        } else {
            None
        },
        None,
    )?;

    let (files, _) = repo.list_tracked_files(Some(pair_id), None, None, 1, i64::MAX)?;
    let action = rebuild_action_for_role(role);
    let mut queued = 0usize;
    for file in files {
        let (_item, created) = repo.create_sync_queue_item_dedup_with_created(file.id, action)?;
        if created {
            queued += 1;
        }
    }

    let _ = event_logger::log_event(
        repo,
        "sync_completed",
        None,
        &format!(
            "Drive pair #{} assigned replacement {} path and queued {} rebuild item(s)",
            pair_id,
            role.as_str(),
            queued
        ),
        Some(new_path),
    );
    Ok((updated, queued))
}

pub fn maybe_finalize_rebuild_for_action(
    repo: &Repository,
    pair_id: i64,
    action: &str,
) -> anyhow::Result<Option<DrivePair>> {
    let pair = repo.get_drive_pair(pair_id)?;
    let role = match action {
        "restore_master" => DriveRole::Primary,
        "restore_mirror" => DriveRole::Secondary,
        _ => return Ok(None),
    };
    if pair.role_state(role) != DriveState::Rebuilding {
        return Ok(None);
    }

    let remaining = repo.count_sync_queue_items_for_drive_pair_action(
        pair_id,
        action,
        &["pending", "in_progress", "failed"],
    )?;
    if remaining > 0 {
        return Ok(None);
    }

    let updated = repo.update_drive_pair_runtime(
        pair_id,
        None,
        None,
        if role == DriveRole::Primary {
            Some(DriveState::Active.as_str())
        } else {
            None
        },
        if role == DriveRole::Secondary {
            Some(DriveState::Active.as_str())
        } else {
            None
        },
        if role == DriveRole::Primary {
            Some(DriveRole::Primary.as_str())
        } else {
            None
        },
    )?;
    repo.mark_drive_pair_unmirrored(pair_id)?;
    if updated.role_state(DriveRole::Primary) == DriveState::Active
        && updated.role_state(DriveRole::Secondary) == DriveState::Active
    {
        let (files, _) = repo.list_tracked_files(Some(pair_id), None, None, 1, i64::MAX)?;
        for file in files {
            repo.update_tracked_file_mirror_status(file.id, true)?;
        }
    }
    let _ = refresh_pair_virtual_paths(repo, &updated);
    let _ = event_logger::log_event(
        repo,
        "sync_completed",
        None,
        &format!(
            "Drive pair #{} finished rebuilding {}; active role is {}",
            pair_id,
            role.as_str(),
            updated.active_role
        ),
        Some(updated.path_for_role(role)),
    );
    Ok(Some(updated))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{mirror, tracker, virtual_path};
    use crate::db::repository::create_memory_pool;
    use crate::db::schema::initialize_schema;
    use tempfile::TempDir;

    fn make_repo() -> Repository {
        let pool = create_memory_pool().unwrap();
        initialize_schema(&pool.get().unwrap()).unwrap();
        Repository::new(pool)
    }

    #[test]
    fn test_drive_pair_role_helpers() {
        let repo = make_repo();
        let primary = TempDir::new().unwrap();
        let secondary = TempDir::new().unwrap();
        let pair = repo
            .create_drive_pair(
                "pair",
                primary.path().to_str().unwrap(),
                secondary.path().to_str().unwrap(),
            )
            .unwrap();

        assert_eq!(pair.active_role_enum(), DriveRole::Primary);
        assert_eq!(pair.active_path(), pair.primary_path);
        assert_eq!(pair.standby_path(), pair.secondary_path);
    }

    #[test]
    fn test_quiesce_confirm_and_assign_replacement() {
        let repo = make_repo();
        let primary = TempDir::new().unwrap();
        let secondary = TempDir::new().unwrap();
        let replacement = TempDir::new().unwrap();
        let pair = repo
            .create_drive_pair(
                "pair",
                primary.path().to_str().unwrap(),
                secondary.path().to_str().unwrap(),
            )
            .unwrap();

        let quiescing = mark_drive_quiescing(&repo, pair.id, DriveRole::Primary).unwrap();
        assert_eq!(quiescing.primary_state, "quiescing");

        let failed = confirm_drive_failure(&repo, pair.id, DriveRole::Primary).unwrap();
        assert_eq!(failed.primary_state, "failed");
        assert_eq!(failed.active_role, "secondary");

        let (rebuilding, queued) = assign_replacement_drive(
            &repo,
            pair.id,
            DriveRole::Primary,
            replacement.path().to_str().unwrap(),
        )
        .unwrap();
        assert_eq!(rebuilding.primary_state, "rebuilding");
        assert_eq!(queued, 0);
    }

    #[test]
    fn test_cancel_drive_quiescing_restores_active_state() {
        let repo = make_repo();
        let primary = TempDir::new().unwrap();
        let secondary = TempDir::new().unwrap();
        let pair = repo
            .create_drive_pair(
                "pair",
                primary.path().to_str().unwrap(),
                secondary.path().to_str().unwrap(),
            )
            .unwrap();

        mark_drive_quiescing(&repo, pair.id, DriveRole::Secondary).unwrap();
        let restored = cancel_drive_quiescing(&repo, pair.id, DriveRole::Secondary).unwrap();
        assert_eq!(restored.secondary_state, "active");
        assert_eq!(restored.active_role, "primary");
    }

    #[test]
    fn test_emergency_failover_switches_active_role_and_refreshes_symlink() {
        let repo = make_repo();
        let primary = TempDir::new().unwrap();
        let secondary = TempDir::new().unwrap();
        let virtual_root = TempDir::new().unwrap();
        let primary_path = primary.path().to_path_buf();
        let secondary_path = secondary.path().to_path_buf();

        std::fs::write(primary_path.join("doc.txt"), b"content").unwrap();
        std::fs::write(secondary_path.join("doc.txt"), b"content").unwrap();

        let pair = repo
            .create_drive_pair(
                "pair",
                primary_path.to_str().unwrap(),
                secondary_path.to_str().unwrap(),
            )
            .unwrap();
        let tracked = tracker::track_file(&repo, &pair, "doc.txt", None).unwrap();
        repo.update_tracked_file_mirror_status(tracked.id, true)
            .unwrap();
        virtual_path::set_virtual_path(
            &repo,
            tracked.id,
            virtual_root.path().join("docs/doc.txt").to_str().unwrap(),
        )
        .unwrap();

        ensure_drive_root_marker(primary_path.to_str().unwrap()).unwrap();
        ensure_drive_root_marker(secondary_path.to_str().unwrap()).unwrap();
        drop(primary);

        let failed_over = load_operational_pair(&repo, pair.id).unwrap();
        let link_target = std::fs::read_link(virtual_root.path().join("docs/doc.txt")).unwrap();

        assert_eq!(failed_over.active_role, "secondary");
        assert_eq!(failed_over.primary_state, "failed");
        assert_eq!(link_target, secondary_path.join("doc.txt"));
    }

    #[test]
    fn test_missing_root_marker_triggers_failover() {
        let repo = make_repo();
        let primary = TempDir::new().unwrap();
        let secondary = TempDir::new().unwrap();
        let pair = repo
            .create_drive_pair(
                "pair",
                primary.path().to_str().unwrap(),
                secondary.path().to_str().unwrap(),
            )
            .unwrap();

        ensure_drive_root_marker(primary.path().to_str().unwrap()).unwrap();
        ensure_drive_root_marker(secondary.path().to_str().unwrap()).unwrap();
        std::fs::remove_file(drive_root_marker_path(primary.path().to_str().unwrap())).unwrap();

        let failed_over = maybe_emergency_failover(&repo, &pair).unwrap();
        assert_eq!(failed_over.active_role, "secondary");
        assert_eq!(failed_over.primary_state, "failed");
    }

    #[test]
    fn test_finalize_primary_rebuild_restores_primary_active_role() {
        let repo = make_repo();
        let primary = TempDir::new().unwrap();
        let secondary = TempDir::new().unwrap();
        let replacement = TempDir::new().unwrap();
        let pair = repo
            .create_drive_pair(
                "pair",
                primary.path().to_str().unwrap(),
                secondary.path().to_str().unwrap(),
            )
            .unwrap();

        std::fs::write(primary.path().join("doc.txt"), b"content").unwrap();
        mirror::mirror_file(&pair, "doc.txt").unwrap();
        let tracked = tracker::track_file(&repo, &pair, "doc.txt", None).unwrap();
        repo.update_tracked_file_mirror_status(tracked.id, true)
            .unwrap();

        mark_drive_quiescing(&repo, pair.id, DriveRole::Primary).unwrap();
        confirm_drive_failure(&repo, pair.id, DriveRole::Primary).unwrap();
        let (_updated, _queued) = assign_replacement_drive(
            &repo,
            pair.id,
            DriveRole::Primary,
            replacement.path().to_str().unwrap(),
        )
        .unwrap();

        let item = repo
            .list_sync_queue(Some("pending"), 1, 10)
            .unwrap()
            .0
            .into_iter()
            .find(|item| item.action == "restore_master")
            .unwrap();
        repo.update_sync_queue_status(item.id, "completed", None)
            .unwrap();

        let finalized = maybe_finalize_rebuild_for_action(&repo, pair.id, "restore_master")
            .unwrap()
            .unwrap();
        assert_eq!(finalized.active_role, "primary");
        assert_eq!(finalized.primary_state, "active");
        assert_eq!(finalized.secondary_state, "active");
    }
}
