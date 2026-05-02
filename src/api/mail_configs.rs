use actix_web::{web, HttpResponse};
use uuid::Uuid;
use validator::Validate;

use crate::domain::mail_config::{CreateMailConfig, UpdateMailConfig};
use crate::error::AppError;
use crate::service::mail_config_service::MailConfigService;
use ods_common::auth::AuthContext;

/// POST /api/v1/mail-configs
pub async fn create_mail_config(
    auth: AuthContext,
    svc: web::Data<MailConfigService>,
    body: web::Json<CreateMailConfig>,
) -> Result<HttpResponse, AppError> {
    let cmd = body.into_inner();
    cmd.validate()
        .map_err(|e| AppError::Validation(e.to_string()))?;

    let response = svc.create(auth.tenant_id, cmd).await?;
    Ok(HttpResponse::Created().json(response))
}

/// GET /api/v1/mail-configs
pub async fn list_mail_configs(
    auth: AuthContext,
    svc: web::Data<MailConfigService>,
    query: web::Query<PaginationQuery>,
) -> Result<HttpResponse, AppError> {
    let page = query.page.unwrap_or(1);
    let per_page = query.per_page.unwrap_or(20);

    let response = svc.list(auth.tenant_id, page, per_page).await?;
    Ok(HttpResponse::Ok().json(response))
}

/// GET /api/v1/mail-configs/{id}
pub async fn get_mail_config(
    auth: AuthContext,
    svc: web::Data<MailConfigService>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();
    let response = svc.get_by_id(auth.tenant_id, id).await?;
    Ok(HttpResponse::Ok().json(response))
}

/// PUT /api/v1/mail-configs/{id}
pub async fn update_mail_config(
    auth: AuthContext,
    svc: web::Data<MailConfigService>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateMailConfig>,
) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();
    let cmd = body.into_inner();
    cmd.validate()
        .map_err(|e| AppError::Validation(e.to_string()))?;

    let response = svc.update(auth.tenant_id, id, cmd).await?;
    Ok(HttpResponse::Ok().json(response))
}

/// DELETE /api/v1/mail-configs/{id}
pub async fn delete_mail_config(
    auth: AuthContext,
    svc: web::Data<MailConfigService>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();
    svc.delete(auth.tenant_id, id).await?;
    Ok(HttpResponse::NoContent().finish())
}

/// POST /api/v1/mail-configs/{id}/verify
pub async fn verify_mail_config(
    auth: AuthContext,
    svc: web::Data<MailConfigService>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();
    let _config = svc.verify(auth.tenant_id, id).await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "verified": true,
        "message": "Connection successful"
    })))
}

#[derive(Debug, serde::Deserialize)]
pub struct PaginationQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}
