use crate::core::virtual_path;
use crate::db::repository::Repository;
use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Deserialize)]
pub struct SetVirtualPathRequest {
    pub virtual_path: String,
}

#[derive(Deserialize)]
pub struct BulkVirtualPathEntry {
    pub file_id: i64,
    pub virtual_path: String,
}

#[derive(Deserialize)]
pub struct BulkVirtualPathRequest {
    pub entries: Vec<BulkVirtualPathEntry>,
}

#[derive(Deserialize)]
pub struct BulkFromRealRequest {
    pub drive_pair_id: i64,
    pub folder_path: String,
    pub publish_root: String,
}

#[derive(Deserialize)]
struct TreeQuery {
    parent: Option<String>,
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

#[derive(Serialize)]
struct TreeResponse {
    parent: String,
    children: Vec<crate::db::repository::VirtualPathTreeNode>,
}

/// PUT /virtual-paths/{file_id}
async fn set_virtual_path(
    repo: web::Data<Repository>,
    path: web::Path<i64>,
    body: web::Json<SetVirtualPathRequest>,
) -> HttpResponse {
    let file_id = path.into_inner();
    match virtual_path::set_virtual_path(&repo, file_id, &body.virtual_path) {
        Ok(_) => HttpResponse::Ok().body(format!("Virtual path set for file #{}", file_id)),
        Err(e) => HttpResponse::BadRequest().body(e.to_string()),
    }
}

/// DELETE /virtual-paths/{file_id}
async fn remove_virtual_path(repo: web::Data<Repository>, path: web::Path<i64>) -> HttpResponse {
    let file_id = path.into_inner();
    match virtual_path::remove_virtual_path(&repo, file_id) {
        Ok(_) => HttpResponse::Ok().body(format!("Virtual path removed for file #{}", file_id)),
        Err(e) => HttpResponse::BadRequest().body(e.to_string()),
    }
}

/// POST /virtual-paths/refresh
async fn refresh_symlinks(repo: web::Data<Repository>) -> HttpResponse {
    let pairs = match repo.list_drive_pairs() {
        Ok(v) => v,
        Err(e) => return HttpResponse::InternalServerError().body(e.to_string()),
    };
    let pairs_map: HashMap<i64, _> = pairs.into_iter().map(|p| (p.id, p)).collect();

    match virtual_path::refresh_all_virtual_paths(&repo, &pairs_map) {
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
    let entries: Vec<_> = body
        .entries
        .iter()
        .map(|entry| (entry.file_id, entry.virtual_path.clone()))
        .collect();
    let result = virtual_path::bulk_set(&repo, &entries);
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
/// Assign publish paths to all tracked files in a folder using a common publish root.
async fn bulk_from_real(
    repo: web::Data<Repository>,
    body: web::Json<BulkFromRealRequest>,
) -> HttpResponse {
    match virtual_path::bulk_from_real(
        &repo,
        body.drive_pair_id,
        &body.folder_path,
        &body.publish_root,
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

/// GET /virtual-paths/tree
///
/// Return one level of path-segment children under a parent publish prefix.
async fn virtual_path_tree(
    repo: web::Data<Repository>,
    query: web::Query<TreeQuery>,
) -> HttpResponse {
    let parent = query
        .parent
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("/");

    if !parent.starts_with('/') {
        return HttpResponse::BadRequest().body("Parent publish prefix must be absolute");
    }
    if parent.contains("..") {
        return HttpResponse::BadRequest().body("Parent publish prefix may not contain '..'");
    }

    match repo.list_virtual_path_tree_nodes(parent) {
        Ok(children) => HttpResponse::Ok().json(TreeResponse {
            parent: parent.to_string(),
            children,
        }),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/virtual-paths")
            .route("/tree", web::get().to(virtual_path_tree))
            .route("/refresh", web::post().to(refresh_symlinks))
            .route("/bulk", web::post().to(bulk_set_virtual_paths))
            .route("/bulk-from-real", web::post().to(bulk_from_real))
            .route("/{file_id}", web::put().to(set_virtual_path))
            .route("/{file_id}", web::delete().to(remove_virtual_path)),
    );
}
