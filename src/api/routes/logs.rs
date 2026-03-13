use actix_web::{web, HttpResponse};
use serde::Deserialize;
use crate::db::repository::Repository;

#[derive(Deserialize)]
pub struct ListLogsQuery {
    pub event_type: Option<String>,
    pub file_id: Option<i64>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

/// GET /logs
async fn list_logs(repo: web::Data<Repository>, query: web::Query<ListLogsQuery>) -> HttpResponse {
    let page = query.page.unwrap_or(1);
    let per_page = query.per_page.unwrap_or(50);
    match repo.list_event_logs(
        query.event_type.as_deref(),
        query.file_id,
        query.from.as_deref(),
        query.to.as_deref(),
        page,
        per_page,
    ) {
        Ok((entries, _)) => HttpResponse::Ok().json(entries),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

/// GET /logs/{id}
async fn get_log(repo: web::Data<Repository>, path: web::Path<i64>) -> HttpResponse {
    match repo.get_event_log(path.into_inner()) {
        Ok(entry) => HttpResponse::Ok().json(entry),
        Err(e) => HttpResponse::NotFound().body(e.to_string()),
    }
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/logs")
            .route("", web::get().to(list_logs))
            .route("/{id}", web::get().to(get_log)),
    );
}
