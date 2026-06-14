use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowWindowStatus {
    Draft,
    Open,
    Closed,
    Archived,
}

impl WorkflowWindowStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Open => "open",
            Self::Closed => "closed",
            Self::Archived => "archived",
        }
    }

    pub fn from_code(status: &str) -> Option<Self> {
        match status {
            "draft" => Some(Self::Draft),
            "open" => Some(Self::Open),
            "closed" => Some(Self::Closed),
            "archived" => Some(Self::Archived),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowWindowMetadata {
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowWindow {
    pub id: Uuid,
    pub module_code: String,
    pub workflow_code: String,
    pub title: String,
    pub description: Option<String>,
    pub organization_unit_id: Option<Uuid>,
    pub managed_by_permission: String,
    pub opens_at: Option<DateTime<Utc>>,
    pub due_at: Option<DateTime<Utc>>,
    pub closes_at: Option<DateTime<Utc>>,
    pub status: String,
    pub metadata: Json<WorkflowWindowMetadata>,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
