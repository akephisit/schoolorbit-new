use std::borrow::Cow;

#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("Database error")]
    DbError(#[from] sqlx::Error),

    #[error("Authentication error: {0}")]
    AuthError(String),

    #[error("Authorization error: {0}")]
    Forbidden(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Internal server error: {0}")]
    InternalServerError(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),

    #[error("Rate limited")]
    RateLimited { retry_after_seconds: u64 },

    #[error("Payload too large")]
    PayloadTooLarge,
}

impl AppError {
    pub fn database_code(&self) -> Option<Cow<'_, str>> {
        match self {
            Self::DbError(sqlx::Error::Database(error)) => error.code(),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[derive(Debug, thiserror::Error)]
    #[error("sensitive bind sentinel")]
    struct DatabaseFailure;
    impl sqlx::error::DatabaseError for DatabaseFailure {
        fn message(&self) -> &str {
            "sensitive bind sentinel"
        }
        fn code(&self) -> Option<Cow<'_, str>> {
            Some(Cow::Borrowed("23505"))
        }
        fn as_error(&self) -> &(dyn Error + Send + Sync + 'static) {
            self
        }
        fn as_error_mut(&mut self) -> &mut (dyn Error + Send + Sync + 'static) {
            self
        }
        fn into_error(self: Box<Self>) -> Box<dyn Error + Send + Sync + 'static> {
            self
        }
        fn kind(&self) -> sqlx::error::ErrorKind {
            sqlx::error::ErrorKind::UniqueViolation
        }
    }

    #[test]
    fn database_display_excludes_payload_while_native_source_and_code_remain_available() {
        let error = AppError::DbError(sqlx::Error::Database(Box::new(DatabaseFailure)));
        assert_eq!(error.to_string(), "Database error");
        assert_eq!(error.database_code().as_deref(), Some("23505"));
        assert!(error
            .source()
            .unwrap()
            .to_string()
            .contains("sensitive bind sentinel"));
        let AppError::DbError(sqlx::Error::Database(native)) = error else {
            panic!("native error retained");
        };
        assert_eq!(native.message(), "sensitive bind sentinel");
        assert!(native.is_unique_violation());
    }
}
