use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};
use crate::db::repository::Repository;
use crate::core::{sync_queue, scheduler};

#[derive(Deserialize)]
pub struct AddQueueItem {
    pub tracked_file_id: i64,
    pub action: String,
}

#[derive(Deserialize)]
pub struct ListQuery {
    pub status: Option<String>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[derive(Serialize)]
struct ProcessResult {
    processed: u32,
}

#[derive(Serialize)]
struct TaskResult {
    task: String,
    count: u32,
}

/// GET /sync/queue
async fn list_queue(repo: web::Data<Repository>, query: web::Query<ListQuery>) -> HttpResponse {
    let page = query.page.unwrap_or(1);
    let per_page = query.per_page.unwrap_or(50);
    match repo.list_sync_queue(query.status.as_deref(), page, per_page) {
        Ok((items, _)) => HttpResponse::Ok().json(items),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

/// POST /sync/queue
async fn add_queue_item(
    repo: web::Data<Repository>,
    body: web::Json<AddQueueItem>,
) -> HttpResponse {
    match repo.create_sync_queue_item(body.tracked_file_id, &body.action) {
        Ok(item) => HttpResponse::Created().json(item),
        Err(e) => HttpResponse::BadRequest().body(e.to_string()),
    }
}

/// GET /sync/queue/{id}
async fn get_queue_item(repo: web::Data<Repository>, path: web::Path<i64>) -> HttpResponse {
    match repo.get_sync_queue_item(path.into_inner()) {
        Ok(item) => HttpResponse::Ok().json(item),
        Err(e) => HttpResponse::NotFound().body(e.to_string()),
    }
}

/// POST /sync/process
async fn process_queue(repo: web::Data<Repository>) -> HttpResponse {
    match sync_queue::process_all_pending(&repo) {
        Ok(processed) => HttpResponse::Ok().json(ProcessResult { processed }),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

/// POST /sync/run/{task}
async fn run_task(repo: web::Data<Repository>, path: web::Path<String>) -> HttpResponse {
    let task_name = path.into_inner();
    let task = match task_name.as_str() {
        "sync" => scheduler::TaskType::Sync,
        "integrity-check" | "integrity_check" => scheduler::TaskType::IntegrityCheck,
        other => return HttpResponse::BadRequest().body(format!("Unknown task: {}", other)),
    };
    match scheduler::run_task(&task, &repo) {
        Ok(count) => HttpResponse::Ok().json(TaskResult { task: task.as_str().to_string(), count }),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/sync")
            .route("/queue", web::get().to(list_queue))
            .route("/queue", web::post().to(add_queue_item))
            .route("/queue/{id}", web::get().to(get_queue_item))
            .route("/process", web::post().to(process_queue))
            .route("/run/{task}", web::post().to(run_task)),
    );
}
