use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};
use crate::db::repository::Repository;
use crate::core::{tracker, change_detection};

#[derive(Deserialize)]
pub struct AddFolderRequest {
    pub drive_pair_id: i64,
    pub folder_path: String,
    pub auto_virtual_path: Option<bool>,
    pub default_virtual_base: Option<String>,
}

#[derive(Serialize)]
struct ScanResult {
    new_files: usize,
    changed_files: usize,
}

/// GET /folders
async fn list_folders(repo: web::Data<Repository>) -> HttpResponse {
    match repo.list_tracked_folders() {
        Ok(folders) => HttpResponse::Ok().json(folders),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

/// POST /folders
async fn add_folder(
    repo: web::Data<Repository>,
    body: web::Json<AddFolderRequest>,
) -> HttpResponse {
    let pair = match repo.get_drive_pair(body.drive_pair_id) {
        Ok(p) => p,
        Err(e) => return HttpResponse::NotFound().body(e.to_string()),
    };
    match tracker::track_folder(
        &repo,
        &pair,
        &body.folder_path,
        body.auto_virtual_path.unwrap_or(false),
        body.default_virtual_base.as_deref(),
    ) {
        Ok(folder) => HttpResponse::Created().json(folder),
        Err(e) => HttpResponse::BadRequest().body(e.to_string()),
    }
}

/// GET /folders/{id}
async fn get_folder(repo: web::Data<Repository>, path: web::Path<i64>) -> HttpResponse {
    match repo.get_tracked_folder(path.into_inner()) {
        Ok(f) => HttpResponse::Ok().json(f),
        Err(e) => HttpResponse::NotFound().body(e.to_string()),
    }
}

/// DELETE /folders/{id}
async fn delete_folder(repo: web::Data<Repository>, path: web::Path<i64>) -> HttpResponse {
    match repo.delete_tracked_folder(path.into_inner()) {
        Ok(_) => HttpResponse::NoContent().finish(),
        Err(e) => HttpResponse::NotFound().body(e.to_string()),
    }
}

/// POST /folders/{id}/scan
async fn scan_folder(repo: web::Data<Repository>, path: web::Path<i64>) -> HttpResponse {
    let folder = match repo.get_tracked_folder(path.into_inner()) {
        Ok(f) => f,
        Err(e) => return HttpResponse::NotFound().body(e.to_string()),
    };
    let pair = match repo.get_drive_pair(folder.drive_pair_id) {
        Ok(p) => p,
        Err(e) => return HttpResponse::InternalServerError().body(e.to_string()),
    };
    let new_files = match tracker::auto_track_folder_files(&repo, &pair, &folder) {
        Ok(v) => v,
        Err(e) => return HttpResponse::InternalServerError().body(e.to_string()),
    };
    let changes = match change_detection::scan_all_changes(&repo, &pair) {
        Ok(v) => v,
        Err(e) => return HttpResponse::InternalServerError().body(e.to_string()),
    };
    let folder_changes = changes
        .iter()
        .filter(|(f, _)| f.relative_path.starts_with(&format!("{}/", folder.folder_path)))
        .count();

    HttpResponse::Ok().json(ScanResult {
        new_files: new_files.len(),
        changed_files: folder_changes,
    })
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/folders")
            .route("", web::get().to(list_folders))
            .route("", web::post().to(add_folder))
            .route("/{id}", web::get().to(get_folder))
            .route("/{id}", web::delete().to(delete_folder))
            .route("/{id}/scan", web::post().to(scan_folder)),
    );
}
