use actix_web::{test, web, App};
use bitprotector_lib::api::auth::{issue_token, JwtSecret};
use bitprotector_lib::api::server::configure_routes;
use bitprotector_lib::core::scheduler::Scheduler;
use bitprotector_lib::db::repository::{create_memory_pool, Repository};
use bitprotector_lib::db::schema::initialize_schema;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const SECRET: &[u8] = b"scaling_100k_test_secret";
const TOTAL_ROWS: usize = 100_000;
const QUERY_BUDGET_MS: u128 = 3_000;

fn bearer() -> String {
    format!("Bearer {}", issue_token("scaletest", SECRET, 3600).unwrap())
}

macro_rules! make_app {
    ($repo:expr) => {{
        let _r = $repo;
        let _ra = Arc::new(_r.clone());
        let _sd = web::Data::new(Arc::new(Mutex::new(Scheduler::new(_ra))));
        test::init_service(
            App::new()
                .app_data(web::Data::new(_r))
                .app_data(web::Data::new(JwtSecret(SECRET.to_vec())))
                .app_data(_sd)
                .configure(configure_routes),
        )
    }};
}

fn seed_repo_with_100k_files() -> (Repository, i64) {
    let pool = create_memory_pool().unwrap();
    let pair_id;

    {
        let mut conn = pool.get().unwrap();
        initialize_schema(&conn).unwrap();

        conn.execute(
            "INSERT INTO drive_pairs (name, primary_path, secondary_path)
             VALUES ('scale-pair', '/tmp/scale-primary', '/tmp/scale-secondary')",
            [],
        )
        .unwrap();
        pair_id = conn.last_insert_rowid();

        let tx = conn.transaction().unwrap();
        {
            let mut stmt = tx
                .prepare(
                    "INSERT INTO tracked_files (
                        drive_pair_id,
                        relative_path,
                        checksum,
                        file_size,
                        virtual_path,
                        is_mirrored,
                        tracked_direct,
                        tracked_via_folder
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                )
                .unwrap();

            for i in 0..TOTAL_ROWS {
                let relative_path = if i % 2 == 0 {
                    format!("docs/report-{i:06}.txt")
                } else {
                    format!("media/photo-{i:06}.jpg")
                };

                let virtual_path = if i % 3 == 0 {
                    if i % 2 == 0 {
                        Some(format!("/published/docs/item-{i:06}.dat"))
                    } else {
                        Some(format!("/published/media/item-{i:06}.dat"))
                    }
                } else {
                    None
                };

                let (tracked_direct, tracked_via_folder) = match i % 5 {
                    0 => (1_i64, 0_i64),
                    1 => (0_i64, 1_i64),
                    2 => (1_i64, 1_i64),
                    3 => (1_i64, 0_i64),
                    _ => (0_i64, 1_i64),
                };

                stmt.execute(rusqlite::params![
                    pair_id,
                    relative_path,
                    format!("h{i:08}"),
                    (i % 8192) as i64 + 1,
                    virtual_path,
                    if i % 2 == 0 { 1_i64 } else { 0_i64 },
                    tracked_direct,
                    tracked_via_folder
                ])
                .unwrap();
            }
        }
        tx.commit().unwrap();
    }

    (Repository::new(pool), pair_id)
}

fn assert_budget(label: &str, elapsed: Duration) {
    assert!(
        elapsed.as_millis() <= QUERY_BUDGET_MS,
        "{label} exceeded budget: {}ms > {}ms",
        elapsed.as_millis(),
        QUERY_BUDGET_MS
    );
}

#[actix_rt::test]
async fn test_tracking_items_scales_to_100k_with_pagination_filters_and_budgets() {
    let (repo, pair_id) = seed_repo_with_100k_files();
    let app = make_app!(repo).await;

    let started = Instant::now();
    let req = test::TestRequest::get()
        .uri(&format!(
            "/api/v1/tracking/items?drive_id={pair_id}&item_kind=file&page=1&per_page=500"
        ))
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let elapsed = started.elapsed();
    assert_eq!(resp.status(), 200);
    assert_budget("unfiltered first page", elapsed);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["total"], TOTAL_ROWS as i64);
    assert_eq!(body["per_page"], 200);
    assert_eq!(body["items"].as_array().unwrap().len(), 200);

    let started = Instant::now();
    let req = test::TestRequest::get()
        .uri(&format!(
            "/api/v1/tracking/items?drive_id={pair_id}&item_kind=file&published=true&publish_prefix=/published/docs&page=1&per_page=200"
        ))
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let elapsed = started.elapsed();
    assert_eq!(resp.status(), 200);
    assert_budget("publish prefix + published filter", elapsed);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["total"], 16_667);
    assert_eq!(body["items"].as_array().unwrap().len(), 200);
    for item in body["items"].as_array().unwrap() {
        let vpath = item["virtual_path"].as_str().unwrap_or_default();
        assert!(vpath.starts_with("/published/docs/"));
    }

    let started = Instant::now();
    let req = test::TestRequest::get()
        .uri(&format!(
            "/api/v1/tracking/items?drive_id={pair_id}&item_kind=file&published=true&source=both&page=1&per_page=200"
        ))
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let elapsed = started.elapsed();
    assert_eq!(resp.status(), 200);
    assert_budget("published + source=both filter", elapsed);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["total"], 6_666);
    assert_eq!(body["items"].as_array().unwrap().len(), 200);

    let started = Instant::now();
    let req = test::TestRequest::get()
        .uri(&format!(
            "/api/v1/tracking/items?drive_id={pair_id}&item_kind=file&q=photo-000001&page=1&per_page=50"
        ))
        .insert_header(("Authorization", bearer()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let elapsed = started.elapsed();
    assert_eq!(resp.status(), 200);
    assert_budget("targeted q filter", elapsed);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["total"], 1);
    assert_eq!(body["items"][0]["path"], "media/photo-000001.jpg");
}
