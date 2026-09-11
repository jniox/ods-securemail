use std::sync::Arc;
use uuid::Uuid;

use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{AeadCore, Aes256Gcm, Nonce};

use crate::domain::mail_config::{
    CreateMailConfig, MailConfig, MailConfigResponse, PaginatedResponse, UpdateMailConfig,
    VerificationResponse,
};
use crate::error::{AppError, AppResult};
use crate::events::producer::{event_types, CloudEvent, EventProducer, EVENT_SOURCE};
use crate::repository::mail_config::MailConfigRepository;
use crate::service::smtp_verifier::{SmtpTarget, SmtpVerifier, SmtpVerifyError};

pub struct MailConfigService {
    repo: Arc<MailConfigRepository>,
    event_producer: Arc<dyn EventProducer>,
    master_key: Vec<u8>,
    smtp_verifier: Arc<dyn SmtpVerifier>,
}

/// Ce que rend une verification : la configuration telle qu'elle est APRES le verdict,
/// et la raison de l'echec s'il y en a une.
pub struct VerificationOutcome {
    pub config: MailConfig,
    pub error: Option<SmtpVerifyError>,
}

impl VerificationOutcome {
    pub fn verified(&self) -> bool {
        self.error.is_none()
    }

    pub fn to_response(&self) -> VerificationResponse {
        VerificationResponse {
            config_id: self.config.id,
            verified: self.verified(),
            reason: self.error.as_ref().map(|e| e.kind().to_string()),
            message: match &self.error {
                None => "SMTP connection verified".to_string(),
                Some(e) => e.message().to_string(),
            },
            checked_at: self.config.updated_at,
        }
    }
}

impl MailConfigService {
    pub fn new(
        repo: Arc<MailConfigRepository>,
        event_producer: Arc<dyn EventProducer>,
        master_key: Vec<u8>,
        smtp_verifier: Arc<dyn SmtpVerifier>,
    ) -> Self {
        Self {
            repo,
            event_producer,
            master_key,
            smtp_verifier,
        }
    }

    /// Encrypt a password using AES-256-GCM with the master key.
    /// Output format: nonce (12 bytes) || ciphertext (variable length).
    fn encrypt_password(&self, password: &str) -> Vec<u8> {
        let key = aes_gcm::Key::<Aes256Gcm>::from_slice(&self.master_key);
        let cipher = Aes256Gcm::new(key);
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ciphertext = cipher
            .encrypt(&nonce, password.as_bytes())
            .expect("AES-256-GCM encryption must not fail with valid key");
        // Prepend nonce to ciphertext
        let mut result = Vec::with_capacity(12 + ciphertext.len());
        result.extend_from_slice(&nonce);
        result.extend_from_slice(&ciphertext);
        result
    }

    /// Decrypt a password encrypted with AES-256-GCM.
    /// Input format: nonce (12 bytes) || ciphertext.
    fn decrypt_password(&self, encrypted: &[u8]) -> AppResult<String> {
        if encrypted.len() < 12 {
            return Err(AppError::Internal(
                "Invalid encrypted data: too short".to_string(),
            ));
        }
        let (nonce_bytes, ciphertext) = encrypted.split_at(12);
        let key = aes_gcm::Key::<Aes256Gcm>::from_slice(&self.master_key);
        let cipher = Aes256Gcm::new(key);
        let nonce = Nonce::from_slice(nonce_bytes);
        let plaintext = cipher.decrypt(nonce, ciphertext).map_err(|_| {
            AppError::Internal(
                "Failed to decrypt password: invalid key or corrupted data".to_string(),
            )
        })?;
        String::from_utf8(plaintext)
            .map_err(|_| AppError::Internal("Decrypted password is not valid UTF-8".to_string()))
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
        let imap_password_encrypted = cmd.imap_password.as_ref().map(|p| self.encrypt_password(p));

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

    pub async fn get_by_id(&self, tenant_id: Uuid, id: Uuid) -> AppResult<MailConfigResponse> {
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

    /// Compose le serveur de messagerie de la configuration et persiste le verdict.
    ///
    /// Le mot de passe est dechiffre ici, utilise le temps de l'appel, et jamais rendu.
    /// Le drapeau `verified` suit le DERNIER dial : une configuration qui cesse de
    /// repondre redevient non verifiee, sinon le drapeau raconte le passe.
    pub async fn verify(&self, tenant_id: Uuid, id: Uuid) -> AppResult<VerificationOutcome> {
        let (config, smtp_password_encrypted) =
            self.repo.get_with_smtp_password(tenant_id, id).await?;

        // Une erreur ici n'est pas un verdict sur le serveur du tenant : c'est notre cle
        // qui ne rouvre plus notre propre coffre. Elle remonte en 500, pas en `verified: false`.
        let password = self.decrypt_password(&smtp_password_encrypted)?;

        let dial = match u16::try_from(config.smtp_port) {
            Ok(port) => {
                let target = SmtpTarget {
                    host: config.smtp_host.clone(),
                    port,
                    username: config.smtp_username.clone(),
                    password,
                    encryption: config.smtp_encryption.clone(),
                };
                self.smtp_verifier.verify(&target).await
            }
            Err(_) => Err(SmtpVerifyError::Config(format!(
                "Invalid SMTP port '{}' stored for this configuration",
                config.smtp_port
            ))),
        };

        let verified = dial.is_ok();
        let config = self.repo.set_verified(tenant_id, id, verified).await?;

        match &dial {
            Ok(()) => {
                tracing::info!(
                    config_id = %config.id,
                    tenant_id = %tenant_id,
                    "SMTP configuration verified"
                );
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
            }
            Err(err) => {
                // Le motif, jamais le texte brut du serveur ni l'identifiant utilise.
                tracing::warn!(
                    config_id = %config.id,
                    tenant_id = %tenant_id,
                    reason = err.kind(),
                    "SMTP verification refused"
                );
            }
        }

        Ok(VerificationOutcome {
            config,
            error: dial.err(),
        })
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
    use crate::service::smtp_verifier::FixedSmtpVerifier;

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
                smtp_verifier: Arc::new(FixedSmtpVerifier::accepting()),
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
    async fn test_encrypt_decrypt_password() {
        let (svc, _) = make_service();
        let encrypted = svc.encrypt_password("hello");
        // AES-256-GCM: 12-byte nonce + plaintext len + 16-byte auth tag
        assert_eq!(encrypted.len(), 12 + 5 + 16);
        assert_ne!(&encrypted[12..], b"hello");

        let decrypted = svc.decrypt_password(&encrypted).unwrap();
        assert_eq!(decrypted, "hello");
    }

    #[tokio::test]
    async fn test_encrypt_produces_different_ciphertext_each_time() {
        let (svc, _) = make_service();
        let enc1 = svc.encrypt_password("same");
        let enc2 = svc.encrypt_password("same");
        // Random nonce means different ciphertext each time
        assert_ne!(enc1, enc2);
    }

    #[tokio::test]
    async fn test_decrypt_invalid_data() {
        let (svc, _) = make_service();
        // Too short
        let result = svc.decrypt_password(&[0u8; 5]);
        assert!(result.is_err());
        // Corrupted ciphertext
        let mut encrypted = svc.encrypt_password("test");
        encrypted[15] ^= 0xFF; // flip a byte in the ciphertext
        let result = svc.decrypt_password(&encrypted);
        assert!(result.is_err());
    }
}
