use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{types::Json, FromRow};
use uuid::Uuid;

#[derive(Clone, FromRow)]
pub struct School {
    pub id: Uuid,
    pub name: String,
    pub subdomain: String,
    pub db_name: String,
    pub db_runtime_password: Option<Vec<u8>>,
    pub db_migration_password: Option<Vec<u8>>,
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

/// Anonymous discovery publishes only already-public school identity.
#[derive(Debug, Serialize, FromRow)]
pub struct PublicSchoolSite {
    pub name: String,
    pub subdomain: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicSchoolSites {
    pub schools: Vec<PublicSchoolSite>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SchoolConfig {
    pub dns_record_id: Option<String>,
    pub deployment_url: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::{School, SchoolConfig, SchoolResponse};
    use sqlx::types::Json;

    #[test]
    fn school_config_defaults_allow_provisioning_records() {
        let config: SchoolConfig = serde_json::from_str("{}").unwrap();

        assert_eq!(config.dns_record_id, None);
        assert_eq!(config.deployment_url, None);
    }

    #[test]
    fn public_school_response_never_serializes_database_credentials() {
        let school = School {
            id: uuid::Uuid::now_v7(),
            name: "Fixture school".into(),
            subdomain: "fixture".into(),
            db_name: "schoolorbit_fixture".into(),
            db_runtime_password: Some(vec![1, 2, 3]),
            db_migration_password: Some(vec![4, 5, 6]),
            status: "active".into(),
            config: Json(SchoolConfig::default()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        let value = serde_json::to_value(SchoolResponse::from(&school)).unwrap();
        for forbidden in [
            "db_runtime_password",
            "db_migration_password",
            "db_connection_string",
            "dbConnectionString",
        ] {
            assert!(value.get(forbidden).is_none());
        }
        assert!(value["config"].get("db_id").is_none());
        assert_eq!(value["id"], school.id.to_string());
    }

    #[test]
    fn school_config_serializes_expected_keys() {
        let config = SchoolConfig {
            dns_record_id: Some(String::new()),
            deployment_url: Some("https://sandbox.schoolorbit.app".to_string()),
        };

        let value = serde_json::to_value(config).unwrap();

        assert!(value.get("db_id").is_none());
        assert_eq!(value["dns_record_id"], "");
        assert_eq!(value["deployment_url"], "https://sandbox.schoolorbit.app");
    }

    #[test]
    fn sqlx_json_school_config_serializes_as_plain_object() {
        let config = Json(SchoolConfig {
            dns_record_id: Some("record_123".to_string()),
            deployment_url: Some("https://sandbox.schoolorbit.app".to_string()),
        });

        let value = serde_json::to_value(config).unwrap();

        assert!(value.get("db_id").is_none());
        assert_eq!(value["dns_record_id"], "record_123");
        assert_eq!(value["deployment_url"], "https://sandbox.schoolorbit.app");
    }
}

/// Public control-plane data, intentionally excludes every database credential.
#[derive(Debug, Clone, Serialize)]
pub struct SchoolResponse {
    pub id: Uuid,
    pub name: String,
    pub subdomain: String,
    pub db_name: String,
    pub status: String,
    pub config: Json<SchoolConfig>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<&School> for SchoolResponse {
    fn from(school: &School) -> Self {
        Self {
            id: school.id,
            name: school.name.clone(),
            subdomain: school.subdomain.clone(),
            db_name: school.db_name.clone(),
            status: school.status.clone(),
            config: school.config.clone(),
            created_at: school.created_at,
            updated_at: school.updated_at,
        }
    }
}
