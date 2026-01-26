use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, FromRow)]
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

#[derive(Debug, Deserialize)]
pub struct PlanQuery {
    pub classroom_id: Option<Uuid>,
    pub instructor_id: Option<Uuid>,
    pub academic_semester_id: Option<Uuid>,
    pub subject_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct AssignCoursesRequest {
    pub classroom_id: Uuid,
    pub academic_semester_id: Uuid,
    pub subject_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateCourseRequest {
    pub primary_instructor_id: Option<Uuid>,
    pub settings: Option<serde_json::Value>,
}
