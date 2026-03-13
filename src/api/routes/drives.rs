use actix_web::{web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use crate::db::repository::Repository;
use crate::core::mirror::validate_drive_pair;
use crate::api::models::ApiError;

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

/// POST /api/v1/drives — create a new drive pair
pub async fn create_drive_pair(
    repo: web::Data<Repository>,
    body: web::Json<CreateDrivePairRequest>,
) -> impl Responder {
    if !body.skip_validation {
        if let Err(e) = validate_drive_pair(&body.primary_path, &body.secondary_path) {
            return HttpResponse::BadRequest().json(ApiError::new("VALIDATION_ERROR", &e.to_string()));
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
pub async fn get_drive_pair(
    repo: web::Data<Repository>,
    path: web::Path<i64>,
) -> impl Responder {
    let id = path.into_inner();
    match repo.get_drive_pair(id) {
        Ok(pair) => HttpResponse::Ok().json(pair),
        Err(_) => HttpResponse::NotFound()
            .json(ApiError::new("RESOURCE_NOT_FOUND", &format!("Drive pair {} not found", id))),
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
        Err(e) => HttpResponse::BadRequest()
            .json(ApiError::new("VALIDATION_ERROR", &e.to_string())),
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
            .route("/{id}", web::delete().to(delete_drive_pair)),
    );
}
