use crate::api::models::ApiError;
use crate::core::sync_queue::SyncEventBus;
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
struct ProcessStarted {
    status: &'static str,
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

/// GET /sync/queue/stream  — Server-Sent Events with live queue summary + scan progress.
async fn queue_stream(repo: web::Data<Repository>, bus: web::Data<SyncEventBus>) -> HttpResponse {
    use actix_web::web::Bytes;
    use futures_util::StreamExt as _;

    // Subscribe before taking the snapshot so no concurrent mutations are missed.
    let rx = bus.subscribe();
    let initial = bus.snapshot(&repo);
    let initial_revision = initial.revision;

    let initial_stream = futures_util::stream::once(std::future::ready({
        let json = serde_json::to_string(&initial).unwrap_or_default();
        Ok::<Bytes, actix_web::Error>(Bytes::from(format!("data: {json}\n\n")))
    }));

    let event_stream = futures_util::stream::unfold(rx, move |mut rx| async move {
        loop {
            match rx.recv().await {
                Ok(summary) => {
                    if summary.revision < initial_revision {
                        continue;
                    }
                    let json = serde_json::to_string(&summary).unwrap_or_default();
                    return Some((
                        Ok::<Bytes, actix_web::Error>(Bytes::from(format!("data: {json}\n\n"))),
                        rx,
                    ));
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return None,
            }
        }
    });

    HttpResponse::Ok()
        .content_type("text/event-stream")
        .insert_header(("Cache-Control", "no-cache"))
        .insert_header(("X-Accel-Buffering", "no"))
        .streaming(initial_stream.chain(event_stream))
}

/// GET /sync/queue
async fn list_queue(repo: web::Data<Repository>, query: web::Query<ListQuery>) -> HttpResponse {
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(50).clamp(1, 200);
    let queue_paused = repo.get_sync_queue_paused().unwrap_or(false);
    let active_items = match repo.count_active_sync_queue() {
        Ok(count) => count,
        Err(e) => return HttpResponse::InternalServerError().body(e.to_string()),
    };
    let in_progress_items = match repo.count_in_progress_sync_queue() {
        Ok(count) => count,
        Err(e) => return HttpResponse::InternalServerError().body(e.to_string()),
    };
    let counts_by_status = match repo.count_sync_queue_by_status() {
        Ok(counts) => counts,
        Err(e) => return HttpResponse::InternalServerError().body(e.to_string()),
    };
    let pending_items = *counts_by_status.get("pending").unwrap_or(&0);
    let completed_items = *counts_by_status.get("completed").unwrap_or(&0);
    let failed_items = *counts_by_status.get("failed").unwrap_or(&0);
    match repo.list_sync_queue(query.status.as_deref(), page, per_page) {
        Ok((items, total)) => HttpResponse::Ok().json(serde_json::json!({
            "queue": items,
            "total": total,
            "page": page,
            "per_page": per_page,
            "queue_paused": queue_paused,
            "active_items": active_items,
            "in_progress_items": in_progress_items,
            "pending_items": pending_items,
            "completed_items": completed_items,
            "failed_items": failed_items,
        })),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

/// POST /sync/queue
async fn add_queue_item(
    repo: web::Data<Repository>,
    bus: web::Data<SyncEventBus>,
    body: web::Json<AddQueueItem>,
) -> HttpResponse {
    match repo.create_sync_queue_item(body.tracked_file_id, &body.action) {
        Ok(item) => {
            bus.publish_snapshot(&repo);
            HttpResponse::Created().json(item)
        }
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
/// Body: `{ "resolution": "keep_master|keep_mirror|provide_new|accept_current|untrack", "new_file_path": "<path>" }`
async fn resolve_queue_item(
    repo: web::Data<Repository>,
    bus: web::Data<SyncEventBus>,
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
        Ok(item) => {
            bus.publish_snapshot(&repo);
            HttpResponse::Ok().json(item)
        }
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
                || msg.contains("requires 'accept_current' or 'untrack'")
                || msg.contains("only valid when reason")
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

/// POST /sync/process — start a non-blocking background worker that drains the queue.
/// Returns 202 Accepted immediately; a second call while the worker is running is idempotent.
async fn process_queue(repo: web::Data<Repository>, bus: web::Data<SyncEventBus>) -> HttpResponse {
    sync_queue::process_all_pending_async(&repo, bus.as_ref().clone());
    HttpResponse::Accepted().json(ProcessStarted { status: "started" })
}

/// DELETE /sync/queue/completed
async fn clear_completed_queue(
    repo: web::Data<Repository>,
    bus: web::Data<SyncEventBus>,
) -> HttpResponse {
    match repo.clear_completed_sync_queue() {
        Ok(deleted) => {
            bus.publish_snapshot(&repo);
            HttpResponse::Ok().json(ClearCompletedResult { deleted })
        }
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

/// POST /sync/run/{task}
async fn run_task(
    repo: web::Data<Repository>,
    bus: web::Data<SyncEventBus>,
    path: web::Path<String>,
    checksum_cfg: web::Data<crate::core::checksum::ChecksumConfig>,
) -> HttpResponse {
    let task_name = path.into_inner();
    let task = match task_name.as_str() {
        "sync" => scheduler::TaskType::Sync,
        "integrity-check" | "integrity_check" => scheduler::TaskType::IntegrityCheck,
        other => return HttpResponse::BadRequest().body(format!("Unknown task: {other}")),
    };
    match scheduler::run_task(&task, &repo, None, &checksum_cfg, Some(bus.as_ref())) {
        Ok(count) => HttpResponse::Ok().json(TaskResult {
            task: task.as_str().to_string(),
            count,
        }),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

/// POST /sync/pause — pause all automatic sync queue processing
async fn pause_queue(repo: web::Data<Repository>, bus: web::Data<SyncEventBus>) -> HttpResponse {
    match repo.set_sync_queue_paused(true) {
        Ok(()) => {
            bus.publish_snapshot(&repo);
            HttpResponse::Ok().json(QueuePausedResult { queue_paused: true })
        }
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

/// POST /sync/resume — resume automatic sync queue processing
async fn resume_queue(repo: web::Data<Repository>, bus: web::Data<SyncEventBus>) -> HttpResponse {
    match repo.set_sync_queue_paused(false) {
        Ok(()) => {
            bus.publish_snapshot(&repo);
            HttpResponse::Ok().json(QueuePausedResult {
                queue_paused: false,
            })
        }
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/sync")
            .route("/queue/stream", web::get().to(queue_stream))
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
