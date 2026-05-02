use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

/// SMTP encryption modes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text")]
pub enum SmtpEncryption {
    #[serde(rename = "tls")]
    #[sqlx(rename = "tls")]
    Tls,
    #[serde(rename = "starttls")]
    #[sqlx(rename = "starttls")]
    Starttls,
    #[serde(rename = "none")]
    #[sqlx(rename = "none")]
    None,
}

impl std::fmt::Display for SmtpEncryption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SmtpEncryption::Tls => write!(f, "tls"),
            SmtpEncryption::Starttls => write!(f, "starttls"),
            SmtpEncryption::None => write!(f, "none"),
        }
    }
}

/// Mail configuration as stored in the database.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct MailConfig {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub smtp_host: String,
    pub smtp_port: i32,
    pub smtp_username: String,
    pub smtp_encryption: String,
    pub imap_host: Option<String>,
    pub imap_port: Option<i32>,
    pub imap_username: Option<String>,
    pub imap_encryption: Option<String>,
    pub from_address: String,
    pub from_name: String,
    pub is_default: bool,
    pub verified: bool,
    pub deleted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// API response for a mail config (passwords redacted).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MailConfigResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub smtp_host: String,
    pub smtp_port: i32,
    pub smtp_encryption: String,
    pub imap_host: Option<String>,
    pub imap_port: Option<i32>,
    pub imap_encryption: Option<String>,
    pub from_address: String,
    pub from_name: String,
    pub is_default: bool,
    pub verified: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<MailConfig> for MailConfigResponse {
    fn from(mc: MailConfig) -> Self {
        Self {
            id: mc.id,
            tenant_id: mc.tenant_id,
            name: mc.name,
            smtp_host: mc.smtp_host,
            smtp_port: mc.smtp_port,
            smtp_encryption: mc.smtp_encryption,
            imap_host: mc.imap_host,
            imap_port: mc.imap_port,
            imap_encryption: mc.imap_encryption,
            from_address: mc.from_address,
            from_name: mc.from_name,
            is_default: mc.is_default,
            verified: mc.verified,
            created_at: mc.created_at,
            updated_at: mc.updated_at,
        }
    }
}

/// Request body for creating a mail configuration.
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct CreateMailConfig {
    #[validate(length(min = 1, max = 255))]
    pub name: String,
    #[validate(length(min = 1, max = 255))]
    pub smtp_host: String,
    pub smtp_port: i32,
    #[validate(length(min = 1, max = 255))]
    pub smtp_username: String,
    #[validate(length(min = 1))]
    pub smtp_password: String,
    pub smtp_encryption: String,
    pub imap_host: Option<String>,
    pub imap_port: Option<i32>,
    pub imap_username: Option<String>,
    pub imap_password: Option<String>,
    pub imap_encryption: Option<String>,
    #[validate(email)]
    pub from_address: String,
    #[validate(length(min = 1, max = 255))]
    pub from_name: String,
    #[serde(default)]
    pub is_default: bool,
}

/// Request body for updating a mail configuration.
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct UpdateMailConfig {
    #[validate(length(min = 1, max = 255))]
    pub name: Option<String>,
    #[validate(length(min = 1, max = 255))]
    pub smtp_host: Option<String>,
    pub smtp_port: Option<i32>,
    #[validate(length(min = 1, max = 255))]
    pub smtp_username: Option<String>,
    pub smtp_password: Option<String>,
    pub smtp_encryption: Option<String>,
    pub imap_host: Option<String>,
    pub imap_port: Option<i32>,
    pub imap_username: Option<String>,
    pub imap_password: Option<String>,
    pub imap_encryption: Option<String>,
    pub from_address: Option<String>,
    pub from_name: Option<String>,
    pub is_default: Option<bool>,
}

/// Paginated list response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedResponse<T> {
    pub data: Vec<T>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mail_config_response_from_mail_config() {
        let mc = MailConfig {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            name: "Test Config".to_string(),
            smtp_host: "smtp.example.com".to_string(),
            smtp_port: 587,
            smtp_username: "user@example.com".to_string(),
            smtp_encryption: "tls".to_string(),
            imap_host: None,
            imap_port: None,
            imap_username: None,
            imap_encryption: None,
            from_address: "noreply@example.com".to_string(),
            from_name: "Test".to_string(),
            is_default: true,
            verified: false,
            deleted_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let resp = MailConfigResponse::from(mc.clone());
        assert_eq!(resp.id, mc.id);
        assert_eq!(resp.name, "Test Config");
        assert_eq!(resp.smtp_host, "smtp.example.com");
        assert!(resp.is_default);
    }

    #[test]
    fn test_smtp_encryption_display() {
        assert_eq!(SmtpEncryption::Tls.to_string(), "tls");
        assert_eq!(SmtpEncryption::Starttls.to_string(), "starttls");
        assert_eq!(SmtpEncryption::None.to_string(), "none");
    }

    #[test]
    fn test_create_mail_config_validation() {
        let cmd = CreateMailConfig {
            name: "".to_string(), // invalid - empty
            smtp_host: "smtp.example.com".to_string(),
            smtp_port: 587,
            smtp_username: "user".to_string(),
            smtp_password: "pass".to_string(),
            smtp_encryption: "tls".to_string(),
            imap_host: None,
            imap_port: None,
            imap_username: None,
            imap_password: None,
            imap_encryption: None,
            from_address: "noreply@example.com".to_string(),
            from_name: "Test".to_string(),
            is_default: false,
        };
        assert!(cmd.validate().is_err());
    }

    #[test]
    fn test_create_mail_config_valid() {
        let cmd = CreateMailConfig {
            name: "My Config".to_string(),
            smtp_host: "smtp.example.com".to_string(),
            smtp_port: 587,
            smtp_username: "user".to_string(),
            smtp_password: "pass".to_string(),
            smtp_encryption: "tls".to_string(),
            imap_host: None,
            imap_port: None,
            imap_username: None,
            imap_password: None,
            imap_encryption: None,
            from_address: "noreply@example.com".to_string(),
            from_name: "Test".to_string(),
            is_default: false,
        };
        assert!(cmd.validate().is_ok());
    }
}
