use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

// ===================================================================
// Consent Record (บันทึกความยินยอม)
// ===================================================================

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ConsentRecord {
    pub id: Uuid,
    pub user_id: Uuid,
    pub user_type: String,
    pub consent_type: String,
    pub purpose: String,
    pub data_categories: serde_json::Value,
    pub consent_status: String,
    pub granted_at: Option<chrono::DateTime<chrono::Utc>>,
    pub withdrawn_at: Option<chrono::DateTime<chrono::Utc>>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub consent_method: String,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub consent_text: Option<String>,
    pub consent_version: String,
    pub is_minor_consent: bool,
    pub parent_guardian_id: Option<Uuid>,
    pub parent_guardian_name: Option<String>,
    pub parent_relationship: Option<String>,
    pub notes: Option<String>,
    pub metadata: serde_json::Value,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsentRecordResponse {
    pub id: Uuid,
    pub user_id: Uuid,
    pub user_type: String,
    pub consent_type: String,
    pub consent_type_name: Option<String>,
    pub purpose: String,
    pub data_categories: Vec<String>,
    pub consent_status: String,
    pub granted_at: Option<chrono::DateTime<chrono::Utc>>,
    pub withdrawn_at: Option<chrono::DateTime<chrono::Utc>>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub is_expired: bool,
    pub is_required: bool,
    pub consent_method: String,
    pub is_minor_consent: bool,
    pub parent_guardian_name: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateConsentRequest {
    pub consent_type: String,
    pub consent_status: String, // 'granted' or 'denied'
    pub is_minor_consent: Option<bool>,
    pub parent_guardian_name: Option<String>,
    pub parent_relationship: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct WithdrawConsentRequest {
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BulkConsentRequest {
    pub consents: Vec<CreateConsentRequest>,
}

// ===================================================================
// Consent Type (ประเภทความยินยอม)
// ===================================================================

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ConsentType {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub name_en: Option<String>,
    pub description: Option<String>,
    pub is_required: bool,
    pub priority: i32,
    pub applicable_user_types: Vec<String>,
    pub consent_text_template: String,
    pub consent_version: String,
    pub default_duration_days: Option<i32>,
    pub is_active: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsentTypeResponse {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub name_en: Option<String>,
    pub description: Option<String>,
    pub is_required: bool,
    pub priority: i32,
    pub applicable_user_types: Vec<String>,
    pub consent_text_template: String,
    pub consent_version: String,
    pub default_duration_days: Option<i32>,
    pub is_active: bool,
}

impl From<ConsentType> for ConsentTypeResponse {
    fn from(ct: ConsentType) -> Self {
        Self {
            id: ct.id,
            code: ct.code,
            name: ct.name,
            name_en: ct.name_en,
            description: ct.description,
            is_required: ct.is_required,
            priority: ct.priority,
            applicable_user_types: ct.applicable_user_types,
            consent_text_template: ct.consent_text_template,
            consent_version: ct.consent_version,
            default_duration_days: ct.default_duration_days,
            is_active: ct.is_active,
        }
    }
}

// ===================================================================
// User Consent Status (สถานะความยินยอมของผู้ใช้)
// ===================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserConsentStatus {
    pub user_id: Uuid,
    pub user_type: String,
    pub total_required: i32,
    pub granted_required: i32,
    pub is_compliant: bool,
    pub missing_required_consents: Vec<String>,
    pub consents: Vec<ConsentRecordResponse>,
}

// ===================================================================
// Consent Summary (สรุปความยินยอม)
// ===================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsentSummary {
    pub total_users: i64,
    pub total_consents: i64,
    pub granted: i64,
    pub denied: i64,
    pub withdrawn: i64,
    pub pending: i64,
    pub compliance_rate: f64, // เปอร์เซ็นต์ผู้ใช้ที่ให้ความยินยอมครบถ้วน
}
