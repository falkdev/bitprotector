mod common;

use actix_web::test;
use bitprotector_lib::core::{checksum, drive, integrity, virtual_path};
use common::{bearer, make_repo};
use std::fs;
use tempfile::TempDir;

// ── Drives ─────────────────────────────────────────────────────────────────

#[actix_rt::test]
async fn test_drives_list_empty() {
    let app = make_app!(make_repo()).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/drives")
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body.as_array().unwrap().is_empty());
}

#[actix_rt::test]
async fn test_drives_create_skip_validation() {
    let app = make_app!(make_repo()).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/drives")
        .insert_header(("Authorization", bearer()))
        .set_json(serde_json::json!({
            "name": "test-pair",
            "primary_path": "/tmp/p",
            "secondary_path": "/tmp/s",
            "skip_validation": true
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["name"], "test-pair");
}

#[actix_rt::test]
async fn test_drives_create_validation_failure_returns_400() {
    let app = make_app!(make_repo()).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/drives")
        .insert_header(("Authorization", bearer()))
        .set_json(serde_json::json!({
            "name": "bad",
            "primary_path": "/nonexistent/a",
            "secondary_path": "/nonexistent/b"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);
}

#[actix_rt::test]
async fn test_drives_get_existing() {
    let repo = make_repo();
    let pair = repo.create_drive_pair("get-pair", "/np", "/ns").unwrap();
    let app = make_app!(repo).await;
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/drives/{}", pair.id))
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["name"], "get-pair");
}

#[actix_rt::test]
async fn test_drives_get_not_found() {
    let app = make_app!(make_repo()).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/drives/999")
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 404);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body["error"]["code"].is_string());
}

#[actix_rt::test]
async fn test_drives_update() {
    let repo = make_repo();
    let pair = repo.create_drive_pair("original", "/p", "/s").unwrap();
    let app = make_app!(repo).await;
    let req = test::TestRequest::put()
        .uri(&format!("/api/v1/drives/{}", pair.id))
        .insert_header(("Authorization", bearer()))
        .set_json(serde_json::json!({ "name": "updated" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["name"], "updated");
}

#[actix_rt::test]
async fn test_drives_delete() {
    let repo = make_repo();
    let pair = repo.create_drive_pair("to-delete", "/p", "/s").unwrap();
    let app = make_app!(repo).await;
    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/drives/{}", pair.id))
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 204);
}

#[actix_rt::test]
async fn test_drives_mark_and_cancel_replacement() {
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let repo = make_repo();
    let pair = repo
        .create_drive_pair(
            "rep-pair",
            primary.path().to_str().unwrap(),
            secondary.path().to_str().unwrap(),
        )
        .unwrap();
    let app = make_app!(repo).await;

    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/drives/{}/replacement/mark", pair.id))
        .insert_header(("Authorization", bearer()))
        .set_json(serde_json::json!({ "role": "primary" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["primary_state"], "quiescing");

    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/drives/{}/replacement/cancel", pair.id))
        .insert_header(("Authorization", bearer()))
        .set_json(serde_json::json!({ "role": "primary" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["primary_state"], "active");
}

#[actix_rt::test]
async fn test_drives_invalid_role_returns_400() {
    let repo = make_repo();
    let pair = repo.create_drive_pair("pair", "/p", "/s").unwrap();
    let app = make_app!(repo).await;
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/drives/{}/replacement/mark", pair.id))
        .insert_header(("Authorization", bearer()))
        .set_json(serde_json::json!({ "role": "invalid_role" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);
}
