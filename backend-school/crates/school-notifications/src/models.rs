use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
#[cfg(feature = "api-docs")]
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Serialize, Clone, FromRow)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
pub struct Notification {
    pub id: Uuid,
    pub title: String,
    pub message: String,
    #[serde(rename = "type")]
    pub type_: String,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub link: Option<String>,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub read_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(IntoParams))]
#[cfg_attr(feature = "api-docs", into_params(parameter_in = Query))]
pub struct ListNotificationsQuery {
    pub page: Option<i64>,
    pub limit: Option<i64>,
    pub unread_only: Option<bool>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
pub struct ListNotificationsResponse {
    pub items: Vec<Notification>,
    pub unread_count: i64,
    pub page: i64,
    pub limit: i64,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
pub struct PushConfigResponse {
    pub available: bool,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub public_key: Option<String>,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub key_id: Option<String>,
    pub max_devices: usize,
    pub device_expiry_days: i64,
}
#[derive(Debug, Serialize, FromRow)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
pub struct PushDevice {
    pub id: Uuid,
    pub installation_id: Uuid,
    pub label: String,
    pub version: Uuid,
    pub binding_token: Uuid,
    pub key_id: String,
    pub last_confirmed_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
pub struct ListPushDevicesResponse {
    pub items: Vec<PushDevice>,
}
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
pub struct RegisterPushDeviceRequest {
    /// Conditional automatic refresh cannot recreate a revoked binding. Omit only for explicit enable.
    pub expected_version: Option<Uuid>,
    pub installation_id: Uuid,
    pub label: String,
    pub endpoint: String,
    pub p256dh: String,
    pub auth: String,
    pub key_id: String,
}
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(IntoParams))]
#[cfg_attr(feature="api-docs", into_params(parameter_in=Query))]
pub struct UnregisterPushDeviceQuery {
    pub version: Uuid,
}
