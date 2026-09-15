use std::borrow::Cow;

#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("Database error: {0}")]
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
