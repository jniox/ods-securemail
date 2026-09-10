//! HTTP handler integration tests for SecureMail API.
//!
//! Tests authentication enforcement and basic CRUD flow using actix_web::test.
//! These tests use a real database connection (DATABASE_URL must be set).

use actix_web::{test, web, App};
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

use ods_common::auth::JwtConfig;
use ods_securemail::api::{health, mail_configs};
use ods_securemail::events::producer::InMemoryProducer;
use ods_securemail::repository::{db, mail_config::MailConfigRepository};
use ods_securemail::service::mail_config_service::MailConfigService;
use sqlx::Executor;

const TEST_SECRET: &[u8] = b"ods-common-test-secret-32-chars!";

fn test_jwt_config() -> web::Data<JwtConfig> {
    web::Data::new(JwtConfig::new_hs256(TEST_SECRET, false))
}

fn make_test_jwt(user_id: &Uuid, tenant_id: &Uuid) -> String {
    use jsonwebtoken::{EncodingKey, Header};

    #[derive(serde::Serialize)]
    struct TestClaims {
        sub: String,
        tenant_id: String,
        email: String,
        name: String,
        roles: Vec<String>,
        permissions: Vec<String>,
    }

    let claims = TestClaims {
        sub: user_id.to_string(),
        tenant_id: tenant_id.to_string(),
        email: "test@example.com".to_string(),
        name: "Test User".to_string(),
        roles: vec!["admin".to_string()],
        permissions: vec![],
    };

    jsonwebtoken::encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(TEST_SECRET),
    )
    .unwrap()
}

async fn setup_app_with_db() -> (
    impl actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
    Uuid,
    Uuid,
) {
    let _ = dotenvy::dotenv();
    // Initialize tracing for test debugging
    let _ = tracing_subscriber::fmt()
        .with_env_filter("ods_securemail=debug,sqlx=warn")
        .with_test_writer()
        .try_init();

    let database_url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for integration tests");

    let pool = db::create_pool(&database_url, "securemail", 5)
        .await
        .expect("Failed to create pool");

    // Clear stale migration tracking so idempotent migrations can re-run cleanly.
    // We delete by version (1-6) which are the securemail-specific migrations.
    pool.execute("DELETE FROM securemail._sqlx_migrations WHERE version IN (1, 2, 3, 4, 5, 6)")
        .await
        .ok();

    // Run migrations (all use IF NOT EXISTS so they're idempotent)
    db::run_migrations(&pool)
        .await
        .expect("Failed to run securemail migrations");

    let producer = Arc::new(InMemoryProducer::new());
    // 32-byte key for AES-256
    let master_key = vec![0x42u8; 32];

    let repo = Arc::new(MailConfigRepository::new(pool.clone()));
    let svc = web::Data::new(MailConfigService::new(
        repo,
        producer as Arc<dyn ods_common::events::EventProducer>,
        master_key,
    ));

    let user_id = Uuid::new_v4();
    let tenant_id = Uuid::new_v4();

    let jwt_config = test_jwt_config();
    let pool_data = web::Data::new(pool);

    let app = test::init_service(
        App::new()
            .app_data(pool_data)
            .app_data(jwt_config)
            .app_data(svc)
            .route("/health", web::get().to(health::health))
            .route(
                "/api/v1/mail-configs",
                web::post().to(mail_configs::create_mail_config),
            )
            .route(
                "/api/v1/mail-configs",
                web::get().to(mail_configs::list_mail_configs),
            )
            .route(
                "/api/v1/mail-configs/{id}",
                web::get().to(mail_configs::get_mail_config),
            )
            .route(
                "/api/v1/mail-configs/{id}",
                web::put().to(mail_configs::update_mail_config),
            )
            .route(
                "/api/v1/mail-configs/{id}",
                web::delete().to(mail_configs::delete_mail_config),
            ),
    )
    .await;

    (app, user_id, tenant_id)
}

#[actix_rt::test]
async fn test_401_no_auth_header() {
    let (app, _, _) = setup_app_with_db().await;

    let req = test::TestRequest::get()
        .uri("/api/v1/mail-configs")
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), actix_web::http::StatusCode::UNAUTHORIZED);
}

#[actix_rt::test]
async fn test_401_invalid_bearer_token() {
    let (app, _, _) = setup_app_with_db().await;

    let req = test::TestRequest::get()
        .uri("/api/v1/mail-configs")
        .insert_header(("Authorization", "Bearer invalid.token.here"))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), actix_web::http::StatusCode::UNAUTHORIZED);
}

#[actix_rt::test]
async fn test_404_config_not_found() {
    let (app, user_id, tenant_id) = setup_app_with_db().await;
    let token = make_test_jwt(&user_id, &tenant_id);
    let fake_id = Uuid::new_v4();

    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/mail-configs/{fake_id}"))
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), actix_web::http::StatusCode::NOT_FOUND);
}

#[actix_rt::test]
async fn test_crud_flow() {
    let (app, user_id, tenant_id) = setup_app_with_db().await;
    let token = make_test_jwt(&user_id, &tenant_id);

    // CREATE
    let create_body = json!({
        "name": "Test SMTP Config",
        "smtp_host": "smtp.example.com",
        "smtp_port": 587,
        "smtp_username": "user@example.com",
        "smtp_password": "secret123",
        "smtp_encryption": "tls",
        "from_address": "noreply@example.com",
        "from_name": "Test Sender",
        "is_default": false
    });

    let req = test::TestRequest::post()
        .uri("/api/v1/mail-configs")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .insert_header(("Content-Type", "application/json"))
        .set_json(&create_body)
        .to_request();

    let resp = test::call_service(&app, req).await;
    if resp.status() != actix_web::http::StatusCode::CREATED {
        let status = resp.status();
        let body = test::read_body(resp).await;
        panic!(
            "Expected 201, got {}: {}",
            status,
            String::from_utf8_lossy(&body)
        );
    }

    let body: serde_json::Value = test::read_body_json(resp).await;
    let config_id = body["id"].as_str().unwrap();
    assert_eq!(body["name"], "Test SMTP Config");
    assert_eq!(body["smtp_host"], "smtp.example.com");
    assert_eq!(body["tenant_id"], tenant_id.to_string());

    // GET by ID
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/mail-configs/{config_id}"))
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), actix_web::http::StatusCode::OK);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["id"], config_id);

    // LIST
    let req = test::TestRequest::get()
        .uri("/api/v1/mail-configs")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), actix_web::http::StatusCode::OK);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body["total"].as_i64().unwrap() >= 1);
    assert!(!body["data"].as_array().unwrap().is_empty());

    // UPDATE
    let update_body = json!({
        "name": "Updated SMTP Config"
    });

    let req = test::TestRequest::put()
        .uri(&format!("/api/v1/mail-configs/{config_id}"))
        .insert_header(("Authorization", format!("Bearer {token}")))
        .insert_header(("Content-Type", "application/json"))
        .set_json(&update_body)
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), actix_web::http::StatusCode::OK);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["name"], "Updated SMTP Config");

    // DELETE (soft)
    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/mail-configs/{config_id}"))
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), actix_web::http::StatusCode::NO_CONTENT);

    // GET after delete should return 404
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/mail-configs/{config_id}"))
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), actix_web::http::StatusCode::NOT_FOUND);
}

#[actix_rt::test]
async fn test_health_endpoint_no_auth_required() {
    let (app, _, _) = setup_app_with_db().await;

    let req = test::TestRequest::get().uri("/health").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), actix_web::http::StatusCode::OK);
}

#[actix_rt::test]
async fn test_db_connectivity() {
    let _ = dotenvy::dotenv();
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let pool = db::create_pool(&database_url, "securemail", 5)
        .await
        .expect("Failed to create pool");

    // Clear stale migration tracking so idempotent migrations can re-run cleanly
    pool.execute(
        "DELETE FROM securemail._sqlx_migrations WHERE description LIKE '%securemail%' OR description LIKE '%create_schema%' OR description LIKE '%create_mail%' OR description LIKE '%create_encryption%' OR description LIKE '%create_template%' OR description LIKE '%create_email%'"
    )
    .await
    .ok();

    // Run migrations
    db::run_migrations(&pool)
        .await
        .expect("Failed to run securemail migrations");

    // Check table exists
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema='securemail' AND table_name='mail_configs'"
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to query information_schema");

    eprintln!("mail_configs table exists: {}", row.0 > 0);
    assert!(row.0 > 0, "securemail.mail_configs table must exist");

    // Try to insert directly
    let tenant_id = Uuid::new_v4();
    let result = sqlx::query("SELECT set_config('app.tenant_id', $1, true)")
        .bind(tenant_id.to_string())
        .execute(&pool)
        .await;
    eprintln!("set_config result: {:?}", result);
}
