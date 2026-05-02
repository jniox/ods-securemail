//! Error types for the SecureMail service.
//!
//! Re-exports `ods_common::error::AppError` as the canonical error type.

pub use ods_common::error::{AppError, AppResult};

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::http::StatusCode;
    use actix_web::ResponseError;

    #[test]
    fn test_not_found_error() {
        let err = AppError::NotFound("mail config xyz".to_string());
        assert_eq!(err.error_response().status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_validation_error() {
        let err = AppError::Validation("missing smtp_host".to_string());
        assert_eq!(err.error_response().status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_conflict_error() {
        let err = AppError::Conflict("duplicate name".to_string());
        assert_eq!(err.error_response().status(), StatusCode::CONFLICT);
    }

    #[test]
    fn test_unauthorized_error() {
        let err = AppError::Unauthorized("invalid token".to_string());
        assert_eq!(err.error_response().status(), StatusCode::UNAUTHORIZED);
    }
}
