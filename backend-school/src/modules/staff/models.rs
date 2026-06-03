use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Role {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub name_en: Option<String>,
    pub description: Option<String>,
    pub user_type: String, // Changed from category to user_type
    pub level: i32,
    pub permissions: Vec<String>, // Changed from serde_json::Value to Vec<String>
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRoleRequest {
    pub code: String,
    pub name: String,
    pub name_en: Option<String>,
    pub description: Option<String>,
    pub user_type: String, // Changed from category to user_type
    pub level: Option<i32>,
    pub permissions: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateRoleRequest {
    pub name: Option<String>,
    pub name_en: Option<String>,
    pub description: Option<String>,
    pub user_type: Option<String>, // Changed from category to user_type
    pub level: Option<i32>,
    pub permissions: Option<Vec<String>>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssignRoleRequest {
    pub role_id: Uuid,
    pub is_primary: Option<bool>,
    pub started_at: Option<NaiveDate>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRoleAssignmentResponse {
    pub id: Uuid,
    pub user_id: Uuid,
    pub role_id: Uuid,
    pub department_id: Option<Uuid>,
    pub role: Role,
    pub is_primary: bool,
    pub started_at: NaiveDate,
    pub ended_at: Option<NaiveDate>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ===================================================================
// Department (ฝ่าย/แผนก)
// ===================================================================

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Department {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub name_en: Option<String>,
    pub description: Option<String>,
    pub parent_department_id: Option<Uuid>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub location: Option<String>,
    pub is_active: bool,
    pub display_order: i32,
    pub category: String, // administrative, academic
    pub org_type: String, // group, unit
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDepartmentRequest {
    pub code: String,
    pub name: String,
    pub name_en: Option<String>,
    pub description: Option<String>,
    pub parent_department_id: Option<Uuid>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub location: Option<String>,
    pub category: Option<String>,
    pub org_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateDepartmentRequest {
    pub name: Option<String>,
    pub name_en: Option<String>,
    pub description: Option<String>,
    pub parent_department_id: Option<Uuid>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub location: Option<String>,
    pub is_active: Option<bool>,
    pub category: Option<String>,
    pub org_type: Option<String>,
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
pub struct DepartmentResponse {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub position: Option<String>,
    pub is_primary_department: Option<bool>,
    pub category: Option<String>,
    pub org_type: Option<String>,
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
    
    // Departments
    pub departments: Vec<DepartmentResponse>,
    
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
    
    // Departments
    pub department_assignments: Option<Vec<DepartmentAssignment>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepartmentAssignment {
    pub department_id: Uuid,
    pub position: String,
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
    
    // Departments
    pub department_assignments: Option<Vec<DepartmentAssignment>>,
}

// ===================================================================
// List Filters
// ===================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaffListFilter {
    pub user_type: Option<String>,
    pub role_id: Option<Uuid>,
    pub department_id: Option<Uuid>,
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
    pub departments: Vec<String>,
    pub status: String,
}

// ===================================================================
// Permission (สิทธิ์การใช้งาน)
// ===================================================================

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Permission {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub module: String,
    pub action: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
}

// ===================================================================


// ===================================================================
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateDepartmentPermissionsRequest {
    pub permission_ids: Vec<Uuid>,
}
