use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// DB row — stores storage_path and file_id (not URL)
#[derive(Debug, FromRow)]
pub struct SchoolSettingsRow {
    pub logo_path: Option<String>,
    pub logo_file_id: Option<Uuid>,
}

/// Response to frontend — logoUrl built from path
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchoolSettingsResponse {
    pub logo_url: Option<String>,
    pub logo_file_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSchoolSettingsRequest {
    pub logo_path: Option<String>,
    pub logo_file_id: Option<Uuid>,
}
