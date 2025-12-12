use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DeployResponse {
    pub success: bool,
    pub message: String,
    pub deployment_url: Option<String>,
    pub github_actions_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BulkDeployResult {
    pub total: usize,
    pub successful: Vec<DeployResult>,
    pub failed: Vec<DeployResult>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DeployResult {
    pub school_id: Uuid,
    pub school_name: String,
    pub success: bool,
    pub message: String,
    pub deployment_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct DeploymentHistory {
    pub id: Uuid,
    pub school_id: Uuid,
    pub status: String,
    pub message: Option<String>,
    pub github_run_id: Option<String>,
    pub github_run_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}
