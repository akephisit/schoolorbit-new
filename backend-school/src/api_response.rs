use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: T,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = String)]
    pub message: Option<String>,
}

impl<T> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data,
            message: None,
        }
    }

    pub fn with_message(data: T, message: impl Into<String>) -> Self {
        Self {
            success: true,
            data,
            message: Some(message.into()),
        }
    }
}

#[derive(Debug, Default, Serialize, ToSchema)]
pub struct EmptyData {}

impl ApiResponse<EmptyData> {
    pub fn empty() -> Self {
        Self::ok(EmptyData::default())
    }

    pub fn empty_with_message(message: impl Into<String>) -> Self {
        Self::with_message(EmptyData::default(), message)
    }
}

#[derive(Debug, Serialize)]
pub struct IdData<T> {
    pub id: T,
}

/// OpenAPI transport schema for the UUID identifier payload emitted by `IdData<Uuid>`.
#[derive(Debug, Serialize, ToSchema)]
pub struct UuidIdData {
    pub id: uuid::Uuid,
}

impl<T> IdData<T> {
    pub fn new(id: T) -> Self {
        Self { id }
    }
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApiErrorResponse {
    pub success: bool,
    pub error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = String)]
    pub message: Option<String>,
}

impl ApiErrorResponse {
    pub fn new(error: impl Into<String>) -> Self {
        Self {
            success: false,
            error: error.into(),
            message: None,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiErrorResponseWithData<T> {
    pub success: bool,
    pub error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub data: T,
}

impl<T> ApiErrorResponseWithData<T> {
    pub fn new(error: impl Into<String>, data: T) -> Self {
        Self {
            success: false,
            error: error.into(),
            message: None,
            data,
        }
    }

    pub fn with_message(error: impl Into<String>, message: impl Into<String>, data: T) -> Self {
        Self {
            success: false,
            error: error.into(),
            message: Some(message.into()),
            data,
        }
    }
}
