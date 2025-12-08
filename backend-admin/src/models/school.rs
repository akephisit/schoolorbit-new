use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct School {
    pub id: Uuid,
    pub name: String,
    pub subdomain: String,
    pub db_name: String,
    pub db_connection_string: Option<String>,
    pub status: String,
    pub config: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSchool {
    pub name: String,
    pub subdomain: String,
    pub admin_national_id: String,
    pub admin_password: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSchool {
    pub name: Option<String>,
    pub status: Option<String>,
    pub config: Option<serde_json::Value>,
}
