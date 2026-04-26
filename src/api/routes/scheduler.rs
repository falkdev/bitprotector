use crate::api::models::ApiError;
use crate::core::scheduler::Scheduler;
use crate::db::repository::Repository;
use actix_web::{web, HttpResponse};
use serde::{Deserialize, Deserializer, Serialize};
use std::sync::{Arc, Mutex};

/// Deserializer helper for `Option<Option<T>>` that distinguishes between:
/// - field absent  → `None`           (don't update this field)
/// - field `null`  → `Some(None)`     (explicitly clear the value)
/// - field value   → `Some(Some(v))`  (set to a new value)
fn double_option<'de, T, D>(d: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Ok(Some(Option::deserialize(d)?))
}

#[derive(Deserialize)]
pub struct CreateScheduleRequest {
    pub task_type: String,
    pub cron_expr: Option<String>,
    pub interval_seconds: Option<i64>,
    pub max_duration_seconds: Option<i64>,
    pub enabled: Option<bool>,
}

#[derive(Deserialize)]
pub struct UpdateScheduleRequest {
    #[serde(default, deserialize_with = "double_option")]
    pub cron_expr: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub interval_seconds: Option<Option<i64>>,
    #[serde(default, deserialize_with = "double_option")]
    pub max_duration_seconds: Option<Option<i64>>,
    pub enabled: Option<bool>,
}

#[derive(Serialize)]
struct SchedulesResponse {
    schedules: Vec<crate::db::repository::ScheduleConfig>,
}

fn reload_scheduler(scheduler: &web::Data<Arc<Mutex<Scheduler>>>) {
    if let Ok(mut sched) = scheduler.lock() {
        if let Err(e) = sched.reload() {
            tracing::error!("Failed to reload scheduler after config change: {}", e);
        }
    }
}

fn validate_create_request(
    cron_expr: &Option<String>,
    interval_seconds: &Option<i64>,
) -> Result<(), HttpResponse> {
    if cron_expr.is_none() && interval_seconds.is_none() {
        return Err(HttpResponse::BadRequest().json(ApiError::new(
            "validation_error",
            "At least one of cron_expr or interval_seconds must be provided",
        )));
    }
    Ok(())
}

/// GET /scheduler/schedules
async fn list_schedules(repo: web::Data<Repository>) -> HttpResponse {
    match repo.list_schedule_configs() {
        Ok(schedules) => HttpResponse::Ok().json(SchedulesResponse { schedules }),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

/// POST /scheduler/schedules
async fn create_schedule(
    repo: web::Data<Repository>,
    scheduler: web::Data<Arc<Mutex<Scheduler>>>,
    body: web::Json<CreateScheduleRequest>,
) -> HttpResponse {
    if let Err(resp) = validate_create_request(&body.cron_expr, &body.interval_seconds) {
        return resp;
    }

    let task_type = body.task_type.as_str();
    if task_type != "sync" && task_type != "integrity_check" {
        return HttpResponse::BadRequest().json(ApiError::new(
            "validation_error",
            "task_type must be 'sync' or 'integrity_check'",
        ));
    }

    match repo.create_schedule_config(
        &body.task_type,
        body.cron_expr.as_deref(),
        body.interval_seconds,
        body.enabled.unwrap_or(true),
        body.max_duration_seconds,
    ) {
        Ok(cfg) => {
            reload_scheduler(&scheduler);
            HttpResponse::Created().json(cfg)
        }
        Err(e) => HttpResponse::BadRequest().body(e.to_string()),
    }
}

/// GET /scheduler/schedules/{id}
async fn get_schedule(repo: web::Data<Repository>, path: web::Path<i64>) -> HttpResponse {
    match repo.get_schedule_config(path.into_inner()) {
        Ok(cfg) => HttpResponse::Ok().json(cfg),
        Err(_) => HttpResponse::NotFound().json(ApiError::new("not_found", "Schedule not found")),
    }
}

/// PUT /scheduler/schedules/{id}
async fn update_schedule(
    repo: web::Data<Repository>,
    scheduler: web::Data<Arc<Mutex<Scheduler>>>,
    path: web::Path<i64>,
    body: web::Json<UpdateScheduleRequest>,
) -> HttpResponse {
    let id = path.into_inner();
    let cron_expr: Option<Option<&str>> = body.cron_expr.as_ref().map(|opt| opt.as_deref());

    match repo.update_schedule_config(id, cron_expr, body.interval_seconds, body.enabled, body.max_duration_seconds) {
        Ok(cfg) => {
            reload_scheduler(&scheduler);
            HttpResponse::Ok().json(cfg)
        }
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("no rows") || msg.contains("QueryReturnedNoRows") {
                HttpResponse::NotFound().json(ApiError::new("not_found", "Schedule not found"))
            } else {
                HttpResponse::InternalServerError().body(msg)
            }
        }
    }
}

/// DELETE /scheduler/schedules/{id}
async fn delete_schedule(
    repo: web::Data<Repository>,
    scheduler: web::Data<Arc<Mutex<Scheduler>>>,
    path: web::Path<i64>,
) -> HttpResponse {
    let id = path.into_inner();
    match repo.delete_schedule_config(id) {
        Ok(()) => {
            reload_scheduler(&scheduler);
            HttpResponse::NoContent().finish()
        }
        Err(_) => HttpResponse::NotFound().json(ApiError::new("not_found", "Schedule not found")),
    }
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/scheduler")
            .route("/schedules", web::get().to(list_schedules))
            .route("/schedules", web::post().to(create_schedule))
            .route("/schedules/{id}", web::get().to(get_schedule))
            .route("/schedules/{id}", web::put().to(update_schedule))
            .route("/schedules/{id}", web::delete().to(delete_schedule)),
    );
}
