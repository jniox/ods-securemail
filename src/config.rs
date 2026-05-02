use std::env;

/// Application configuration loaded from environment variables.
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub host: String,
    pub port: u16,
    pub database_url: String,
    pub kafka_brokers: String,
    pub kafka_topic: String,
    pub log_level: String,
    pub cors_allowed_origins: Vec<String>,
    pub max_body_size: usize,
    pub master_encryption_key: String,
}

impl AppConfig {
    pub fn from_env() -> Result<Self, String> {
        let cors_allowed_origins = env::var("CORS_ALLOWED_ORIGINS")
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect();

        let max_body_size = env::var("MAX_BODY_SIZE_BYTES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(26_214_400); // 25 MiB (max attachment total)

        let port = env::var("PORT")
            .or_else(|_| env::var("SERVICE_PORT"))
            .unwrap_or_else(|_| "8086".to_string())
            .parse()
            .unwrap_or(8086);

        let kafka_brokers = env::var("KAFKA_BROKERS")
            .or_else(|_| env::var("REDPANDA_BROKERS"))
            .unwrap_or_else(|_| "localhost:9092".to_string());

        let kafka_topic = env::var("KAFKA_TOPIC")
            .or_else(|_| env::var("REDPANDA_TOPIC"))
            .unwrap_or_else(|_| "ods.securemail.events".to_string());

        let database_url = env::var("DATABASE_URL")
            .map_err(|_| "DATABASE_URL environment variable is required".to_string())?;

        let master_encryption_key = env::var("MASTER_ENCRYPTION_KEY")
            .map_err(|_| "MASTER_ENCRYPTION_KEY environment variable is required".to_string())?;

        Ok(Self {
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port,
            database_url,
            kafka_brokers,
            kafka_topic,
            log_level: env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string()),
            cors_allowed_origins,
            max_body_size,
            master_encryption_key,
        })
    }

    pub fn bind_address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    pub fn build_cors(&self) -> actix_cors::Cors {
        let mut cors = actix_cors::Cors::default()
            .allowed_methods(vec!["GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"])
            .allowed_headers(vec![
                actix_web::http::header::AUTHORIZATION,
                actix_web::http::header::CONTENT_TYPE,
                actix_web::http::header::HeaderName::from_static("x-tenant-id"),
                actix_web::http::header::HeaderName::from_static("x-correlation-id"),
                actix_web::http::header::HeaderName::from_static("x-source-service"),
            ])
            .max_age(3600);

        for origin in &self.cors_allowed_origins {
            cors = cors.allowed_origin(origin);
        }

        cors
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn with_env_vars<F: FnOnce()>(vars: &[(&str, &str)], f: F) {
        let _lock = ENV_LOCK.lock().unwrap();
        let originals: Vec<(&str, Option<String>)> =
            vars.iter().map(|(k, _)| (*k, env::var(k).ok())).collect();

        for (k, v) in vars {
            unsafe { env::set_var(k, v) };
        }

        f();

        for (k, original) in &originals {
            match original {
                Some(v) => unsafe { env::set_var(k, v) },
                None => unsafe { env::remove_var(k) },
            }
        }
    }

    #[test]
    fn test_config_from_env_with_required_vars() {
        with_env_vars(
            &[
                ("DATABASE_URL", "postgres://test:test@localhost/test"),
                ("MASTER_ENCRYPTION_KEY", "0000000000000000000000000000000000000000000000000000000000000000"),
            ],
            || {
                let config = AppConfig::from_env().unwrap();
                assert_eq!(config.database_url, "postgres://test:test@localhost/test");
                assert_eq!(config.port, 8086);
            },
        );
    }

    #[test]
    fn test_config_missing_database_url() {
        let _lock = ENV_LOCK.lock().unwrap();
        unsafe { env::remove_var("DATABASE_URL") };
        let result = AppConfig::from_env();
        assert!(result.is_err());
    }

    #[test]
    fn test_config_missing_master_key() {
        with_env_vars(
            &[("DATABASE_URL", "postgres://test:test@localhost/test")],
            || {
                unsafe { env::remove_var("MASTER_ENCRYPTION_KEY") };
                let result = AppConfig::from_env();
                assert!(result.is_err());
            },
        );
    }

    #[test]
    fn test_bind_address() {
        with_env_vars(
            &[
                ("DATABASE_URL", "postgres://test:test@localhost/test"),
                ("MASTER_ENCRYPTION_KEY", "0000000000000000000000000000000000000000000000000000000000000000"),
                ("HOST", "127.0.0.1"),
                ("PORT", "9090"),
            ],
            || {
                let config = AppConfig::from_env().unwrap();
                assert_eq!(config.bind_address(), "127.0.0.1:9090");
            },
        );
    }
}
