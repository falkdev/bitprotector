use crate::api::auth::{issue_token, JwtAuth, JwtSecret};
use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Serialize)]
struct LoginResponse {
    token: String,
    username: String,
    expires_in: i64,
}

#[derive(Serialize)]
struct ValidateResponse {
    username: String,
    valid: bool,
}

const TOKEN_EXPIRES_SECS: i64 = 86400; // 24 hours

async fn login(secret: web::Data<JwtSecret>, body: web::Json<LoginRequest>) -> HttpResponse {
    if !crate::api::auth::authenticate_user(&body.username, &body.password) {
        return HttpResponse::Unauthorized().body("Invalid credentials");
    }

    match issue_token(&body.username, &secret.0, TOKEN_EXPIRES_SECS) {
        Ok(token) => HttpResponse::Ok().json(LoginResponse {
            token,
            username: body.username.clone(),
            expires_in: TOKEN_EXPIRES_SECS,
        }),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

async fn validate(auth: JwtAuth) -> HttpResponse {
    HttpResponse::Ok().json(ValidateResponse {
        username: auth.claims.sub,
        valid: true,
    })
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/auth")
            .route("/login", web::post().to(login))
            .route("/validate", web::get().to(validate)),
    );
}
