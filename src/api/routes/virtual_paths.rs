use crate::core::virtual_path;
use crate::db::repository::Repository;
use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct SetVirtualPathRequest {
    pub virtual_path: String,
}

#[derive(Deserialize)]
struct TreeQuery {
    parent: Option<String>,
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

/// GET /virtual-paths/tree
///
/// Return one level of path-segment children under a parent virtual prefix.
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
        return HttpResponse::BadRequest().body("Parent virtual prefix must be absolute");
    }
    if parent.contains("..") {
        return HttpResponse::BadRequest().body("Parent virtual prefix may not contain '..'");
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
            .route("/{file_id}", web::put().to(set_virtual_path))
            .route("/{file_id}", web::delete().to(remove_virtual_path)),
    );
}
