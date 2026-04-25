mod common;

use actix_web::test;
use bitprotector_lib::core::{checksum, drive, integrity, virtual_path};
use common::{bearer, make_repo};
use std::fs;
use tempfile::TempDir;

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
    let updated_folder = repo.get_tracked_folder(folder.id).unwrap();
    assert!(
        updated_folder.last_scanned_at.is_some(),
        "Successful scan should stamp folder scan history"
    );
    let (files, total_files) = repo
        .list_tracked_files(Some(pair.id), None, None, 1, 20)
        .unwrap();
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
