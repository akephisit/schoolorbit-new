//! Date-effective enrollment projection for attendance consumers.
use chrono::NaiveDate;
use school_errors::AppError;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;
#[derive(Debug, Clone, FromRow)]
pub struct AttendanceCoreStudent {
    pub student_id: Uuid,
    pub student_academic_year_id: Uuid,
    pub display_name: String,
    pub homeroom_id: Option<Uuid>,
    pub homeroom_name: Option<String>,
    pub class_number: Option<i32>,
}
#[derive(Debug, Clone, FromRow)]
pub struct AttendanceCoreRoom {
    pub id: Uuid,
    pub name: String,
    pub teacher_ids: Vec<Uuid>,
}
pub async fn students(
    pool: &PgPool,
    term: Uuid,
    date: NaiveDate,
) -> Result<Vec<AttendanceCoreStudent>, AppError> {
    let rows=sqlx::query_as(r#"SELECT y.student_id,y.id student_academic_year_id,concat_ws(' ',u.title,u.first_name,u.last_name) display_name,p.homeroom_id,h.name homeroom_name,p.class_number
 FROM student_academic_years y JOIN academic_terms t ON t.academic_year_id=y.academic_year_id JOIN users u ON u.id=y.student_id AND u.user_type='student'
 LEFT JOIN LATERAL(SELECT p.homeroom_id,p.class_number FROM homeroom_placements p WHERE p.student_academic_year_id=y.id AND p.status IN ('current','ended') AND p.start_date<=$2 AND (p.end_date IS NULL OR p.end_date>=$2) ORDER BY p.start_date DESC,p.id LIMIT 1) p ON true
 LEFT JOIN homerooms h ON h.id=p.homeroom_id WHERE t.id=$1 AND p.homeroom_id IS NOT NULL ORDER BY h.name,p.class_number NULLS LAST,u.first_name,u.id LIMIT 10001"#).bind(term).bind(date).fetch_all(pool).await?;
    let rows: Vec<AttendanceCoreStudent> = rows;
    if rows.len() > 10000 {
        return Err(AppError::ValidationError(
            "นักเรียนเกินขนาดที่รองรับ กรุณาแบ่งขอบเขต".into(),
        ));
    }
    Ok(rows)
}
pub async fn rooms(pool: &PgPool, term: Uuid) -> Result<Vec<AttendanceCoreRoom>, AppError> {
    Ok(sqlx::query_as("SELECT h.id,h.name,ARRAY(SELECT a.user_id FROM homeroom_advisors a WHERE a.homeroom_id=h.id ORDER BY a.user_id) teacher_ids FROM homerooms h JOIN academic_terms t ON t.academic_year_id=h.academic_year_id WHERE t.id=$1 AND h.is_active ORDER BY h.name,h.id").bind(term).fetch_all(pool).await?)
}
pub async fn advisor_for_student(
    pool: &PgPool,
    teacher: Uuid,
    student: Uuid,
    date: NaiveDate,
) -> Result<bool, AppError> {
    Ok(sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM homeroom_placements p JOIN student_academic_years y ON y.id=p.student_academic_year_id JOIN homeroom_advisors a ON a.homeroom_id=p.homeroom_id WHERE a.user_id=$1 AND y.student_id=$2 AND p.status IN ('current','ended') AND p.start_date<=$3 AND (p.end_date IS NULL OR p.end_date>=$3))").bind(teacher).bind(student).bind(date).fetch_one(pool).await?)
}
