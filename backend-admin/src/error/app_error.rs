use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AppError {
    NotFound(String),
    Unauthorized(String),
    ValidationError(String),
    DatabaseError(String),
    InternalServerError(String),
    BadRequest(String),
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            AppError::NotFound(msg) => write!(f, "Not Found: {}", msg),
            AppError::Unauthorized(msg) => write!(f, "Unauthorized: {}", msg),
            AppError::ValidationError(msg) => write!(f, "Validation Error: {}", msg),
            AppError::DatabaseError(msg) => write!(f, "Database Error: {}", msg),
            AppError::InternalServerError(msg) => write!(f, "Internal Server Error: {}", msg),
            AppError::BadRequest(msg) => write!(f, "Bad Request: {}", msg),
        }
    }
}

impl std::error::Error for AppError {}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub success: bool,
    pub error: ErrorDetails,
}

#[derive(Serialize)]
pub struct ErrorDetails {
    pub code: String,
    pub message: String,
}

impl AppError {
    pub fn to_response(&self) -> ErrorResponse {
        let (code, message) = match self {
            AppError::NotFound(msg) => ("NOT_FOUND", msg),
            AppError::Unauthorized(msg) => ("UNAUTHORIZED", msg),
            AppError::ValidationError(msg) => ("VALIDATION_ERROR", msg),
            AppError::DatabaseError(msg) => ("DATABASE_ERROR", msg),
            AppError::InternalServerError(msg) => ("INTERNAL_SERVER_ERROR", msg),
            AppError::BadRequest(msg) => ("BAD_REQUEST", msg),
        };

        ErrorResponse {
            success: false,
            error: ErrorDetails {
                code: code.to_string(),
                message: message.clone(),
            },
        }
    }
}
