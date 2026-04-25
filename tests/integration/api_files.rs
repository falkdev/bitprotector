mod common;

use actix_web::test;
use bitprotector_lib::core::{checksum, drive, integrity, virtual_path};
use common::{bearer, make_repo};
use std::fs;
use tempfile::TempDir;

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
async fn test_tracking_items_rejects_invalid_source_filter() {
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
    let pair = repo
        .create_drive_pair("tracking-derived-vpath", "/p", "/s")
        .unwrap();
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
async fn test_tracking_items_folder_status_counts_match_underlying_file_states() {
    let repo = make_repo();
    let pair = repo
        .create_drive_pair("tracking-folder-statuses", "/p", "/s")
        .unwrap();

    let not_scanned_folder = repo
        .create_tracked_folder(
            pair.id,
            "status/not-scanned",
            Some("/virtual/status/not-scanned"),
        )
        .unwrap();
    let empty_folder = repo
        .create_tracked_folder(pair.id, "status/empty", Some("/virtual/status/empty"))
        .unwrap();
    repo.create_tracked_folder(pair.id, "status/tracked", Some("/virtual/status/tracked"))
        .unwrap();
    repo.create_tracked_folder(pair.id, "status/partial", Some("/virtual/status/partial"))
        .unwrap();
    repo.create_tracked_folder(pair.id, "status/mirrored", Some("/virtual/status/mirrored"))
        .unwrap();
    repo.mark_tracked_folder_scanned(empty_folder.id).unwrap();

    for i in 0..10 {
        repo.create_tracked_file_with_source(
            pair.id,
            &format!("status/tracked/file-{i}.txt"),
            "tracked-hash",
            10,
            None,
            false,
            true,
        )
        .unwrap();
    }

    for i in 0..10 {
        let file = repo
            .create_tracked_file_with_source(
                pair.id,
                &format!("status/partial/file-{i}.txt"),
                "partial-hash",
                10,
                None,
                false,
                true,
            )
            .unwrap();
        if i < 4 {
            repo.update_tracked_file_mirror_status(file.id, true)
                .unwrap();
        }
    }

    for i in 0..10 {
        let file = repo
            .create_tracked_file_with_source(
                pair.id,
                &format!("status/mirrored/file-{i}.txt"),
                "mirrored-hash",
                10,
                None,
                false,
                true,
            )
            .unwrap();
        repo.update_tracked_file_mirror_status(file.id, true)
            .unwrap();
    }

    let app = make_app!(repo).await;
    let req = test::TestRequest::get()
        .uri(&format!(
            "/api/v1/tracking/items?drive_id={}&item_kind=folder&per_page=50",
            pair.id
        ))
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["total"], 5);

    let items = body["items"].as_array().unwrap();
    let not_scanned_item = items
        .iter()
        .find(|item| item["path"] == not_scanned_folder.folder_path)
        .unwrap();
    assert_eq!(not_scanned_item["folder_status"], "not_scanned");
    assert_eq!(not_scanned_item["folder_total_files"], 0);
    assert_eq!(not_scanned_item["folder_mirrored_files"], 0);

    let empty_item = items
        .iter()
        .find(|item| item["path"] == "status/empty")
        .unwrap();
    assert_eq!(empty_item["folder_status"], "empty");
    assert_eq!(empty_item["folder_total_files"], 0);
    assert_eq!(empty_item["folder_mirrored_files"], 0);

    let tracked_item = items
        .iter()
        .find(|item| item["path"] == "status/tracked")
        .unwrap();
    assert_eq!(tracked_item["folder_status"], "tracked");
    assert_eq!(tracked_item["folder_total_files"], 10);
    assert_eq!(tracked_item["folder_mirrored_files"], 0);

    let partial_item = items
        .iter()
        .find(|item| item["path"] == "status/partial")
        .unwrap();
    assert_eq!(partial_item["folder_status"], "partial");
    assert_eq!(partial_item["folder_total_files"], 10);
    assert_eq!(partial_item["folder_mirrored_files"], 4);

    let mirrored_item = items
        .iter()
        .find(|item| item["path"] == "status/mirrored")
        .unwrap();
    assert_eq!(mirrored_item["folder_status"], "mirrored");
    assert_eq!(mirrored_item["folder_total_files"], 10);
    assert_eq!(mirrored_item["folder_mirrored_files"], 10);
}

#[actix_rt::test]
async fn test_tracking_items_folder_status_transitions_from_not_scanned_to_empty_after_scan() {
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let empty_folder_path = primary.path().join("empty-scan");
    fs::create_dir(&empty_folder_path).unwrap();

    let repo = make_repo();
    let pair = repo
        .create_drive_pair(
            "tracking-empty-transition",
            primary.path().to_str().unwrap(),
            secondary.path().to_str().unwrap(),
        )
        .unwrap();
    let folder = repo
        .create_tracked_folder(pair.id, "empty-scan", Some("/virtual/empty-scan"))
        .unwrap();
    let app = make_app!(repo).await;

    let req = test::TestRequest::get()
        .uri(&format!(
            "/api/v1/tracking/items?drive_id={}&item_kind=folder&per_page=50",
            pair.id
        ))
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    let before_scan = body["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["path"] == folder.folder_path)
        .unwrap();
    assert_eq!(before_scan["folder_status"], "not_scanned");
    assert_eq!(before_scan["folder_total_files"], 0);
    assert_eq!(before_scan["folder_mirrored_files"], 0);

    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/folders/{}/scan", folder.id))
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::get()
        .uri(&format!(
            "/api/v1/tracking/items?drive_id={}&item_kind=folder&per_page=50",
            pair.id
        ))
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    let after_scan = body["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["path"] == folder.folder_path)
        .unwrap();
    assert_eq!(after_scan["folder_status"], "empty");
    assert_eq!(after_scan["folder_total_files"], 0);
    assert_eq!(after_scan["folder_mirrored_files"], 0);
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
