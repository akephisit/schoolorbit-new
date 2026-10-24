//! Historical group membership projection; current status alone is insufficient.
use chrono::NaiveDate;
use school_database::DbSession;
use school_errors::AppError;
use sqlx::{FromRow, PgConnection, PgPool, Postgres, QueryBuilder};
use uuid::Uuid;
#[derive(Debug, Clone, FromRow)]
pub struct AttendanceGroupStudent {
    pub learning_group_id: Uuid,
    pub student_id: Uuid,
    pub student_academic_year_id: Uuid,
}
#[derive(Debug, Clone, FromRow)]
pub struct AttendanceMembershipSpan {
    pub learning_group_id: Uuid,
    pub student_id: Uuid,
    pub joined_at: NaiveDate,
    pub left_at: Option<NaiveDate>,
}
pub async fn membership_spans(
    connection: &mut PgConnection,
    groups: &[Uuid],
    start: NaiveDate,
    end: NaiveDate,
) -> Result<Vec<AttendanceMembershipSpan>, AppError> {
    membership_spans_scoped(connection, groups, start, end, None).await
}

/// Resolve one student's historical membership without loading their classmates.
pub async fn membership_spans_for_student(
    connection: &mut PgConnection,
    groups: &[Uuid],
    start: NaiveDate,
    end: NaiveDate,
    student: Uuid,
) -> Result<Vec<AttendanceMembershipSpan>, AppError> {
    membership_spans_scoped(connection, groups, start, end, Some(student)).await
}

async fn membership_spans_scoped(
    connection: &mut PgConnection,
    groups: &[Uuid],
    start: NaiveDate,
    end: NaiveDate,
    student: Option<Uuid>,
) -> Result<Vec<AttendanceMembershipSpan>, AppError> {
    if groups.len() > 20000 {
        return Err(AppError::ValidationError("กลุ่มเกินขนาดที่รองรับ".into()));
    }
    let mut query = QueryBuilder::<Postgres>::new("SELECT learning_group_id,student_id,joined_at,left_at FROM learning_group_students WHERE learning_group_id=ANY(");
    query
        .push_bind(groups)
        .push(") AND membership_status<>'removed' AND joined_at<=")
        .push_bind(end)
        .push(" AND (left_at IS NULL OR left_at>=")
        .push_bind(start)
        .push(")");
    if let Some(student) = student {
        query.push(" AND student_id=").push_bind(student);
    }
    query.push(" ORDER BY learning_group_id,student_id LIMIT 100001");
    let rows: Vec<AttendanceMembershipSpan> = DbSession::new(&mut *connection)
        .prepared_query_as(
            "academic_delivery.services.attendance_roster.membership_spans_scoped",
            query.build_query_as(),
        )
        .fetch_all()
        .await?;
    if rows.len() > 100000 {
        return Err(AppError::ValidationError(
            "ประวัติกลุ่มเรียนเกินขนาดที่รองรับ".into(),
        ));
    }
    Ok(rows)
}
pub async fn students(
    pool: &PgPool,
    groups: &[Uuid],
    date: NaiveDate,
) -> Result<Vec<AttendanceGroupStudent>, AppError> {
    students_on_connection(
        &mut *school_tenancy::TenantPool::observed_checkout_pool(
            pool,
            "academic_delivery.services.attendance_roster.students",
        )
        .await?,
        groups,
        date,
    )
    .await
}
pub async fn students_on_connection(
    connection: &mut PgConnection,
    groups: &[Uuid],
    date: NaiveDate,
) -> Result<Vec<AttendanceGroupStudent>, AppError> {
    if groups.len() > 2000 {
        return Err(AppError::ValidationError("กลุ่มเรียนเกินขนาดที่รองรับ".into()));
    }
    let rows:Vec<AttendanceGroupStudent>=DbSession::new(&mut *connection).query_as("academic_delivery.services.attendance_roster.students_on_connection", "SELECT learning_group_id,student_id,student_academic_year_id FROM learning_group_students WHERE learning_group_id=ANY($1) AND membership_status<>'removed' AND joined_at<=$2 AND (left_at IS NULL OR left_at>=$2) ORDER BY learning_group_id,student_id LIMIT 100001").bind(groups).bind(date).fetch_all().await?;
    if rows.len() > 100000 {
        return Err(AppError::ValidationError("สมาชิกกลุ่มเกินขนาดที่รองรับ".into()));
    }
    Ok(rows)
}
