//! Date-effective enrollment projection for attendance consumers.
use chrono::NaiveDate;
use school_errors::AppError;
use sqlx::{FromRow, PgConnection, PgPool, Postgres, QueryBuilder};
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
#[derive(Debug, Clone, FromRow)]
pub struct AttendanceCoreRosterStudent {
    pub student_id: Uuid,
    pub student_academic_year_id: Uuid,
    pub class_number: Option<i32>,
}
pub const ROOM_ROSTER_SQL: &str = r#"WITH dated_placements AS (
    SELECT DISTINCT ON (p.student_academic_year_id)
        p.student_academic_year_id,p.homeroom_id,p.class_number
    FROM homeroom_placements p
    JOIN academic_terms t ON t.academic_year_id=p.academic_year_id
    WHERE t.id=$1 AND p.status IN ('current','ended')
      AND p.start_date<=$2 AND (p.end_date IS NULL OR p.end_date>=$2)
    ORDER BY p.student_academic_year_id,p.start_date DESC,p.id
) SELECT y.student_id,y.id student_academic_year_id,p.class_number
FROM dated_placements p
JOIN student_academic_years y ON y.id=p.student_academic_year_id
JOIN users u ON u.id=y.student_id AND u.user_type='student'
WHERE p.homeroom_id=$3
ORDER BY p.class_number NULLS LAST,y.student_id LIMIT 10001"#;

/// Resolve the latest dated placement before filtering the selected room.
pub async fn students_in_room(
    pool: &PgPool,
    term: Uuid,
    date: NaiveDate,
    homeroom: Uuid,
) -> Result<Vec<AttendanceCoreRosterStudent>, AppError> {
    let rows: Vec<AttendanceCoreRosterStudent> = sqlx::query_as(ROOM_ROSTER_SQL)
        .bind(term)
        .bind(date)
        .bind(homeroom)
        .fetch_all(pool)
        .await?;
    if rows.len() > 10000 {
        return Err(AppError::ValidationError(
            "นักเรียนเกินขนาดที่รองรับ กรุณาแบ่งขอบเขต".into(),
        ));
    }
    Ok(rows)
}

pub async fn room(
    pool: &PgPool,
    term: Uuid,
    homeroom: Uuid,
) -> Result<Option<AttendanceCoreRoom>, AppError> {
    Ok(sqlx::query_as("SELECT h.id,h.name,ARRAY(SELECT a.user_id FROM homeroom_advisors a WHERE a.homeroom_id=h.id ORDER BY a.user_id) teacher_ids FROM homerooms h JOIN academic_terms t ON t.academic_year_id=h.academic_year_id WHERE t.id=$1 AND h.id=$2 AND h.is_active")
        .bind(term).bind(homeroom).fetch_optional(pool).await?)
}
#[derive(Debug, Clone, FromRow)]
pub struct AttendancePlacementSpan {
    pub student_id: Uuid,
    pub student_academic_year_id: Uuid,
    pub homeroom_id: Uuid,
    pub class_number: Option<i32>,
    pub start_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
}
/// One bounded read for historical coverage, rather than a read for every date.
pub async fn placement_spans(
    connection: &mut PgConnection,
    term: Uuid,
    start: NaiveDate,
    end: NaiveDate,
) -> Result<Vec<AttendancePlacementSpan>, AppError> {
    placement_spans_scoped(connection, term, start, end, None).await
}

/// The same dated-placement precedence, scoped before transfer and row limits.
pub async fn placement_spans_for_student(
    connection: &mut PgConnection,
    term: Uuid,
    start: NaiveDate,
    end: NaiveDate,
    student: Uuid,
) -> Result<Vec<AttendancePlacementSpan>, AppError> {
    placement_spans_scoped(connection, term, start, end, Some(student)).await
}

async fn placement_spans_scoped(
    connection: &mut PgConnection,
    term: Uuid,
    start: NaiveDate,
    end: NaiveDate,
    student: Option<Uuid>,
) -> Result<Vec<AttendancePlacementSpan>, AppError> {
    let mut query = QueryBuilder::<Postgres>::new("SELECT y.student_id,y.id student_academic_year_id,p.homeroom_id,p.class_number,p.start_date,p.end_date FROM student_academic_years y JOIN academic_terms t ON t.academic_year_id=y.academic_year_id JOIN homeroom_placements p ON p.student_academic_year_id=y.id WHERE t.id=");
    query
        .push_bind(term)
        .push(" AND p.status IN ('current','ended') AND p.start_date<=")
        .push_bind(end)
        .push(" AND (p.end_date IS NULL OR p.end_date>=")
        .push_bind(start)
        .push(")");
    if let Some(student) = student {
        query.push(" AND y.student_id=").push_bind(student);
    }
    query.push(" ORDER BY y.student_id,p.start_date DESC,p.id LIMIT 100001");
    let rows: Vec<AttendancePlacementSpan> =
        query.build_query_as().fetch_all(&mut *connection).await?;
    if rows.len() > 100000 {
        return Err(AppError::ValidationError(
            "ประวัติห้องเรียนเกินขนาดที่รองรับ".into(),
        ));
    }
    Ok(rows)
}
pub const ALL_STUDENTS_SQL: &str = r#"SELECT y.student_id,y.id student_academic_year_id,concat_ws(' ',u.title,u.first_name,u.last_name) display_name,p.homeroom_id,h.name homeroom_name,p.class_number
 FROM student_academic_years y JOIN academic_terms t ON t.academic_year_id=y.academic_year_id JOIN users u ON u.id=y.student_id AND u.user_type='student'
 LEFT JOIN LATERAL(SELECT p.homeroom_id,p.class_number FROM homeroom_placements p WHERE p.student_academic_year_id=y.id AND p.status IN ('current','ended') AND p.start_date<=$2 AND (p.end_date IS NULL OR p.end_date>=$2) ORDER BY p.start_date DESC,p.id LIMIT 1) p ON true
 LEFT JOIN homerooms h ON h.id=p.homeroom_id WHERE t.id=$1 AND p.homeroom_id IS NOT NULL ORDER BY h.name,p.class_number NULLS LAST,u.first_name,u.id LIMIT 10001"#;

pub async fn students(
    pool: &PgPool,
    term: Uuid,
    date: NaiveDate,
) -> Result<Vec<AttendanceCoreStudent>, AppError> {
    students_on_connection(&mut *pool.acquire().await?, term, date).await
}
pub async fn students_on_connection(
    connection: &mut PgConnection,
    term: Uuid,
    date: NaiveDate,
) -> Result<Vec<AttendanceCoreStudent>, AppError> {
    let rows = sqlx::query_as(ALL_STUDENTS_SQL)
        .bind(term)
        .bind(date)
        .fetch_all(&mut *connection)
        .await?;
    let rows: Vec<AttendanceCoreStudent> = rows;
    if rows.len() > 10000 {
        return Err(AppError::ValidationError(
            "นักเรียนเกินขนาดที่รองรับ กรุณาแบ่งขอบเขต".into(),
        ));
    }
    Ok(rows)
}
pub async fn rooms(pool: &PgPool, term: Uuid) -> Result<Vec<AttendanceCoreRoom>, AppError> {
    rooms_on_connection(&mut *pool.acquire().await?, term).await
}
pub async fn rooms_on_connection(
    connection: &mut PgConnection,
    term: Uuid,
) -> Result<Vec<AttendanceCoreRoom>, AppError> {
    Ok(sqlx::query_as("SELECT h.id,h.name,ARRAY(SELECT a.user_id FROM homeroom_advisors a WHERE a.homeroom_id=h.id ORDER BY a.user_id) teacher_ids FROM homerooms h JOIN academic_terms t ON t.academic_year_id=h.academic_year_id WHERE t.id=$1 AND h.is_active ORDER BY h.name,h.id").bind(term).fetch_all(&mut *connection).await?)
}
/// Historical attendance retains rooms with participants on the requested date,
/// even when the current UI has subsequently deactivated that room.
pub async fn rooms_for_date_on_connection(
    connection: &mut PgConnection,
    term: Uuid,
    date: NaiveDate,
) -> Result<Vec<AttendanceCoreRoom>, AppError> {
    Ok(sqlx::query_as("SELECT h.id,h.name,ARRAY(SELECT a.user_id FROM homeroom_advisors a WHERE a.homeroom_id=h.id ORDER BY a.user_id) teacher_ids FROM homerooms h JOIN academic_terms t ON t.academic_year_id=h.academic_year_id WHERE t.id=$1 AND EXISTS(SELECT 1 FROM homeroom_placements p WHERE p.homeroom_id=h.id AND p.status IN ('current','ended') AND p.start_date<=$2 AND (p.end_date IS NULL OR p.end_date>=$2)) ORDER BY h.name,h.id")
        .bind(term).bind(date).fetch_all(&mut *connection).await?)
}
pub async fn advisor_for_student(
    pool: &PgPool,
    teacher: Uuid,
    student: Uuid,
    date: NaiveDate,
) -> Result<bool, AppError> {
    Ok(sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM homeroom_placements p JOIN student_academic_years y ON y.id=p.student_academic_year_id JOIN homeroom_advisors a ON a.homeroom_id=p.homeroom_id WHERE a.user_id=$1 AND y.student_id=$2 AND p.status IN ('current','ended') AND p.start_date<=$3 AND (p.end_date IS NULL OR p.end_date>=$3))").bind(teacher).bind(student).bind(date).fetch_one(pool).await?)
}
