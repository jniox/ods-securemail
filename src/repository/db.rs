//! Database utilities for the SecureMail service.
//!
//! Re-exports ods-common's pool creation and tenant-scoped transactions.

pub use ods_common::db::{begin_tenant_tx, create_pool};

/// Run sqlx migrations from the ./migrations directory.
pub async fn run_migrations(pool: &sqlx::PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}
