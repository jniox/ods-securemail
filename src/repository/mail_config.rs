use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::mail_config::{CreateMailConfig, MailConfig, UpdateMailConfig};
use crate::error::{AppError, AppResult};
use crate::repository::db::begin_tenant_tx;

pub struct MailConfigRepository {
    pool: PgPool,
}

impl MailConfigRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(
        &self,
        tenant_id: Uuid,
        cmd: &CreateMailConfig,
        smtp_password_encrypted: &[u8],
        imap_password_encrypted: Option<&[u8]>,
    ) -> AppResult<MailConfig> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;

        // If is_default, unset other defaults for this tenant
        if cmd.is_default {
            sqlx::query(
                "UPDATE securemail.mail_configs SET is_default = false WHERE tenant_id = $1 AND deleted_at IS NULL",
            )
            .bind(tenant_id)
            .execute(&mut *tx)
            .await?;
        }

        // If this is the first config for the tenant, make it default
        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM securemail.mail_configs WHERE tenant_id = $1 AND deleted_at IS NULL",
        )
        .bind(tenant_id)
        .fetch_one(&mut *tx)
        .await?;

        let is_default = cmd.is_default || count.0 == 0;

        let config = sqlx::query_as::<_, MailConfig>(
            r#"
            INSERT INTO securemail.mail_configs (
                tenant_id, name, smtp_host, smtp_port, smtp_username,
                smtp_password_encrypted, smtp_encryption,
                imap_host, imap_port, imap_username, imap_password_encrypted, imap_encryption,
                from_address, from_name, is_default
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
            RETURNING id, tenant_id, name, smtp_host, smtp_port, smtp_username,
                      smtp_encryption, imap_host, imap_port, imap_username, imap_encryption,
                      from_address, from_name, is_default, verified, deleted_at, created_at, updated_at
            "#,
        )
        .bind(tenant_id)
        .bind(&cmd.name)
        .bind(&cmd.smtp_host)
        .bind(cmd.smtp_port)
        .bind(&cmd.smtp_username)
        .bind(smtp_password_encrypted)
        .bind(&cmd.smtp_encryption)
        .bind(&cmd.imap_host)
        .bind(cmd.imap_port)
        .bind(&cmd.imap_username)
        .bind(imap_password_encrypted)
        .bind(&cmd.imap_encryption)
        .bind(&cmd.from_address)
        .bind(&cmd.from_name)
        .bind(is_default)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| {
            if let sqlx::Error::Database(ref db_err) = e {
                if db_err.constraint() == Some("mail_configs_tenant_id_name_key") {
                    return AppError::Conflict(format!(
                        "Mail configuration with name '{}' already exists",
                        cmd.name
                    ));
                }
            }
            AppError::Internal(e.to_string())
        })?;

        tx.commit().await?;
        Ok(config)
    }

    pub async fn get_by_id(&self, tenant_id: Uuid, id: Uuid) -> AppResult<MailConfig> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;

        let config = sqlx::query_as::<_, MailConfig>(
            r#"
            SELECT id, tenant_id, name, smtp_host, smtp_port, smtp_username,
                   smtp_encryption, imap_host, imap_port, imap_username, imap_encryption,
                   from_address, from_name, is_default, verified, deleted_at, created_at, updated_at
            FROM securemail.mail_configs
            WHERE id = $1 AND tenant_id = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .bind(tenant_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("mail config {id} not found")))?;

        tx.commit().await?;
        Ok(config)
    }

    pub async fn list(
        &self,
        tenant_id: Uuid,
        page: i64,
        per_page: i64,
    ) -> AppResult<(Vec<MailConfig>, i64)> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;

        let total: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM securemail.mail_configs WHERE tenant_id = $1 AND deleted_at IS NULL",
        )
        .bind(tenant_id)
        .fetch_one(&mut *tx)
        .await?;

        let offset = (page - 1) * per_page;
        let configs = sqlx::query_as::<_, MailConfig>(
            r#"
            SELECT id, tenant_id, name, smtp_host, smtp_port, smtp_username,
                   smtp_encryption, imap_host, imap_port, imap_username, imap_encryption,
                   from_address, from_name, is_default, verified, deleted_at, created_at, updated_at
            FROM securemail.mail_configs
            WHERE tenant_id = $1 AND deleted_at IS NULL
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(tenant_id)
        .bind(per_page)
        .bind(offset)
        .fetch_all(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok((configs, total.0))
    }

    pub async fn update(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        cmd: &UpdateMailConfig,
        smtp_password_encrypted: Option<&[u8]>,
        imap_password_encrypted: Option<&[u8]>,
    ) -> AppResult<MailConfig> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;

        // Verify exists
        let existing = sqlx::query_as::<_, MailConfig>(
            r#"
            SELECT id, tenant_id, name, smtp_host, smtp_port, smtp_username,
                   smtp_encryption, imap_host, imap_port, imap_username, imap_encryption,
                   from_address, from_name, is_default, verified, deleted_at, created_at, updated_at
            FROM securemail.mail_configs
            WHERE id = $1 AND tenant_id = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .bind(tenant_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("mail config {id} not found")))?;

        let name = cmd.name.as_deref().unwrap_or(&existing.name);
        let smtp_host = cmd.smtp_host.as_deref().unwrap_or(&existing.smtp_host);
        let smtp_port = cmd.smtp_port.unwrap_or(existing.smtp_port);
        let smtp_username = cmd
            .smtp_username
            .as_deref()
            .unwrap_or(&existing.smtp_username);
        let smtp_encryption = cmd
            .smtp_encryption
            .as_deref()
            .unwrap_or(&existing.smtp_encryption);
        let from_address = cmd
            .from_address
            .as_deref()
            .unwrap_or(&existing.from_address);
        let from_name = cmd.from_name.as_deref().unwrap_or(&existing.from_name);
        let is_default = cmd.is_default.unwrap_or(existing.is_default);

        // If setting as default, unset others
        if is_default && !existing.is_default {
            sqlx::query(
                "UPDATE securemail.mail_configs SET is_default = false WHERE tenant_id = $1 AND id != $2 AND deleted_at IS NULL",
            )
            .bind(tenant_id)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        }

        let config = sqlx::query_as::<_, MailConfig>(
            r#"
            UPDATE securemail.mail_configs
            SET name = $3, smtp_host = $4, smtp_port = $5, smtp_username = $6,
                smtp_encryption = $7, from_address = $8, from_name = $9,
                is_default = $10, updated_at = now(),
                smtp_password_encrypted = COALESCE($11, smtp_password_encrypted),
                imap_host = COALESCE($12, imap_host),
                imap_port = COALESCE($13, imap_port),
                imap_username = COALESCE($14, imap_username),
                imap_password_encrypted = COALESCE($15, imap_password_encrypted),
                imap_encryption = COALESCE($16, imap_encryption)
            WHERE id = $1 AND tenant_id = $2 AND deleted_at IS NULL
            RETURNING id, tenant_id, name, smtp_host, smtp_port, smtp_username,
                      smtp_encryption, imap_host, imap_port, imap_username, imap_encryption,
                      from_address, from_name, is_default, verified, deleted_at, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(tenant_id)
        .bind(name)
        .bind(smtp_host)
        .bind(smtp_port)
        .bind(smtp_username)
        .bind(smtp_encryption)
        .bind(from_address)
        .bind(from_name)
        .bind(is_default)
        .bind(smtp_password_encrypted)
        .bind(&cmd.imap_host)
        .bind(cmd.imap_port)
        .bind(&cmd.imap_username)
        .bind(imap_password_encrypted)
        .bind(&cmd.imap_encryption)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| {
            if let sqlx::Error::Database(ref db_err) = e {
                if db_err.constraint() == Some("mail_configs_tenant_id_name_key") {
                    return AppError::Conflict(format!(
                        "Mail configuration with name '{}' already exists",
                        name
                    ));
                }
            }
            AppError::Internal(e.to_string())
        })?;

        tx.commit().await?;
        Ok(config)
    }

    pub async fn soft_delete(&self, tenant_id: Uuid, id: Uuid) -> AppResult<()> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;

        let result = sqlx::query(
            r#"
            UPDATE securemail.mail_configs
            SET deleted_at = now()
            WHERE id = $1 AND tenant_id = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .bind(tenant_id)
        .execute(&mut *tx)
        .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound(format!("mail config {id} not found")));
        }

        tx.commit().await?;
        Ok(())
    }

    pub async fn set_verified(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        verified: bool,
    ) -> AppResult<MailConfig> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;

        let config = sqlx::query_as::<_, MailConfig>(
            r#"
            UPDATE securemail.mail_configs
            SET verified = $3, updated_at = now()
            WHERE id = $1 AND tenant_id = $2 AND deleted_at IS NULL
            RETURNING id, tenant_id, name, smtp_host, smtp_port, smtp_username,
                      smtp_encryption, imap_host, imap_port, imap_username, imap_encryption,
                      from_address, from_name, is_default, verified, deleted_at, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(tenant_id)
        .bind(verified)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("mail config {id} not found")))?;

        tx.commit().await?;
        Ok(config)
    }
}
