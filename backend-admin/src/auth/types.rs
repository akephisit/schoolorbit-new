use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminClaims {
    pub sub: String, // User ID
    pub email: String,
    pub role: AdminRole,
    pub exp: usize, // Expiration timestamp
    pub iat: usize, // Issued at timestamp
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AdminRole {
    SuperAdmin,
    Admin,
    SchoolAdmin,
    Teacher,
    Student,
}

impl AdminRole {
    pub fn can_access_admin_backend(&self) -> bool {
        matches!(self, AdminRole::SuperAdmin | AdminRole::Admin)
    }
}

impl TryFrom<&str> for AdminRole {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "super_admin" => Ok(AdminRole::SuperAdmin),
            "admin" => Ok(AdminRole::Admin),
            "school_admin" => Ok(AdminRole::SchoolAdmin),
            "teacher" => Ok(AdminRole::Teacher),
            "student" => Ok(AdminRole::Student),
            _ => Err(format!("Unsupported admin role: {}", value)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AdminClaims, AdminRole};
    use serde_json::json;

    #[test]
    fn admin_claims_do_not_serialize_school_scoped_fields() {
        let claims = AdminClaims {
            sub: "admin-user-id".to_string(),
            email: "1234567890123".to_string(),
            role: AdminRole::Admin,
            exp: 1_900_000_000,
            iat: 1_800_000_000,
        };

        let value = serde_json::to_value(claims).expect("claims should serialize");

        assert_eq!(value["role"], json!("admin"));
        assert!(value.get("school_id").is_none());
        assert!(value.get("subdomain").is_none());
    }

    #[test]
    fn only_admin_roles_can_access_admin_backend() {
        assert!(AdminRole::SuperAdmin.can_access_admin_backend());
        assert!(AdminRole::Admin.can_access_admin_backend());
        assert!(!AdminRole::SchoolAdmin.can_access_admin_backend());
        assert!(!AdminRole::Teacher.can_access_admin_backend());
        assert!(!AdminRole::Student.can_access_admin_backend());
    }
}
