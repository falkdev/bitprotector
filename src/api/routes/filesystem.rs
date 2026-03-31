use crate::api::models::ApiError;
use actix_web::{web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fs;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Deserialize)]
pub struct BrowseQuery {
    pub path: Option<String>,
    pub include_hidden: Option<bool>,
    pub directories_only: Option<bool>,
}

#[derive(Debug, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum EntryKind {
    Directory,
    File,
}

#[derive(Debug, Serialize)]
struct FilesystemEntry {
    name: String,
    path: String,
    kind: EntryKind,
    is_hidden: bool,
    is_selectable: bool,
    has_children: bool,
}

#[derive(Debug, Serialize)]
struct BrowseResponse {
    path: String,
    canonical_path: String,
    parent_path: Option<String>,
    entries: Vec<FilesystemEntry>,
}

fn contains_parent_dir(path: &Path) -> bool {
    path.components()
        .any(|component| matches!(component, Component::ParentDir))
}

fn path_kind(path: &Path) -> EntryKind {
    if path.is_dir() {
        EntryKind::Directory
    } else {
        EntryKind::File
    }
}

fn has_children(path: &Path, include_hidden: bool) -> bool {
    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(_) => return false,
    };

    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !include_hidden && name.starts_with('.') {
            continue;
        }
        return true;
    }

    false
}

fn sort_entries(a: &FilesystemEntry, b: &FilesystemEntry) -> Ordering {
    match (&a.kind, &b.kind) {
        (EntryKind::Directory, EntryKind::File) => Ordering::Less,
        (EntryKind::File, EntryKind::Directory) => Ordering::Greater,
        _ => a
            .name
            .to_ascii_lowercase()
            .cmp(&b.name.to_ascii_lowercase())
            .then_with(|| a.name.cmp(&b.name)),
    }
}

pub async fn browse_filesystem(query: web::Query<BrowseQuery>) -> impl Responder {
    let requested_path = query.path.as_deref().unwrap_or("/");
    let include_hidden = query.include_hidden.unwrap_or(false);
    let directories_only = query.directories_only.unwrap_or(false);

    let requested = Path::new(requested_path);
    if contains_parent_dir(requested) {
        return HttpResponse::BadRequest().json(ApiError::new(
            "VALIDATION_ERROR",
            "Parent-directory traversal is not allowed",
        ));
    }

    let canonical = match requested.canonicalize() {
        Ok(path) => path,
        Err(e) => {
            return HttpResponse::BadRequest()
                .json(ApiError::new("VALIDATION_ERROR", &e.to_string()))
        }
    };

    if !canonical.is_dir() {
        return HttpResponse::BadRequest().json(ApiError::new(
            "VALIDATION_ERROR",
            &format!("Path is not a directory: {}", canonical.display()),
        ));
    }

    let entries = match fs::read_dir(&canonical) {
        Ok(entries) => entries,
        Err(e) => {
            return HttpResponse::BadRequest()
                .json(ApiError::new("VALIDATION_ERROR", &e.to_string()))
        }
    };

    let mut children = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy().to_string();
        let is_hidden = name.starts_with('.');
        if !include_hidden && is_hidden {
            continue;
        }

        let kind = path_kind(&path);
        if directories_only && kind != EntryKind::Directory {
            continue;
        }

        let path = match path.canonicalize() {
            Ok(path) => path,
            Err(_) => continue,
        };
        let path_string = path.to_string_lossy().to_string();
        let is_directory = kind == EntryKind::Directory;
        children.push(FilesystemEntry {
            name,
            path: path_string,
            kind,
            is_hidden,
            is_selectable: if directories_only { is_directory } else { true },
            has_children: if is_directory {
                has_children(&path, include_hidden)
            } else {
                false
            },
        });
    }

    children.sort_by(sort_entries);

    let canonical_string = canonical.to_string_lossy().to_string();
    let parent_path = canonical
        .parent()
        .map(PathBuf::from)
        .filter(|parent| parent != &canonical)
        .map(|parent| parent.to_string_lossy().to_string());

    HttpResponse::Ok().json(BrowseResponse {
        path: canonical_string.clone(),
        canonical_path: canonical_string,
        parent_path,
        entries: children,
    })
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("/filesystem").route("/children", web::get().to(browse_filesystem)));
}
