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
