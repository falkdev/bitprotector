use crate::api::auth::{issue_token, JwtAuth, JwtSecret, RevokedTokens};
use crate::api::models::LoginResponse;
use actix_web::{web, HttpResponse};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Serialize)]
struct ValidateResponse {
    username: String,
    valid: bool,
}

const TOKEN_EXPIRES_SECS: i64 = 86400; // 24 hours

pub async fn login(secret: web::Data<JwtSecret>, body: web::Json<LoginRequest>) -> HttpResponse {
    if !crate::api::auth::authenticate_user(&body.username, &body.password) {
        return HttpResponse::Unauthorized().body("Invalid credentials");
    }

    match issue_token(&body.username, &secret.0, TOKEN_EXPIRES_SECS) {
        Ok(token) => {
            let expires_at = (Utc::now() + Duration::seconds(TOKEN_EXPIRES_SECS))
                .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
            HttpResponse::Ok().json(LoginResponse {
                token,
                username: body.username.clone(),
                expires_at,
            })
        }
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

async fn validate(auth: JwtAuth) -> HttpResponse {
    HttpResponse::Ok().json(ValidateResponse {
        username: auth.claims.sub,
        valid: true,
    })
}

/// POST /auth/logout — invalidate the current token immediately.
async fn logout(auth: JwtAuth, revoked: web::Data<RevokedTokens>) -> HttpResponse {
    revoked.revoke(&auth.claims);
    HttpResponse::Ok().json(serde_json::json!({ "message": "Logged out" }))
}

/// Register the public login endpoint (no JWT required).
pub fn configure_public(cfg: &mut web::ServiceConfig) {
    cfg.route("/auth/login", web::post().to(login));
}

/// Register the protected validate and logout endpoints (JWT required).
pub fn configure_protected(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/auth")
            .route("/validate", web::get().to(validate))
            .route("/logout", web::post().to(logout)),
    );
}

/// Register all auth endpoints (used internally or for backward-compat in tests
/// where the test app doesn't apply the JWT middleware).
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/auth")
            .route("/login", web::post().to(login))
            .route("/validate", web::get().to(validate))
            .route("/logout", web::post().to(logout)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::auth::{issue_token, JwtSecret, RevokedTokens};
    use actix_web::{test, web, App};

    const SECRET: &[u8] = b"test_secret_for_auth_routes";

    fn make_app_data() -> (web::Data<JwtSecret>, web::Data<RevokedTokens>) {
        (
            web::Data::new(JwtSecret(SECRET.to_vec())),
            web::Data::new(RevokedTokens::default()),
        )
    }

    #[actix_rt::test]
    async fn test_login_bad_credentials_returns_401() {
        let (jwt_secret, revoked) = make_app_data();
        let app = test::init_service(
            App::new()
                .app_data(jwt_secret)
                .app_data(revoked)
                .configure(configure),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/auth/login")
            .set_json(serde_json::json!({
                "username": "nonexistent_user_xyz",
                "password": "wrong_password_xyz"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 401);
    }

    #[actix_rt::test]
    async fn test_validate_with_valid_token_returns_200() {
        let (jwt_secret, revoked) = make_app_data();
        let token = issue_token("alice", SECRET, 3600).unwrap();

        let app = test::init_service(
            App::new()
                .app_data(jwt_secret)
                .app_data(revoked)
                .configure(configure),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/auth/validate")
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["username"], "alice");
        assert_eq!(body["valid"], true);
    }

    #[actix_rt::test]
    async fn test_validate_without_token_returns_401() {
        let (jwt_secret, revoked) = make_app_data();
        let app = test::init_service(
            App::new()
                .app_data(jwt_secret)
                .app_data(revoked)
                .configure(configure),
        )
        .await;

        let req = test::TestRequest::get().uri("/auth/validate").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 401);
    }

    #[actix_rt::test]
    async fn test_logout_with_valid_token_returns_200() {
        let (jwt_secret, revoked) = make_app_data();
        let token = issue_token("bob", SECRET, 3600).unwrap();

        let app = test::init_service(
            App::new()
                .app_data(jwt_secret)
                .app_data(revoked)
                .configure(configure),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/auth/logout")
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[actix_rt::test]
    async fn test_token_rejected_after_logout() {
        let (jwt_secret, revoked) = make_app_data();
        let token = issue_token("carol", SECRET, 3600).unwrap();

        let app = test::init_service(
            App::new()
                .app_data(jwt_secret)
                .app_data(revoked)
                .configure(configure),
        )
        .await;

        // Logout first
        let logout_req = test::TestRequest::post()
            .uri("/auth/logout")
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .to_request();
        let logout_resp = test::call_service(&app, logout_req).await;
        assert_eq!(logout_resp.status(), 200);

        // Validate with same token should now fail
        let validate_req = test::TestRequest::get()
            .uri("/auth/validate")
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .to_request();
        let validate_resp = test::call_service(&app, validate_req).await;
        assert_eq!(validate_resp.status(), 401);
    }
}
