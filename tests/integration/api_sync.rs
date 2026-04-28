mod common;

use actix_web::test;
use bitprotector_lib::core::{checksum, drive, integrity, virtual_path};
use common::{bearer, make_repo};
use std::fs;
use tempfile::TempDir;

// ── Sync ───────────────────────────────────────────────────────────────────

#[actix_rt::test]
async fn test_sync_queue_list_empty() {
    let app = make_app!(make_repo()).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/sync/queue")
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body["queue"].as_array().unwrap().is_empty());
}

#[actix_rt::test]
async fn test_sync_queue_add_and_get() {
    let repo = make_repo();
    let pair = repo.create_drive_pair("sp", "/p", "/s").unwrap();
    let file = repo
        .create_tracked_file(pair.id, "sync.txt", "abc", 3, None)
        .unwrap();
    let app = make_app!(repo).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/sync/queue")
        .insert_header(("Authorization", bearer()))
        .set_json(serde_json::json!({
            "tracked_file_id": file.id,
            "action": "verify"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: serde_json::Value = test::read_body_json(resp).await;
    let item_id = body["id"].as_i64().unwrap();
    assert_eq!(body["action"], "verify");

    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/sync/queue/{}", item_id))
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

#[actix_rt::test]
async fn test_sync_queue_get_not_found() {
    let app = make_app!(make_repo()).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/sync/queue/999")
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 404);
}

#[actix_rt::test]
async fn test_sync_resolve_keep_master_via_api() {
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let content = b"master data";
    fs::write(primary.path().join("res.txt"), content).unwrap();
    let repo = make_repo();
    let pair = repo
        .create_drive_pair(
            "rp",
            primary.path().to_str().unwrap(),
            secondary.path().to_str().unwrap(),
        )
        .unwrap();
    let hash = checksum::checksum_bytes(content);
    let file = repo
        .create_tracked_file(pair.id, "res.txt", &hash, content.len() as i64, None)
        .unwrap();
    let item = repo
        .create_sync_queue_item(file.id, "user_action_required")
        .unwrap();
    let app = make_app!(repo).await;
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/sync/queue/{}/resolve", item.id))
        .insert_header(("Authorization", bearer()))
        .set_json(serde_json::json!({ "resolution": "keep_master" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["status"], "completed");
    assert!(secondary.path().join("res.txt").exists());
    assert_eq!(fs::read(secondary.path().join("res.txt")).unwrap(), content);
}

#[actix_rt::test]
async fn test_sync_resolve_keep_mirror_via_api() {
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let content = b"mirror wins";
    fs::write(primary.path().join("mir.txt"), content).unwrap();
    fs::write(secondary.path().join("mir.txt"), content).unwrap();
    let repo = make_repo();
    let pair = repo
        .create_drive_pair(
            "rp2",
            primary.path().to_str().unwrap(),
            secondary.path().to_str().unwrap(),
        )
        .unwrap();
    let hash = checksum::checksum_bytes(content);
    let file = repo
        .create_tracked_file(pair.id, "mir.txt", &hash, content.len() as i64, None)
        .unwrap();
    let item = repo
        .create_sync_queue_item(file.id, "user_action_required")
        .unwrap();
    let app = make_app!(repo).await;
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/sync/queue/{}/resolve", item.id))
        .insert_header(("Authorization", bearer()))
        .set_json(serde_json::json!({ "resolution": "keep_mirror" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["status"], "completed");
}

#[actix_rt::test]
async fn test_sync_resolve_provide_new_via_api() {
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let new_file_dir = TempDir::new().unwrap();
    let new_content = b"replacement file";
    let new_file = new_file_dir.path().join("new.txt");
    fs::write(&new_file, new_content).unwrap();
    let repo = make_repo();
    let pair = repo
        .create_drive_pair(
            "rp3",
            primary.path().to_str().unwrap(),
            secondary.path().to_str().unwrap(),
        )
        .unwrap();
    fs::write(primary.path().join("pnew.txt"), b"old data").unwrap();
    let hash = checksum::checksum_bytes(b"old data");
    let file = repo
        .create_tracked_file(pair.id, "pnew.txt", &hash, 8, None)
        .unwrap();
    let item = repo
        .create_sync_queue_item(file.id, "user_action_required")
        .unwrap();
    let app = make_app!(repo).await;
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/sync/queue/{}/resolve", item.id))
        .insert_header(("Authorization", bearer()))
        .set_json(serde_json::json!({
            "resolution": "provide_new",
            "new_file_path": new_file.to_str().unwrap()
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    assert_eq!(
        fs::read(primary.path().join("pnew.txt")).unwrap(),
        new_content
    );
    assert_eq!(
        fs::read(secondary.path().join("pnew.txt")).unwrap(),
        new_content
    );
}

#[actix_rt::test]
async fn test_sync_resolve_invalid_resolution_returns_400() {
    let repo = make_repo();
    let pair = repo.create_drive_pair("rp4", "/p", "/s").unwrap();
    let file = repo
        .create_tracked_file(pair.id, "x.txt", "abc", 3, None)
        .unwrap();
    let item = repo
        .create_sync_queue_item(file.id, "user_action_required")
        .unwrap();
    let app = make_app!(repo).await;
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/sync/queue/{}/resolve", item.id))
        .insert_header(("Authorization", bearer()))
        .set_json(serde_json::json!({ "resolution": "destroy_everything" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);
}

#[actix_rt::test]
async fn test_sync_resolve_wrong_action_type_returns_400() {
    let repo = make_repo();
    let pair = repo.create_drive_pair("rp5", "/p", "/s").unwrap();
    let file = repo
        .create_tracked_file(pair.id, "y.txt", "abc", 3, None)
        .unwrap();
    // This item has action "mirror", not "user_action_required"
    let item = repo.create_sync_queue_item(file.id, "mirror").unwrap();
    let app = make_app!(repo).await;
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/sync/queue/{}/resolve", item.id))
        .insert_header(("Authorization", bearer()))
        .set_json(serde_json::json!({ "resolution": "keep_master" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);
}

#[actix_rt::test]
async fn test_sync_process_empty_queue() {
    let app = make_app!(make_repo()).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/sync/process")
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["processed"], 0);
}

#[actix_rt::test]
async fn test_sync_clear_completed_queue_deletes_only_completed() {
    let repo = make_repo();
    let pair = repo.create_drive_pair("sc1", "/p", "/s").unwrap();
    let file1 = repo
        .create_tracked_file(pair.id, "a.txt", "h1", 1, None)
        .unwrap();
    let file2 = repo
        .create_tracked_file(pair.id, "b.txt", "h2", 1, None)
        .unwrap();
    let file3 = repo
        .create_tracked_file(pair.id, "c.txt", "h3", 1, None)
        .unwrap();

    let completed = repo.create_sync_queue_item(file1.id, "mirror").unwrap();
    repo.update_sync_queue_status(completed.id, "completed", None)
        .unwrap();
    let _pending = repo.create_sync_queue_item(file2.id, "verify").unwrap();
    let failed = repo
        .create_sync_queue_item(file3.id, "restore_master")
        .unwrap();
    repo.update_sync_queue_status(failed.id, "failed", Some("forced failure"))
        .unwrap();

    let app = make_app!(repo).await;
    let req = test::TestRequest::delete()
        .uri("/api/v1/sync/queue/completed")
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["deleted"], 1);

    let req = test::TestRequest::get()
        .uri("/api/v1/sync/queue")
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    let queue = body["queue"].as_array().unwrap();
    assert_eq!(queue.len(), 2);
    assert!(queue.iter().all(|item| item["status"] != "completed"));
}

#[actix_rt::test]
async fn test_sync_clear_completed_queue_when_none_exists_returns_zero() {
    let repo = make_repo();
    let pair = repo.create_drive_pair("sc2", "/p", "/s").unwrap();
    let file = repo
        .create_tracked_file(pair.id, "a.txt", "h1", 1, None)
        .unwrap();
    let _pending = repo.create_sync_queue_item(file.id, "mirror").unwrap();

    let app = make_app!(repo).await;
    let req = test::TestRequest::delete()
        .uri("/api/v1/sync/queue/completed")
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["deleted"], 0);
}

#[actix_rt::test]
async fn test_sync_run_sync_task() {
    let app = make_app!(make_repo()).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/sync/run/sync")
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["task"], "sync");
}

#[actix_rt::test]
async fn test_sync_run_integrity_check_task() {
    let app = make_app!(make_repo()).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/sync/run/integrity-check")
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["task"], "integrity_check");
}

#[actix_rt::test]
async fn test_sync_run_unknown_task_returns_400() {
    let app = make_app!(make_repo()).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/sync/run/blorp")
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);
}

// ── Pause / Resume ────────────────────────────────────────────────────────

#[actix_rt::test]
async fn test_sync_queue_list_includes_queue_paused_field() {
    let app = make_app!(make_repo()).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/sync/queue")
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(
        body["queue_paused"], false,
        "queue_paused should default to false"
    );
}

#[actix_rt::test]
async fn test_sync_pause_and_resume() {
    let app = make_app!(make_repo()).await;

    // Pause
    let req = test::TestRequest::post()
        .uri("/api/v1/sync/pause")
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["queue_paused"], true);

    // Verify list reflects paused state
    let req = test::TestRequest::get()
        .uri("/api/v1/sync/queue")
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["queue_paused"], true);

    // Resume
    let req = test::TestRequest::post()
        .uri("/api/v1/sync/resume")
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["queue_paused"], false);

    // Verify list reflects resumed state
    let req = test::TestRequest::get()
        .uri("/api/v1/sync/queue")
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["queue_paused"], false);
}

#[actix_rt::test]
async fn test_sync_process_is_noop_when_paused() {
    let repo = make_repo();
    let pair = repo.create_drive_pair("paused_pair", "/p", "/s").unwrap();
    let file = repo
        .create_tracked_file(pair.id, "paused.txt", "h1", 1, None)
        .unwrap();
    repo.create_sync_queue_item(file.id, "mirror").unwrap();
    repo.set_sync_queue_paused(true).unwrap();
    let app = make_app!(repo).await;

    let req = test::TestRequest::post()
        .uri("/api/v1/sync/process")
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(
        body["processed"], 0,
        "No items should be processed while queue is paused"
    );
}
