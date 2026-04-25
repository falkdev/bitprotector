mod common;

use actix_web::test;
use bitprotector_lib::core::{checksum, drive, integrity, virtual_path};
use common::{bearer, make_repo};
use std::fs;
use tempfile::TempDir;

// ── Logs ───────────────────────────────────────────────────────────────────

#[actix_rt::test]
async fn test_logs_list_empty() {
    let app = make_app!(make_repo()).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/logs")
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body["logs"].as_array().unwrap().is_empty());
}

#[actix_rt::test]
async fn test_logs_get_not_found() {
    let app = make_app!(make_repo()).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/logs/999")
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 404);
}

#[actix_rt::test]
async fn test_logs_filter_by_event_type() {
    let repo = make_repo();
    let pair = repo.create_drive_pair("lp", "/p", "/s").unwrap();
    let file = repo
        .create_tracked_file(pair.id, "l.txt", "abc", 3, None)
        .unwrap();
    repo.create_event_log("sync_completed", Some(file.id), "sync done", None)
        .unwrap();
    repo.create_event_log("file_mirrored", Some(file.id), "mirrored", None)
        .unwrap();
    let app = make_app!(repo).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/logs?event_type=sync_completed")
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    let entries = body["logs"].as_array().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["event_type"], "sync_completed");
}

#[actix_rt::test]
async fn test_logs_get_existing() {
    let repo = make_repo();
    let entry = repo
        .create_event_log("file_created", None, "test message", None)
        .unwrap();
    let app = make_app!(repo).await;
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/logs/{}", entry.id))
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["event_type"], "file_created");
}

#[actix_rt::test]
async fn test_logs_date_range_filter() {
    let repo = make_repo();
    repo.create_event_log("sync_completed", None, "entry", None)
        .unwrap();
    let app = make_app!(repo).await;
    // Dates in the future should return empty list
    let req = test::TestRequest::get()
        .uri("/api/v1/logs?from=2099-01-01T00:00:00Z")
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body["logs"].as_array().unwrap().is_empty());
}
