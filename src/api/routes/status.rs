use actix_web::{web, HttpResponse};

use crate::api::models::ApiError;
use crate::db::repository::Repository;

pub async fn get_status(repo: web::Data<Repository>) -> HttpResponse {
    let base = match repo.get_system_status() {
        Ok(status) => status,
        Err(e) => {
            return HttpResponse::InternalServerError()
                .json(ApiError::new("INTERNAL_ERROR", &e.to_string()))
        }
    };
    let pairs = match repo.list_drive_pairs() {
        Ok(pairs) => pairs,
        Err(e) => {
            return HttpResponse::InternalServerError()
                .json(ApiError::new("INTERNAL_ERROR", &e.to_string()))
        }
    };

    let degraded_pairs = pairs.iter().filter(|pair| pair.is_degraded()).count();
    let active_secondary_pairs = pairs
        .iter()
        .filter(|pair| pair.active_role == "secondary")
        .count();
    let rebuilding_pairs = pairs
        .iter()
        .filter(|pair| pair.primary_state == "rebuilding" || pair.secondary_state == "rebuilding")
        .count();
    let quiescing_pairs = pairs
        .iter()
        .filter(|pair| pair.primary_state == "quiescing" || pair.secondary_state == "quiescing")
        .count();

    HttpResponse::Ok().json(serde_json::json!({
        "files_tracked": base.files_tracked,
        "files_mirrored": base.files_mirrored,
        "pending_sync": base.pending_sync,
        "integrity_issues": base.integrity_issues,
        "drive_pairs": base.drive_pairs,
        "degraded_pairs": degraded_pairs,
        "active_secondary_pairs": active_secondary_pairs,
        "rebuilding_pairs": rebuilding_pairs,
        "quiescing_pairs": quiescing_pairs,
    }))
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/status", web::get().to(get_status));
}
