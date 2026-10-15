use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    pub role: String,
}
impl Account {
    pub fn is_operator(&self) -> bool {
        self.role == "operator"
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Credentials {
    pub email: String,
    pub password: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Registration {
    pub email: String,
    pub password: String,
    pub name: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmailInput {
    pub email: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TokenInput {
    pub token: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResetInput {
    pub token: String,
    pub password: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionResponse {
    pub user: Account,
    pub csrf_token: String,
}

#[derive(Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct CustomerSchool {
    pub id: Uuid,
    pub name: String,
    pub subdomain: String,
    pub cadence: Option<String>,
    pub ends_at: Option<DateTime<Utc>>,
    pub expired: bool,
}
#[derive(Clone, Deserialize, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServiceRequestInput {
    pub id: Uuid,
    pub school_id: Option<Uuid>,
    pub kind: String,
    pub school_name: String,
    pub subdomain: String,
    pub cadence: String,
}
#[derive(Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ServiceRequest {
    pub id: Uuid,
    pub account_id: Uuid,
    pub email: String,
    pub school_id: Option<Uuid>,
    pub kind: String,
    pub school_name: String,
    pub subdomain: String,
    pub cadence: String,
    pub status: String,
    pub decision_reason: Option<String>,
    pub created_at: DateTime<Utc>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DecisionInput {
    pub approve: bool,
    pub school_id: Option<Uuid>,
    pub reason: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssignmentInput {
    pub email: String,
}
