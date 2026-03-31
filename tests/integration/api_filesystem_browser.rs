use actix_web::{test, web, App};
use bitprotector_lib::api::auth::{issue_token, JwtSecret};
use bitprotector_lib::api::server::configure_routes;
use bitprotector_lib::core::scheduler::Scheduler;
use bitprotector_lib::db::repository::{create_memory_pool, Repository};
use bitprotector_lib::db::schema::initialize_schema;
use std::fs;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

const SECRET: &[u8] = b"api_filesystem_browser_secret";

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

#[actix_rt::test]
async fn test_filesystem_children_defaults_to_root() {
    let app = make_app!(make_repo()).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/filesystem/children")
        .insert_header(("Authorization", bearer()))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["path"], "/");
    assert!(body["entries"].is_array());
}

#[actix_rt::test]
async fn test_filesystem_children_lists_nested_entries() {
    let root = TempDir::new().unwrap();
    fs::create_dir(root.path().join("docs")).unwrap();
    fs::write(root.path().join("notes.txt"), b"notes").unwrap();

    let app = make_app!(make_repo()).await;
    let req = test::TestRequest::get()
        .uri(&format!(
            "/api/v1/filesystem/children?path={}",
            root.path().to_str().unwrap()
        ))
        .insert_header(("Authorization", bearer()))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    let entries = body["entries"].as_array().unwrap();

    assert_eq!(entries[0]["name"], "docs");
    assert_eq!(entries[0]["kind"], "directory");
    assert_eq!(entries[1]["name"], "notes.txt");
    assert_eq!(entries[1]["kind"], "file");
}

#[actix_rt::test]
async fn test_filesystem_children_hides_hidden_entries_by_default() {
    let root = TempDir::new().unwrap();
    fs::write(root.path().join(".secret"), b"hidden").unwrap();
    fs::write(root.path().join("visible.txt"), b"visible").unwrap();

    let app = make_app!(make_repo()).await;
    let req = test::TestRequest::get()
        .uri(&format!(
            "/api/v1/filesystem/children?path={}",
            root.path().to_str().unwrap()
        ))
        .insert_header(("Authorization", bearer()))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    let names: Vec<_> = body["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["name"].as_str().unwrap())
        .collect();

    assert_eq!(names, vec!["visible.txt"]);
}

#[actix_rt::test]
async fn test_filesystem_children_can_include_hidden_entries() {
    let root = TempDir::new().unwrap();
    fs::write(root.path().join(".secret"), b"hidden").unwrap();

    let app = make_app!(make_repo()).await;
    let req = test::TestRequest::get()
        .uri(&format!(
            "/api/v1/filesystem/children?path={}&include_hidden=true",
            root.path().to_str().unwrap()
        ))
        .insert_header(("Authorization", bearer()))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    let entry = &body["entries"].as_array().unwrap()[0];
    assert_eq!(entry["name"], ".secret");
    assert_eq!(entry["is_hidden"], true);
}

#[actix_rt::test]
async fn test_filesystem_children_rejects_invalid_path() {
    let app = make_app!(make_repo()).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/filesystem/children?path=/definitely/not/a/real/path")
        .insert_header(("Authorization", bearer()))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);
}

#[cfg(unix)]
#[actix_rt::test]
async fn test_filesystem_children_handles_unreadable_directory() {
    let root = TempDir::new().unwrap();
    let unreadable = root.path().join("private");
    fs::create_dir(&unreadable).unwrap();

    let original_permissions = fs::metadata(&unreadable).unwrap().permissions();
    let mut locked_permissions = original_permissions.clone();
    locked_permissions.set_mode(0o000);
    fs::set_permissions(&unreadable, locked_permissions).unwrap();

    let app = make_app!(make_repo()).await;
    let req = test::TestRequest::get()
        .uri(&format!(
            "/api/v1/filesystem/children?path={}",
            unreadable.to_str().unwrap()
        ))
        .insert_header(("Authorization", bearer()))
        .to_request();

    let resp = test::call_service(&app, req).await;
    fs::set_permissions(&unreadable, original_permissions).unwrap();
    assert_eq!(resp.status(), 400);
}

#[actix_rt::test]
async fn test_filesystem_children_can_filter_to_directories() {
    let root = TempDir::new().unwrap();
    fs::create_dir(root.path().join("docs")).unwrap();
    fs::write(root.path().join("notes.txt"), b"notes").unwrap();

    let app = make_app!(make_repo()).await;
    let req = test::TestRequest::get()
        .uri(&format!(
            "/api/v1/filesystem/children?path={}&directories_only=true",
            root.path().to_str().unwrap()
        ))
        .insert_header(("Authorization", bearer()))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    let entries = body["entries"].as_array().unwrap();

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["name"], "docs");
    assert_eq!(entries[0]["is_selectable"], true);
}
