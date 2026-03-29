use crate::api::models::ApiError;
use crate::core::drive::{self, DriveRole};
use crate::core::mirror::validate_drive_pair;
use crate::db::repository::Repository;
use actix_web::{web, HttpResponse, Responder};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct CreateDrivePairRequest {
    pub name: String,
    pub primary_path: String,
    pub secondary_path: String,
    #[serde(default)]
    pub skip_validation: bool,
}

#[derive(Debug, Deserialize)]
pub struct UpdateDrivePairRequest {
    pub name: Option<String>,
    pub primary_path: Option<String>,
    pub secondary_path: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ReplaceRoleRequest {
    pub role: String,
}

#[derive(Debug, Deserialize)]
pub struct AssignReplacementRequest {
    pub role: String,
    pub new_path: String,
    #[serde(default)]
    pub skip_validation: bool,
}

fn parse_role(role: &str) -> anyhow::Result<DriveRole> {
    match role {
        "primary" => Ok(DriveRole::Primary),
        "secondary" => Ok(DriveRole::Secondary),
        other => anyhow::bail!("Unknown drive role '{}'", other),
    }
}

/// POST /api/v1/drives — create a new drive pair
pub async fn create_drive_pair(
    repo: web::Data<Repository>,
    body: web::Json<CreateDrivePairRequest>,
) -> impl Responder {
    if !body.skip_validation {
        if let Err(e) = validate_drive_pair(&body.primary_path, &body.secondary_path) {
            return HttpResponse::BadRequest()
                .json(ApiError::new("VALIDATION_ERROR", &e.to_string()));
        }
    }
    match repo.create_drive_pair(&body.name, &body.primary_path, &body.secondary_path) {
        Ok(pair) => HttpResponse::Created().json(pair),
        Err(e) => HttpResponse::InternalServerError()
            .json(ApiError::new("INTERNAL_ERROR", &e.to_string())),
    }
}

/// GET /api/v1/drives — list all drive pairs
pub async fn list_drive_pairs(repo: web::Data<Repository>) -> impl Responder {
    match repo.list_drive_pairs() {
        Ok(pairs) => HttpResponse::Ok().json(pairs),
        Err(e) => HttpResponse::InternalServerError()
            .json(ApiError::new("INTERNAL_ERROR", &e.to_string())),
    }
}

/// GET /api/v1/drives/{id} — get a single drive pair
pub async fn get_drive_pair(repo: web::Data<Repository>, path: web::Path<i64>) -> impl Responder {
    let id = path.into_inner();
    match repo.get_drive_pair(id) {
        Ok(pair) => HttpResponse::Ok().json(pair),
        Err(_) => HttpResponse::NotFound().json(ApiError::new(
            "RESOURCE_NOT_FOUND",
            &format!("Drive pair {} not found", id),
        )),
    }
}

/// PUT /api/v1/drives/{id} — update a drive pair
pub async fn update_drive_pair(
    repo: web::Data<Repository>,
    path: web::Path<i64>,
    body: web::Json<UpdateDrivePairRequest>,
) -> impl Responder {
    let id = path.into_inner();
    match repo.update_drive_pair(
        id,
        body.name.as_deref(),
        body.primary_path.as_deref(),
        body.secondary_path.as_deref(),
    ) {
        Ok(pair) => HttpResponse::Ok().json(pair),
        Err(e) => HttpResponse::InternalServerError()
            .json(ApiError::new("INTERNAL_ERROR", &e.to_string())),
    }
}

/// DELETE /api/v1/drives/{id} — delete a drive pair
pub async fn delete_drive_pair(
    repo: web::Data<Repository>,
    path: web::Path<i64>,
) -> impl Responder {
    let id = path.into_inner();
    match repo.delete_drive_pair(id) {
        Ok(()) => HttpResponse::NoContent().finish(),
        Err(e) => {
            HttpResponse::BadRequest().json(ApiError::new("VALIDATION_ERROR", &e.to_string()))
        }
    }
}

pub async fn mark_replacement(
    repo: web::Data<Repository>,
    path: web::Path<i64>,
    body: web::Json<ReplaceRoleRequest>,
) -> impl Responder {
    let id = path.into_inner();
    let role = match parse_role(&body.role) {
        Ok(role) => role,
        Err(e) => {
            return HttpResponse::BadRequest()
                .json(ApiError::new("VALIDATION_ERROR", &e.to_string()))
        }
    };
    match drive::mark_drive_quiescing(&repo, id, role) {
        Ok(pair) => HttpResponse::Ok().json(pair),
        Err(e) => {
            HttpResponse::BadRequest().json(ApiError::new("VALIDATION_ERROR", &e.to_string()))
        }
    }
}

pub async fn confirm_replacement(
    repo: web::Data<Repository>,
    path: web::Path<i64>,
    body: web::Json<ReplaceRoleRequest>,
) -> impl Responder {
    let id = path.into_inner();
    let role = match parse_role(&body.role) {
        Ok(role) => role,
        Err(e) => {
            return HttpResponse::BadRequest()
                .json(ApiError::new("VALIDATION_ERROR", &e.to_string()))
        }
    };
    match drive::confirm_drive_failure(&repo, id, role) {
        Ok(pair) => HttpResponse::Ok().json(pair),
        Err(e) => {
            HttpResponse::BadRequest().json(ApiError::new("VALIDATION_ERROR", &e.to_string()))
        }
    }
}

pub async fn cancel_replacement(
    repo: web::Data<Repository>,
    path: web::Path<i64>,
    body: web::Json<ReplaceRoleRequest>,
) -> impl Responder {
    let id = path.into_inner();
    let role = match parse_role(&body.role) {
        Ok(role) => role,
        Err(e) => {
            return HttpResponse::BadRequest()
                .json(ApiError::new("VALIDATION_ERROR", &e.to_string()))
        }
    };
    match drive::cancel_drive_quiescing(&repo, id, role) {
        Ok(pair) => HttpResponse::Ok().json(pair),
        Err(e) => {
            HttpResponse::BadRequest().json(ApiError::new("VALIDATION_ERROR", &e.to_string()))
        }
    }
}

pub async fn assign_replacement(
    repo: web::Data<Repository>,
    path: web::Path<i64>,
    body: web::Json<AssignReplacementRequest>,
) -> impl Responder {
    let id = path.into_inner();
    let role = match parse_role(&body.role) {
        Ok(role) => role,
        Err(e) => {
            return HttpResponse::BadRequest()
                .json(ApiError::new("VALIDATION_ERROR", &e.to_string()))
        }
    };
    let pair = match repo.get_drive_pair(id) {
        Ok(pair) => pair,
        Err(_) => {
            return HttpResponse::NotFound().json(ApiError::new(
                "RESOURCE_NOT_FOUND",
                &format!("Drive pair {} not found", id),
            ))
        }
    };

    if !body.skip_validation {
        let validation = match role {
            DriveRole::Primary => validate_drive_pair(&body.new_path, &pair.secondary_path),
            DriveRole::Secondary => validate_drive_pair(&pair.primary_path, &body.new_path),
        };
        if let Err(e) = validation {
            return HttpResponse::BadRequest()
                .json(ApiError::new("VALIDATION_ERROR", &e.to_string()));
        }
    }

    match drive::assign_replacement_drive(&repo, id, role, &body.new_path) {
        Ok((pair, queued)) => HttpResponse::Ok().json(serde_json::json!({
            "drive_pair": pair,
            "queued_rebuild_items": queued,
        })),
        Err(e) => {
            HttpResponse::BadRequest().json(ApiError::new("VALIDATION_ERROR", &e.to_string()))
        }
    }
}

/// POST /api/v1/drives/{id}/failover — trigger an emergency failover for a drive pair.
///
/// This endpoint evaluates the pair's current active root. If the active drive
/// is unavailable and the standby is healthy, it immediately switches
/// `active_role` to the standby and retargets all virtual-path symlinks.
/// If no failover is needed (both drives are reachable) the endpoint returns
/// the unchanged pair with `"failover_performed": false`.
pub async fn emergency_failover(
    repo: web::Data<Repository>,
    path: web::Path<i64>,
) -> impl Responder {
    let id = path.into_inner();
    let pair = match repo.get_drive_pair(id) {
        Ok(p) => p,
        Err(_) => {
            return HttpResponse::NotFound().json(ApiError::new(
                "RESOURCE_NOT_FOUND",
                &format!("Drive pair {} not found", id),
            ))
        }
    };
    let previous_role = pair.active_role.clone();
    match drive::maybe_emergency_failover(&repo, &pair) {
        Ok(updated) => {
            let failover_performed = updated.active_role != previous_role;
            HttpResponse::Ok().json(serde_json::json!({
                "drive_pair": updated,
                "failover_performed": failover_performed,
            }))
        }
        Err(e) => HttpResponse::InternalServerError()
            .json(ApiError::new("INTERNAL_ERROR", &e.to_string())),
    }
}

/// Register drive pair routes on an actix-web ServiceConfig.
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/drives")
            .route("", web::post().to(create_drive_pair))
            .route("", web::get().to(list_drive_pairs))
            .route("/{id}", web::get().to(get_drive_pair))
            .route("/{id}", web::put().to(update_drive_pair))
            .route("/{id}", web::delete().to(delete_drive_pair))
            .route("/{id}/failover", web::post().to(emergency_failover))
            .route("/{id}/replacement/mark", web::post().to(mark_replacement))
            .route(
                "/{id}/replacement/confirm",
                web::post().to(confirm_replacement),
            )
            .route(
                "/{id}/replacement/cancel",
                web::post().to(cancel_replacement),
            )
            .route(
                "/{id}/replacement/assign",
                web::post().to(assign_replacement),
            ),
    );
}
