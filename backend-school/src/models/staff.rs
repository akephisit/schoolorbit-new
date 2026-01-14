use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

// ===================================================================
// Core User Types & Enums
// ===================================================================

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text")]
pub enum UserType {
    #[serde(rename = "student")]
    Student,
    #[serde(rename = "staff")]
    Staff,
    #[serde(rename = "parent")]
    Parent,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text")]
pub enum EmploymentType {
    #[serde(rename = "permanent")]
    Permanent,
    #[serde(rename = "contract")]
    Contract,
    #[serde(rename = "temporary")]
    Temporary,
    #[serde(rename = "part_time")]
    PartTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text")]
pub enum RoleCategory {
    #[serde(rename = "administrative")]
    Administrative,
    #[serde(rename = "teaching")]
    Teaching,
    #[serde(rename = "operational")]
    Operational,
    #[serde(rename = "support")]
    Support,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text")]
pub enum TeacherType {
    #[serde(rename = "main_teacher")]
    MainTeacher,
    #[serde(rename = "co_teacher")]
    CoTeacher,
    #[serde(rename = "substitute")]
    Substitute,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text")]
pub enum DepartmentPosition {
    #[serde(rename = "head")]
    Head,
    #[serde(rename = "deputy_head")]
    DeputyHead,
    #[serde(rename = "member")]
    Member,
    #[serde(rename = "coordinator")]
    Coordinator,
}

// ===================================================================
// Role (บทบาท)
// ===================================================================

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

// ===================================================================
// User Role (ความสัมพันธ์ User-Role)
// ===================================================================

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UserRole {
    pub id: Uuid,
    pub user_id: Uuid,
    pub role_id: Uuid,
    pub is_primary: bool,
    pub started_at: NaiveDate,
    pub ended_at: Option<NaiveDate>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssignRoleRequest {
    pub role_id: Uuid,
    pub is_primary: Option<bool>,
    pub started_at: Option<NaiveDate>,
    pub notes: Option<String>,
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
}

// ===================================================================
// Department Member (สมาชิกในฝ่าย)
// ===================================================================

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DepartmentMember {
    pub id: Uuid,
    pub user_id: Uuid,
    pub department_id: Uuid,
    pub position: String,
    pub is_primary_department: bool,
    pub responsibilities: Option<String>,
    pub started_at: NaiveDate,
    pub ended_at: Option<NaiveDate>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddDepartmentMemberRequest {
    pub user_id: Uuid,
    pub position: String,
    pub is_primary_department: Option<bool>,
    pub responsibilities: Option<String>,
    pub started_at: Option<NaiveDate>,
}

// ===================================================================
// Teaching Assignment (การมอบหมายการสอน)
// ===================================================================

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TeachingAssignment {
    pub id: Uuid,
    pub teacher_id: Uuid,
    pub class_id: Uuid,
    pub subject: String,
    pub grade_level: Option<String>,
    pub hours_per_week: Option<f64>,
    pub teacher_type: String,
    pub is_homeroom_teacher: bool,
    pub academic_year: String,
    pub semester: String,
    pub started_at: NaiveDate,
    pub ended_at: Option<NaiveDate>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTeachingAssignmentRequest {
    pub teacher_id: Uuid,
    pub class_id: Uuid,
    pub subject: String,
    pub grade_level: Option<String>,
    pub hours_per_week: Option<f64>,
    pub teacher_type: Option<String>,
    pub is_homeroom_teacher: Option<bool>,
    pub academic_year: String,
    pub semester: String,
}

// ===================================================================
// Staff Info (ข้อมูลเฉพาะบุคลากร)
// ===================================================================

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct StaffInfo {
    pub id: Uuid,
    pub user_id: Uuid,
    pub education_level: Option<String>,
    pub major: Option<String>,
    pub university: Option<String>,
    // Teaching License
    pub teaching_license_number: Option<String>,
    pub teaching_license_expiry: Option<NaiveDate>,
    // Additional Data
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeachingAssignmentResponse {
    pub id: Uuid,
    pub subject: String,
    pub grade_level: Option<String>,
    pub class_code: Option<String>,
    pub class_name: Option<String>,
    pub is_homeroom_teacher: bool,
    pub hours_per_week: Option<f64>,
    pub academic_year: String,
    pub semester: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaffProfileResponse {
    pub id: Uuid,
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
    
    // Teaching assignments
    pub teaching_assignments: Vec<TeachingAssignmentResponse>,
    
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
pub struct StaffListResponse {
    pub success: bool,
    pub data: Vec<StaffListItem>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
    pub total_pages: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaffListItem {
    pub id: Uuid,
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

