use crate::api::auth::JwtSecret;
use crate::api::models::ApiError;
use crate::core::scheduler::Scheduler;
use crate::db::repository::{create_pool, Repository};
use actix_cors::Cors;
use actix_web::{
    body::{BoxBody, EitherBody, MessageBody},
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    middleware, web, App, Error, HttpMessage, HttpResponse, HttpServer,
};
use futures_util::future::{ready, LocalBoxFuture, Ready};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// JWT authentication middleware
// ---------------------------------------------------------------------------

/// Global JWT authentication middleware. Validates the Bearer token from the
/// Authorization header and inserts the decoded `Claims` into request extensions.
/// Must be applied to the protected route scope (not the public login scope).
pub struct JwtMiddlewareLayer;

impl<S, B> Transform<S, ServiceRequest> for JwtMiddlewareLayer
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<EitherBody<B, BoxBody>>;
    type Error = Error;
    type Transform = JwtMiddlewareService<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(JwtMiddlewareService { service }))
    }
}

pub struct JwtMiddlewareService<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for JwtMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<EitherBody<B, BoxBody>>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let secret = match req.app_data::<web::Data<JwtSecret>>() {
            Some(s) => s.0.clone(),
            None => {
                let (req, _) = req.into_parts();
                let resp = HttpResponse::InternalServerError()
                    .json(ApiError::new("internal_error", "JWT secret not configured"));
                return Box::pin(async move {
                    Ok(ServiceResponse::new(req, resp).map_into_right_body())
                });
            }
        };

        let token = req
            .headers()
            .get("Authorization")
            .and_then(|h| h.to_str().ok())
            .and_then(|h| h.strip_prefix("Bearer "))
            .map(|t| t.to_string());

        match token {
            None => {
                let (req, _) = req.into_parts();
                let resp = HttpResponse::Unauthorized()
                    .json(ApiError::new("unauthorized", "Missing authorization header"));
                Box::pin(async move {
                    Ok(ServiceResponse::new(req, resp).map_into_right_body())
                })
            }
            Some(token) => match crate::api::auth::validate_token(&token, &secret) {
                Err(_) => {
                    let (req, _) = req.into_parts();
                    let resp = HttpResponse::Unauthorized()
                        .json(ApiError::new("unauthorized", "Invalid or expired token"));
                    Box::pin(async move {
                        Ok(ServiceResponse::new(req, resp).map_into_right_body())
                    })
                }
                Ok(claims) => {
                    req.extensions_mut().insert(claims);
                    let fut = self.service.call(req);
                    Box::pin(async move {
                        let res = fut.await?;
                        Ok(res.map_into_left_body())
                    })
                }
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Rate limiter
// ---------------------------------------------------------------------------

/// Sliding-window per-IP rate limiter.
pub struct RateLimiter {
    requests: Mutex<HashMap<String, Vec<Instant>>>,
    max_requests: usize,
    window: Duration,
}

impl RateLimiter {
    pub fn new(max_requests: usize, window_secs: u64) -> Self {
        Self {
            requests: Mutex::new(HashMap::new()),
            max_requests,
            window: Duration::from_secs(window_secs),
        }
    }

    pub fn is_allowed(&self, key: &str) -> bool {
        let mut map = self.requests.lock().unwrap();
        let now = Instant::now();
        let window = self.window;
        let entry = map.entry(key.to_string()).or_default();
        entry.retain(|&t| now.duration_since(t) < window);
        if entry.len() < self.max_requests {
            entry.push(now);
            true
        } else {
            false
        }
    }
}

// actix-web Transform + Service implementation for rate limiting ------------

pub struct RateLimitLayer(pub Arc<RateLimiter>);

impl<S, B> Transform<S, ServiceRequest> for RateLimitLayer
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<EitherBody<B, BoxBody>>;
    type Error = Error;
    type Transform = RateLimitService<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(RateLimitService {
            service,
            limiter: self.0.clone(),
        }))
    }
}

pub struct RateLimitService<S> {
    service: S,
    limiter: Arc<RateLimiter>,
}

impl<S, B> Service<ServiceRequest> for RateLimitService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<EitherBody<B, BoxBody>>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let ip = req
            .peer_addr()
            .map(|a| a.ip().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        if !self.limiter.is_allowed(&ip) {
            let (req, _) = req.into_parts();
            let resp = HttpResponse::TooManyRequests()
                .json(ApiError::new("too_many_requests", "Rate limit exceeded"));
            return Box::pin(
                async move { Ok(ServiceResponse::new(req, resp).map_into_right_body()) },
            );
        }

        let fut = self.service.call(req);
        Box::pin(async move {
            let res = fut.await?;
            Ok(res.map_into_left_body())
        })
    }
}

// ---------------------------------------------------------------------------
// Route configuration
// ---------------------------------------------------------------------------

/// Register all API routes under `/api/v1`.
/// - `POST /api/v1/auth/login` is public (no JWT required).
/// - All other routes are wrapped with `JwtMiddlewareLayer`.
pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    use crate::api::routes::{
        auth, database, drives, files, folders, integrity, logs, scheduler, status, sync,
        virtual_paths,
    };
    cfg.service(
        web::scope("/api/v1")
            // ── Public: login only ──────────────────────────────────────────
            .configure(auth::configure_public)
            // ── Protected: everything else ──────────────────────────────────
            .service(
                web::scope("")
                    .wrap(JwtMiddlewareLayer)
                    .configure(auth::configure_protected)
                    .configure(drives::configure)
                    .configure(files::configure)
                    .configure(virtual_paths::configure)
                    .configure(integrity::configure)
                    .configure(folders::configure)
                    .configure(sync::configure)
                    .configure(logs::configure)
                    .configure(database::configure)
                    .configure(scheduler::configure)
                    .configure(status::configure),
            ),
    );
}

// ---------------------------------------------------------------------------
// TLS helper
// ---------------------------------------------------------------------------

#[cfg(not(test))]
fn load_tls_config(cert_path: &str, key_path: &str) -> anyhow::Result<rustls::ServerConfig> {
    use rustls_pemfile::{certs, private_key};
    use std::fs::File;
    use std::io::BufReader;

    let cert_file = &mut BufReader::new(File::open(cert_path)?);
    let key_file = &mut BufReader::new(File::open(key_path)?);

    let tls_certs: Vec<_> = certs(cert_file).collect::<Result<_, _>>()?;
    let tls_key = private_key(key_file)?
        .ok_or_else(|| anyhow::anyhow!("No private key found in {}", key_path))?;

    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(tls_certs, tls_key)?;
    Ok(config)
}

// ---------------------------------------------------------------------------
// Public server entry point
// ---------------------------------------------------------------------------

/// Start the HTTP (or HTTPS) server.
pub async fn run_server(
    host: &str,
    port: u16,
    db_path: &str,
    jwt_secret: Vec<u8>,
    tls_cert: Option<&str>,
    tls_key: Option<&str>,
    rate_limit_rps: usize,
) -> anyhow::Result<()> {
    let pool = create_pool(db_path)?;
    let repo = Repository::new(pool);
    let repo_arc = Arc::new(repo.clone());

    // Create and load the scheduler from persisted DB schedules.
    let scheduler = {
        let mut sched = Scheduler::new(Arc::clone(&repo_arc));
        let _ = sched.reload(); // ignore startup errors; schedules may be empty
        Arc::new(Mutex::new(sched))
    };

    let repo_data = web::Data::new(repo);
    let jwt_data = web::Data::new(JwtSecret(jwt_secret));
    let scheduler_data = web::Data::new(scheduler);
    let limiter = Arc::new(RateLimiter::new(rate_limit_rps, 1));

    let server = HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header()
            .max_age(3600);

        App::new()
            .wrap(cors)
            .wrap(middleware::Logger::default())
            .wrap(RateLimitLayer(limiter.clone()))
            .app_data(repo_data.clone())
            .app_data(jwt_data.clone())
            .app_data(scheduler_data.clone())
            .configure(configure_routes)
    });

    let bind_addr = format!("{}:{}", host, port);

    #[cfg(not(test))]
    if let (Some(cert), Some(key)) = (tls_cert, tls_key) {
        let tls_config = load_tls_config(cert, key)?;
        server
            .bind_rustls_0_23(&bind_addr, tls_config)?
            .run()
            .await?;
        return Ok(());
    }
    let _ = (tls_cert, tls_key);

    server.bind(&bind_addr)?.run().await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::auth::{issue_token, JwtSecret};
    use crate::db::repository::{create_memory_pool, Repository};
    use crate::db::schema::initialize_schema;
    use actix_web::{test, App};
    use std::fs;
    use tempfile::TempDir;

    const SECRET: &[u8] = b"test_secret_key";

    fn make_repo() -> Repository {
        let pool = create_memory_pool().unwrap();
        initialize_schema(&pool.get().unwrap()).unwrap();
        Repository::new(pool)
    }

    /// Issue a valid test token and return the Authorization header value.
    fn bearer_token() -> String {
        format!("Bearer {}", issue_token("test_user", SECRET, 3600).unwrap())
    }

    #[actix_rt::test]
    async fn test_drives_list_returns_ok() {
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(make_repo()))
                .app_data(web::Data::new(JwtSecret(SECRET.to_vec())))
                .configure(configure_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/api/v1/drives")
            .insert_header(("Authorization", bearer_token()))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[actix_rt::test]
    async fn test_404_endpoint() {
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(make_repo()))
                .app_data(web::Data::new(JwtSecret(SECRET.to_vec())))
                .configure(configure_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/api/v1/nonexistent")
            .insert_header(("Authorization", bearer_token()))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);
    }

    #[actix_rt::test]
    async fn test_logs_list_returns_ok() {
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(make_repo()))
                .app_data(web::Data::new(JwtSecret(SECRET.to_vec())))
                .configure(configure_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/api/v1/logs")
            .insert_header(("Authorization", bearer_token()))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[actix_rt::test]
    async fn test_database_list_returns_ok() {
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(make_repo()))
                .app_data(web::Data::new(JwtSecret(SECRET.to_vec())))
                .configure(configure_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/api/v1/database/backups")
            .insert_header(("Authorization", bearer_token()))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[actix_rt::test]
    async fn test_unauthenticated_request_returns_401() {
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(make_repo()))
                .app_data(web::Data::new(JwtSecret(SECRET.to_vec())))
                .configure(configure_routes),
        )
        .await;

        // Any protected endpoint without a token must return 401
        let req = test::TestRequest::get()
            .uri("/api/v1/auth/validate")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 401);

        let req = test::TestRequest::get()
            .uri("/api/v1/drives")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 401);
    }

    #[actix_rt::test]
    async fn test_valid_token_can_access_validate() {
        let token = issue_token("alice", SECRET, 3600).unwrap();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(make_repo()))
                .app_data(web::Data::new(JwtSecret(SECRET.to_vec())))
                .configure(configure_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/api/v1/auth/validate")
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[actix_rt::test]
    async fn test_error_response_format_on_bad_drive_id() {
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(make_repo()))
                .app_data(web::Data::new(JwtSecret(SECRET.to_vec())))
                .configure(configure_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/api/v1/drives/999")
            .insert_header(("Authorization", bearer_token()))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert!(
            body["error"].is_object(),
            "Error response must have 'error' object"
        );
        assert!(
            body["error"]["code"].is_string(),
            "Error must have 'code' field"
        );
        assert!(
            body["error"]["message"].is_string(),
            "Error must have 'message' field"
        );
    }

    #[actix_rt::test]
    async fn test_cors_headers_present() {
        let app = test::init_service(
            App::new()
                .wrap(
                    Cors::default()
                        .allow_any_origin()
                        .allow_any_method()
                        .allow_any_header(),
                )
                .app_data(web::Data::new(make_repo()))
                .app_data(web::Data::new(JwtSecret(SECRET.to_vec())))
                .configure(configure_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/api/v1/drives")
            .insert_header(("Origin", "http://localhost:3000"))
            .insert_header(("Authorization", bearer_token()))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
        let headers = resp.headers();
        assert!(
            headers.contains_key("access-control-allow-origin"),
            "CORS header must be present"
        );
    }

    #[actix_rt::test]
    async fn test_rate_limiter_allows_within_limit() {
        let limiter = RateLimiter::new(10, 1);
        for _ in 0..5 {
            assert!(limiter.is_allowed("127.0.0.1"), "Should allow within limit");
        }
    }

    #[actix_rt::test]
    async fn test_rate_limiter_blocks_over_limit() {
        let limiter = RateLimiter::new(3, 60);
        assert!(limiter.is_allowed("192.168.1.1"));
        assert!(limiter.is_allowed("192.168.1.1"));
        assert!(limiter.is_allowed("192.168.1.1"));
        assert!(
            !limiter.is_allowed("192.168.1.1"),
            "Should block when over limit"
        );
    }

    #[actix_rt::test]
    async fn test_api_versioning_prefix() {
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(make_repo()))
                .app_data(web::Data::new(JwtSecret(SECRET.to_vec())))
                .configure(configure_routes),
        )
        .await;

        // Unversioned path should 404 (behind middleware → 401 if unauthed, but
        // the empty path is not under /api/v1 so it hits actix's 404 directly)
        let req = test::TestRequest::get().uri("/drives").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);

        // Versioned path with token should 200
        let req = test::TestRequest::get()
            .uri("/api/v1/drives")
            .insert_header(("Authorization", bearer_token()))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[actix_rt::test]
    async fn test_status_route_returns_extended_drive_state_fields() {
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(make_repo()))
                .app_data(web::Data::new(JwtSecret(SECRET.to_vec())))
                .configure(configure_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/api/v1/status")
            .insert_header(("Authorization", bearer_token()))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert!(body["degraded_pairs"].is_number());
        assert!(body["active_secondary_pairs"].is_number());
    }

    #[actix_rt::test]
    async fn test_drive_replacement_api_flow() {
        let repo = make_repo();
        let primary = TempDir::new().unwrap();
        let secondary = TempDir::new().unwrap();
        let replacement = TempDir::new().unwrap();
        fs::write(primary.path().join("api.txt"), b"api").unwrap();
        let pair = repo
            .create_drive_pair(
                "pair",
                primary.path().to_str().unwrap(),
                secondary.path().to_str().unwrap(),
            )
            .unwrap();
        let tracked = repo
            .create_tracked_file(pair.id, "api.txt", "ignored", 3, None)
            .unwrap();
        let checksum = crate::core::checksum::checksum_bytes(b"api");
        repo.update_tracked_file_checksum(tracked.id, &checksum, 3)
            .unwrap();
        fs::write(secondary.path().join("api.txt"), b"api").unwrap();

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(repo.clone()))
                .app_data(web::Data::new(JwtSecret(SECRET.to_vec())))
                .configure(configure_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri(&format!("/api/v1/drives/{}/replacement/mark", pair.id))
            .insert_header(("Authorization", bearer_token()))
            .set_json(serde_json::json!({ "role": "primary" }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["primary_state"], "quiescing");

        let req = test::TestRequest::post()
            .uri(&format!("/api/v1/drives/{}/replacement/cancel", pair.id))
            .insert_header(("Authorization", bearer_token()))
            .set_json(serde_json::json!({ "role": "primary" }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["primary_state"], "active");

        let req = test::TestRequest::post()
            .uri(&format!("/api/v1/drives/{}/replacement/mark", pair.id))
            .insert_header(("Authorization", bearer_token()))
            .set_json(serde_json::json!({ "role": "primary" }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let req = test::TestRequest::post()
            .uri(&format!("/api/v1/drives/{}/replacement/confirm", pair.id))
            .insert_header(("Authorization", bearer_token()))
            .set_json(serde_json::json!({ "role": "primary" }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["active_role"], "secondary");

        let req = test::TestRequest::post()
            .uri(&format!("/api/v1/drives/{}/replacement/assign", pair.id))
            .insert_header(("Authorization", bearer_token()))
            .set_json(serde_json::json!({
                "role": "primary",
                "new_path": replacement.path().to_str().unwrap(),
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["drive_pair"]["primary_state"], "rebuilding");
        assert_eq!(body["queued_rebuild_items"], 1);
    }
}
