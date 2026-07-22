use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;
use uuid::Uuid;

/// DB row — stores storage_path and file_id (not URL)
#[derive(Debug, FromRow)]
pub struct SchoolSettingsRow {
    pub logo_path: Option<String>,
    pub logo_file_id: Option<Uuid>,
}

/// Response to frontend — logoUrl built from path
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SchoolSettingsResponse {
    #[schema(required = true)]
    pub logo_url: Option<String>,
    #[schema(required = true)]
    pub logo_file_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSchoolSettingsRequest {
    pub logo_path: Option<String>,
    pub logo_file_id: Option<Uuid>,
}
