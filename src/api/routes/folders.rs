use crate::api::path_resolution::{resolve_path_within_drive_root, PathTargetKind};
use crate::core::{drive, mirror, tracker, virtual_path};
use crate::db::repository::Repository;
use actix_web::{web, HttpResponse};
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Deserialize)]
pub struct AddFolderRequest {
    pub drive_pair_id: i64,
    pub folder_path: String,
    pub virtual_path: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateFolderRequest {
    /// Pass `null` explicitly to clear the virtual path; omit the field to leave it unchanged.
    #[serde(default, deserialize_with = "deserialize_patch_field")]
    pub virtual_path: Option<Option<String>>,
}

fn deserialize_patch_field<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(Some(Option::<T>::deserialize(deserializer)?))
}

#[derive(Serialize)]
struct ScanActiveResponse {
    scanning: bool,
    scanned: i64,
    total: i64,
}

#[derive(Serialize)]
struct MirrorResult {
    mirrored_files: usize,
}

impl From<&crate::db::repository::TrackedFolder> for ScanActiveResponse {
    fn from(folder: &crate::db::repository::TrackedFolder) -> Self {
        Self {
            scanning: folder.scanning,
            scanned: folder.scan_scanned_files,
            total: folder.scan_total_files,
        }
    }
}

/// GET /folders
async fn list_folders(repo: web::Data<Repository>) -> HttpResponse {
    match repo.list_tracked_folders() {
        Ok(folders) => HttpResponse::Ok().json(folders),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

/// POST /folders
async fn add_folder(
    repo: web::Data<Repository>,
    body: web::Json<AddFolderRequest>,
) -> HttpResponse {
    let pair = match drive::load_operational_pair(&repo, body.drive_pair_id) {
        Ok(p) => p,
        Err(e) => return HttpResponse::NotFound().body(e.to_string()),
    };
    let folder_path = match resolve_path_within_drive_root(
        pair.active_path(),
        &body.folder_path,
        PathTargetKind::Directory,
    ) {
        Ok(path) => path,
        Err(e) => {
            return HttpResponse::BadRequest().body(e.to_string());
        }
    };
    match tracker::track_folder(&repo, &pair, &folder_path, body.virtual_path.as_deref()) {
        Ok(folder) => HttpResponse::Created().json(folder),
        Err(e) => HttpResponse::BadRequest().body(e.to_string()),
    }
}

/// GET /folders/{id}
async fn get_folder(repo: web::Data<Repository>, path: web::Path<i64>) -> HttpResponse {
    match repo.get_tracked_folder(path.into_inner()) {
        Ok(f) => HttpResponse::Ok().json(f),
        Err(e) => HttpResponse::NotFound().body(e.to_string()),
    }
}

/// DELETE /folders/{id}
async fn delete_folder(repo: web::Data<Repository>, path: web::Path<i64>) -> HttpResponse {
    let id = path.into_inner();
    let folder = match repo.get_tracked_folder(id) {
        Ok(folder) => folder,
        Err(e) => return HttpResponse::NotFound().body(e.to_string()),
    };

    let drive_pair = match repo.get_drive_pair(folder.drive_pair_id) {
        Ok(drive_pair) => drive_pair,
        Err(e) => return HttpResponse::InternalServerError().body(e.to_string()),
    };

    if let Err(e) = drive::require_pair_mutation_allowed(&drive_pair) {
        return HttpResponse::BadRequest().body(e.to_string());
    }

    match tracker::untrack_folder_cascade(&repo, id) {
        Ok(_) => HttpResponse::NoContent().finish(),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

/// PUT /folders/{id}
async fn update_folder(
    repo: web::Data<Repository>,
    path: web::Path<i64>,
    body: web::Json<UpdateFolderRequest>,
) -> HttpResponse {
    let id = path.into_inner();
    if let Err(e) = repo.get_tracked_folder(id) {
        return HttpResponse::NotFound().body(e.to_string());
    }

    match body.virtual_path.as_ref() {
        Some(Some(path)) => {
            if let Err(e) = virtual_path::set_folder_virtual_path(&repo, id, path) {
                return HttpResponse::BadRequest().body(e.to_string());
            }
        }
        Some(None) => {
            if let Err(e) = virtual_path::remove_folder_virtual_path(&repo, id) {
                return HttpResponse::BadRequest().body(e.to_string());
            }
        }
        None => {}
    }

    match repo.get_tracked_folder(id) {
        Ok(folder) => HttpResponse::Ok().json(folder),
        Err(e) => HttpResponse::NotFound().body(e.to_string()),
    }
}

/// POST /folders/{id}/scan
async fn scan_folder(repo: web::Data<Repository>, path: web::Path<i64>) -> HttpResponse {
    let folder_id = path.into_inner();
    let folder = match repo.get_tracked_folder(folder_id) {
        Ok(f) => f,
        Err(e) => return HttpResponse::NotFound().body(e.to_string()),
    };
    let pair = match drive::load_operational_pair(&repo, folder.drive_pair_id) {
        Ok(p) => p,
        Err(e) => return HttpResponse::InternalServerError().body(e.to_string()),
    };
    let total_files = match tracker::count_folder_files(&pair, &folder) {
        Ok(v) => v,
        Err(e) => return HttpResponse::InternalServerError().body(e.to_string()),
    };
    let status = match repo.start_folder_scan(folder.id, total_files) {
        Ok(status) => status,
        Err(e) if e.to_string().contains("already active") => {
            return HttpResponse::Conflict().body(e.to_string());
        }
        Err(e) => return HttpResponse::InternalServerError().body(e.to_string()),
    };

    let repo_clone = repo.get_ref().clone();
    std::thread::spawn(move || {
        let result = (|| -> anyhow::Result<()> {
            let folder = repo_clone.get_tracked_folder(folder_id)?;
            let pair = drive::load_operational_pair(&repo_clone, folder.drive_pair_id)?;
            tracker::scan_tracked_folder(&repo_clone, &pair, &folder, |scanned, total| {
                repo_clone.update_scan_progress(folder_id, scanned, total)
            })?;
            Ok(())
        })();

        if let Err(error) = result {
            eprintln!("folder scan #{folder_id} failed: {error}");
        }

        let _ = repo_clone.finish_folder_scan(folder_id);
    });

    HttpResponse::Accepted().json(ScanActiveResponse::from(&status))
}

/// GET /folders/{id}/scan/active
async fn scan_folder_active(repo: web::Data<Repository>, path: web::Path<i64>) -> HttpResponse {
    match repo.get_tracked_folder(path.into_inner()) {
        Ok(folder) => HttpResponse::Ok().json(ScanActiveResponse::from(&folder)),
        Err(e) => HttpResponse::NotFound().body(e.to_string()),
    }
}

/// POST /folders/{id}/mirror
async fn mirror_folder(repo: web::Data<Repository>, path: web::Path<i64>) -> HttpResponse {
    let folder = match repo.get_tracked_folder(path.into_inner()) {
        Ok(f) => f,
        Err(e) => return HttpResponse::NotFound().body(e.to_string()),
    };
    let pair = match drive::load_operational_pair(&repo, folder.drive_pair_id) {
        Ok(p) => p,
        Err(e) => return HttpResponse::InternalServerError().body(e.to_string()),
    };
    if !pair.standby_accepts_sync() {
        return HttpResponse::BadRequest()
            .body("Standby drive is not currently available for mirroring");
    }

    let files = match repo.list_tracked_files_under_folder(pair.id, &folder.folder_path) {
        Ok(files) => files,
        Err(e) => return HttpResponse::InternalServerError().body(e.to_string()),
    };

    let mut mirrored_files = 0usize;
    for file in files.into_iter().filter(|file| !file.is_mirrored) {
        if let Err(e) = mirror::mirror_file(&pair, &file.relative_path) {
            return HttpResponse::InternalServerError().body(e.to_string());
        }
        if let Err(e) = repo.update_tracked_file_mirror_status(file.id, true) {
            return HttpResponse::InternalServerError().body(e.to_string());
        }
        let _ = repo.complete_pending_mirror_queue_for_file(file.id);
        mirrored_files += 1;
    }

    HttpResponse::Ok().json(MirrorResult { mirrored_files })
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/folders")
            .route("", web::get().to(list_folders))
            .route("", web::post().to(add_folder))
            .route("/{id}", web::get().to(get_folder))
            .route("/{id}", web::put().to(update_folder))
            .route("/{id}", web::delete().to(delete_folder))
            .route("/{id}/mirror", web::post().to(mirror_folder))
            .route("/{id}/scan/active", web::get().to(scan_folder_active))
            .route("/{id}/scan", web::post().to(scan_folder)),
    );
}
