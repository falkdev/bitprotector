use bitprotector_lib::api::auth::issue_token;
use bitprotector_lib::db::repository::{create_memory_pool, Repository};
use bitprotector_lib::db::schema::initialize_schema;

pub const SECRET: &[u8] = b"api_routes_test_secret";

pub fn make_repo() -> Repository {
    let pool = create_memory_pool().unwrap();
    initialize_schema(&pool.get().unwrap()).unwrap();
    Repository::new(pool)
}

pub fn bearer() -> String {
    format!("Bearer {}", issue_token("testuser", SECRET, 3600).unwrap())
}

#[macro_export]
macro_rules! make_app {
    ($repo:expr) => {{
        $crate::make_app_with_db_path!($repo, "/tmp/bitprotector-test.db")
    }};
}

#[macro_export]
macro_rules! make_app_with_db_path {
    ($repo:expr, $db_path:expr) => {{
        let _r = $repo;
        let _ra = std::sync::Arc::new(_r.clone());
        let _sd = actix_web::web::Data::new(std::sync::Arc::new(std::sync::Mutex::new(
            bitprotector_lib::core::scheduler::Scheduler::new(_ra),
        )));
        actix_web::test::init_service(
            actix_web::App::new()
                .app_data(actix_web::web::Data::new(_r))
                .app_data(actix_web::web::Data::new(
                    bitprotector_lib::api::routes::database::DatabasePath($db_path.to_string()),
                ))
                .app_data(actix_web::web::Data::new(
                    bitprotector_lib::api::auth::JwtSecret($crate::common::SECRET.to_vec()),
                ))
                .app_data(_sd)
                .configure(bitprotector_lib::api::server::configure_routes),
        )
    }};
}
