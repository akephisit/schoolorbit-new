use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// DB row — stores storage_path (not URL)
#[derive(Debug, FromRow)]
pub struct SchoolSettingsRow {
    pub logo_path: Option<String>,
}

/// Response to frontend — logoUrl built from path
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchoolSettingsResponse {
    pub logo_url: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSchoolSettingsRequest {
    pub logo_path: Option<String>,
}
