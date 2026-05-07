mod common;

use actix_web::test;
use bitprotector_lib::core::{checksum, drive, integrity};
use common::{bearer, make_repo};
use std::fs;
use tempfile::TempDir;

// ── Integrity ──────────────────────────────────────────────────────────────

#[actix_rt::test]
async fn test_integrity_check_file_ok() {
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let content = b"integrity check";
    fs::write(primary.path().join("ic.txt"), content).unwrap();
    fs::write(secondary.path().join("ic.txt"), content).unwrap();
    let repo = make_repo();
    let pair = repo
        .create_drive_pair(
            "ip",
            primary.path().to_str().unwrap(),
            secondary.path().to_str().unwrap(),
        )
        .unwrap();
    let hash = checksum::checksum_bytes(content);
    let file = repo
        .create_tracked_file(pair.id, "ic.txt", &hash, content.len() as i64, None)
        .unwrap();
    let app = make_app!(repo).await;
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/integrity/check/{}", file.id))
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["status"], "ok");
    assert_eq!(body["master_valid"], true);
    assert_eq!(body["mirror_valid"], true);
}

#[actix_rt::test]
async fn test_integrity_check_file_not_found() {
    let app = make_app!(make_repo()).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/integrity/check/999")
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 404);
}

#[actix_rt::test]
async fn test_integrity_check_with_recovery_reconciles_mirror_queue_and_logs_fix() {
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let content = b"integrity auto-recover";
    fs::write(primary.path().join("recover.txt"), content).unwrap();
    fs::write(secondary.path().join("recover.txt"), b"corrupt mirror").unwrap();

    let repo = make_repo();
    let pair = repo
        .create_drive_pair(
            "ip-recover",
            primary.path().to_str().unwrap(),
            secondary.path().to_str().unwrap(),
        )
        .unwrap();
    drive::ensure_drive_root_marker(primary.path().to_str().unwrap()).unwrap();
    drive::ensure_drive_root_marker(secondary.path().to_str().unwrap()).unwrap();
    let hash = checksum::checksum_bytes(content);
    let file = repo
        .create_tracked_file(pair.id, "recover.txt", &hash, content.len() as i64, None)
        .unwrap();
    let _pending = repo.create_sync_queue_item(file.id, "mirror").unwrap();

    let app = make_app!(repo.clone()).await;
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/integrity/check/{}?recover=true", file.id))
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["status"], "mirror_corrupted");
    assert_eq!(body["recovered"], true);

    let recovered_file = repo.get_tracked_file(file.id).unwrap();
    assert!(recovered_file.is_mirrored);
    assert_eq!(
        fs::read(secondary.path().join("recover.txt")).unwrap(),
        content
    );

    let (pending, pending_total) = repo.list_sync_queue(Some("pending"), 1, 50).unwrap();
    assert_eq!(pending_total, 0);
    assert!(pending.is_empty());
    let (all, total) = repo.list_sync_queue(None, 1, 50).unwrap();
    assert_eq!(total, 1);
    assert_eq!(all[0].status, "completed");
    assert_eq!(all[0].action, "mirror");

    let req = test::TestRequest::post()
        .uri("/api/v1/sync/process")
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let processed: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(processed["processed"], 0);

    let (logs, _) = repo
        .list_event_logs(None, Some(file.id), None, None, 1, 50)
        .unwrap();
    assert!(logs
        .iter()
        .any(|entry| entry.event_type == "recovery_success"));
    assert!(logs
        .iter()
        .any(|entry| entry.event_type == "sync_completed"));
}

#[actix_rt::test]
async fn test_integrity_check_with_recovery_unrecoverable_keeps_queue_pending_and_logs_failure() {
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let original = b"expected bytes";
    let hash = checksum::checksum_bytes(original);
    fs::write(primary.path().join("unrecoverable.txt"), b"bad master").unwrap();
    fs::write(secondary.path().join("unrecoverable.txt"), b"bad mirror").unwrap();

    let repo = make_repo();
    let pair = repo
        .create_drive_pair(
            "ip-unrecoverable",
            primary.path().to_str().unwrap(),
            secondary.path().to_str().unwrap(),
        )
        .unwrap();
    drive::ensure_drive_root_marker(primary.path().to_str().unwrap()).unwrap();
    drive::ensure_drive_root_marker(secondary.path().to_str().unwrap()).unwrap();
    let file = repo
        .create_tracked_file(
            pair.id,
            "unrecoverable.txt",
            &hash,
            original.len() as i64,
            None,
        )
        .unwrap();
    let _pending = repo.create_sync_queue_item(file.id, "mirror").unwrap();

    let app = make_app!(repo.clone()).await;
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/integrity/check/{}?recover=true", file.id))
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["status"], "both_corrupted");
    assert_eq!(body["recovered"], false);

    let unrecovered = repo.get_tracked_file(file.id).unwrap();
    assert!(!unrecovered.is_mirrored);
    let (_, pending_total) = repo.list_sync_queue(Some("pending"), 1, 50).unwrap();
    assert_eq!(pending_total, 1);

    let (logs, _) = repo
        .list_event_logs(None, Some(file.id), None, None, 1, 50)
        .unwrap();
    assert!(logs.iter().any(|entry| entry.event_type == "recovery_fail"));
    assert!(!logs
        .iter()
        .any(|entry| entry.event_type == "recovery_success"));
    assert!(!logs
        .iter()
        .any(|entry| entry.event_type == "sync_completed"));
}

#[actix_rt::test]
async fn test_integrity_run_with_recovery_reconciles_queue_and_logs_per_file() {
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let repo = make_repo();
    let pair = repo
        .create_drive_pair(
            "ip-run-recover",
            primary.path().to_str().unwrap(),
            secondary.path().to_str().unwrap(),
        )
        .unwrap();
    drive::ensure_drive_root_marker(primary.path().to_str().unwrap()).unwrap();
    drive::ensure_drive_root_marker(secondary.path().to_str().unwrap()).unwrap();

    for idx in 0..2 {
        let relative = format!("recover/file-{idx}.txt");
        let content = format!("good-content-{idx}");
        let primary_path = primary.path().join(&relative);
        let secondary_path = secondary.path().join(&relative);
        if let Some(parent) = primary_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        if let Some(parent) = secondary_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&primary_path, content.as_bytes()).unwrap();
        fs::write(&secondary_path, b"corrupt").unwrap();

        let hash =
            checksum::checksum_file(&primary_path, checksum::ChecksumStrategy::Streaming).unwrap();
        let file = repo
            .create_tracked_file(pair.id, &relative, &hash, content.len() as i64, None)
            .unwrap();
        repo.create_sync_queue_item(file.id, "mirror").unwrap();
        let before_run = integrity::check_file_integrity(
            &pair,
            &file,
            checksum::ChecksumStrategy::Streaming,
            checksum::ChecksumStrategy::Streaming,
        )
        .unwrap();
        assert_eq!(
            before_run.status,
            integrity::IntegrityStatus::MirrorCorrupted
        );
    }

    let app = make_app!(repo.clone()).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/integrity/runs")
        .insert_header(("Authorization", bearer()))
        .set_json(serde_json::json!({ "recover": true }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 202);

    for _ in 0..60 {
        let req = test::TestRequest::get()
            .uri("/api/v1/integrity/runs/active")
            .insert_header(("Authorization", bearer()))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
        let body: serde_json::Value = test::read_body_json(resp).await;
        if body["run"].is_null() {
            break;
        }
        actix_rt::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    let req = test::TestRequest::get()
        .uri("/api/v1/integrity/runs/latest?issues_only=false&page=1&per_page=50")
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let latest: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(latest["run"]["status"], "completed");
    let recovered_file_ids: Vec<i64> = latest["results"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["recovered"].as_bool().unwrap_or(false))
        .filter_map(|row| row["file_id"].as_i64())
        .collect();
    assert!(
        !recovered_file_ids.is_empty(),
        "expected at least one file to be auto-recovered"
    );

    let (pending, _) = repo.list_sync_queue(Some("pending"), 1, 50).unwrap();
    assert!(
        pending
            .iter()
            .all(|item| !recovered_file_ids.contains(&item.tracked_file_id)),
        "Recovered files should not keep pending mirror queue rows"
    );
    let (queue, queue_total) = repo.list_sync_queue(None, 1, 50).unwrap();
    assert_eq!(queue_total, 2);

    for file_id in recovered_file_ids {
        let file = repo.get_tracked_file(file_id).unwrap();
        assert!(file.is_mirrored);
        let file_queue_rows: Vec<_> = queue
            .iter()
            .filter(|item| item.tracked_file_id == file_id)
            .collect();
        assert!(
            !file_queue_rows.is_empty(),
            "Recovered file should have queue history rows"
        );
        assert!(
            file_queue_rows
                .iter()
                .all(|item| item.status == "completed"),
            "Recovered file queue rows should be completed"
        );

        let (logs, _) = repo
            .list_event_logs(None, Some(file_id), None, None, 1, 50)
            .unwrap();
        assert!(logs
            .iter()
            .any(|entry| entry.event_type == "recovery_success"));
        assert!(logs
            .iter()
            .any(|entry| entry.event_type == "sync_completed"));
    }
}

#[actix_rt::test]
async fn test_integrity_run_start_and_latest_results() {
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let content = b"good content";
    fs::write(primary.path().join("r.txt"), content).unwrap();
    // Mirror is intentionally missing, so this run should report one issue row.
    let repo = make_repo();
    let pair = repo
        .create_drive_pair(
            "rp",
            primary.path().to_str().unwrap(),
            secondary.path().to_str().unwrap(),
        )
        .unwrap();
    let hash = checksum::checksum_bytes(content);
    repo.create_tracked_file(pair.id, "r.txt", &hash, content.len() as i64, None)
        .unwrap();

    let app = make_app!(repo).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/integrity/runs")
        .insert_header(("Authorization", bearer()))
        .set_json(serde_json::json!({ "recover": false }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 202);
    let body: serde_json::Value = test::read_body_json(resp).await;
    let run_id = body["id"].as_i64().unwrap();

    for _ in 0..60 {
        let req = test::TestRequest::get()
            .uri("/api/v1/integrity/runs/active")
            .insert_header(("Authorization", bearer()))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
        let body: serde_json::Value = test::read_body_json(resp).await;
        if body["run"].is_null() {
            break;
        }
        actix_rt::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    let req = test::TestRequest::get()
        .uri("/api/v1/integrity/runs/latest?issues_only=true&page=1&per_page=50")
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let latest: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(latest["run"]["id"], run_id);
    assert_eq!(latest["run"]["status"], "completed");
    assert_eq!(latest["run"]["active_workers"], 0);
    assert_eq!(latest["total"], 1);
    assert_eq!(latest["results"][0]["file_id"], 1);
    assert_eq!(latest["results"][0]["status"], "mirror_missing");
    assert_eq!(latest["results"][0]["needs_attention"], true);
}

#[actix_rt::test]
async fn test_integrity_run_stop_and_results_endpoint() {
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let payload = vec![b'x'; 256 * 1024];
    let repo = make_repo();
    let pair = repo
        .create_drive_pair(
            "run-stop",
            primary.path().to_str().unwrap(),
            secondary.path().to_str().unwrap(),
        )
        .unwrap();

    for idx in 0..20 {
        let relative = format!("docs/file-{idx:03}.txt");
        let full = primary.path().join(&relative);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&full, &payload).unwrap();
        let hash = checksum::checksum_bytes(&payload);
        repo.create_tracked_file(pair.id, &relative, &hash, payload.len() as i64, None)
            .unwrap();
    }

    let app = make_app!(repo).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/integrity/runs")
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 202);
    let started: serde_json::Value = test::read_body_json(resp).await;
    let run_id = started["id"].as_i64().unwrap();
    assert_eq!(started["active_workers"], 0);

    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/integrity/runs/{run_id}/stop"))
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let stopped: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(stopped["stop_requested"], true);

    for _ in 0..80 {
        let req = test::TestRequest::get()
            .uri(&format!(
                "/api/v1/integrity/runs/{run_id}/results?issues_only=true&page=1&per_page=10"
            ))
            .insert_header(("Authorization", bearer()))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
        let body: serde_json::Value = test::read_body_json(resp).await;
        let status = body["run"]["status"].as_str().unwrap_or_default();
        if !matches!(status, "running" | "stopping") {
            assert!(matches!(status, "stopped" | "completed"));
            break;
        }
        actix_rt::time::sleep(std::time::Duration::from_millis(50)).await;
    }
}

#[actix_rt::test]
async fn test_integrity_run_start_conflicts_when_active_run_exists() {
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let payload = vec![b'y'; 1024 * 1024];
    let repo = make_repo();
    let pair = repo
        .create_drive_pair(
            "run-conflict",
            primary.path().to_str().unwrap(),
            secondary.path().to_str().unwrap(),
        )
        .unwrap();

    for idx in 0..8 {
        let relative = format!("big/file-{idx:03}.bin");
        let full = primary.path().join(&relative);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&full, &payload).unwrap();
        let hash = checksum::checksum_bytes(&payload);
        repo.create_tracked_file(pair.id, &relative, &hash, payload.len() as i64, None)
            .unwrap();
    }

    let app = make_app!(repo).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/integrity/runs")
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 202);

    let req = test::TestRequest::post()
        .uri("/api/v1/integrity/runs")
        .insert_header(("Authorization", bearer()))
        .to_request();
    let conflict = test::call_service(&app, req).await;
    assert_eq!(conflict.status(), 409);
}
