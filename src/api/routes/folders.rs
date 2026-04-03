use crate::api::path_resolution::{resolve_path_within_drive_root, PathTargetKind};
use crate::core::{change_detection, drive, tracker, virtual_path};
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
struct ScanResult {
    new_files: usize,
    changed_files: usize,
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

    if folder.virtual_path.is_some() {
        if let Err(e) = virtual_path::remove_folder_virtual_path(&repo, id) {
            return HttpResponse::BadRequest().body(e.to_string());
        }
    }

    match repo.delete_tracked_folder(id) {
        Ok(_) => {
            if let Err(e) = repo.recompute_folder_provenance_for_drive(folder.drive_pair_id) {
                return HttpResponse::InternalServerError().body(e.to_string());
            }
            HttpResponse::NoContent().finish()
        }
        Err(e) => HttpResponse::NotFound().body(e.to_string()),
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
    let folder = match repo.get_tracked_folder(path.into_inner()) {
        Ok(f) => f,
        Err(e) => return HttpResponse::NotFound().body(e.to_string()),
    };
    let pair = match drive::load_operational_pair(&repo, folder.drive_pair_id) {
        Ok(p) => p,
        Err(e) => return HttpResponse::InternalServerError().body(e.to_string()),
    };
    let new_files = match tracker::auto_track_folder_files(&repo, &pair, &folder) {
        Ok(v) => v,
        Err(e) => return HttpResponse::InternalServerError().body(e.to_string()),
    };
    let changes = match change_detection::scan_and_record_changes(&repo, &pair) {
        Ok(v) => v,
        Err(e) => return HttpResponse::InternalServerError().body(e.to_string()),
    };
    let folder_changes = changes
        .iter()
        .filter(|(f, _)| {
            f.relative_path
                .starts_with(&format!("{}/", folder.folder_path))
        })
        .count();

    HttpResponse::Ok().json(ScanResult {
        new_files: new_files.len(),
        changed_files: folder_changes,
    })
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/folders")
            .route("", web::get().to(list_folders))
            .route("", web::post().to(add_folder))
            .route("/{id}", web::get().to(get_folder))
            .route("/{id}", web::put().to(update_folder))
            .route("/{id}", web::delete().to(delete_folder))
            .route("/{id}/scan", web::post().to(scan_folder)),
    );
}
