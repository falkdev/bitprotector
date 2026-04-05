use actix_web::{test, web, App};
use bitprotector_lib::api::auth::{issue_token, JwtSecret};
use bitprotector_lib::api::server::configure_routes;
use bitprotector_lib::core::{checksum, scheduler::Scheduler, virtual_path};
use bitprotector_lib::db::repository::{create_memory_pool, Repository};
use bitprotector_lib::db::schema::initialize_schema;
use std::fs;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

const SECRET: &[u8] = b"api_routes_test_secret";

fn make_repo() -> Repository {
    let pool = create_memory_pool().unwrap();
    initialize_schema(&pool.get().unwrap()).unwrap();
    Repository::new(pool)
}

fn bearer() -> String {
    format!("Bearer {}", issue_token("testuser", SECRET, 3600).unwrap())
}

macro_rules! make_app {
    ($repo:expr) => {{
        let _r = $repo;
        let _ra = Arc::new(_r.clone());
        let _sd = web::Data::new(Arc::new(Mutex::new(Scheduler::new(_ra))));
        test::init_service(
            App::new()
                .app_data(web::Data::new(_r))
                .app_data(web::Data::new(JwtSecret(SECRET.to_vec())))
                .app_data(_sd)
                .configure(configure_routes),
        )
    }};
}

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

// ── Files ──────────────────────────────────────────────────────────────────

#[actix_rt::test]
async fn test_files_track() {
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    fs::write(primary.path().join("doc.txt"), b"file content").unwrap();
    let repo = make_repo();
    let pair = repo
        .create_drive_pair(
            "fp",
            primary.path().to_str().unwrap(),
            secondary.path().to_str().unwrap(),
        )
        .unwrap();
    let app = make_app!(repo.clone()).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/files")
        .insert_header(("Authorization", bearer()))
        .set_json(serde_json::json!({
            "drive_pair_id": pair.id,
            "relative_path": "doc.txt"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["relative_path"], "doc.txt");
    assert!(
        !secondary.path().join("doc.txt").exists(),
        "Tracking should queue mirror work instead of mirroring immediately"
    );
    let (queue_items, total) = repo.list_sync_queue(Some("pending"), 1, 20).unwrap();
    assert_eq!(total, 1);
    assert_eq!(queue_items[0].action, "mirror");
}

#[actix_rt::test]
async fn test_files_track_existing_promotes_direct_source_and_returns_ok() {
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    fs::write(primary.path().join("doc.txt"), b"file content").unwrap();
    let repo = make_repo();
    let pair = repo
        .create_drive_pair(
            "fp-existing",
            primary.path().to_str().unwrap(),
            secondary.path().to_str().unwrap(),
        )
        .unwrap();
    let checksum = checksum::checksum_bytes(b"file content");
    let existing = repo
        .create_tracked_file_with_source(pair.id, "doc.txt", &checksum, 12, None, false, true)
        .unwrap();
    assert!(!existing.tracked_direct);
    assert!(existing.tracked_via_folder);

    let app = make_app!(repo.clone()).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/files")
        .insert_header(("Authorization", bearer()))
        .set_json(serde_json::json!({
            "drive_pair_id": pair.id,
            "relative_path": "doc.txt"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["tracked_direct"], true);
    assert_eq!(body["tracked_via_folder"], false);

    let (files, total) = repo
        .list_tracked_files(Some(pair.id), None, None, 1, 10)
        .unwrap();
    assert_eq!(
        total, 1,
        "Idempotent track should not create duplicate rows"
    );
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].relative_path, "doc.txt");
}

#[actix_rt::test]
async fn test_files_track_accepts_absolute_path_within_active_root() {
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let absolute_path = primary.path().join("nested/doc.txt");
    fs::create_dir_all(absolute_path.parent().unwrap()).unwrap();
    fs::write(&absolute_path, b"file content").unwrap();
    let repo = make_repo();
    let pair = repo
        .create_drive_pair(
            "fp-abs",
            primary.path().to_str().unwrap(),
            secondary.path().to_str().unwrap(),
        )
        .unwrap();
    let app = make_app!(repo).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/files")
        .insert_header(("Authorization", bearer()))
        .set_json(serde_json::json!({
            "drive_pair_id": pair.id,
            "relative_path": absolute_path.to_str().unwrap()
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["relative_path"], "nested/doc.txt");
}

#[actix_rt::test]
async fn test_files_track_rejects_path_outside_active_root() {
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    let outside_file = outside.path().join("outside.txt");
    fs::write(&outside_file, b"outside").unwrap();
    let repo = make_repo();
    let pair = repo
        .create_drive_pair(
            "fp-outside",
            primary.path().to_str().unwrap(),
            secondary.path().to_str().unwrap(),
        )
        .unwrap();
    let app = make_app!(repo).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/files")
        .insert_header(("Authorization", bearer()))
        .set_json(serde_json::json!({
            "drive_pair_id": pair.id,
            "relative_path": outside_file.to_str().unwrap()
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);
}

#[actix_rt::test]
async fn test_files_track_rejects_parent_directory_traversal() {
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    fs::write(primary.path().join("doc.txt"), b"file content").unwrap();
    let repo = make_repo();
    let pair = repo
        .create_drive_pair(
            "fp-traversal",
            primary.path().to_str().unwrap(),
            secondary.path().to_str().unwrap(),
        )
        .unwrap();
    let app = make_app!(repo).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/files")
        .insert_header(("Authorization", bearer()))
        .set_json(serde_json::json!({
            "drive_pair_id": pair.id,
            "relative_path": "../doc.txt"
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);
}

#[cfg(unix)]
#[actix_rt::test]
async fn test_files_track_rejects_symlink_escape() {
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    let outside_file = outside.path().join("secret.txt");
    fs::write(&outside_file, b"secret").unwrap();
    std::os::unix::fs::symlink(&outside_file, primary.path().join("secret-link.txt")).unwrap();
    let repo = make_repo();
    let pair = repo
        .create_drive_pair(
            "fp-symlink",
            primary.path().to_str().unwrap(),
            secondary.path().to_str().unwrap(),
        )
        .unwrap();
    let app = make_app!(repo).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/files")
        .insert_header(("Authorization", bearer()))
        .set_json(serde_json::json!({
            "drive_pair_id": pair.id,
            "relative_path": "secret-link.txt"
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);
}

#[actix_rt::test]
async fn test_files_list() {
    let app = make_app!(make_repo()).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/files")
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["total"], 0);
}

#[actix_rt::test]
async fn test_tracking_items_filters() {
    let repo = make_repo();
    let pair = repo.create_drive_pair("tracking", "/p", "/s").unwrap();
    repo.create_tracked_file_with_source(
        pair.id,
        "docs/alpha.txt",
        "hash-a",
        10,
        Some("/virtual/docs/alpha.txt"),
        true,
        false,
    )
    .unwrap();
    repo.create_tracked_file_with_source(
        pair.id,
        "docs/beta.txt",
        "hash-b",
        12,
        Some("/virtual/docs/beta.txt"),
        false,
        true,
    )
    .unwrap();
    repo.create_tracked_folder(pair.id, "docs", Some("/virtual/docs"))
        .unwrap();

    let app = make_app!(repo).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/tracking/items?item_kind=file&source=folder&virtual_prefix=/virtual/docs")
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["total"], 1);
    assert_eq!(body["items"][0]["source"], "folder");
    assert_eq!(body["items"][0]["kind"], "file");
}

#[actix_rt::test]
async fn test_tracking_items_rejects_legacy_both_source_filter() {
    let app = make_app!(make_repo()).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/tracking/items?item_kind=file&source=both")
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);
}

#[actix_rt::test]
async fn test_tracking_items_derives_file_virtual_path_from_tracked_folder_virtual_path() {
    let repo = make_repo();
    let pair = repo.create_drive_pair("tracking-derived-vpath", "/p", "/s").unwrap();
    repo.create_tracked_folder(pair.id, "docs", Some("/virtual/docs"))
        .unwrap();
    repo.create_tracked_file_with_source(pair.id, "docs/a.txt", "h1", 10, None, false, true)
        .unwrap();
    repo.create_tracked_file_with_source(pair.id, "misc/b.txt", "h2", 10, None, true, false)
        .unwrap();

    let app = make_app!(repo).await;

    let req = test::TestRequest::get()
        .uri(&format!(
            "/api/v1/tracking/items?drive_id={}&item_kind=file&has_virtual_path=true",
            pair.id
        ))
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["total"], 1);
    assert_eq!(body["items"][0]["path"], "docs/a.txt");
    assert_eq!(body["items"][0]["virtual_path"], "/virtual/docs/a.txt");

    let req = test::TestRequest::get()
        .uri(&format!(
            "/api/v1/tracking/items?drive_id={}&item_kind=file&virtual_prefix=/virtual/docs",
            pair.id
        ))
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["total"], 1);
    assert_eq!(body["items"][0]["virtual_path"], "/virtual/docs/a.txt");
}

#[actix_rt::test]
async fn test_tracking_items_filter_and_pagination_combinations() {
    let repo = make_repo();
    let pair_a = repo.create_drive_pair("tracking-a", "/p1", "/s1").unwrap();
    let pair_b = repo.create_drive_pair("tracking-b", "/p2", "/s2").unwrap();

    repo.create_tracked_file_with_source(pair_a.id, "docs/a.txt", "h1", 10, None, true, false)
        .unwrap();
    repo.create_tracked_file_with_source(
        pair_a.id,
        "docs/b.txt",
        "h2",
        11,
        Some("/virtual/docs/b.txt"),
        false,
        true,
    )
    .unwrap();
    repo.create_tracked_file_with_source(
        pair_a.id,
        "media/c.txt",
        "h3",
        12,
        Some("/virtual/media/c.txt"),
        false,
        true,
    )
    .unwrap();
    repo.create_tracked_file_with_source(
        pair_b.id,
        "docs/d.txt",
        "h4",
        13,
        Some("/virtual/docs/d.txt"),
        true,
        false,
    )
    .unwrap();
    repo.create_tracked_folder(pair_a.id, "docs", Some("/virtual/docs"))
        .unwrap();

    let app = make_app!(repo).await;

    let req = test::TestRequest::get()
        .uri(&format!(
            "/api/v1/tracking/items?drive_id={}&item_kind=file&has_virtual_path=true&source=folder&page=1&per_page=1",
            pair_a.id
        ))
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["total"], 2);
    assert_eq!(body["items"][0]["path"], "docs/b.txt");
    assert_eq!(body["items"][0]["source"], "folder");

    let req = test::TestRequest::get()
        .uri(&format!(
            "/api/v1/tracking/items?drive_id={}&item_kind=file&page=2&per_page=2",
            pair_a.id
        ))
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["total"], 3);
    assert_eq!(body["items"].as_array().unwrap().len(), 1);
    assert_eq!(body["items"][0]["path"], "media/c.txt");

    let req = test::TestRequest::get()
        .uri(&format!(
            "/api/v1/tracking/items?drive_id={}&item_kind=folder&virtual_prefix=/virtual/docs",
            pair_a.id
        ))
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["total"], 1);
    assert_eq!(body["items"][0]["kind"], "folder");
    assert_eq!(body["items"][0]["path"], "docs");
}

#[actix_rt::test]
async fn test_tracking_provenance_lifecycle_folder_scan_direct_track_and_folder_removal() {
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    fs::create_dir_all(primary.path().join("docs")).unwrap();
    fs::write(primary.path().join("docs/alpha.txt"), b"alpha-content").unwrap();

    let repo = make_repo();
    let pair = repo
        .create_drive_pair(
            "tracking-provenance",
            primary.path().to_str().unwrap(),
            secondary.path().to_str().unwrap(),
        )
        .unwrap();
    let app = make_app!(repo).await;

    let req = test::TestRequest::post()
        .uri("/api/v1/folders")
        .insert_header(("Authorization", bearer()))
        .set_json(serde_json::json!({
            "drive_pair_id": pair.id,
            "folder_path": "docs"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let folder_body: serde_json::Value = test::read_body_json(resp).await;
    let folder_id = folder_body["id"].as_i64().unwrap();

    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/folders/{folder_id}/scan"))
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::get()
        .uri(&format!(
            "/api/v1/tracking/items?drive_id={}&item_kind=file&per_page=50",
            pair.id
        ))
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["total"], 1);
    assert_eq!(body["items"][0]["path"], "docs/alpha.txt");
    assert_eq!(body["items"][0]["source"], "folder");
    assert_eq!(body["items"][0]["tracked_direct"], false);
    assert_eq!(body["items"][0]["tracked_via_folder"], true);

    let req = test::TestRequest::post()
        .uri("/api/v1/files")
        .insert_header(("Authorization", bearer()))
        .set_json(serde_json::json!({
            "drive_pair_id": pair.id,
            "relative_path": "docs/alpha.txt"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::get()
        .uri(&format!(
            "/api/v1/tracking/items?drive_id={}&item_kind=file&per_page=50",
            pair.id
        ))
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["items"][0]["source"], "direct");
    assert_eq!(body["items"][0]["tracked_direct"], true);
    assert_eq!(body["items"][0]["tracked_via_folder"], false);

    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/folders/{folder_id}"))
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 204);

    let req = test::TestRequest::get()
        .uri(&format!(
            "/api/v1/tracking/items?drive_id={}&item_kind=file&per_page=50",
            pair.id
        ))
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["items"][0]["source"], "direct");
    assert_eq!(body["items"][0]["tracked_direct"], true);
    assert_eq!(body["items"][0]["tracked_via_folder"], false);
}

#[actix_rt::test]
async fn test_files_get() {
    let repo = make_repo();
    let pair = repo.create_drive_pair("pair", "/p", "/s").unwrap();
    let file = repo
        .create_tracked_file(pair.id, "f.txt", "abc123", 10, None)
        .unwrap();
    let app = make_app!(repo).await;
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/files/{}", file.id))
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["relative_path"], "f.txt");
}

#[actix_rt::test]
async fn test_files_get_not_found() {
    let app = make_app!(make_repo()).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/files/999")
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 404);
}

#[actix_rt::test]
async fn test_files_delete() {
    let repo = make_repo();
    let pair = repo.create_drive_pair("pair", "/p", "/s").unwrap();
    let file = repo
        .create_tracked_file(pair.id, "del.txt", "abc", 3, None)
        .unwrap();
    let app = make_app!(repo).await;
    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/files/{}", file.id))
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 204);
}

#[actix_rt::test]
async fn test_files_mirror_via_api() {
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let content = b"mirror me";
    fs::write(primary.path().join("m.txt"), content).unwrap();
    let repo = make_repo();
    let pair = repo
        .create_drive_pair(
            "mp",
            primary.path().to_str().unwrap(),
            secondary.path().to_str().unwrap(),
        )
        .unwrap();
    let hash = checksum::checksum_bytes(content);
    let file = repo
        .create_tracked_file(pair.id, "m.txt", &hash, content.len() as i64, None)
        .unwrap();
    let queue_item = repo.create_sync_queue_item(file.id, "mirror").unwrap();
    let app = make_app!(repo.clone()).await;
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/files/{}/mirror", file.id))
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["mirrored"], true);
    assert!(secondary.path().join("m.txt").exists());
    let updated = repo.get_sync_queue_item(queue_item.id).unwrap();
    assert_eq!(updated.status, "completed");
}

// ── Folders ────────────────────────────────────────────────────────────────

#[actix_rt::test]
async fn test_folders_list_empty() {
    let app = make_app!(make_repo()).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/folders")
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body.as_array().unwrap().is_empty());
}

#[actix_rt::test]
async fn test_folders_add() {
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let sub = primary.path().join("docs");
    fs::create_dir(&sub).unwrap();
    let repo = make_repo();
    let pair = repo
        .create_drive_pair(
            "fp",
            primary.path().to_str().unwrap(),
            secondary.path().to_str().unwrap(),
        )
        .unwrap();
    let app = make_app!(repo).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/folders")
        .insert_header(("Authorization", bearer()))
        .set_json(serde_json::json!({
            "drive_pair_id": pair.id,
            "folder_path": "docs"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["folder_path"], "docs");
}

#[actix_rt::test]
async fn test_folders_add_with_virtual_path_creates_symlink() {
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let virtual_root = TempDir::new().unwrap();
    let sub = primary.path().join("docs");
    fs::create_dir(&sub).unwrap();
    let virtual_path_on_disk = virtual_root.path().join("virtual/docs");
    let repo = make_repo();
    let pair = repo
        .create_drive_pair(
            "fp-virtual",
            primary.path().to_str().unwrap(),
            secondary.path().to_str().unwrap(),
        )
        .unwrap();
    let app = make_app!(repo).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/folders")
        .insert_header(("Authorization", bearer()))
        .set_json(serde_json::json!({
            "drive_pair_id": pair.id,
            "folder_path": "docs",
            "virtual_path": virtual_path_on_disk.to_str().unwrap()
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["virtual_path"], virtual_path_on_disk.to_str().unwrap());
    assert!(virtual_path_on_disk.is_symlink());
}

#[actix_rt::test]
async fn test_folders_add_accepts_absolute_path_within_active_root() {
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let folder = primary.path().join("projects/docs");
    fs::create_dir_all(&folder).unwrap();
    let repo = make_repo();
    let pair = repo
        .create_drive_pair(
            "fp-folder-abs",
            primary.path().to_str().unwrap(),
            secondary.path().to_str().unwrap(),
        )
        .unwrap();
    let app = make_app!(repo).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/folders")
        .insert_header(("Authorization", bearer()))
        .set_json(serde_json::json!({
            "drive_pair_id": pair.id,
            "folder_path": folder.to_str().unwrap()
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["folder_path"], "projects/docs");
}

#[actix_rt::test]
async fn test_folders_update_virtual_path() {
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let virtual_root = TempDir::new().unwrap();
    let sub = primary.path().join("docs");
    fs::create_dir(&sub).unwrap();
    let virtual_path_on_disk = virtual_root.path().join("virtual/docs");
    let repo = make_repo();
    let pair = repo
        .create_drive_pair(
            "fp-folder-update",
            primary.path().to_str().unwrap(),
            secondary.path().to_str().unwrap(),
        )
        .unwrap();
    let folder = repo.create_tracked_folder(pair.id, "docs", None).unwrap();
    let app = make_app!(repo).await;

    let req = test::TestRequest::put()
        .uri(&format!("/api/v1/folders/{}", folder.id))
        .insert_header(("Authorization", bearer()))
        .set_json(serde_json::json!({
            "virtual_path": virtual_path_on_disk.to_str().unwrap()
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["virtual_path"], virtual_path_on_disk.to_str().unwrap());
    assert!(virtual_path_on_disk.is_symlink());

    let req = test::TestRequest::put()
        .uri(&format!("/api/v1/folders/{}", folder.id))
        .insert_header(("Authorization", bearer()))
        .set_json(serde_json::json!({
            "virtual_path": null
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body["virtual_path"].is_null());
    assert!(!virtual_path_on_disk.exists());
}

#[actix_rt::test]
async fn test_folders_add_rejects_path_outside_active_root() {
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    let outside_folder = outside.path().join("outside");
    fs::create_dir_all(&outside_folder).unwrap();
    let repo = make_repo();
    let pair = repo
        .create_drive_pair(
            "fp-folder-outside",
            primary.path().to_str().unwrap(),
            secondary.path().to_str().unwrap(),
        )
        .unwrap();
    let app = make_app!(repo).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/folders")
        .insert_header(("Authorization", bearer()))
        .set_json(serde_json::json!({
            "drive_pair_id": pair.id,
            "folder_path": outside_folder.to_str().unwrap()
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);
}

#[actix_rt::test]
async fn test_folders_get_and_delete() {
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let repo = make_repo();
    let pair = repo
        .create_drive_pair(
            "fp",
            primary.path().to_str().unwrap(),
            secondary.path().to_str().unwrap(),
        )
        .unwrap();
    let folder = repo
        .create_tracked_folder(pair.id, "reports", None)
        .unwrap();
    let app = make_app!(repo).await;

    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/folders/{}", folder.id))
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["folder_path"], "reports");

    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/folders/{}", folder.id))
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 204);
}

#[actix_rt::test]
async fn test_folders_scan() {
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let sub = primary.path().join("scandir");
    fs::create_dir(&sub).unwrap();
    fs::write(sub.join("a.txt"), b"scan content").unwrap();
    let repo = make_repo();
    let pair = repo
        .create_drive_pair(
            "sp",
            primary.path().to_str().unwrap(),
            secondary.path().to_str().unwrap(),
        )
        .unwrap();
    let folder = repo
        .create_tracked_folder(pair.id, "scandir", None)
        .unwrap();
    let app = make_app!(repo.clone()).await;
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/folders/{}/scan", folder.id))
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body["new_files"].as_u64().unwrap() >= 1);
    let (files, total_files) = repo.list_tracked_files(Some(pair.id), None, None, 1, 20).unwrap();
    assert_eq!(total_files, 1);
    assert!(!files[0].is_mirrored);
    let (queue, total_queue) = repo.list_sync_queue(Some("pending"), 1, 20).unwrap();
    assert_eq!(total_queue, 1);
    assert_eq!(queue[0].action, "mirror");
}

#[actix_rt::test]
async fn test_folders_mirror_endpoint_processes_unmirrored_files_under_folder() {
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let sub = primary.path().join("docs");
    fs::create_dir(&sub).unwrap();
    fs::create_dir(secondary.path().join("docs")).unwrap();
    fs::write(sub.join("a.txt"), b"a").unwrap();
    fs::write(sub.join("b.txt"), b"b").unwrap();

    let repo = make_repo();
    let pair = repo
        .create_drive_pair(
            "mirror-folder",
            primary.path().to_str().unwrap(),
            secondary.path().to_str().unwrap(),
        )
        .unwrap();
    let folder = repo.create_tracked_folder(pair.id, "docs", None).unwrap();
    let checksum_a = checksum::checksum_bytes(b"a");
    let checksum_b = checksum::checksum_bytes(b"b");
    let file_a = repo
        .create_tracked_file_with_source(pair.id, "docs/a.txt", &checksum_a, 1, None, false, true)
        .unwrap();
    let file_b = repo
        .create_tracked_file_with_source(pair.id, "docs/b.txt", &checksum_b, 1, None, false, true)
        .unwrap();
    let q1 = repo.create_sync_queue_item(file_a.id, "mirror").unwrap();
    let q2 = repo.create_sync_queue_item(file_b.id, "mirror").unwrap();

    let app = make_app!(repo.clone()).await;
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/folders/{}/mirror", folder.id))
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["mirrored_files"], 2);
    assert!(secondary.path().join("docs/a.txt").exists());
    assert!(secondary.path().join("docs/b.txt").exists());
    assert_eq!(repo.get_sync_queue_item(q1.id).unwrap().status, "completed");
    assert_eq!(repo.get_sync_queue_item(q2.id).unwrap().status, "completed");
}

// ── Virtual paths ──────────────────────────────────────────────────────────

#[actix_rt::test]
async fn test_virtual_paths_set() {
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let virtual_root = TempDir::new().unwrap();
    let virtual_path_on_disk = virtual_root.path().join("data/vp.txt");
    fs::write(primary.path().join("vp.txt"), b"content").unwrap();
    let repo = make_repo();
    let pair = repo
        .create_drive_pair(
            "vpp",
            primary.path().to_str().unwrap(),
            secondary.path().to_str().unwrap(),
        )
        .unwrap();
    let hash = checksum::checksum_bytes(b"content");
    let file = repo
        .create_tracked_file(pair.id, "vp.txt", &hash, 7, None)
        .unwrap();
    let app = make_app!(repo).await;
    let req = test::TestRequest::put()
        .uri(&format!("/api/v1/virtual-paths/{}", file.id))
        .insert_header(("Authorization", bearer()))
        .set_json(serde_json::json!({
            "virtual_path": virtual_path_on_disk.to_str().unwrap()
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    assert!(virtual_path_on_disk.is_symlink());
}

#[actix_rt::test]
async fn test_virtual_paths_remove() {
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let virtual_root = TempDir::new().unwrap();
    let virtual_path_on_disk = virtual_root.path().join("virts/rem.txt");
    let content = b"data";
    fs::write(primary.path().join("rem.txt"), content).unwrap();
    let repo = make_repo();
    let pair = repo
        .create_drive_pair(
            "vpp",
            primary.path().to_str().unwrap(),
            secondary.path().to_str().unwrap(),
        )
        .unwrap();
    let hash = checksum::checksum_bytes(content);
    let file = repo
        .create_tracked_file(pair.id, "rem.txt", &hash, 4, None)
        .unwrap();
    // Set the virtual path directly via the library before testing the DELETE endpoint
    virtual_path::set_virtual_path(&repo, file.id, virtual_path_on_disk.to_str().unwrap()).unwrap();
    let app = make_app!(repo).await;
    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/virtual-paths/{}", file.id))
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    assert!(!virtual_path_on_disk.is_symlink());
}

#[actix_rt::test]
async fn test_virtual_paths_tree_returns_lazy_children_with_counts() {
    let repo = make_repo();
    let pair = repo.create_drive_pair("vp-tree", "/p", "/s").unwrap();

    repo.create_tracked_file_with_source(
        pair.id,
        "docs/a.txt",
        "h1",
        10,
        Some("/virtual/docs/a.txt"),
        true,
        false,
    )
    .unwrap();
    repo.create_tracked_file_with_source(
        pair.id,
        "docs/archive/b.txt",
        "h2",
        10,
        Some("/virtual/docs/archive/b.txt"),
        true,
        false,
    )
    .unwrap();
    repo.create_tracked_file_with_source(
        pair.id,
        "media/c.txt",
        "h3",
        10,
        Some("/virtual/media/c.txt"),
        true,
        false,
    )
    .unwrap();
    repo.create_tracked_folder(pair.id, "folder-only", Some("/virtual/folder-only"))
        .unwrap();

    let app = make_app!(repo).await;

    let req = test::TestRequest::get()
        .uri("/api/v1/virtual-paths/tree?parent=/virtual")
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    let children = body["children"].as_array().unwrap();

    let docs = children
        .iter()
        .find(|child| child["name"] == "docs")
        .expect("docs child should exist");
    assert_eq!(docs["path"], "/virtual/docs");
    assert_eq!(docs["item_count"], 2);
    assert_eq!(docs["has_children"], true);

    let media = children
        .iter()
        .find(|child| child["name"] == "media")
        .expect("media child should exist");
    assert_eq!(media["path"], "/virtual/media");
    assert_eq!(media["item_count"], 1);
    assert_eq!(media["has_children"], true);

    let folder_only = children
        .iter()
        .find(|child| child["name"] == "folder-only")
        .expect("folder-only child should exist");
    assert_eq!(folder_only["path"], "/virtual/folder-only");
    assert_eq!(folder_only["item_count"], 1);
    assert_eq!(folder_only["has_children"], false);

    let req = test::TestRequest::get()
        .uri("/api/v1/virtual-paths/tree?parent=/virtual/docs")
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    let nested = body["children"].as_array().unwrap();

    let a_txt = nested
        .iter()
        .find(|child| child["name"] == "a.txt")
        .expect("a.txt child should exist");
    assert_eq!(a_txt["path"], "/virtual/docs/a.txt");
    assert_eq!(a_txt["item_count"], 1);
    assert_eq!(a_txt["has_children"], false);

    let archive = nested
        .iter()
        .find(|child| child["name"] == "archive")
        .expect("archive child should exist");
    assert_eq!(archive["path"], "/virtual/docs/archive");
    assert_eq!(archive["item_count"], 1);
    assert_eq!(archive["has_children"], true);
}

#[actix_rt::test]
async fn test_virtual_paths_tree_includes_folder_derived_file_virtual_paths() {
    let repo = make_repo();
    let pair = repo.create_drive_pair("vp-tree-derived", "/p", "/s").unwrap();

    repo.create_tracked_folder(pair.id, "docs", Some("/virtual/docs"))
        .unwrap();
    repo.create_tracked_file_with_source(pair.id, "docs/a.txt", "h1", 10, None, false, true)
        .unwrap();

    let app = make_app!(repo).await;

    let req = test::TestRequest::get()
        .uri("/api/v1/virtual-paths/tree?parent=/virtual")
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    let children = body["children"].as_array().unwrap();

    let docs = children
        .iter()
        .find(|child| child["name"] == "docs")
        .expect("docs child should exist");
    assert_eq!(docs["path"], "/virtual/docs");
    assert_eq!(docs["item_count"], 2);
    assert_eq!(docs["has_children"], true);
}

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
async fn test_integrity_check_all_empty() {
    let app = make_app!(make_repo()).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/integrity/check-all")
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["results"].as_array().unwrap().len(), 0);
}

#[actix_rt::test]
async fn test_integrity_check_all_with_recover() {
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let content = b"good content";
    fs::write(primary.path().join("r.txt"), content).unwrap();
    // Mirror is corrupt
    fs::write(secondary.path().join("r.txt"), b"corrupt").unwrap();
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
    let req = test::TestRequest::get()
        .uri("/api/v1/integrity/check-all?recover=true")
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    let results = body["results"].as_array().unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["recovered"], true);
    // Mirror should now match primary
    let mirror = fs::read(secondary.path().join("r.txt")).unwrap();
    assert_eq!(mirror, content);
}

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
        .create_schedule_config("sync", None, Some(300), true)
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
        .create_schedule_config("integrity_check", None, Some(600), false)
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

// ── Status ─────────────────────────────────────────────────────────────────

#[actix_rt::test]
async fn test_status_route_shape() {
    let app = make_app!(make_repo()).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/status")
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body["files_tracked"].is_number());
    assert!(body["drive_pairs"].is_number());
    assert!(body["degraded_pairs"].is_number());
    assert!(body["active_secondary_pairs"].is_number());
    assert!(body["rebuilding_pairs"].is_number());
    assert!(body["quiescing_pairs"].is_number());
}

#[actix_rt::test]
async fn test_status_reflects_degraded_pair() {
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let repo = make_repo();
    let pair = repo
        .create_drive_pair(
            "degraded",
            primary.path().to_str().unwrap(),
            secondary.path().to_str().unwrap(),
        )
        .unwrap();
    // Mark primary as quiescing then failed to make it degraded
    bitprotector_lib::core::drive::mark_drive_quiescing(
        &repo,
        pair.id,
        bitprotector_lib::core::drive::DriveRole::Primary,
    )
    .unwrap();
    bitprotector_lib::core::drive::confirm_drive_failure(
        &repo,
        pair.id,
        bitprotector_lib::core::drive::DriveRole::Primary,
    )
    .unwrap();
    let app = make_app!(repo).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/status")
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["active_secondary_pairs"].as_i64().unwrap(), 1);
}
