mod common;

use actix_web::test;
use bitprotector_lib::core::checksum;
use common::{bearer, make_repo};
use std::fs;
use std::time::Duration;
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
    let app = make_app!(repo.clone()).await;
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
    let app = make_app!(repo.clone()).await;
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
    let app = make_app!(repo.clone()).await;

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
    assert_eq!(resp.status(), 202);

    actix_rt::time::timeout(Duration::from_secs(2), async {
        loop {
            if repo.get_tracked_folder(folder.id).is_err() {
                break;
            }
            actix_rt::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("timed out waiting for folder delete");
}

#[actix_rt::test]
async fn test_folders_delete_cascades_folder_origin_descendants_and_preserves_direct_files() {
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let virtual_root = TempDir::new().unwrap();
    fs::create_dir_all(primary.path().join("docs")).unwrap();
    fs::write(primary.path().join("docs/folder-only.txt"), b"folder-only").unwrap();
    fs::write(primary.path().join("docs/direct.txt"), b"direct").unwrap();

    let repo = make_repo();
    let pair = repo
        .create_drive_pair(
            "fp-cascade",
            primary.path().to_str().unwrap(),
            secondary.path().to_str().unwrap(),
        )
        .unwrap();
    let folder = repo
        .create_tracked_folder(
            pair.id,
            "docs",
            Some(virtual_root.path().join("virtual/docs").to_str().unwrap()),
        )
        .unwrap();
    let checksum_folder = checksum::checksum_bytes(b"folder-only");
    let checksum_direct = checksum::checksum_bytes(b"direct");
    let folder_only = repo
        .create_tracked_file_with_source(
            pair.id,
            "docs/folder-only.txt",
            &checksum_folder,
            11,
            Some(
                virtual_root
                    .path()
                    .join("virtual/docs-folder-only.txt")
                    .to_str()
                    .unwrap(),
            ),
            false,
            true,
        )
        .unwrap();
    let direct = repo
        .create_tracked_file_with_source(
            pair.id,
            "docs/direct.txt",
            &checksum_direct,
            6,
            None,
            true,
            false,
        )
        .unwrap();
    let folder_queue = repo
        .create_sync_queue_item(folder_only.id, "mirror")
        .unwrap();
    let direct_queue = repo.create_sync_queue_item(direct.id, "mirror").unwrap();
    let app = make_app!(repo.clone()).await;

    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/folders/{}", folder.id))
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 202);

    actix_rt::time::timeout(Duration::from_secs(2), async {
        loop {
            if repo.get_tracked_folder(folder.id).is_err() {
                break;
            }
            actix_rt::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("timed out waiting for folder cascade delete");

    assert!(repo.get_tracked_folder(folder.id).is_err());
    assert!(repo.get_tracked_file(folder_only.id).is_err());
    assert!(repo.get_sync_queue_item(folder_queue.id).is_err());
    assert!(repo.get_sync_queue_item(direct_queue.id).is_ok());

    let preserved = repo.get_tracked_file(direct.id).unwrap();
    assert!(preserved.tracked_direct);
    assert!(!preserved.tracked_via_folder);
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
    assert_eq!(resp.status(), 202);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["scanning"], true);
    assert_eq!(body["scanned"], 0);
    assert_eq!(body["total"], 1);

    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/folders/{}/scan/active", folder.id))
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let updated_folder = actix_rt::time::timeout(Duration::from_secs(5), async {
        loop {
            let updated_folder = repo.get_tracked_folder(folder.id).unwrap();
            if !updated_folder.scanning {
                break updated_folder;
            }
            actix_rt::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("timed out waiting for folder scan to complete");

    assert!(
        updated_folder.last_scanned_at.is_some(),
        "Successful scan should stamp folder scan history"
    );
    assert_eq!(updated_folder.scan_scanned_files, 1);
    assert_eq!(updated_folder.scan_total_files, 1);
    let (files, total_files) = repo
        .list_tracked_files(Some(pair.id), None, None, 1, 20)
        .unwrap();
    assert_eq!(total_files, 1);
    assert!(!files[0].is_mirrored);
    let (queue, total_queue) = repo.list_sync_queue(Some("pending"), 1, 20).unwrap();
    assert_eq!(total_queue, 1);
    assert_eq!(queue[0].action, "adopt_mirror");
}

#[actix_rt::test]
async fn test_folders_scan_publishes_sync_summary_updates() {
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let sub = primary.path().join("scandir");
    fs::create_dir(&sub).unwrap();
    fs::write(sub.join("a.txt"), b"scan content").unwrap();

    let repo = make_repo();
    let pair = repo
        .create_drive_pair(
            "sp-bus",
            primary.path().to_str().unwrap(),
            secondary.path().to_str().unwrap(),
        )
        .unwrap();
    let folder = repo
        .create_tracked_folder(pair.id, "scandir", None)
        .unwrap();
    let (app, bus) = make_app_and_bus!(repo.clone());
    let mut receiver = bus.subscribe();

    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/folders/{}/scan", folder.id))
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 202);

    let started = actix_rt::time::timeout(Duration::from_secs(2), async {
        loop {
            if let Ok(summary) = receiver.recv().await {
                if summary.scanning {
                    break summary;
                }
            }
        }
    })
    .await
    .expect("timed out waiting for scan start snapshot");

    assert_eq!(started.scan_active_folders, 1);
    assert_eq!(started.scan_scanned_files, 0);

    let finished = actix_rt::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Ok(summary) = receiver.recv().await {
                if !summary.scanning {
                    break summary;
                }
            }
        }
    })
    .await
    .expect("timed out waiting for scan completion snapshot");

    assert_eq!(finished.scan_active_folders, 0);
    assert_eq!(finished.scanning, false);

    let updated_folder = repo.get_tracked_folder(folder.id).unwrap();
    assert!(!updated_folder.scanning);
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
    assert_eq!(resp.status(), 202);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["mirroring"], true);
    assert_eq!(body["mirrored"], 0);
    assert_eq!(body["total"], 2);
    assert_eq!(body["mirrored_files"], 0);

    actix_rt::time::timeout(Duration::from_secs(5), async {
        loop {
            let req = test::TestRequest::get()
                .uri(&format!("/api/v1/folders/{}/mirror/active", folder.id))
                .insert_header(("Authorization", bearer()))
                .to_request();
            let resp = test::call_service(&app, req).await;
            assert_eq!(resp.status(), 200);
            let status: serde_json::Value = test::read_body_json(resp).await;
            if !status["mirroring"].as_bool().unwrap_or(false) {
                break;
            }
            actix_rt::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("timed out waiting for folder mirror to complete");

    assert!(secondary.path().join("docs/a.txt").exists());
    assert!(secondary.path().join("docs/b.txt").exists());
    let q1_updated = repo.get_sync_queue_item(q1.id).unwrap();
    let q2_updated = repo.get_sync_queue_item(q2.id).unwrap();
    assert_eq!(q1_updated.status, "completed");
    assert_eq!(q2_updated.status, "completed");
    assert!(q1_updated.completed_at.is_some());
    assert!(q2_updated.completed_at.is_some());
    assert!(repo
        .get_tracked_folder(folder.id)
        .unwrap()
        .last_mirrored_at
        .is_some());
}

#[actix_rt::test]
async fn test_folders_mirror_endpoint_returns_409_when_already_mirroring() {
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    fs::create_dir(primary.path().join("docs")).unwrap();
    fs::create_dir(secondary.path().join("docs")).unwrap();

    let repo = make_repo();
    let pair = repo
        .create_drive_pair(
            "mirror-conflict",
            primary.path().to_str().unwrap(),
            secondary.path().to_str().unwrap(),
        )
        .unwrap();
    let folder = repo.create_tracked_folder(pair.id, "docs", None).unwrap();
    repo.start_folder_mirror(folder.id, 1).unwrap();

    let app = make_app!(repo).await;
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/folders/{}/mirror", folder.id))
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 409);
}

#[actix_rt::test]
async fn test_folders_delete_returns_409_when_scanning_active() {
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    fs::create_dir(primary.path().join("docs")).unwrap();

    let repo = make_repo();
    let pair = repo
        .create_drive_pair(
            "delete-scan-conflict",
            primary.path().to_str().unwrap(),
            secondary.path().to_str().unwrap(),
        )
        .unwrap();
    let folder = repo.create_tracked_folder(pair.id, "docs", None).unwrap();
    repo.start_folder_scan(folder.id, 1).unwrap();

    let app = make_app!(repo).await;
    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/folders/{}", folder.id))
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 409);
}

#[actix_rt::test]
async fn test_folders_delete_returns_409_when_mirroring_active() {
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    fs::create_dir(primary.path().join("docs")).unwrap();

    let repo = make_repo();
    let pair = repo
        .create_drive_pair(
            "delete-mirror-conflict",
            primary.path().to_str().unwrap(),
            secondary.path().to_str().unwrap(),
        )
        .unwrap();
    let folder = repo.create_tracked_folder(pair.id, "docs", None).unwrap();
    repo.start_folder_mirror(folder.id, 1).unwrap();

    let app = make_app!(repo).await;
    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/folders/{}", folder.id))
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 409);
}

#[actix_rt::test]
async fn test_folders_mirror_active_returns_zero_state_when_idle() {
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    fs::create_dir(primary.path().join("docs")).unwrap();
    fs::create_dir(secondary.path().join("docs")).unwrap();

    let repo = make_repo();
    let pair = repo
        .create_drive_pair(
            "mirror-active-idle",
            primary.path().to_str().unwrap(),
            secondary.path().to_str().unwrap(),
        )
        .unwrap();
    let folder = repo.create_tracked_folder(pair.id, "docs", None).unwrap();

    let app = make_app!(repo).await;
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/folders/{}/mirror/active", folder.id))
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["mirroring"], false);
    assert_eq!(body["mirrored"], 0);
    assert_eq!(body["total"], 0);
    assert_eq!(body["mirrored_files"], 0);
}

#[actix_rt::test]
async fn test_folders_scan_pre_existing_mirror_queues_adopt_mirror() {
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let sub_primary = primary.path().join("scandir");
    let sub_secondary = secondary.path().join("scandir");
    fs::create_dir(&sub_primary).unwrap();
    fs::create_dir(&sub_secondary).unwrap();
    let content = b"same content";
    fs::write(sub_primary.join("a.txt"), content).unwrap();
    fs::write(sub_secondary.join("a.txt"), content).unwrap();

    let repo = make_repo();
    let pair = repo
        .create_drive_pair(
            "sp-adopt",
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
    assert_eq!(resp.status(), 202);

    actix_rt::time::timeout(Duration::from_secs(5), async {
        loop {
            let current = repo.get_tracked_folder(folder.id).unwrap();
            if !current.scanning {
                break;
            }
            actix_rt::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("timed out waiting for folder scan to complete");

    let (queue, total) = repo.list_sync_queue(Some("pending"), 1, 20).unwrap();
    assert_eq!(total, 1);
    assert_eq!(queue[0].action, "adopt_mirror");
}

#[actix_rt::test]
async fn test_folders_delete_preserves_files_under_nested_tracked_subfolder() {
    // Deleting a parent tracked folder must NOT remove files that belong to a
    // separately tracked nested subfolder.
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    fs::create_dir_all(primary.path().join("docs/sub")).unwrap();
    fs::write(primary.path().join("docs/parent-only.txt"), b"parent").unwrap();
    fs::write(primary.path().join("docs/sub/sub-file.txt"), b"sub").unwrap();

    let repo = make_repo();
    let pair = repo
        .create_drive_pair(
            "fp-nested",
            primary.path().to_str().unwrap(),
            secondary.path().to_str().unwrap(),
        )
        .unwrap();

    // Track parent folder "docs" and nested subfolder "docs/sub".
    let parent_folder = repo.create_tracked_folder(pair.id, "docs", None).unwrap();
    let sub_folder = repo
        .create_tracked_folder(pair.id, "docs/sub", None)
        .unwrap();

    let checksum_parent = checksum::checksum_bytes(b"parent");
    let checksum_sub = checksum::checksum_bytes(b"sub");

    // parent-only.txt is tracked via the parent folder only.
    let parent_file = repo
        .create_tracked_file_with_source(
            pair.id,
            "docs/parent-only.txt",
            &checksum_parent,
            6,
            None,
            false,
            true,
        )
        .unwrap();
    // sub-file.txt is tracked via the nested subfolder.
    let sub_file = repo
        .create_tracked_file_with_source(
            pair.id,
            "docs/sub/sub-file.txt",
            &checksum_sub,
            3,
            None,
            false,
            true,
        )
        .unwrap();

    let parent_queue = repo
        .create_sync_queue_item(parent_file.id, "mirror")
        .unwrap();
    let sub_queue = repo.create_sync_queue_item(sub_file.id, "mirror").unwrap();

    let app = make_app!(repo.clone()).await;

    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/folders/{}", parent_folder.id))
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 202);

    actix_rt::time::timeout(Duration::from_secs(2), async {
        loop {
            if repo.get_tracked_folder(parent_folder.id).is_err() {
                break;
            }
            actix_rt::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("timed out waiting for parent folder delete");

    // Parent folder and its exclusive file must be gone.
    assert!(repo.get_tracked_folder(parent_folder.id).is_err());
    assert!(repo.get_tracked_file(parent_file.id).is_err());
    assert!(repo.get_sync_queue_item(parent_queue.id).is_err());

    // Nested subfolder and its file must survive.
    assert!(repo.get_tracked_folder(sub_folder.id).is_ok());
    assert!(repo.get_tracked_file(sub_file.id).is_ok());
    assert!(repo.get_sync_queue_item(sub_queue.id).is_ok());
}

#[actix_rt::test]
async fn test_scan_progress_scanned_never_exceeds_total() {
    // Regression test: scan_total_files is set once by start_folder_scan and must
    // not be overwritten by progress updates. Without the fix, if a file appeared
    // between the initial count and the directory walk, scanned could exceed total.
    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let sub = primary.path().join("scandir");
    fs::create_dir(&sub).unwrap();
    fs::write(sub.join("a.txt"), b"file a").unwrap();
    fs::write(sub.join("b.txt"), b"file b").unwrap();
    fs::write(sub.join("c.txt"), b"file c").unwrap();

    let repo = make_repo();
    let pair = repo
        .create_drive_pair(
            "sp-progress",
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
    assert_eq!(resp.status(), 202);
    let body: serde_json::Value = test::read_body_json(resp).await;
    let total_at_start = body["total"].as_i64().unwrap();
    assert!(
        total_at_start > 0,
        "total must be positive after scan start"
    );

    // Poll until the scan finishes, checking that scanned <= total on every tick.
    actix_rt::time::timeout(Duration::from_secs(5), async {
        loop {
            let folder_state = repo.get_tracked_folder(folder.id).unwrap();
            assert!(
                folder_state.scan_scanned_files <= folder_state.scan_total_files,
                "scanned ({}) exceeded total ({}) during scan",
                folder_state.scan_scanned_files,
                folder_state.scan_total_files,
            );
            if !folder_state.scanning {
                break;
            }
            actix_rt::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("timed out waiting for folder scan to complete");

    let final_state = repo.get_tracked_folder(folder.id).unwrap();
    assert_eq!(
        final_state.scan_total_files, total_at_start,
        "scan_total_files must not change after start"
    );
    assert_eq!(
        final_state.scan_scanned_files, total_at_start,
        "all files must be scanned"
    );
}
