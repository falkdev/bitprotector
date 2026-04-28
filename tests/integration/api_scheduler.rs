mod common;

use actix_web::test;
use bitprotector_lib::core::{checksum, drive, integrity, virtual_path};
use common::{bearer, make_repo};
use std::fs;
use tempfile::TempDir;

// ── Scheduler ─────────────────────────────────────────────────────────────

#[actix_rt::test]
async fn test_scheduler_list_empty() {
    let app = make_app!(make_repo()).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/scheduler/schedules")
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["schedules"].as_array().unwrap().len(), 0);
}

#[actix_rt::test]
async fn test_scheduler_create_and_get() {
    let app = make_app!(make_repo()).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/scheduler/schedules")
        .insert_header(("Authorization", bearer()))
        .set_json(serde_json::json!({
            "task_type": "sync",
            "interval_seconds": 300,
            "enabled": false
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: serde_json::Value = test::read_body_json(resp).await;
    let id = body["id"].as_i64().unwrap();
    assert_eq!(body["task_type"], "sync");
    assert_eq!(body["interval_seconds"], 300);

    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/scheduler/schedules/{}", id))
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

#[actix_rt::test]
async fn test_scheduler_create_missing_timing_returns_400() {
    let app = make_app!(make_repo()).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/scheduler/schedules")
        .insert_header(("Authorization", bearer()))
        .set_json(serde_json::json!({ "task_type": "sync" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);
}

#[actix_rt::test]
async fn test_scheduler_create_invalid_task_type_returns_400() {
    let app = make_app!(make_repo()).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/scheduler/schedules")
        .insert_header(("Authorization", bearer()))
        .set_json(serde_json::json!({
            "task_type": "unknown_task",
            "interval_seconds": 60
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);
}

#[actix_rt::test]
async fn test_scheduler_update() {
    let repo = make_repo();
    let cfg = repo
        .create_schedule_config("sync", None, Some(300), true, None)
        .unwrap();
    let app = make_app!(repo).await;
    let req = test::TestRequest::put()
        .uri(&format!("/api/v1/scheduler/schedules/{}", cfg.id))
        .insert_header(("Authorization", bearer()))
        .set_json(serde_json::json!({ "enabled": false }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["enabled"], false);
}

#[actix_rt::test]
async fn test_scheduler_delete() {
    let repo = make_repo();
    let cfg = repo
        .create_schedule_config("integrity_check", None, Some(600), false, None)
        .unwrap();
    let app = make_app!(repo).await;
    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/scheduler/schedules/{}", cfg.id))
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 204);
}

#[actix_rt::test]
async fn test_scheduler_get_not_found() {
    let app = make_app!(make_repo()).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/scheduler/schedules/999")
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 404);
}

#[actix_rt::test]
async fn test_scheduler_create_with_max_duration() {
    let app = make_app!(make_repo()).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/scheduler/schedules")
        .insert_header(("Authorization", bearer()))
        .set_json(serde_json::json!({
            "task_type": "sync",
            "interval_seconds": 3600,
            "enabled": false,
            "max_duration_seconds": 1800
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(
        body["max_duration_seconds"], 1800,
        "max_duration_seconds should be returned in the response"
    );
}

#[actix_rt::test]
async fn test_scheduler_update_max_duration() {
    let repo = make_repo();
    let cfg = repo
        .create_schedule_config("integrity_check", None, Some(3600), false, None)
        .unwrap();
    let app = make_app!(repo).await;

    // Set a max duration
    let req = test::TestRequest::put()
        .uri(&format!("/api/v1/scheduler/schedules/{}", cfg.id))
        .insert_header(("Authorization", bearer()))
        .set_json(serde_json::json!({ "max_duration_seconds": 900 }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["max_duration_seconds"], 900);

    // Clear the max duration
    let req = test::TestRequest::put()
        .uri(&format!("/api/v1/scheduler/schedules/{}", cfg.id))
        .insert_header(("Authorization", bearer()))
        .set_json(serde_json::json!({ "max_duration_seconds": null }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(
        body["max_duration_seconds"].is_null(),
        "max_duration_seconds should be null after clearing"
    );
}
