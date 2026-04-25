mod common;

use actix_web::test;
use common::{bearer, make_repo};
use tempfile::TempDir;

// ── Status / residual cross-route checks ───────────────────────────────────

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
