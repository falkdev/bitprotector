mod common;

use actix_web::test;
use bitprotector_lib::core::{checksum, drive, integrity, virtual_path};
use common::{bearer, make_repo};
use std::fs;
use tempfile::TempDir;

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
    let pair = repo
        .create_drive_pair("vp-tree-derived", "/p", "/s")
        .unwrap();

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
