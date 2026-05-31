use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{types::Json, FromRow};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct School {
    pub id: Uuid,
    pub name: String,
    pub subdomain: String,
    pub db_name: String,
    pub db_connection_string: Option<String>,
    pub status: String,
    pub config: Json<SchoolConfig>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSchool {
    pub name: String,
    pub subdomain: String,
    pub admin_username: Option<String>,
    pub admin_password: String,
    pub admin_title: String,
    pub admin_first_name: String,
    pub admin_last_name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSchool {
    pub name: Option<String>,
    pub status: Option<String>,
    pub config: Option<SchoolConfig>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SchoolConfig {
    pub db_id: Option<i64>,
    pub dns_record_id: Option<String>,
    pub deployment_url: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::SchoolConfig;
    use sqlx::types::Json;

    #[test]
    fn school_config_defaults_allow_provisioning_records() {
        let config: SchoolConfig = serde_json::from_str("{}").unwrap();

        assert_eq!(config.db_id, None);
        assert_eq!(config.dns_record_id, None);
        assert_eq!(config.deployment_url, None);
    }

    #[test]
    fn school_config_serializes_expected_keys() {
        let config = SchoolConfig {
            db_id: Some(42),
            dns_record_id: Some(String::new()),
            deployment_url: Some("https://sandbox.schoolorbit.app".to_string()),
        };

        let value = serde_json::to_value(config).unwrap();

        assert_eq!(value["db_id"], 42);
        assert_eq!(value["dns_record_id"], "");
        assert_eq!(value["deployment_url"], "https://sandbox.schoolorbit.app");
    }

    #[test]
    fn sqlx_json_school_config_serializes_as_plain_object() {
        let config = Json(SchoolConfig {
            db_id: Some(42),
            dns_record_id: Some("record_123".to_string()),
            deployment_url: Some("https://sandbox.schoolorbit.app".to_string()),
        });

        let value = serde_json::to_value(config).unwrap();

        assert_eq!(value["db_id"], 42);
        assert_eq!(value["dns_record_id"], "record_123");
        assert_eq!(value["deployment_url"], "https://sandbox.schoolorbit.app");
    }
}
