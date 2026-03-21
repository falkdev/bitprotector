use crate::db::backup;
use crate::db::repository::{DbBackupConfig, Repository};
use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct BackupConfigResponse {
    id: i64,
    backup_path: String,
    drive_label: Option<String>,
    max_copies: i64,
    enabled: bool,
    last_backup: Option<String>,
    created_at: String,
}

impl From<DbBackupConfig> for BackupConfigResponse {
    fn from(c: DbBackupConfig) -> Self {
        BackupConfigResponse {
            id: c.id,
            backup_path: c.backup_path,
            drive_label: c.drive_label,
            max_copies: c.max_copies,
            enabled: c.enabled,
            last_backup: c.last_backup,
            created_at: c.created_at,
        }
    }
}

#[derive(Deserialize)]
struct CreateBackupBody {
    backup_path: String,
    drive_label: Option<String>,
    #[serde(default = "default_max_copies")]
    max_copies: i64,
    #[serde(default = "default_enabled")]
    enabled: bool,
}

fn default_max_copies() -> i64 {
    5
}
fn default_enabled() -> bool {
    true
}

#[derive(Deserialize)]
struct UpdateBackupBody {
    max_copies: Option<i64>,
    enabled: Option<bool>,
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
    match data.create_db_backup_config(
        &body.backup_path,
        body.drive_label.as_deref(),
        body.max_copies,
        body.enabled,
    ) {
        Ok(cfg) => HttpResponse::Created().json(BackupConfigResponse::from(cfg)),
        Err(e) => HttpResponse::BadRequest().body(e.to_string()),
    }
}

async fn get_backup(data: web::Data<Repository>, path: web::Path<i64>) -> HttpResponse {
    match data.get_db_backup_config(*path) {
        Ok(cfg) => HttpResponse::Ok().json(BackupConfigResponse::from(cfg)),
        Err(_) => HttpResponse::NotFound().body("Backup configuration not found"),
    }
}

async fn update_backup(
    data: web::Data<Repository>,
    path: web::Path<i64>,
    body: web::Json<UpdateBackupBody>,
) -> HttpResponse {
    match data.update_db_backup_config(*path, body.max_copies, body.enabled) {
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

#[derive(Deserialize)]
struct RunBackupQuery {
    db_path: String,
}

async fn run_backups(
    data: web::Data<Repository>,
    query: web::Query<RunBackupQuery>,
) -> HttpResponse {
    match backup::run_all_backups(&data, &query.db_path) {
        Ok(results) => {
            let body: Vec<serde_json::Value> = results
                .iter()
                .map(|r| {
                    serde_json::json!({
                        "backup_config_id": r.backup_config_id,
                        "backup_path": r.backup_path,
                        "status": r.status,
                        "error": r.error,
                    })
                })
                .collect();
            HttpResponse::Ok().json(body)
        }
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/database/backups")
            .route("", web::get().to(list_backups))
            .route("", web::post().to(create_backup))
            .route("/run", web::post().to(run_backups))
            .route("/{id}", web::get().to(get_backup))
            .route("/{id}", web::put().to(update_backup))
            .route("/{id}", web::delete().to(delete_backup)),
    );
}
