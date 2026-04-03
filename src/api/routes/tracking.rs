use crate::api::models::ApiError;
use crate::db::repository::Repository;
use actix_web::{web, HttpResponse, Responder};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ListTrackingItemsQuery {
    pub drive_id: Option<i64>,
    pub q: Option<String>,
    pub virtual_prefix: Option<String>,
    pub has_virtual_path: Option<bool>,
    pub item_kind: Option<String>,
    pub source: Option<String>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

pub async fn list_tracking_items(
    repo: web::Data<Repository>,
    query: web::Query<ListTrackingItemsQuery>,
) -> impl Responder {
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(50).clamp(1, 200);

    match repo.list_tracking_items(
        query.drive_id,
        query.q.as_deref(),
        query.virtual_prefix.as_deref(),
        query.has_virtual_path,
        query.item_kind.as_deref(),
        query.source.as_deref(),
        page,
        per_page,
    ) {
        Ok((items, total)) => HttpResponse::Ok().json(serde_json::json!({
            "items": items,
            "total": total,
            "page": page,
            "per_page": per_page
        })),
        Err(e) => HttpResponse::InternalServerError()
            .json(ApiError::new("INTERNAL_ERROR", &e.to_string())),
    }
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("/tracking").route("/items", web::get().to(list_tracking_items)));
}
