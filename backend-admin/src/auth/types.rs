use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,              // User ID
    pub email: String,
    pub role: UserRole,
    pub school_id: Option<String>,
    pub subdomain: Option<String>,
    pub exp: usize,               // Expiration timestamp
    pub iat: usize,               // Issued at timestamp
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum UserRole {
    SuperAdmin,
    SchoolAdmin,
    Teacher,
    Student,
}

impl UserRole {
    pub fn has_permission(&self, required: &UserRole) -> bool {
        match (self, required) {
            (UserRole::SuperAdmin, _) => true,
            (UserRole::SchoolAdmin, UserRole::SuperAdmin) => false,
            (UserRole::SchoolAdmin, _) => true,
            (UserRole::Teacher, UserRole::Student) => true,
            (UserRole::Teacher, UserRole::Teacher) => true,
            (UserRole::Student, UserRole::Student) => true,
            _ => false,
        }
    }
}
