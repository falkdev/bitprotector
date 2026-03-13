use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};
use crate::db::repository::Repository;
use crate::core::virtual_path;
use std::collections::HashMap;

#[derive(Deserialize)]
pub struct SetVirtualPathRequest {
    pub virtual_path: String,
    pub symlink_base: Option<String>,
}

#[derive(Deserialize)]
pub struct RefreshQuery {
    pub symlink_base: Option<String>,
}

#[derive(Serialize)]
struct RefreshResponse {
    created: u32,
    removed: u32,
    errors: Vec<String>,
}

fn default_symlink_base() -> String {
    "/var/lib/bitprotector/virtual".to_string()
}

/// PUT /virtual-paths/{file_id}
async fn set_virtual_path(
    repo: web::Data<Repository>,
    path: web::Path<i64>,
    body: web::Json<SetVirtualPathRequest>,
) -> HttpResponse {
    let file_id = path.into_inner();
    let symlink_base = body.symlink_base.clone().unwrap_or_else(default_symlink_base);

    let file = match repo.get_tracked_file(file_id) {
        Ok(f) => f,
        Err(e) => return HttpResponse::NotFound().body(e.to_string()),
    };
    let pair = match repo.get_drive_pair(file.drive_pair_id) {
        Ok(p) => p,
        Err(e) => return HttpResponse::InternalServerError().body(e.to_string()),
    };
    let real_path = std::path::PathBuf::from(&pair.primary_path).join(&file.relative_path);
    let real_path_str = match real_path.to_str() {
        Some(s) => s.to_string(),
        None => return HttpResponse::InternalServerError().body("Invalid path"),
    };

    match virtual_path::set_virtual_path(&repo, &symlink_base, file_id, &body.virtual_path, &real_path_str) {
        Ok(_) => HttpResponse::Ok().body(format!("Virtual path set for file #{}", file_id)),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

/// DELETE /virtual-paths/{file_id}
async fn remove_virtual_path(
    repo: web::Data<Repository>,
    path: web::Path<i64>,
    query: web::Query<RefreshQuery>,
) -> HttpResponse {
    let file_id = path.into_inner();
    let symlink_base = query.symlink_base.clone().unwrap_or_else(default_symlink_base);

    let file = match repo.get_tracked_file(file_id) {
        Ok(f) => f,
        Err(e) => return HttpResponse::NotFound().body(e.to_string()),
    };
    let vp = match file.virtual_path {
        Some(ref vp) => vp.clone(),
        None => return HttpResponse::BadRequest().body("File has no virtual path"),
    };

    match virtual_path::remove_virtual_path(&repo, &symlink_base, file_id, &vp) {
        Ok(_) => HttpResponse::Ok().body(format!("Virtual path removed for file #{}", file_id)),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

/// POST /virtual-paths/refresh
async fn refresh_symlinks(
    repo: web::Data<Repository>,
    query: web::Query<RefreshQuery>,
) -> HttpResponse {
    let symlink_base = query.symlink_base.clone().unwrap_or_else(default_symlink_base);

    let pairs = match repo.list_drive_pairs() {
        Ok(v) => v,
        Err(e) => return HttpResponse::InternalServerError().body(e.to_string()),
    };
    let pairs_map: HashMap<i64, _> = pairs.into_iter().map(|p| (p.id, p)).collect();

    match virtual_path::refresh_all_symlinks(&repo, &symlink_base, &pairs_map) {
        Ok(result) => HttpResponse::Ok().json(RefreshResponse {
            created: result.created,
            removed: result.removed,
            errors: result.errors,
        }),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/virtual-paths")
            .route("/{file_id}", web::put().to(set_virtual_path))
            .route("/{file_id}", web::delete().to(remove_virtual_path))
            .route("/refresh", web::post().to(refresh_symlinks)),
    );
}
