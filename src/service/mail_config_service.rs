use std::sync::Arc;
use uuid::Uuid;

use crate::domain::mail_config::{
    CreateMailConfig, MailConfig, MailConfigResponse, PaginatedResponse, UpdateMailConfig,
};
use crate::error::{AppError, AppResult};
use crate::events::producer::{event_types, CloudEvent, EventProducer, EVENT_SOURCE};
use crate::repository::mail_config::MailConfigRepository;

pub struct MailConfigService {
    repo: Arc<MailConfigRepository>,
    event_producer: Arc<dyn EventProducer>,
    master_key: Vec<u8>,
}

impl MailConfigService {
    pub fn new(
        repo: Arc<MailConfigRepository>,
        event_producer: Arc<dyn EventProducer>,
        master_key: Vec<u8>,
    ) -> Self {
        Self {
            repo,
            event_producer,
            master_key,
        }
    }

    /// Encrypt a password using the master key (placeholder XOR for now, will be AES-256-GCM).
    fn encrypt_password(&self, password: &str) -> Vec<u8> {
        // Simple XOR with master key for development; production will use AES-256-GCM
        let key = &self.master_key;
        password
            .as_bytes()
            .iter()
            .enumerate()
            .map(|(i, b)| b ^ key[i % key.len()])
            .collect()
    }

    pub async fn create(
        &self,
        tenant_id: Uuid,
        cmd: CreateMailConfig,
    ) -> AppResult<MailConfigResponse> {
        // Validate smtp_encryption
        Self::validate_encryption(&cmd.smtp_encryption)?;
        if let Some(ref enc) = cmd.imap_encryption {
            Self::validate_encryption(enc)?;
        }

        // Validate smtp_port
        Self::validate_smtp_port(cmd.smtp_port)?;

        // Encrypt passwords
        let smtp_password_encrypted = self.encrypt_password(&cmd.smtp_password);
        let imap_password_encrypted = cmd
            .imap_password
            .as_ref()
            .map(|p| self.encrypt_password(p));

        let config = self
            .repo
            .create(
                tenant_id,
                &cmd,
                &smtp_password_encrypted,
                imap_password_encrypted.as_deref(),
            )
            .await?;

        // Emit event
        let event = CloudEvent::new(
            EVENT_SOURCE,
            event_types::CONFIG_CREATED,
            &config.id.to_string(),
            tenant_id,
            serde_json::json!({
                "config_id": config.id,
                "name": config.name,
                "smtp_host": config.smtp_host,
            }),
        );
        let _ = self.event_producer.publish(event).await;

        Ok(MailConfigResponse::from(config))
    }

    pub async fn get_by_id(
        &self,
        tenant_id: Uuid,
        id: Uuid,
    ) -> AppResult<MailConfigResponse> {
        let config = self.repo.get_by_id(tenant_id, id).await?;
        Ok(MailConfigResponse::from(config))
    }

    pub async fn list(
        &self,
        tenant_id: Uuid,
        page: i64,
        per_page: i64,
    ) -> AppResult<PaginatedResponse<MailConfigResponse>> {
        let page = page.max(1);
        let per_page = per_page.clamp(1, 100);

        let (configs, total) = self.repo.list(tenant_id, page, per_page).await?;
        let data = configs.into_iter().map(MailConfigResponse::from).collect();

        Ok(PaginatedResponse {
            data,
            total,
            page,
            per_page,
        })
    }

    pub async fn update(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        cmd: UpdateMailConfig,
    ) -> AppResult<MailConfigResponse> {
        if let Some(ref enc) = cmd.smtp_encryption {
            Self::validate_encryption(enc)?;
        }
        if let Some(ref enc) = cmd.imap_encryption {
            Self::validate_encryption(enc)?;
        }
        if let Some(port) = cmd.smtp_port {
            Self::validate_smtp_port(port)?;
        }

        let smtp_password_encrypted = cmd.smtp_password.as_ref().map(|p| self.encrypt_password(p));
        let imap_password_encrypted = cmd.imap_password.as_ref().map(|p| self.encrypt_password(p));

        let config = self
            .repo
            .update(
                tenant_id,
                id,
                &cmd,
                smtp_password_encrypted.as_deref(),
                imap_password_encrypted.as_deref(),
            )
            .await?;

        Ok(MailConfigResponse::from(config))
    }

    pub async fn delete(&self, tenant_id: Uuid, id: Uuid) -> AppResult<()> {
        self.repo.soft_delete(tenant_id, id).await
    }

    pub async fn verify(
        &self,
        tenant_id: Uuid,
        id: Uuid,
    ) -> AppResult<MailConfig> {
        // In a real implementation, we would attempt SMTP connection here.
        // For now, we just mark it as verified.
        let config = self.repo.set_verified(tenant_id, id, true).await?;

        // Emit verified event
        let event = CloudEvent::new(
            EVENT_SOURCE,
            event_types::CONFIG_VERIFIED,
            &config.id.to_string(),
            tenant_id,
            serde_json::json!({
                "config_id": config.id,
                "name": config.name,
                "smtp_host": config.smtp_host,
            }),
        );
        let _ = self.event_producer.publish(event).await;

        Ok(config)
    }

    fn validate_encryption(enc: &str) -> AppResult<()> {
        match enc {
            "tls" | "starttls" | "none" => Ok(()),
            _ => Err(AppError::Validation(format!(
                "Invalid encryption mode '{}'. Must be: tls, starttls, or none",
                enc
            ))),
        }
    }

    fn validate_smtp_port(port: i32) -> AppResult<()> {
        match port {
            25 | 465 | 587 => Ok(()),
            _ => Err(AppError::Validation(format!(
                "Invalid SMTP port '{}'. Must be: 25, 465, or 587",
                port
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::producer::InMemoryProducer;

    fn make_service() -> (MailConfigService, Arc<InMemoryProducer>) {
        // We cannot actually test with the repo without a DB, so we test validation logic
        let producer = Arc::new(InMemoryProducer::new());
        let master_key = (1u8..=32).collect::<Vec<u8>>();
        // We'll create a dummy service for validation tests
        // The repo needs a pool, so we can only test pure logic here
        (
            MailConfigService {
                repo: Arc::new(MailConfigRepository::new(
                    // This will panic if used without a real pool, which is fine for unit tests
                    // that only test validation
                    sqlx::PgPool::connect_lazy("postgres://fake").unwrap(),
                )),
                event_producer: producer.clone(),
                master_key,
            },
            producer,
        )
    }

    #[test]
    fn test_validate_encryption_valid() {
        assert!(MailConfigService::validate_encryption("tls").is_ok());
        assert!(MailConfigService::validate_encryption("starttls").is_ok());
        assert!(MailConfigService::validate_encryption("none").is_ok());
    }

    #[test]
    fn test_validate_encryption_invalid() {
        assert!(MailConfigService::validate_encryption("ssl").is_err());
        assert!(MailConfigService::validate_encryption("").is_err());
    }

    #[test]
    fn test_validate_smtp_port_valid() {
        assert!(MailConfigService::validate_smtp_port(25).is_ok());
        assert!(MailConfigService::validate_smtp_port(465).is_ok());
        assert!(MailConfigService::validate_smtp_port(587).is_ok());
    }

    #[test]
    fn test_validate_smtp_port_invalid() {
        assert!(MailConfigService::validate_smtp_port(80).is_err());
        assert!(MailConfigService::validate_smtp_port(0).is_err());
        assert!(MailConfigService::validate_smtp_port(8080).is_err());
    }

    #[tokio::test]
    async fn test_encrypt_password() {
        let (svc, _) = make_service();
        let encrypted = svc.encrypt_password("hello");
        assert_ne!(encrypted, b"hello");
        assert_eq!(encrypted.len(), 5);
    }
}
