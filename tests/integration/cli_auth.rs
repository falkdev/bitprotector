use actix_web::{test, web, App, HttpResponse};
use bitprotector_lib::api::auth::{issue_token, validate_token, JwtAuth, JwtSecret};

const SECRET: &[u8] = b"integration_test_secret_key";

async fn protected_handler(auth: JwtAuth) -> HttpResponse {
    HttpResponse::Ok().body(format!("authenticated:{}", auth.claims.sub))
}

#[actix_rt::test]
async fn test_auth_middleware_rejects_no_header() {
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(JwtSecret(SECRET.to_vec())))
            .route("/secret", web::get().to(protected_handler)),
    )
    .await;

    let req = test::TestRequest::get().uri("/secret").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 401);
}

#[actix_rt::test]
async fn test_auth_middleware_rejects_bad_token() {
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(JwtSecret(SECRET.to_vec())))
            .route("/secret", web::get().to(protected_handler)),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/secret")
        .insert_header(("Authorization", "Bearer totally.invalid.token"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 401);
}

#[actix_rt::test]
async fn test_auth_middleware_accepts_valid_token() {
    let token = issue_token("carol", SECRET, 3600).unwrap();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(JwtSecret(SECRET.to_vec())))
            .route("/secret", web::get().to(protected_handler)),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/secret")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body = test::read_body(resp).await;
    assert_eq!(body, "authenticated:carol");
}

#[actix_rt::test]
async fn test_expired_token_rejected() {
    let expired_token = issue_token("dave", SECRET, -7200).unwrap();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(JwtSecret(SECRET.to_vec())))
            .route("/secret", web::get().to(protected_handler)),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/secret")
        .insert_header(("Authorization", format!("Bearer {}", expired_token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 401);
}

#[actix_rt::test]
async fn test_full_token_lifecycle() {
    // Module test: issue → validate → check sub
    let token = issue_token("moduletest_user", SECRET, 3600).unwrap();
    let claims = validate_token(&token, SECRET).unwrap();
    assert_eq!(claims.sub, "moduletest_user");
    assert!(claims.exp > claims.iat);

    // Wrong secret fails
    assert!(validate_token(&token, b"wrong").is_err());

    // Expired fails
    let expired = issue_token("user", SECRET, -7200).unwrap();
    assert!(validate_token(&expired, SECRET).is_err());
}
