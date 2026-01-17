use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProvisionRequest {
    pub school_id: String,
    pub db_connection_string: String,
    pub subdomain: String,
    pub admin_username: Option<String>,
    pub admin_password: String,
}

#[derive(Debug, Serialize)]
pub struct ProvisionResponse {
    pub success: bool,
    pub message: String,
    pub school_id: String,
}
