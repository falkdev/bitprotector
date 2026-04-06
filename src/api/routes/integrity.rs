use crate::api::models::ApiError;
use crate::core::drive;
use crate::core::integrity::{self, IntegrityStatus};
use crate::core::integrity_runs;
use crate::db::repository::Repository;
use actix_web::{web, HttpResponse, Responder};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct StartRunBody {
    pub drive_id: Option<i64>,
    pub recover: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct ListRunResultsQuery {
    pub issues_only: Option<bool>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
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
            }
            Err(e) => {
                return HttpResponse::InternalServerError()
                    .json(ApiError::new("INTERNAL_ERROR", &e.to_string()))
            }
        }
    }
    let _ = repo.update_tracked_file_last_integrity_check_at(id);

    HttpResponse::Ok().json(serde_json::json!({
        "file_id": result.file_id,
        "status": integrity_runs::status_str(&result.status),
        "master_valid": result.master_valid,
        "mirror_valid": result.mirror_valid,
        "recovered": recovered,
    }))
}

/// POST /api/v1/integrity/runs — start a batch integrity run
pub async fn start_run(
    repo: web::Data<Repository>,
    body: Option<web::Json<StartRunBody>>,
) -> impl Responder {
    let body = body.map(|b| b.into_inner()).unwrap_or(StartRunBody {
        drive_id: None,
        recover: None,
    });
    match integrity_runs::start_run_async(
        &repo,
        body.drive_id,
        body.recover.unwrap_or(false),
        "api",
    ) {
        Ok(run) => HttpResponse::Accepted().json(run),
        Err(e) if e.to_string().contains("already active") => {
            HttpResponse::Conflict().json(ApiError::new("CONFLICT", &e.to_string()))
        }
        Err(e) => HttpResponse::InternalServerError()
            .json(ApiError::new("INTERNAL_ERROR", &e.to_string())),
    }
}

/// GET /api/v1/integrity/runs/active — active run and progress
pub async fn active_run(repo: web::Data<Repository>) -> impl Responder {
    match repo.get_active_integrity_run() {
        Ok(run) => HttpResponse::Ok().json(serde_json::json!({ "run": run })),
        Err(e) => HttpResponse::InternalServerError()
            .json(ApiError::new("INTERNAL_ERROR", &e.to_string())),
    }
}

/// POST /api/v1/integrity/runs/{id}/stop — request stop for an active run
pub async fn stop_run(repo: web::Data<Repository>, path: web::Path<i64>) -> impl Responder {
    let run_id = path.into_inner();
    match repo.get_integrity_run(run_id) {
        Ok(_) => {}
        Err(_) => {
            return HttpResponse::NotFound().json(ApiError::new(
                "RESOURCE_NOT_FOUND",
                &format!("Integrity run {} not found", run_id),
            ))
        }
    }

    if let Err(e) = repo.request_integrity_run_stop(run_id) {
        return HttpResponse::InternalServerError()
            .json(ApiError::new("INTERNAL_ERROR", &e.to_string()));
    }
    match repo.get_integrity_run(run_id) {
        Ok(run) => HttpResponse::Ok().json(run),
        Err(e) => HttpResponse::InternalServerError()
            .json(ApiError::new("INTERNAL_ERROR", &e.to_string())),
    }
}

/// GET /api/v1/integrity/runs/latest — latest run summary and paged results
pub async fn latest_run(
    repo: web::Data<Repository>,
    query: web::Query<ListRunResultsQuery>,
) -> impl Responder {
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(50).clamp(1, 200);
    let issues_only = query.issues_only.unwrap_or(true);

    let run = match repo.get_latest_integrity_run() {
        Ok(run) => run,
        Err(e) => {
            return HttpResponse::InternalServerError()
                .json(ApiError::new("INTERNAL_ERROR", &e.to_string()))
        }
    };
    let Some(run) = run else {
        return HttpResponse::Ok().json(serde_json::json!({
            "run": serde_json::Value::Null,
            "results": [],
            "total": 0,
            "page": page,
            "per_page": per_page
        }));
    };

    match repo.list_integrity_run_results(run.id, issues_only, page, per_page) {
        Ok((results, total)) => HttpResponse::Ok().json(serde_json::json!({
            "run": run,
            "results": results,
            "total": total,
            "page": page,
            "per_page": per_page
        })),
        Err(e) => HttpResponse::InternalServerError()
            .json(ApiError::new("INTERNAL_ERROR", &e.to_string())),
    }
}

/// GET /api/v1/integrity/runs/{id}/results — paged results for a specific run
pub async fn run_results(
    repo: web::Data<Repository>,
    path: web::Path<i64>,
    query: web::Query<ListRunResultsQuery>,
) -> impl Responder {
    let run_id = path.into_inner();
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(50).clamp(1, 200);
    let issues_only = query.issues_only.unwrap_or(true);

    let run = match repo.get_integrity_run(run_id) {
        Ok(run) => run,
        Err(_) => {
            return HttpResponse::NotFound().json(ApiError::new(
                "RESOURCE_NOT_FOUND",
                &format!("Integrity run {} not found", run_id),
            ))
        }
    };

    match repo.list_integrity_run_results(run_id, issues_only, page, per_page) {
        Ok((results, total)) => HttpResponse::Ok().json(serde_json::json!({
            "run": run,
            "results": results,
            "total": total,
            "page": page,
            "per_page": per_page
        })),
        Err(e) => HttpResponse::InternalServerError()
            .json(ApiError::new("INTERNAL_ERROR", &e.to_string())),
    }
}

/// Register integrity routes on an actix-web ServiceConfig.
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/integrity")
            .route("/check/{id}", web::post().to(check_file))
            .route("/runs", web::post().to(start_run))
            .route("/runs/active", web::get().to(active_run))
            .route("/runs/latest", web::get().to(latest_run))
            .route("/runs/{id}/stop", web::post().to(stop_run))
            .route("/runs/{id}/results", web::get().to(run_results)),
    );
}
