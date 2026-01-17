use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProvisionRequest {
    pub school_id: String,
    pub db_connection_string: String,
    pub subdomain: String,
    pub admin_username: Option<String>,
    pub admin_password: String,
    // Admin Profile
    pub admin_title: String,
    pub admin_first_name: String,
    pub admin_last_name: String,
}

#[derive(Debug, Serialize)]
pub struct ProvisionResponse {
    pub success: bool,
    pub message: String,
    pub school_id: String,
}
