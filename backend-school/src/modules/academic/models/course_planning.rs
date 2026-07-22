use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum CourseInstructorRole {
    Primary,
    Secondary,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum OptionalUuidPatch {
    #[default]
    Unspecified,
    Null,
    Value(Uuid),
}

impl<'de> Deserialize<'de> for OptionalUuidPatch {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Option::<Uuid>::deserialize(deserializer).map(|value| match value {
            Some(id) => Self::Value(id),
            None => Self::Null,
        })
    }
}

#[derive(Debug, Serialize, Deserialize, FromRow, ToSchema)]
pub struct ClassroomCourse {
    pub id: Uuid,
    pub classroom_id: Uuid,
    pub subject_id: Uuid,
    pub academic_semester_id: Uuid,
    pub primary_instructor_id: Option<Uuid>,
    #[sqlx(default)]
    pub settings: serde_json::Value,

    // Joined Fields
    #[sqlx(default)]
    pub subject_code: Option<String>,
    #[sqlx(default)]
    pub subject_name_th: Option<String>,
    #[sqlx(default)]
    pub subject_name_en: Option<String>,
    #[sqlx(default)]
    pub subject_credit: Option<f64>,
    #[sqlx(default)]
    pub subject_hours: Option<i32>,
    #[sqlx(default)]
    pub instructor_name: Option<String>,
    #[sqlx(default)]
    #[serde(rename = "subject_type")]
    pub subject_type: Option<String>,
    #[sqlx(default)]
    pub classroom_name: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct PlanQuery {
    pub classroom_id: Option<Uuid>,
    pub instructor_id: Option<Uuid>,
    pub academic_semester_id: Option<Uuid>,
    pub subject_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AssignCoursesRequest {
    pub classroom_id: Uuid,
    pub academic_semester_id: Uuid,
    pub subject_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateCourseRequest {
    #[serde(default)]
    #[schema(value_type = Option<Uuid>, nullable = true)]
    pub primary_instructor_id: OptionalUuidPatch,
    pub settings: Option<serde_json::Value>,
}

// ==========================================
// Classroom Course Instructors (team teaching)
// ==========================================

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct CourseInstructor {
    pub id: Uuid,
    pub classroom_course_id: Uuid,
    pub instructor_id: Uuid,
    #[schema(value_type = CourseInstructorRole)]
    pub role: String,
    pub created_at: chrono::DateTime<chrono::Utc>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[sqlx(default)]
    pub instructor_name: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AddCourseInstructorRequest {
    pub instructor_id: Uuid,
    #[schema(value_type = Option<CourseInstructorRole>)]
    pub role: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct BatchListCourseInstructorsRequest {
    pub course_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateCourseInstructorRoleRequest {
    #[schema(value_type = CourseInstructorRole)]
    pub role: String,
}

#[derive(Debug, Deserialize, ToSchema, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct BatchListCourseInstructorsQuery {
    pub course_ids: String,
}

#[derive(Debug, Deserialize, ToSchema, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ClassroomActivityQuery {
    pub semester_id: Uuid,
}
