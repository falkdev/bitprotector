use actix_web::{web, FromRequest, HttpMessage, HttpRequest};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::future::{ready, Ready};
use std::sync::Mutex;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
    pub iat: usize,
}

/// Shared JWT secret stored as app data.
#[derive(Clone)]
pub struct JwtSecret(pub Vec<u8>);

/// In-memory set of revoked token identifiers (`"<sub>:<iat>"`).
/// Tokens added here are rejected by the JWT middleware and extractor even
/// if their signature and expiry are otherwise valid.
/// The set is intentionally process-scoped: it is cleared on restart, which
/// is acceptable because the daemon is single-host and tokens are short-lived.
#[derive(Default)]
pub struct RevokedTokens(pub Mutex<HashSet<String>>);

impl RevokedTokens {
    /// Derive the revocation key for a set of claims.
    pub fn key(claims: &Claims) -> String {
        format!("{}:{}", claims.sub, claims.iat)
    }

    /// Revoke the token described by `claims`.
    pub fn revoke(&self, claims: &Claims) {
        if let Ok(mut set) = self.0.lock() {
            set.insert(Self::key(claims));
        }
    }

    /// Return `true` if the token has been revoked.
    pub fn is_revoked(&self, claims: &Claims) -> bool {
        self.0
            .lock()
            .map(|set| set.contains(&Self::key(claims)))
            .unwrap_or(false)
    }
}

/// Extractor that validates a Bearer JWT token from the Authorization header.
pub struct JwtAuth {
    pub claims: Claims,
}

impl FromRequest for JwtAuth {
    type Error = actix_web::Error;
    type Future = Ready<Result<Self, actix_web::Error>>;

    fn from_request(req: &HttpRequest, _payload: &mut actix_web::dev::Payload) -> Self::Future {
        // Fast path: claims already inserted by the global JWT middleware.
        if let Some(claims) = req.extensions().get::<Claims>().cloned() {
            // The middleware already checked revocation; trust the inserted claims.
            return ready(Ok(JwtAuth { claims }));
        }

        // Fallback: validate directly from the Authorization header.
        // This supports handler-level use in tests or contexts without the middleware.
        let secret = match req.app_data::<web::Data<JwtSecret>>() {
            Some(s) => s.0.clone(),
            None => {
                return ready(Err(actix_web::error::ErrorInternalServerError(
                    "JWT secret not configured",
                )))
            }
        };

        let token = match req
            .headers()
            .get("Authorization")
            .and_then(|h| h.to_str().ok())
            .and_then(|h| h.strip_prefix("Bearer "))
        {
            Some(t) => t.to_string(),
            None => {
                return ready(Err(actix_web::error::ErrorUnauthorized(
                    "Missing or invalid authorization header",
                )))
            }
        };

        let claims = match validate_token(&token, &secret) {
            Ok(c) => c,
            Err(_) => {
                return ready(Err(actix_web::error::ErrorUnauthorized(
                    "Invalid or expired token",
                )))
            }
        };

        if let Some(revoked) = req.app_data::<web::Data<RevokedTokens>>() {
            if revoked.is_revoked(&claims) {
                return ready(Err(actix_web::error::ErrorUnauthorized("Token revoked")));
            }
        }

        ready(Ok(JwtAuth { claims }))
    }
}

/// Issue a JWT token for the given username.
pub fn issue_token(username: &str, secret: &[u8], expires_in_secs: i64) -> anyhow::Result<String> {
    let now = Utc::now();
    let exp = (now + Duration::seconds(expires_in_secs)).timestamp() as usize;
    let iat = now.timestamp() as usize;

    let claims = Claims {
        sub: username.to_string(),
        exp,
        iat,
    };
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret),
    )?;
    Ok(token)
}

/// Validate a JWT token and return the claims.
pub fn validate_token(token: &str, secret: &[u8]) -> anyhow::Result<Claims> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = true;
    let data = decode::<Claims>(token, &DecodingKey::from_secret(secret), &validation)?;
    Ok(data.claims)
}

/// Authenticate a user via PAM. Returns true on success.
pub fn authenticate_user(username: &str, password: &str) -> bool {
    let mut client = match pam::Client::with_password("bitprotector") {
        Ok(c) => c,
        Err(_) => return false,
    };
    client
        .conversation_mut()
        .set_credentials(username, password);
    client.authenticate().is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, web, App, HttpResponse};

    const SECRET: &[u8] = b"test_secret_key_for_jwt_testing";

    #[actix_rt::test]
    async fn test_jwt_issue_and_validate() {
        let token = issue_token("testuser", SECRET, 3600).unwrap();
        assert!(!token.is_empty());
        let claims = validate_token(&token, SECRET).unwrap();
        assert_eq!(claims.sub, "testuser");
    }

    #[actix_rt::test]
    async fn test_jwt_invalid_secret_rejected() {
        let token = issue_token("user", SECRET, 3600).unwrap();
        let result = validate_token(&token, b"wrong_secret");
        assert!(result.is_err());
    }

    #[actix_rt::test]
    async fn test_jwt_expired_token_rejected() {
        let token = issue_token("user", SECRET, -3600).unwrap();
        let result = validate_token(&token, SECRET);
        assert!(result.is_err(), "Expired token should be rejected");
    }

    async fn protected_route(auth: JwtAuth) -> HttpResponse {
        HttpResponse::Ok().body(format!("Hello, {}", auth.claims.sub))
    }

    #[actix_rt::test]
    async fn test_middleware_rejects_missing_auth() {
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(JwtSecret(SECRET.to_vec())))
                .route("/protected", web::get().to(protected_route)),
        )
        .await;

        let req = test::TestRequest::get().uri("/protected").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 401);
    }

    #[actix_rt::test]
    async fn test_middleware_rejects_invalid_token() {
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(JwtSecret(SECRET.to_vec())))
                .route("/protected", web::get().to(protected_route)),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/protected")
            .insert_header(("Authorization", "Bearer invalid.token.here"))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 401);
    }

    #[actix_rt::test]
    async fn test_middleware_accepts_valid_token() {
        let token = issue_token("alice", SECRET, 3600).unwrap();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(JwtSecret(SECRET.to_vec())))
                .route("/protected", web::get().to(protected_route)),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/protected")
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
        let body = test::read_body(resp).await;
        assert_eq!(body, "Hello, alice");
    }

    #[actix_rt::test]
    async fn test_login_token_auth_cycle() {
        // Module test: issue token → validate → confirm it can be used in extractor logic
        let token = issue_token("bob", SECRET, 3600).unwrap();
        let claims = validate_token(&token, SECRET).unwrap();
        assert_eq!(claims.sub, "bob");
        assert!(claims.exp > claims.iat, "expiry must be after issue time");
    }
}
