use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;
use uuid::Uuid;

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

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct OrganizationUnit {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub name_en: Option<String>,
    pub description: Option<String>,
    pub parent_unit_id: Option<Uuid>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub location: Option<String>,
    pub is_active: bool,
    pub display_order: i32,
    pub category: String,
    pub unit_type: String,
    pub subject_group_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateStaffInfoRequest {
    pub education_level: Option<String>,
    pub major: Option<String>,
    pub university: Option<String>,
    pub teaching_license_number: Option<String>,
    pub teaching_license_expiry: Option<NaiveDate>,
}

// ===================================================================
// Response Models
// ===================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleResponse {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub name_en: Option<String>,
    pub user_type: String, // Changed from category to user_type
    pub level: i32,
    pub is_primary: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizationUnitResponse {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub position_code: Option<String>,
    pub position_title: Option<String>,
    pub is_primary: Option<bool>,
    pub category: Option<String>,
    pub unit_type: Option<String>,
    pub subject_group_id: Option<Uuid>,
    pub responsibilities: Option<String>,
}

/// วิชาที่ครูสอน — ดึงจาก classroom_courses (+ classroom_course_instructors)
/// Source of truth: ระบบ Course Planning ที่ assign วิชาให้ห้อง
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeachingCourseItem {
    pub classroom_course_id: Uuid,
    pub subject_code: String,
    pub subject_name: String,
    pub hours_per_semester: Option<i32>,
    pub classroom_name: String,
    pub classroom_code: String,
    pub academic_year: i32,
    pub academic_year_label: String,
    pub term: String,
    pub role: String, // 'primary' | 'secondary'
}

/// ห้องที่ครูเป็นครูที่ปรึกษา — ดึงจาก classroom_advisors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvisorClassroomItem {
    pub classroom_id: Uuid,
    pub classroom_name: String,
    pub classroom_code: String,
    pub academic_year: i32,
    pub academic_year_label: String,
    pub role: String, // 'primary' | 'secondary'
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaffProfileResponse {
    pub id: Uuid,
    pub username: String,
    pub national_id: Option<String>,
    pub email: Option<String>,
    pub title: Option<String>,
    pub first_name: String,
    pub last_name: String,
    pub nickname: Option<String>,
    pub phone: Option<String>,
    pub emergency_contact: Option<String>,
    pub line_id: Option<String>,
    pub date_of_birth: Option<String>,
    pub gender: Option<String>,
    pub address: Option<String>,
    pub hired_date: Option<String>,
    pub user_type: String,
    pub status: String,
    pub profile_image_url: Option<String>,

    // Staff specific info
    pub staff_info: Option<StaffInfoResponse>,

    // Roles
    pub roles: Vec<RoleResponse>,

    // Organization units
    pub organization_units: Vec<OrganizationUnitResponse>,

    // วิชาที่สอน (จาก classroom_courses)
    pub teaching_courses: Vec<TeachingCourseItem>,

    // ห้องที่เป็นครูที่ปรึกษา (จาก classroom_advisors)
    pub advisor_classrooms: Vec<AdvisorClassroomItem>,

    // Permissions
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaffInfoResponse {
    pub education_level: Option<String>,
    pub major: Option<String>,
    pub university: Option<String>,
}

// ===================================================================
// Create Staff Request
// ===================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub profile_image_url: Option<String>,

    // Roles
    pub role_ids: Vec<Uuid>,
    pub primary_role_id: Option<Uuid>,

    // Organization units
    pub organization_assignments: Option<Vec<OrganizationAssignment>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub profile_image_url: Option<String>,
    pub staff_info: Option<CreateStaffInfoRequest>,

    // Roles
    pub role_ids: Option<Vec<Uuid>>,
    pub primary_role_id: Option<Uuid>,

    // Organization units
    pub organization_assignments: Option<Vec<OrganizationAssignment>>,
}

// ===================================================================
// List Filters
// ===================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaffListFilter {
    pub user_type: Option<String>,
    pub role_id: Option<Uuid>,
    pub organization_unit_id: Option<Uuid>,
    pub status: Option<String>,
    pub search: Option<String>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaffListItem {
    pub id: Uuid,
    pub username: String,
    pub title: String,
    pub first_name: String,
    pub last_name: String,
    pub roles: Vec<String>,
    pub organization_units: Vec<String>,
    pub status: String,
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizationPermissionGrantInput {
    pub permission_id: Uuid,
    pub position_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateOrganizationPermissionsRequest {
    pub grants: Vec<OrganizationPermissionGrantInput>,
}
