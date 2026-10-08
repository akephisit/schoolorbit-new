use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

pub use crate::personnel::*;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct Role {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    #[schema(required = true)]
    pub name_en: Option<String>,
    #[schema(required = true)]
    pub description: Option<String>,
    pub user_type: String, // Changed from category to user_type
    pub level: i32,
    pub permissions: Vec<String>, // Changed from serde_json::Value to Vec<String>
    pub is_active: bool,
    pub is_system: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateRoleRequest {
    pub code: String,
    pub name: String,
    pub name_en: Option<String>,
    pub description: Option<String>,
    pub user_type: String, // Changed from category to user_type
    pub level: Option<i32>,
    pub permissions: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UpdateRoleRequest {
    pub name: Option<String>,
    pub name_en: Option<String>,
    pub description: Option<String>,
    pub user_type: Option<String>, // Changed from category to user_type
    pub level: Option<i32>,
    pub permissions: Option<Vec<String>>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AssignRoleRequest {
    pub role_id: Uuid,
    pub is_primary: Option<bool>,
    pub started_at: Option<NaiveDate>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UserRoleAssignmentResponse {
    pub id: Uuid,
    pub user_id: Uuid,
    pub role_id: Uuid,
    #[schema(required = true)]
    pub organization_unit_id: Option<Uuid>,
    pub role: Role,
    pub is_primary: bool,
    pub started_at: NaiveDate,
    #[schema(required = true)]
    pub ended_at: Option<NaiveDate>,
    #[schema(required = true)]
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ===================================================================
// Organization Unit (หน่วยงาน/กลุ่ม/ฝ่าย/กลุ่มสาระ)
// ===================================================================

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct OrganizationUnit {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    #[schema(required = true)]
    pub name_en: Option<String>,
    #[schema(required = true)]
    pub description: Option<String>,
    #[schema(required = true)]
    pub parent_unit_id: Option<Uuid>,
    #[schema(required = true)]
    pub phone: Option<String>,
    #[schema(required = true)]
    pub email: Option<String>,
    #[schema(required = true)]
    pub location: Option<String>,
    pub is_active: bool,
    pub is_system: bool,
    pub display_order: i32,
    pub category: String,
    pub unit_type: String,
    #[schema(required = true)]
    pub subject_group_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateOrganizationUnitRequest {
    pub code: String,
    pub name: String,
    pub name_en: Option<String>,
    pub description: Option<String>,
    pub parent_unit_id: Option<Uuid>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub location: Option<String>,
    pub category: Option<String>,
    pub unit_type: Option<String>,
    pub subject_group_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UpdateOrganizationUnitRequest {
    pub name: Option<String>,
    pub name_en: Option<String>,
    pub description: Option<String>,
    pub parent_unit_id: Option<Uuid>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub location: Option<String>,
    pub is_active: Option<bool>,
    pub category: Option<String>,
    pub unit_type: Option<String>,
    pub subject_group_id: Option<Uuid>,
}

// ===================================================================
// Response Models
// ===================================================================

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RoleResponse {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    #[schema(required = true)]
    pub name_en: Option<String>,
    pub user_type: String, // Changed from category to user_type
    pub level: i32,
    #[schema(required = true)]
    pub is_primary: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OrganizationUnitResponse {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    #[schema(required = true)]
    pub position_code: Option<String>,
    #[schema(required = true)]
    pub position_title: Option<String>,
    #[schema(required = true)]
    pub is_primary: Option<bool>,
    #[schema(required = true)]
    pub category: Option<String>,
    #[schema(required = true)]
    pub unit_type: Option<String>,
    #[schema(required = true)]
    pub subject_group_id: Option<Uuid>,
    #[schema(required = true)]
    pub responsibilities: Option<String>,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[into_params(parameter_in = Query)]
pub struct StaffDashboardQuery {
    pub academic_year_id: Uuid,
}

/// กลุ่มการเรียนที่ครูสอนใน Academic Delivery
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TeachingAssignmentItem {
    pub learning_group_id: Uuid,
    pub learning_group_code: String,
    pub learning_group_name: String,
    pub subject_id: Uuid,
    pub subject_code: String,
    pub subject_name: String,
    #[schema(required = true)]
    pub hours: Option<String>,
    pub academic_year_id: Uuid,
    pub academic_year: i32,
    pub academic_year_label: String,
    pub academic_term_id: Uuid,
    pub term_code: String,
    pub term_name: String,
    pub role: String,
}

/// ห้องประจำชั้นที่ครูเป็นที่ปรึกษา
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AdvisorHomeroomItem {
    pub homeroom_id: Uuid,
    pub homeroom_name: String,
    pub homeroom_code: String,
    pub academic_year_id: Uuid,
    pub academic_year: i32,
    pub academic_year_label: String,
    pub role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct StaffProfileResponse {
    pub id: Uuid,
    pub username: String,
    #[schema(required = true)]
    pub national_id: Option<String>,
    #[schema(required = true)]
    pub email: Option<String>,
    #[schema(required = true)]
    pub title: Option<String>,
    pub first_name: String,
    pub last_name: String,
    #[schema(required = true)]
    pub nickname: Option<String>,
    #[schema(required = true)]
    pub phone: Option<String>,
    #[schema(required = true)]
    pub emergency_contact: Option<String>,
    #[schema(required = true)]
    pub line_id: Option<String>,
    #[schema(required = true)]
    pub date_of_birth: Option<String>,
    #[schema(required = true)]
    pub gender: Option<String>,
    #[schema(required = true)]
    pub address: Option<String>,
    #[schema(required = true)]
    pub hired_date: Option<String>,
    pub user_type: String,
    pub status: String,
    #[schema(required = true)]
    pub profile_image_file_id: Option<Uuid>,

    // Staff specific info
    #[schema(required = true)]
    pub staff_info: Option<StaffInfoResponse>,

    // Roles
    pub roles: Vec<RoleResponse>,

    // Organization units
    pub organization_units: Vec<OrganizationUnitResponse>,
    pub subject_groups: Vec<StaffSubjectGroupSummary>,

    // กลุ่มการเรียนที่สอน
    pub teaching_assignments: Vec<TeachingAssignmentItem>,

    // ห้องประจำชั้นที่เป็นครูที่ปรึกษา
    pub advisor_homerooms: Vec<AdvisorHomeroomItem>,

    // Permissions
    pub permissions: Vec<String>,
}

// ===================================================================
// Create Staff Request
// ===================================================================

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateStaffRequest {
    // Basic User Info
    pub username: Option<String>,
    pub national_id: Option<String>,
    pub email: Option<String>,
    pub password: String,
    pub title: Option<String>,
    pub first_name: String,
    pub last_name: String,
    pub nickname: Option<String>,
    pub phone: Option<String>,
    pub emergency_contact: Option<String>,
    pub line_id: Option<String>,
    pub date_of_birth: Option<NaiveDate>,
    pub gender: Option<String>,
    pub address: Option<String>,
    pub hired_date: Option<NaiveDate>,

    // Staff Info (Optional - can be added later)
    pub staff_info: Option<CreateStaffInfoRequest>,
    pub profile_image_file_id: Option<Uuid>,

    // Roles
    pub role_ids: Vec<Uuid>,
    pub primary_role_id: Option<Uuid>,

    // Organization units
    pub organization_assignments: Option<Vec<OrganizationAssignment>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OrganizationAssignment {
    pub organization_unit_id: Uuid,
    pub position_code: String,
    pub position_title: Option<String>,
    pub is_primary: Option<bool>,
    pub responsibilities: Option<String>,
}

// ===================================================================
// Update Staff Request
// ===================================================================

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UpdateStaffRequest {
    pub title: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub nickname: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub emergency_contact: Option<String>,
    pub line_id: Option<String>,
    pub date_of_birth: Option<NaiveDate>,
    pub gender: Option<String>,
    pub address: Option<String>,
    pub hired_date: Option<NaiveDate>,
    pub status: Option<String>,
    pub profile_image_file_id: Option<Uuid>,
    pub staff_info: Option<UpdateStaffInfoRequest>,

    // Roles
    pub role_ids: Option<Vec<Uuid>>,
    pub primary_role_id: Option<Uuid>,

    // Organization units
    pub organization_assignments: Option<Vec<OrganizationAssignment>>,
}

// ===================================================================
// List Filters
// ===================================================================

#[derive(Debug, Clone, Default, Serialize, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct StaffListFilter {
    pub user_type: Option<String>,
    pub role_id: Option<Uuid>,
    pub organization_unit_id: Option<Uuid>,
    pub status: Option<String>,
    pub search: Option<String>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
    pub job_position_id: Option<String>,
    pub academic_rank: Option<String>,
    pub education_level: Option<String>,
    pub subject_group_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct StaffListItem {
    pub id: Uuid,
    pub username: String,
    pub title: String,
    pub first_name: String,
    pub last_name: String,
    pub roles: Vec<String>,
    pub organization_units: Vec<String>,
    pub status: String,
    #[schema(required = true)]
    pub job_position: Option<StaffJobPositionSummary>,
    #[schema(required = true)]
    pub academic_rank: Option<StaffAcademicRank>,
}

// ===================================================================
// Permission (สิทธิ์การใช้งาน)
// ===================================================================

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct Permission {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub module: String,
    pub action: String,
    pub scope: String,
    #[schema(required = true)]
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
}

// ===================================================================

// ===================================================================
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OrganizationPermissionGrantInput {
    pub permission_id: Uuid,
    pub position_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UpdateOrganizationPermissionsRequest {
    pub grants: Vec<OrganizationPermissionGrantInput>,
}

/// Access scope already resolved by the application authorization policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StaffListAccess {
    Own(Uuid),
    Assigned(Uuid),
    OrganizationUnit(Uuid),
    OrganizationTree(Uuid),
    School,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct OrganizationMemberItem {
    pub user_id: Uuid,
    pub organization_unit_id: Uuid,
    pub organization_unit_name: String,
    pub name: String,
    pub title: String,
    pub position_code: String,
    #[schema(required = true)]
    pub position_title: Option<String>,
    pub is_primary: bool,
    #[schema(required = true)]
    pub responsibilities: Option<String>,
    pub started_at: NaiveDate,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct DelegationItem {
    pub id: Uuid,
    pub from_user_id: Uuid,
    pub from_user_name: String,
    pub to_user_id: Uuid,
    pub to_user_name: String,
    pub permission_id: Uuid,
    pub permission_code: String,
    pub permission_name: String,
    #[schema(required = true)]
    pub reason: Option<String>,
    pub started_at: DateTime<Utc>,
    #[schema(required = true)]
    pub expires_at: Option<DateTime<Utc>>,
}

/// Organization unit lookup item
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OrganizationUnitLookupItem {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = String)]
    pub name_en: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = String)]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = String)]
    pub category: Option<String>,
    pub display_order: i32,
    pub is_active: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Uuid)]
    pub parent_unit_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = String)]
    pub unit_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Uuid)]
    pub subject_group_id: Option<Uuid>,
}
