use crate::api::models::ApiError;
use crate::core::{scheduler, sync_queue};
use crate::db::repository::Repository;
use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};

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

#[derive(Deserialize)]
pub struct ResolveRequest {
    pub resolution: String,
    pub new_file_path: Option<String>,
}

#[derive(Serialize)]
struct ProcessResult {
    processed: u32,
}

#[derive(Serialize)]
struct ClearCompletedResult {
    deleted: u64,
}

#[derive(Serialize)]
struct TaskResult {
    task: String,
    count: u32,
}

#[derive(Serialize)]
struct QueuePausedResult {
    queue_paused: bool,
}

/// GET /sync/queue
async fn list_queue(repo: web::Data<Repository>, query: web::Query<ListQuery>) -> HttpResponse {
    let page = query.page.unwrap_or(1);
    let per_page = query.per_page.unwrap_or(50);
    let queue_paused = repo.get_sync_queue_paused().unwrap_or(false);
    match repo.list_sync_queue(query.status.as_deref(), page, per_page) {
        Ok((items, total)) => HttpResponse::Ok().json(serde_json::json!({
            "queue": items,
            "total": total,
            "page": page,
            "per_page": per_page,
            "queue_paused": queue_paused,
        })),
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

/// POST /sync/queue/{id}/resolve
///
/// Resolve a `user_action_required` sync queue item.
/// Body: `{ "resolution": "keep_master|keep_mirror|provide_new", "new_file_path": "<path>" }`
async fn resolve_queue_item(
    repo: web::Data<Repository>,
    path: web::Path<i64>,
    body: web::Json<ResolveRequest>,
) -> HttpResponse {
    let item_id = path.into_inner();
    match sync_queue::resolve_queue_item(
        &repo,
        item_id,
        &body.resolution,
        body.new_file_path.as_deref(),
    ) {
        Ok(item) => HttpResponse::Ok().json(item),
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("does not exist")
                || msg.contains("not a regular file")
                || msg.contains("not readable")
            {
                HttpResponse::BadRequest().json(ApiError::new("validation_error", &msg))
            } else if msg.contains("only 'user_action_required'")
                || msg.contains("only 'pending'")
                || msg.contains("Unknown resolution")
            {
                HttpResponse::BadRequest().json(ApiError::new("bad_request", &msg))
            } else if msg.contains("no rows") || msg.contains("QueryReturnedNoRows") {
                HttpResponse::NotFound().json(ApiError::new("not_found", "Queue item not found"))
            } else {
                HttpResponse::InternalServerError().body(msg)
            }
        }
    }
}

/// POST /sync/process
async fn process_queue(repo: web::Data<Repository>) -> HttpResponse {
    match sync_queue::process_all_pending(&repo, None) {
        Ok(processed) => HttpResponse::Ok().json(ProcessResult { processed }),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

/// DELETE /sync/queue/completed
async fn clear_completed_queue(repo: web::Data<Repository>) -> HttpResponse {
    match repo.clear_completed_sync_queue() {
        Ok(deleted) => HttpResponse::Ok().json(ClearCompletedResult { deleted }),
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
    match scheduler::run_task(&task, &repo, None) {
        Ok(count) => HttpResponse::Ok().json(TaskResult {
            task: task.as_str().to_string(),
            count,
        }),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

/// POST /sync/pause — pause all automatic sync queue processing
async fn pause_queue(repo: web::Data<Repository>) -> HttpResponse {
    match repo.set_sync_queue_paused(true) {
        Ok(()) => HttpResponse::Ok().json(QueuePausedResult { queue_paused: true }),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

/// POST /sync/resume — resume automatic sync queue processing
async fn resume_queue(repo: web::Data<Repository>) -> HttpResponse {
    match repo.set_sync_queue_paused(false) {
        Ok(()) => HttpResponse::Ok().json(QueuePausedResult {
            queue_paused: false,
        }),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/sync")
            .route("/queue", web::get().to(list_queue))
            .route("/queue", web::post().to(add_queue_item))
            .route("/queue/completed", web::delete().to(clear_completed_queue))
            .route("/queue/{id}", web::get().to(get_queue_item))
            .route("/queue/{id}/resolve", web::post().to(resolve_queue_item))
            .route("/process", web::post().to(process_queue))
            .route("/pause", web::post().to(pause_queue))
            .route("/resume", web::post().to(resume_queue))
            .route("/run/{task}", web::post().to(run_task)),
    );
}
