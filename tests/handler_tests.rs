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
use ods_securemail::service::smtp_verifier::{FixedSmtpVerifier, SmtpVerifier, SmtpVerifyError};
use sqlx::Executor;

const TEST_SECRET: &[u8] = b"ods-common-test-secret-32-chars!";

/// Les migrations tournent UNE fois par binaire de test, pas une fois par test.
///
/// Chaque test ouvrait sa propre piscine, effacait les lignes 1-6 du registre puis
/// rejouait les migrations. A onze tests en parallele sur le MEME schema, un test pouvait
/// effacer le registre pendant qu'un autre migrait, et un `ALTER TABLE ... ENABLE ROW
/// LEVEL SECURITY` concurrent d'un INSERT rendait un 500 qui n'avait rien a voir avec le
/// code teste. L'ordre destructeur reste qualifie par son schema (lecon du 2026-09-10).
static SCHEMA_READY: tokio::sync::OnceCell<()> = tokio::sync::OnceCell::const_new();

async fn ensure_schema(pool: &sqlx::PgPool) {
    SCHEMA_READY
        .get_or_init(|| async {
            pool.execute(
                "DELETE FROM securemail._sqlx_migrations WHERE version IN (1, 2, 3, 4, 5, 6)",
            )
            .await
            .ok();

            db::run_migrations(pool)
                .await
                .expect("Failed to run securemail migrations");
        })
        .await;
}

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
    // Par defaut le dial est une doublure qui accepte : les tests de CRUD n'ont rien a
    // dire sur la messagerie. Les tests de `/verify` injectent le verdict qu'ils veulent.
    setup_app_with_verifier(Arc::new(FixedSmtpVerifier::accepting()), None).await
}

/// `identity` fixe le couple (utilisateur, tenant) — utile quand un test doit rejouer
/// une requete sur la MEME configuration avec un autre verdict de dial.
async fn setup_app_with_verifier(
    smtp_verifier: Arc<dyn SmtpVerifier>,
    identity: Option<(Uuid, Uuid)>,
) -> (
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

    ensure_schema(&pool).await;

    let producer = Arc::new(InMemoryProducer::new());
    // 32-byte key for AES-256
    let master_key = vec![0x42u8; 32];

    let repo = Arc::new(MailConfigRepository::new(pool.clone()));
    let svc = web::Data::new(MailConfigService::new(
        repo,
        producer as Arc<dyn ods_common::events::EventProducer>,
        master_key,
        smtp_verifier,
    ));

    let (user_id, tenant_id) = identity.unwrap_or_else(|| (Uuid::new_v4(), Uuid::new_v4()));

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
            )
            .route(
                "/api/v1/mail-configs/{id}/verify",
                web::post().to(mail_configs::verify_mail_config),
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

    ensure_schema(&pool).await;

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

/// Cree une configuration de messagerie et rend son identifiant.
async fn create_config<S>(app: &S, token: &str, name: &str) -> String
where
    S: actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
{
    let req = test::TestRequest::post()
        .uri("/api/v1/mail-configs")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .insert_header(("Content-Type", "application/json"))
        .set_json(json!({
            "name": name,
            "smtp_host": "smtp.example.com",
            "smtp_port": 587,
            "smtp_username": "user@example.com",
            "smtp_password": "secret123",
            "smtp_encryption": "starttls",
            "from_address": "noreply@example.com",
            "from_name": "Test Sender",
            "is_default": false
        }))
        .to_request();

    let resp = test::call_service(app, req).await;
    let status = resp.status();
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(
        status,
        actix_web::http::StatusCode::CREATED,
        "create failed: {body}"
    );
    body["id"].as_str().expect("id").to_string()
}

async fn read_verified_flag<S>(app: &S, token: &str, config_id: &str) -> bool
where
    S: actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
{
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/mail-configs/{config_id}"))
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();
    let resp = test::call_service(app, req).await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    body["verified"].as_bool().expect("verified flag")
}

/// AC-006 — le verdict rendu est celui du dial, et il est persiste.
#[actix_rt::test]
async fn test_verify_accepts_when_the_dial_succeeds() {
    let (app, user_id, tenant_id) =
        setup_app_with_verifier(Arc::new(FixedSmtpVerifier::accepting()), None).await;
    let token = make_test_jwt(&user_id, &tenant_id);
    let config_id = create_config(&app, &token, "Verify OK").await;

    assert!(
        !read_verified_flag(&app, &token, &config_id).await,
        "a fresh config starts unverified"
    );

    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/mail-configs/{config_id}/verify"))
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), actix_web::http::StatusCode::OK);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["verified"], true, "body: {body}");

    assert!(
        read_verified_flag(&app, &token, &config_id).await,
        "a successful dial must be persisted"
    );
}

/// AC-006 — le coeur du defaut signale : un serveur qui refuse ne doit PAS passer.
#[actix_rt::test]
async fn test_verify_refuses_when_the_dial_fails() {
    let (app, user_id, tenant_id) = setup_app_with_verifier(
        Arc::new(FixedSmtpVerifier::rejecting(SmtpVerifyError::Auth(
            "The mail server rejected the credentials for this configuration".to_string(),
        ))),
        None,
    )
    .await;
    let token = make_test_jwt(&user_id, &tenant_id);
    let config_id = create_config(&app, &token, "Verify KO").await;

    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/mail-configs/{config_id}/verify"))
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), actix_web::http::StatusCode::OK);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["verified"], false, "body: {body}");
    assert_eq!(body["reason"], "auth", "body: {body}");

    assert!(
        !read_verified_flag(&app, &token, &config_id).await,
        "a refused dial must never leave the config marked verified"
    );
}

/// AC-006 — une configuration deja verifiee redevient non verifiee si le serveur cesse
/// de repondre : le drapeau suit le monde reel, il ne se contente pas d'etre mis une fois.
#[actix_rt::test]
async fn test_verify_unsets_the_flag_when_the_server_stops_answering() {
    let user_id = Uuid::new_v4();
    let tenant_id = Uuid::new_v4();
    let token = make_test_jwt(&user_id, &tenant_id);

    let (app_ok, _, _) = setup_app_with_verifier(
        Arc::new(FixedSmtpVerifier::accepting()),
        Some((user_id, tenant_id)),
    )
    .await;
    let config_id = create_config(&app_ok, &token, "Verify then break").await;

    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/mail-configs/{config_id}/verify"))
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();
    test::call_service(&app_ok, req).await;
    assert!(read_verified_flag(&app_ok, &token, &config_id).await);

    // Meme tenant, meme configuration, mais le serveur ne repond plus.
    let (app_ko, _, _) = setup_app_with_verifier(
        Arc::new(FixedSmtpVerifier::rejecting(SmtpVerifyError::Connect(
            "The mail server could not be reached".to_string(),
        ))),
        Some((user_id, tenant_id)),
    )
    .await;

    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/mail-configs/{config_id}/verify"))
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();
    let resp = test::call_service(&app_ko, req).await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["verified"], false, "body: {body}");
    assert_eq!(body["reason"], "connect", "body: {body}");

    assert!(
        !read_verified_flag(&app_ko, &token, &config_id).await,
        "the flag must follow the last dial, not the first one"
    );
}

/// AC-007 — le 409 passe par la route, pas seulement par le type d'erreur.
#[actix_rt::test]
async fn test_409_duplicate_config_name_for_the_same_tenant() {
    let (app, user_id, tenant_id) = setup_app_with_db().await;
    let token = make_test_jwt(&user_id, &tenant_id);

    let name = format!("Duplicate {}", Uuid::new_v4());
    create_config(&app, &token, &name).await;

    let req = test::TestRequest::post()
        .uri("/api/v1/mail-configs")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .insert_header(("Content-Type", "application/json"))
        .set_json(json!({
            "name": name,
            "smtp_host": "smtp.other.example.com",
            "smtp_port": 465,
            "smtp_username": "someone@example.com",
            "smtp_password": "another-secret",
            "smtp_encryption": "tls",
            "from_address": "noreply@example.com",
            "from_name": "Test Sender",
            "is_default": false
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        actix_web::http::StatusCode::CONFLICT,
        "a second config with the same name for the same tenant must be refused"
    );
    // Le corps est celui d'`ods-common` : il donne la FAMILLE d'erreur et rien du contenu
    // en conflit — c'est voulu, un message d'erreur ne doit pas confirmer l'existence d'une
    // ressource. La preuve attendue ici est le 409 rendu par la route, pas le texte.
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["error"], "conflict", "body: {body}");
}

/// Le meme nom chez un AUTRE tenant n'est pas un conflit — l'unicite est par tenant.
#[actix_rt::test]
async fn test_the_same_name_is_free_for_another_tenant() {
    let name = format!("Shared name {}", Uuid::new_v4());

    let (app_a, user_a, tenant_a) = setup_app_with_db().await;
    create_config(&app_a, &make_test_jwt(&user_a, &tenant_a), &name).await;

    let (app_b, user_b, tenant_b) = setup_app_with_db().await;
    create_config(&app_b, &make_test_jwt(&user_b, &tenant_b), &name).await;
}
