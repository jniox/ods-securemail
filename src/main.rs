use std::sync::Arc;

use actix_web::{web, App, HttpServer};
use tracing_actix_web::TracingLogger;

use ods_common::auth::JwtConfig;

use ods_securemail::api::{health, mail_configs};
use ods_securemail::config::AppConfig;
use ods_securemail::events::producer::{InMemoryProducer, RedpandaProducer};
use ods_securemail::repository::{db, mail_config::MailConfigRepository};
use ods_securemail::service::mail_config_service::MailConfigService;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    // Load .env file (optional, not required in production)
    let _ = dotenvy::dotenv();

    // Initialize structured JSON logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .json()
        .init();

    // Load application config
    let config = AppConfig::from_env().expect("Failed to load config");
    tracing::info!(
        host = %config.host,
        port = %config.port,
        "Starting securemail"
    );

    // Initialize JWT config from environment
    let jwt_config = JwtConfig::from_env().expect("Failed to configure JWT");
    tracing::info!("JWT configured");

    // Create database pool with schema search_path
    let pool = db::create_pool(&config.database_url, "securemail", 20)
        .await
        .expect("Failed to create database pool");

    // Run migrations
    db::run_migrations(&pool)
        .await
        .expect("Failed to run database migrations");

    tracing::info!("Database pool created and migrations applied");

    // Create event producer
    let event_producer: Arc<dyn ods_common::events::EventProducer> =
        if config.kafka_brokers != "localhost:9092" || std::env::var("FORCE_KAFKA").is_ok() {
            tracing::info!(brokers = %config.kafka_brokers, topic = %config.kafka_topic, "Connecting to Redpanda");
            Arc::new(
                RedpandaProducer::new(&config.kafka_brokers, &config.kafka_topic)
                    .expect("Failed to create Redpanda producer"),
            )
        } else {
            tracing::warn!("Using in-memory event producer (dev mode)");
            Arc::new(InMemoryProducer::new())
        };

    // Parse master encryption key (hex string to bytes)
    let master_key = hex::decode(&config.master_encryption_key)
        .expect("MASTER_ENCRYPTION_KEY must be valid hex (validated at startup)");

    // Create repository and service
    let mail_config_repo = Arc::new(MailConfigRepository::new(pool.clone()));
    let mail_config_svc = web::Data::new(MailConfigService::new(
        mail_config_repo,
        event_producer.clone(),
        master_key,
    ));

    let pool_data = web::Data::new(pool.clone());
    let jwt_config_data = web::Data::new(jwt_config);
    let max_body_size = config.max_body_size;
    let bind_addr = config.bind_address();

    tracing::info!(addr = %bind_addr, "Starting HTTP server");

    HttpServer::new(move || {
        let cors = config.build_cors();

        App::new()
            .wrap(TracingLogger::default())
            .wrap(cors)
            .app_data(web::JsonConfig::default().limit(max_body_size))
            .app_data(web::PayloadConfig::default().limit(max_body_size))
            .app_data(pool_data.clone())
            .app_data(jwt_config_data.clone())
            .app_data(mail_config_svc.clone())
            // Health endpoints (public, no auth)
            .route("/health", web::get().to(health::health))
            .route("/ready", web::get().to(health::ready))
            // Mail config CRUD (authenticated)
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
            )
    })
    .bind(&bind_addr)?
    .run()
    .await
}
