use crate::db::repository::Repository;

/// Log an event to the database.
pub fn log_event(
    repo: &Repository,
    event_type: &str,
    tracked_file_id: Option<i64>,
    message: &str,
    details: Option<&str>,
) -> anyhow::Result<()> {
    repo.create_event_log(event_type, tracked_file_id, message, details)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repository::{create_memory_pool, Repository};
    use crate::db::schema::initialize_schema;

    fn setup_repo() -> Repository {
        let pool = create_memory_pool().unwrap();
        {
            let conn = pool.get().unwrap();
            initialize_schema(&conn).unwrap();
        }
        Repository::new(pool)
    }

    #[test]
    fn test_log_event_recorded() {
        let repo = setup_repo();
        log_event(&repo, "integrity_pass", None, "All good", None).unwrap();
        let (logs, total) = repo.list_event_logs(None, None, None, None, 1, 50).unwrap();
        assert_eq!(total, 1);
        assert_eq!(logs[0].event_type, "integrity_pass");
    }

    #[test]
    fn test_log_event_with_details() {
        let repo = setup_repo();
        log_event(&repo, "file_mirrored", None, "Mirrored file", Some("checksum=abc")).unwrap();
        let (logs, _) = repo.list_event_logs(None, None, None, None, 1, 50).unwrap();
        assert_eq!(logs[0].details, Some("checksum=abc".to_string()));
    }

    #[test]
    fn test_log_filter_by_type() {
        let repo = setup_repo();
        log_event(&repo, "integrity_pass", None, "pass", None).unwrap();
        log_event(&repo, "file_mirrored", None, "mirror", None).unwrap();

        let (logs, total) = repo.list_event_logs(Some("integrity_pass"), None, None, None, 1, 50).unwrap();
        assert_eq!(total, 1);
        assert_eq!(logs[0].event_type, "integrity_pass");
    }
}
