use crate::api::models::ApiError;
use crate::core::drive;
use crate::core::integrity::{self, IntegrityStatus};
use crate::db::repository::Repository;
use actix_web::{web, HttpResponse, Responder};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct CheckAllQuery {
    pub drive_id: Option<i64>,
    pub recover: Option<bool>,
}

fn status_str(status: &IntegrityStatus) -> &'static str {
    match status {
        IntegrityStatus::Ok => "ok",
        IntegrityStatus::MasterCorrupted => "master_corrupted",
        IntegrityStatus::MirrorCorrupted => "mirror_corrupted",
        IntegrityStatus::BothCorrupted => "both_corrupted",
        IntegrityStatus::MirrorMissing => "mirror_missing",
        IntegrityStatus::MasterMissing => "master_missing",
        IntegrityStatus::PrimaryDriveUnavailable => "primary_drive_unavailable",
        IntegrityStatus::SecondaryDriveUnavailable => "secondary_drive_unavailable",
    }
}

/// POST /api/v1/integrity/check/{id} — check a single file
pub async fn check_file(
    repo: web::Data<Repository>,
    path: web::Path<i64>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> impl Responder {
    let id = path.into_inner();
    let recover = query.get("recover").map(|v| v == "true").unwrap_or(false);

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
    let result = match integrity::check_file_integrity(&pair, &file) {
        Ok(r) => r,
        Err(e) => {
            return HttpResponse::InternalServerError()
                .json(ApiError::new("INTERNAL_ERROR", &e.to_string()))
        }
    };

    let mut recovered = false;
    if recover && result.status != IntegrityStatus::Ok {
        match integrity::attempt_recovery(&pair, &file, &result) {
            Ok(r) => {
                recovered = r;
                if r {
                    let _ = repo.update_tracked_file_last_verified(id);
                }
            }
            Err(e) => {
                return HttpResponse::InternalServerError()
                    .json(ApiError::new("INTERNAL_ERROR", &e.to_string()))
            }
        }
    } else if result.status == IntegrityStatus::Ok {
        let _ = repo.update_tracked_file_last_verified(id);
    }

    HttpResponse::Ok().json(serde_json::json!({
        "file_id": result.file_id,
        "status": status_str(&result.status),
        "master_valid": result.master_valid,
        "mirror_valid": result.mirror_valid,
        "recovered": recovered,
    }))
}

/// GET /api/v1/integrity/check-all — batch integrity check
pub async fn check_all(
    repo: web::Data<Repository>,
    query: web::Query<CheckAllQuery>,
) -> impl Responder {
    let recover = query.recover.unwrap_or(false);
    let mut results = Vec::new();
    let mut page = 1i64;

    loop {
        let (files, total) = match repo.list_tracked_files(query.drive_id, None, None, page, 100) {
            Ok(r) => r,
            Err(e) => {
                return HttpResponse::InternalServerError()
                    .json(ApiError::new("INTERNAL_ERROR", &e.to_string()))
            }
        };
        if files.is_empty() {
            break;
        }

        for file in &files {
            let pair = match drive::load_operational_pair(&repo, file.drive_pair_id) {
                Ok(p) => p,
                Err(_) => continue,
            };
            if pair.is_quiescing() {
                continue;
            }
            let result = match integrity::check_file_integrity(&pair, file) {
                Ok(r) => r,
                Err(_) => continue,
            };
            let mut recovered = false;
            if recover && result.status != IntegrityStatus::Ok {
                recovered = integrity::attempt_recovery(&pair, file, &result).unwrap_or(false);
                if recovered {
                    let _ = repo.update_tracked_file_last_verified(file.id);
                }
            } else if result.status == IntegrityStatus::Ok {
                let _ = repo.update_tracked_file_last_verified(file.id);
            }
            results.push(serde_json::json!({
                "file_id": result.file_id,
                "status": status_str(&result.status),
                "recovered": recovered,
            }));
        }

        if (page * 100) >= total {
            break;
        }
        page += 1;
    }

    HttpResponse::Ok().json(serde_json::json!({ "results": results }))
}

/// Register integrity routes on an actix-web ServiceConfig.
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/integrity")
            .route("/check/{id}", web::post().to(check_file))
            .route("/check-all", web::get().to(check_all)),
    );
}
