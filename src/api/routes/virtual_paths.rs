use crate::core::{drive, virtual_path};
use crate::db::repository::Repository;
use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};
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

#[derive(Deserialize)]
pub struct BulkVirtualPathEntry {
    pub file_id: i64,
    pub virtual_path: String,
    pub real_path: Option<String>,
}

#[derive(Deserialize)]
pub struct BulkVirtualPathRequest {
    pub entries: Vec<BulkVirtualPathEntry>,
    pub symlink_base: Option<String>,
}

#[derive(Deserialize)]
pub struct BulkFromRealRequest {
    pub drive_pair_id: i64,
    pub folder_path: String,
    pub virtual_base: String,
    pub symlink_base: Option<String>,
}

#[derive(Serialize)]
struct RefreshResponse {
    created: u32,
    removed: u32,
    errors: Vec<String>,
}

#[derive(Serialize)]
struct BulkResponse {
    succeeded: Vec<i64>,
    failed: Vec<BulkFailureEntry>,
}

#[derive(Serialize)]
struct BulkFailureEntry {
    file_id: i64,
    error: String,
}

/// Resolve symlink base: request field > env var > compiled default.
fn resolve_symlink_base(request_override: Option<String>) -> String {
    request_override
        .or_else(|| std::env::var("BITPROTECTOR_SYMLINK_BASE").ok())
        .unwrap_or_else(|| "/var/lib/bitprotector/virtual".to_string())
}

/// PUT /virtual-paths/{file_id}
async fn set_virtual_path(
    repo: web::Data<Repository>,
    path: web::Path<i64>,
    body: web::Json<SetVirtualPathRequest>,
) -> HttpResponse {
    let file_id = path.into_inner();
    let symlink_base = resolve_symlink_base(body.symlink_base.clone());

    let file = match repo.get_tracked_file(file_id) {
        Ok(f) => f,
        Err(e) => return HttpResponse::NotFound().body(e.to_string()),
    };
    let pair = match drive::load_operational_pair(&repo, file.drive_pair_id) {
        Ok(p) => p,
        Err(e) => return HttpResponse::InternalServerError().body(e.to_string()),
    };
    let real_path = std::path::PathBuf::from(pair.active_path()).join(&file.relative_path);
    let real_path_str = match real_path.to_str() {
        Some(s) => s.to_string(),
        None => return HttpResponse::InternalServerError().body("Invalid path"),
    };

    match virtual_path::set_virtual_path(
        &repo,
        &symlink_base,
        file_id,
        &body.virtual_path,
        &real_path_str,
    ) {
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
    let symlink_base = resolve_symlink_base(query.symlink_base.clone());

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
    let symlink_base = resolve_symlink_base(query.symlink_base.clone());

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

/// POST /virtual-paths/bulk
///
/// Assign virtual paths to multiple files at once.
/// Each entry requires `file_id` and `virtual_path`; `real_path` is auto-resolved
/// from the drive pair if omitted.
async fn bulk_set_virtual_paths(
    repo: web::Data<Repository>,
    body: web::Json<BulkVirtualPathRequest>,
) -> HttpResponse {
    let symlink_base = resolve_symlink_base(body.symlink_base.clone());

    let mut entries: Vec<(i64, String, String)> = Vec::new();

    for entry in &body.entries {
        let real_path = if let Some(ref rp) = entry.real_path {
            rp.clone()
        } else {
            // Auto-resolve real path from the drive pair
            let file = match repo.get_tracked_file(entry.file_id) {
                Ok(f) => f,
                Err(e) => {
                    return HttpResponse::BadRequest()
                        .body(format!("File #{}: {}", entry.file_id, e))
                }
            };
            let pair = match drive::load_operational_pair(&repo, file.drive_pair_id) {
                Ok(p) => p,
                Err(e) => return HttpResponse::InternalServerError().body(e.to_string()),
            };
            std::path::PathBuf::from(pair.active_path())
                .join(&file.relative_path)
                .to_string_lossy()
                .to_string()
        };
        entries.push((entry.file_id, entry.virtual_path.clone(), real_path));
    }

    let result = virtual_path::bulk_set(&repo, &symlink_base, &entries);
    HttpResponse::Ok().json(BulkResponse {
        succeeded: result.succeeded,
        failed: result
            .failed
            .into_iter()
            .map(|(id, err)| BulkFailureEntry {
                file_id: id,
                error: err,
            })
            .collect(),
    })
}

/// POST /virtual-paths/bulk-from-real
///
/// Assign virtual paths to all files in a folder using a common virtual base path.
async fn bulk_from_real(
    repo: web::Data<Repository>,
    body: web::Json<BulkFromRealRequest>,
) -> HttpResponse {
    let symlink_base = resolve_symlink_base(body.symlink_base.clone());

    let pair = match repo.get_drive_pair(body.drive_pair_id) {
        Ok(p) => p,
        Err(e) => return HttpResponse::NotFound().body(e.to_string()),
    };

    let op_pair = match drive::load_operational_pair(&repo, body.drive_pair_id) {
        Ok(p) => p,
        Err(e) => return HttpResponse::InternalServerError().body(e.to_string()),
    };
    let _ = pair;

    match virtual_path::bulk_from_real(
        &repo,
        &symlink_base,
        body.drive_pair_id,
        &body.folder_path,
        &body.virtual_base,
        op_pair.active_path(),
    ) {
        Ok(result) => HttpResponse::Ok().json(BulkResponse {
            succeeded: result.succeeded,
            failed: result
                .failed
                .into_iter()
                .map(|(id, err)| BulkFailureEntry {
                    file_id: id,
                    error: err,
                })
                .collect(),
        }),
        Err(e) => HttpResponse::BadRequest().body(e.to_string()),
    }
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/virtual-paths")
            .route("/refresh", web::post().to(refresh_symlinks))
            .route("/bulk", web::post().to(bulk_set_virtual_paths))
            .route("/bulk-from-real", web::post().to(bulk_from_real))
            .route("/{file_id}", web::put().to(set_virtual_path))
            .route("/{file_id}", web::delete().to(remove_virtual_path)),
    );
}
