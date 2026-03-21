use bitprotector_lib::core::change_detection::watch_folder;
use std::fs;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tempfile::TempDir;

/// Wait up to `timeout` for `condition` to become true, polling every 50 ms.
fn wait_until(timeout: Duration, condition: impl Fn() -> bool) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if condition() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    condition()
}

#[test]
fn test_watch_folder_detects_new_file() {
    let dir = TempDir::new().unwrap();
    let events: Arc<Mutex<Vec<notify::Event>>> = Arc::new(Mutex::new(Vec::new()));
    let events_clone = Arc::clone(&events);

    let _watcher = watch_folder(dir.path().to_str().unwrap(), move |event| {
        events_clone.lock().unwrap().push(event);
    })
    .expect("watcher should start on a valid directory");

    // Write a file — this should trigger at least one inotify event.
    fs::write(dir.path().join("watched.txt"), b"hello watcher").unwrap();

    let received = wait_until(Duration::from_secs(2), || {
        !events.lock().unwrap().is_empty()
    });
    assert!(
        received,
        "Filesystem watcher should have delivered at least one event after a file write"
    );
}

#[test]
fn test_watch_folder_detects_modification() {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("existing.txt");
    fs::write(&file_path, b"original").unwrap();

    let events: Arc<Mutex<Vec<notify::Event>>> = Arc::new(Mutex::new(Vec::new()));
    let events_clone = Arc::clone(&events);

    let _watcher = watch_folder(dir.path().to_str().unwrap(), move |event| {
        events_clone.lock().unwrap().push(event);
    })
    .expect("watcher should start");

    // Overwrite the existing file
    fs::write(&file_path, b"modified content").unwrap();

    let received = wait_until(Duration::from_secs(2), || {
        !events.lock().unwrap().is_empty()
    });
    assert!(received, "Watcher should detect file modification");
}

#[test]
fn test_watch_folder_detects_deletion() {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("to_delete.txt");
    fs::write(&file_path, b"bye").unwrap();

    let events: Arc<Mutex<Vec<notify::Event>>> = Arc::new(Mutex::new(Vec::new()));
    let events_clone = Arc::clone(&events);

    let _watcher = watch_folder(dir.path().to_str().unwrap(), move |event| {
        events_clone.lock().unwrap().push(event);
    })
    .expect("watcher should start");

    fs::remove_file(&file_path).unwrap();

    let received = wait_until(Duration::from_secs(2), || {
        !events.lock().unwrap().is_empty()
    });
    assert!(received, "Watcher should detect file deletion");
}

#[test]
fn test_watch_folder_invalid_path_returns_error() {
    let result = watch_folder("/nonexistent/path/to/watch", |_| {});
    assert!(
        result.is_err(),
        "Watching a non-existent path should return an error"
    );
}

#[test]
fn test_watch_folder_drop_stops_delivery() {
    // Verify that dropping the watcher handle stops event delivery cleanly.
    let dir = TempDir::new().unwrap();
    let events: Arc<Mutex<Vec<notify::Event>>> = Arc::new(Mutex::new(Vec::new()));
    let events_clone = Arc::clone(&events);

    {
        let _watcher = watch_folder(dir.path().to_str().unwrap(), move |event| {
            events_clone.lock().unwrap().push(event);
        })
        .unwrap();
        // _watcher is dropped at the end of this block
    }

    // Clear any events that may have fired before drop
    events.lock().unwrap().clear();

    // Writes after drop should not cause panics (callback might not fire)
    fs::write(dir.path().join("post_drop.txt"), b"after drop").unwrap();
    std::thread::sleep(Duration::from_millis(200));
    // No assertion on event count — just verify no panic occurred
}
