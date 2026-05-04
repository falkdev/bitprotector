mod common;

use actix_web::test;
use common::{bearer, make_repo};
use rusqlite::Connection;
use std::fs;
use tempfile::TempDir;

fn make_sqlite_db(dir: &TempDir, name: &str) -> String {
    let path = dir.path().join(name);
    let conn = Connection::open(&path).unwrap();
    conn.execute(
        "CREATE TABLE sample (id INTEGER PRIMARY KEY, name TEXT)",
        [],
    )
    .unwrap();
    conn.execute("INSERT INTO sample (name) VALUES ('alpha')", [])
        .unwrap();
    drop(conn);
    path.to_string_lossy().to_string()
}

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
            "enabled": true
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: serde_json::Value = test::read_body_json(resp).await;
    let id = body["id"].as_i64().unwrap();
    assert_eq!(body["priority"], 0);
    assert_eq!(body["enabled"], true);
    assert!(body.get("max_copies").is_none());

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
        .create_db_backup_config(backup_dir.path().to_str().unwrap(), None, true)
        .unwrap();
    let app = make_app!(repo).await;
    let req = test::TestRequest::put()
        .uri(&format!("/api/v1/database/backups/{}", cfg.id))
        .insert_header(("Authorization", bearer()))
        .set_json(serde_json::json!({ "drive_label": "usb", "enabled": false }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["drive_label"], "usb");
    assert_eq!(body["enabled"], false);
}

#[actix_rt::test]
async fn test_database_delete_backup() {
    let backup_dir = TempDir::new().unwrap();
    let repo = make_repo();
    let cfg = repo
        .create_db_backup_config(backup_dir.path().to_str().unwrap(), None, true)
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

#[actix_rt::test]
async fn test_database_settings_get_and_put() {
    let app = make_app!(make_repo()).await;

    let req = test::TestRequest::get()
        .uri("/api/v1/database/backups/settings")
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["backup_enabled"], false);

    let req = test::TestRequest::put()
        .uri("/api/v1/database/backups/settings")
        .insert_header(("Authorization", bearer()))
        .set_json(serde_json::json!({
            "backup_enabled": true,
            "backup_interval_seconds": 3600,
            "integrity_enabled": true,
            "integrity_interval_seconds": 7200
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["backup_enabled"], true);
    assert_eq!(body["backup_interval_seconds"], 3600);
    assert_eq!(body["integrity_enabled"], true);
    assert_eq!(body["integrity_interval_seconds"], 7200);
}

#[actix_rt::test]
async fn test_database_run_backup_without_db_path_query() {
    let repo = make_repo();
    let backup_dir = TempDir::new().unwrap();
    let db_dir = TempDir::new().unwrap();
    let db_path = make_sqlite_db(&db_dir, "live.db");
    repo.create_db_backup_config(backup_dir.path().to_str().unwrap(), None, true)
        .unwrap();
    let app = make_app_with_db_path!(repo, db_path).await;

    let req = test::TestRequest::post()
        .uri("/api/v1/database/backups/run")
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body.as_array().unwrap().len(), 1);
    assert!(backup_dir.path().join("bitprotector.db").exists());
}

#[actix_rt::test]
async fn test_database_integrity_check_repairs_corrupt_primary() {
    let repo = make_repo();
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let db_dir = TempDir::new().unwrap();
    let db_path = make_sqlite_db(&db_dir, "live.db");
    repo.create_db_backup_config(primary.path().to_str().unwrap(), None, true)
        .unwrap();
    repo.create_db_backup_config(secondary.path().to_str().unwrap(), None, true)
        .unwrap();
    let app = make_app_with_db_path!(repo, db_path).await;

    let req = test::TestRequest::post()
        .uri("/api/v1/database/backups/run")
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    fs::write(primary.path().join("bitprotector.db"), b"not sqlite").unwrap();

    let req = test::TestRequest::post()
        .uri("/api/v1/database/backups/integrity-check")
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body[0]["status"], "repaired");
}

#[actix_rt::test]
async fn test_database_integrity_check_reports_no_healthy_backup() {
    let repo = make_repo();
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    repo.create_db_backup_config(primary.path().to_str().unwrap(), None, true)
        .unwrap();
    repo.create_db_backup_config(secondary.path().to_str().unwrap(), None, true)
        .unwrap();
    fs::write(primary.path().join("bitprotector.db"), b"not sqlite").unwrap();
    fs::write(secondary.path().join("bitprotector.db"), b"also not sqlite").unwrap();
    let app = make_app!(repo).await;

    let req = test::TestRequest::post()
        .uri("/api/v1/database/backups/integrity-check")
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body[0]["status"], "corrupt");
    assert_eq!(body[1]["status"], "corrupt");
}

#[actix_rt::test]
async fn test_database_restore_stages_valid_backup() {
    let repo = make_repo();
    let db_dir = TempDir::new().unwrap();
    let live_db = make_sqlite_db(&db_dir, "live.db");
    let restore_db = make_sqlite_db(&db_dir, "restore.db");
    let app = make_app_with_db_path!(repo, live_db).await;

    let req = test::TestRequest::post()
        .uri("/api/v1/database/backups/restore")
        .insert_header(("Authorization", bearer()))
        .set_json(serde_json::json!({ "source_path": restore_db }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["restart_required"], true);
    assert!(body["safety_backup_path"]
        .as_str()
        .unwrap()
        .contains("safety"));
}

#[actix_rt::test]
async fn test_database_restore_rejects_corrupt_backup() {
    let repo = make_repo();
    let db_dir = TempDir::new().unwrap();
    let live_db = make_sqlite_db(&db_dir, "live.db");
    let corrupt_path = db_dir.path().join("bad.db");
    fs::write(&corrupt_path, b"not sqlite").unwrap();
    let app = make_app_with_db_path!(repo, live_db).await;

    let req = test::TestRequest::post()
        .uri("/api/v1/database/backups/restore")
        .insert_header(("Authorization", bearer()))
        .set_json(serde_json::json!({ "source_path": corrupt_path }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);
}
