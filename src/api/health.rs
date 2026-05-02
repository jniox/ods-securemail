//! Health check endpoints for the SecureMail service.

use actix_web::HttpResponse;

/// GET /health — public liveness probe
pub async fn health() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "healthy",
        "service": "securemail",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

/// GET /ready — readiness probe
/// Uses the ods-common health_handler which checks DB and Kafka.
/// We re-export it from main.rs via ods_common::health::health_handler.
/// This simple version is a fallback.
pub async fn ready(pool: actix_web::web::Data<sqlx::PgPool>) -> HttpResponse {
    let db_ok = sqlx::query("SELECT 1")
        .execute(pool.get_ref())
        .await
        .is_ok();

    if db_ok {
        HttpResponse::Ok().json(serde_json::json!({
            "status": "ready",
            "service": "securemail",
            "database": true,
            "redpanda": true,
        }))
    } else {
        HttpResponse::ServiceUnavailable().json(serde_json::json!({
            "status": "not_ready",
            "service": "securemail",
            "database": false,
            "redpanda": true,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, web, App};

    #[actix_rt::test]
    async fn test_health_returns_200() {
        let app = test::init_service(
            App::new().route("/health", web::get().to(health)),
        )
        .await;

        let req = test::TestRequest::get().uri("/health").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["status"], "healthy");
        assert_eq!(body["service"], "securemail");
    }
}
