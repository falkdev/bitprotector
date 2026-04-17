use crate::api::models::ApiError;
use crate::api::path_resolution::{resolve_path_within_drive_root, PathTargetKind};
use crate::core::{drive, mirror, tracker, virtual_path};
use crate::db::repository::Repository;
use crate::logging::event_logger;
use actix_web::{web, HttpResponse, Responder};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct TrackFileRequest {
    pub drive_pair_id: i64,
    pub relative_path: String,
    pub virtual_path: Option<String>,
    pub mirror: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct ListFilesQuery {
    pub drive_id: Option<i64>,
    pub virtual_prefix: Option<String>,
    pub mirrored: Option<bool>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

/// POST /api/v1/files — track a new file
pub async fn track_file(
    repo: web::Data<Repository>,
    body: web::Json<TrackFileRequest>,
) -> impl Responder {
    let pair = match drive::load_operational_pair(&repo, body.drive_pair_id) {
        Ok(p) => p,
        Err(_) => {
            return HttpResponse::NotFound()
                .json(ApiError::new("RESOURCE_NOT_FOUND", "Drive pair not found"))
        }
    };
    let relative_path = match resolve_path_within_drive_root(
        pair.active_path(),
        &body.relative_path,
        PathTargetKind::File,
    ) {
        Ok(path) => path,
        Err(e) => {
            return HttpResponse::BadRequest()
                .json(ApiError::new("VALIDATION_ERROR", &e.to_string()))
        }
    };
    let existed_before = repo
        .get_tracked_file_by_path(pair.id, &relative_path)
        .is_ok();
    let tracked =
        match tracker::track_file(&repo, &pair, &relative_path, body.virtual_path.as_deref()) {
            Ok(t) => t,
            Err(e) => {
                return HttpResponse::BadRequest()
                    .json(ApiError::new("VALIDATION_ERROR", &e.to_string()))
            }
        };
    if !existed_before && pair.standby_accepts_sync() {
        if let Err(e) = repo.create_sync_queue_item_dedup(tracked.id, "mirror") {
            return HttpResponse::InternalServerError()
                .json(ApiError::new("INTERNAL_ERROR", &e.to_string()));
        }
    }
    if existed_before {
        HttpResponse::Ok().json(tracked)
    } else {
        HttpResponse::Created().json(tracked)
    }
}

/// GET /api/v1/files — list tracked files
pub async fn list_files(
    repo: web::Data<Repository>,
    query: web::Query<ListFilesQuery>,
) -> impl Responder {
    let page = query.page.unwrap_or(1);
    let per_page = query.per_page.unwrap_or(50);
    match repo.list_tracked_files(
        query.drive_id,
        query.virtual_prefix.as_deref(),
        query.mirrored,
        page,
        per_page,
    ) {
        Ok((files, total)) => {
            let body = serde_json::json!({ "files": files, "total": total, "page": page, "per_page": per_page });
            HttpResponse::Ok().json(body)
        }
        Err(e) => HttpResponse::InternalServerError()
            .json(ApiError::new("INTERNAL_ERROR", &e.to_string())),
    }
}

/// GET /api/v1/files/{id} — get a single tracked file
pub async fn get_file(repo: web::Data<Repository>, path: web::Path<i64>) -> impl Responder {
    let id = path.into_inner();
    match repo.get_tracked_file(id) {
        Ok(file) => HttpResponse::Ok().json(file),
        Err(_) => HttpResponse::NotFound().json(ApiError::new(
            "RESOURCE_NOT_FOUND",
            &format!("Tracked file {} not found", id),
        )),
    }
}

/// POST /api/v1/files/{id}/mirror — mirror a file to secondary
pub async fn mirror_file(repo: web::Data<Repository>, path: web::Path<i64>) -> impl Responder {
    let id = path.into_inner();
    let file = match repo.get_tracked_file(id) {
        Ok(f) => f,
        Err(_) => {
            return HttpResponse::NotFound().json(ApiError::new(
                "RESOURCE_NOT_FOUND",
                &format!("Tracked file {} not found", id),
            ))
        }
    };
    let pair = match drive::load_operational_pair(&repo, file.drive_pair_id) {
        Ok(p) => p,
        Err(e) => {
            return HttpResponse::InternalServerError()
                .json(ApiError::new("INTERNAL_ERROR", &e.to_string()))
        }
    };
    if let Err(e) = mirror::mirror_file(&pair, &file.relative_path) {
        return HttpResponse::InternalServerError()
            .json(ApiError::new("INTERNAL_ERROR", &e.to_string()));
    }
    match repo.update_tracked_file_mirror_status(id, true) {
        Ok(()) => {
            let _ = repo.complete_pending_mirror_queue_for_file(id);
            HttpResponse::Ok().json(serde_json::json!({ "mirrored": true }))
        }
        Err(e) => HttpResponse::InternalServerError()
            .json(ApiError::new("INTERNAL_ERROR", &e.to_string())),
    }
}

/// DELETE /api/v1/files/{id} — untrack a file
pub async fn delete_file(repo: web::Data<Repository>, path: web::Path<i64>) -> impl Responder {
    let id = path.into_inner();
    let file = match repo.get_tracked_file(id) {
        Ok(file) => file,
        Err(_) => {
            return HttpResponse::NotFound().json(ApiError::new(
                "RESOURCE_NOT_FOUND",
                &format!("Tracked file {} not found", id),
            ))
        }
    };
    if file.virtual_path.is_some() {
        if let Err(e) = virtual_path::remove_virtual_path(&repo, id) {
            return HttpResponse::BadRequest()
                .json(ApiError::new("VALIDATION_ERROR", &e.to_string()));
        }
    }
    match repo.delete_tracked_file(id) {
        Ok(()) => {
            let full_path = repo.get_drive_pair(file.drive_pair_id)
                .map(|dp| format!("{}/{}", dp.primary_path, file.relative_path))
                .unwrap_or_else(|_| file.relative_path.clone());
            let _ = event_logger::log_file_untracked(&repo, id, &full_path);
            HttpResponse::NoContent().finish()
        }
        Err(e) => {
            HttpResponse::BadRequest().json(ApiError::new("VALIDATION_ERROR", &e.to_string()))
        }
    }
}

/// Register file tracking routes on an actix-web ServiceConfig.
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/files")
            .route("", web::post().to(track_file))
            .route("", web::get().to(list_files))
            .route("/{id}", web::get().to(get_file))
            .route("/{id}/mirror", web::post().to(mirror_file))
            .route("/{id}", web::delete().to(delete_file)),
    );
}
