use crate::api::models::ApiError;
use crate::core::scheduler::Scheduler;
use crate::db::backup;
use crate::db::repository::{DbBackupConfig, Repository};
use actix_web::{web, HttpResponse};
use serde::{Deserialize, Deserializer, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct DatabasePath(pub String);

fn double_option<'de, T, D>(d: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Ok(Some(Option::deserialize(d)?))
}

#[derive(Serialize)]
struct BackupConfigResponse {
    id: i64,
    backup_path: String,
    drive_label: Option<String>,
    priority: i64,
    enabled: bool,
    last_backup: Option<String>,
    last_integrity_check: Option<String>,
    last_integrity_status: Option<String>,
    last_error: Option<String>,
    created_at: String,
}

impl From<DbBackupConfig> for BackupConfigResponse {
    fn from(c: DbBackupConfig) -> Self {
        BackupConfigResponse {
            id: c.id,
            backup_path: c.backup_path,
            drive_label: c.drive_label,
            priority: c.priority,
            enabled: c.enabled,
            last_backup: c.last_backup,
            last_integrity_check: c.last_integrity_check,
            last_integrity_status: c.last_integrity_status,
            last_error: c.last_error,
            created_at: c.created_at,
        }
    }
}

#[derive(Deserialize)]
struct CreateBackupBody {
    backup_path: String,
    drive_label: Option<String>,
    #[serde(default = "default_enabled")]
    enabled: bool,
}

fn default_enabled() -> bool {
    true
}

#[derive(Deserialize)]
struct UpdateBackupBody {
    backup_path: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    drive_label: Option<Option<String>>,
    enabled: Option<bool>,
}

#[derive(Deserialize)]
struct UpdateBackupSettingsBody {
    backup_enabled: Option<bool>,
    backup_interval_seconds: Option<i64>,
    integrity_enabled: Option<bool>,
    integrity_interval_seconds: Option<i64>,
}

#[derive(Deserialize)]
struct RestoreBackupBody {
    source_path: String,
}

fn validate_interval(value: Option<i64>, name: &str) -> Result<(), HttpResponse> {
    if let Some(seconds) = value {
        if seconds <= 0 {
            return Err(HttpResponse::BadRequest().json(ApiError::new(
                "validation_error",
                &format!("{} must be greater than zero", name),
            )));
        }
    }
    Ok(())
}

async fn list_backups(data: web::Data<Repository>) -> HttpResponse {
    match data.list_db_backup_configs() {
        Ok(configs) => {
            let resp: Vec<BackupConfigResponse> = configs.into_iter().map(Into::into).collect();
            HttpResponse::Ok().json(resp)
        }
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

async fn create_backup(
    data: web::Data<Repository>,
    body: web::Json<CreateBackupBody>,
) -> HttpResponse {
    if body.backup_path.trim().is_empty() {
        return HttpResponse::BadRequest()
            .json(ApiError::new("validation_error", "backup_path is required"));
    }

    match data.create_db_backup_config(
        body.backup_path.trim(),
        body.drive_label.as_deref(),
        body.enabled,
    ) {
        Ok(cfg) => HttpResponse::Created().json(BackupConfigResponse::from(cfg)),
        Err(e) => HttpResponse::BadRequest().body(e.to_string()),
    }
}

async fn get_backup(data: web::Data<Repository>, path: web::Path<i64>) -> HttpResponse {
    match data.get_db_backup_config(*path) {
        Ok(cfg) => HttpResponse::Ok().json(BackupConfigResponse::from(cfg)),
        Err(_) => HttpResponse::NotFound()
            .json(ApiError::new("not_found", "Backup configuration not found")),
    }
}

async fn update_backup(
    data: web::Data<Repository>,
    path: web::Path<i64>,
    body: web::Json<UpdateBackupBody>,
) -> HttpResponse {
    let backup_path = body.backup_path.as_deref().map(str::trim);
    if backup_path.map_or(false, str::is_empty) {
        return HttpResponse::BadRequest()
            .json(ApiError::new("validation_error", "backup_path cannot be empty"));
    }
    let drive_label = body.drive_label.as_ref().map(|opt| opt.as_deref());
    match data.update_db_backup_config(
        *path,
        backup_path,
        drive_label,
        body.enabled,
    ) {
        Ok(cfg) => HttpResponse::Ok().json(BackupConfigResponse::from(cfg)),
        Err(e) => HttpResponse::BadRequest().body(e.to_string()),
    }
}

async fn delete_backup(data: web::Data<Repository>, path: web::Path<i64>) -> HttpResponse {
    match data.delete_db_backup_config(*path) {
        Ok(_) => HttpResponse::NoContent().finish(),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

async fn run_backups(
    data: web::Data<Repository>,
    db_path: web::Data<DatabasePath>,
) -> HttpResponse {
    match backup::run_all_backups(&data, &db_path.0) {
        Ok(results) => HttpResponse::Ok().json(results),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

async fn get_settings(data: web::Data<Repository>) -> HttpResponse {
    match data.get_db_backup_settings() {
        Ok(settings) => HttpResponse::Ok().json(settings),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

async fn update_settings(
    data: web::Data<Repository>,
    scheduler: web::Data<Arc<Mutex<Scheduler>>>,
    body: web::Json<UpdateBackupSettingsBody>,
) -> HttpResponse {
    if let Err(resp) = validate_interval(body.backup_interval_seconds, "backup_interval_seconds") {
        return resp;
    }
    if let Err(resp) = validate_interval(
        body.integrity_interval_seconds,
        "integrity_interval_seconds",
    ) {
        return resp;
    }

    match data.update_db_backup_settings(
        body.backup_enabled,
        body.backup_interval_seconds,
        body.integrity_enabled,
        body.integrity_interval_seconds,
    ) {
        Ok(settings) => {
            if let Ok(mut sched) = scheduler.lock() {
                if let Err(e) = sched.reload() {
                    tracing::error!(
                        "Failed to reload scheduler after backup settings change: {}",
                        e
                    );
                }
            }
            HttpResponse::Ok().json(settings)
        }
        Err(e) => HttpResponse::BadRequest().body(e.to_string()),
    }
}

async fn integrity_check(data: web::Data<Repository>) -> HttpResponse {
    match backup::run_backup_integrity_check(&data) {
        Ok(results) => HttpResponse::Ok().json(results),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

async fn restore_backup(
    db_path: web::Data<DatabasePath>,
    body: web::Json<RestoreBackupBody>,
) -> HttpResponse {
    if body.source_path.trim().is_empty() {
        return HttpResponse::BadRequest()
            .json(ApiError::new("validation_error", "source_path is required"));
    }

    match backup::stage_restore(&db_path.0, body.source_path.trim()) {
        Ok(result) => HttpResponse::Ok().json(result),
        Err(e) => HttpResponse::BadRequest().json(ApiError::new("restore_failed", &e.to_string())),
    }
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/database/backups")
            .route("", web::get().to(list_backups))
            .route("", web::post().to(create_backup))
            .route("/settings", web::get().to(get_settings))
            .route("/settings", web::put().to(update_settings))
            .route("/run", web::post().to(run_backups))
            .route("/integrity-check", web::post().to(integrity_check))
            .route("/restore", web::post().to(restore_backup))
            .route("/{id}", web::get().to(get_backup))
            .route("/{id}", web::put().to(update_backup))
            .route("/{id}", web::delete().to(delete_backup)),
    );
}
