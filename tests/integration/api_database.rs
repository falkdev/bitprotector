mod common;

use actix_web::test;
use bitprotector_lib::core::{checksum, drive, integrity, virtual_path};
use common::{bearer, make_repo};
use std::fs;
use tempfile::TempDir;

// ── Database backups ───────────────────────────────────────────────────────

#[actix_rt::test]
async fn test_database_list_backups_empty() {
    let app = make_app!(make_repo()).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/database/backups")
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body.as_array().unwrap().is_empty());
}

#[actix_rt::test]
async fn test_database_create_and_get_backup() {
    let backup_dir = TempDir::new().unwrap();
    let app = make_app!(make_repo()).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/database/backups")
        .insert_header(("Authorization", bearer()))
        .set_json(serde_json::json!({
            "backup_path": backup_dir.path().to_str().unwrap(),
            "max_copies": 3,
            "enabled": true
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: serde_json::Value = test::read_body_json(resp).await;
    let id = body["id"].as_i64().unwrap();
    assert_eq!(body["max_copies"], 3);
    assert_eq!(body["enabled"], true);

    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/database/backups/{}", id))
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

#[actix_rt::test]
async fn test_database_update_backup() {
    let backup_dir = TempDir::new().unwrap();
    let repo = make_repo();
    let cfg = repo
        .create_db_backup_config(backup_dir.path().to_str().unwrap(), None, 5, true)
        .unwrap();
    let app = make_app!(repo).await;
    let req = test::TestRequest::put()
        .uri(&format!("/api/v1/database/backups/{}", cfg.id))
        .insert_header(("Authorization", bearer()))
        .set_json(serde_json::json!({ "max_copies": 10, "enabled": false }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["max_copies"], 10);
    assert_eq!(body["enabled"], false);
}

#[actix_rt::test]
async fn test_database_delete_backup() {
    let backup_dir = TempDir::new().unwrap();
    let repo = make_repo();
    let cfg = repo
        .create_db_backup_config(backup_dir.path().to_str().unwrap(), None, 5, true)
        .unwrap();
    let app = make_app!(repo).await;
    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/database/backups/{}", cfg.id))
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 204);
}

#[actix_rt::test]
async fn test_database_get_not_found() {
    let app = make_app!(make_repo()).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/database/backups/999")
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 404);
}
