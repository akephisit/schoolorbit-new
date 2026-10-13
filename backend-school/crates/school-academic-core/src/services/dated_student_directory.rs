//! Core-owned dated student directory and roster stability for external consumers.
use chrono::NaiveDate;
use school_errors::AppError;
use sqlx::{FromRow, PgConnection};
use uuid::Uuid;

#[derive(Debug, Clone, FromRow)]
pub struct DatedStudentDirectoryEntry {
    pub id: Uuid,
    pub student_academic_year_id: Uuid,
    pub name: String,
    pub student_code: Option<String>,
    pub grade_level_id: Uuid,
    pub grade_level: String,
    pub homeroom_id: Option<Uuid>,
    pub homeroom: String,
    pub class_number: Option<i32>,
    pub advisor_ids: Vec<Uuid>,
    pub parent_ids: Vec<Uuid>,
}

/// Dated placement owner uses the latest applicable span before room filtering,
/// so a moved student's earlier room never wins merely because it was requested.
pub async fn students_on_connection(
    connection: &mut PgConnection,
    term: Uuid,
    date: NaiveDate,
) -> Result<Vec<DatedStudentDirectoryEntry>, AppError> {
    let rows:Vec<DatedStudentDirectoryEntry>=sqlx::query_as(r#"SELECT y.student_id id,y.id student_academic_year_id,concat_ws(' ',u.title,u.first_name,u.last_name) name,s.student_id student_code,
      y.grade_level_id,CASE g.level_type WHEN 'kindergarten' THEN 'อ.' WHEN 'primary' THEN 'ป.' ELSE 'ม.' END||g.year::text grade_level,
      p.homeroom_id,COALESCE(h.name,'ยังไม่จัดห้อง') homeroom,p.class_number,
      ARRAY(SELECT a.user_id FROM homeroom_advisors a WHERE a.homeroom_id=h.id ORDER BY a.user_id) advisor_ids,
      ARRAY(SELECT parent.parent_user_id FROM student_parents parent JOIN users pu ON pu.id=parent.parent_user_id AND pu.status='active' WHERE parent.student_user_id=y.student_id ORDER BY parent.parent_user_id) parent_ids
      FROM student_academic_years y JOIN academic_terms t ON t.academic_year_id=y.academic_year_id
      JOIN users u ON u.id=y.student_id AND u.user_type='student' LEFT JOIN student_info s ON s.user_id=u.id
      JOIN grade_levels g ON g.id=y.grade_level_id
      LEFT JOIN LATERAL(SELECT p.homeroom_id,p.class_number FROM homeroom_placements p WHERE p.student_academic_year_id=y.id AND p.status IN('current','ended') AND p.start_date<=$2 AND(p.end_date IS NULL OR p.end_date>=$2) ORDER BY p.start_date DESC,p.id LIMIT 1)p ON true
      LEFT JOIN homerooms h ON h.id=p.homeroom_id WHERE t.id=$1 AND(p.homeroom_id IS NOT NULL OR y.status='active') ORDER BY y.student_id LIMIT 10001"#)
      .bind(term).bind(date).fetch_all(connection).await?;
    if rows.len() > 10000 {
        return Err(AppError::ValidationError("รายชื่อนักเรียนเกินขนาดที่รองรับ".into()));
    }
    Ok(rows)
}

/// Caller owns the transaction and lifecycle lock. Selected year/room parent
/// locks serialize Core placement/advisor changes until the consumer commits.
pub async fn locked_students_on_connection(
    connection: &mut PgConnection,
    term: Uuid,
    date: NaiveDate,
    student_ids: &[Uuid],
) -> Result<Vec<DatedStudentDirectoryEntry>, AppError> {
    let selected: std::collections::BTreeSet<_> = student_ids.iter().copied().collect();
    // Core placement creation/transfer locks these rows FOR UPDATE. Preserve the
    // selected dated membership until score commit, including unassigned pupils.
    sqlx::query("SELECT y.id FROM student_academic_years y JOIN academic_terms t ON t.academic_year_id=y.academic_year_id WHERE t.id=$1 AND y.student_id=ANY($2) ORDER BY y.id FOR SHARE OF y").bind(term).bind(student_ids).fetch_all(&mut *connection).await?;
    let preliminary = students_on_connection(connection, term, date).await?;
    let mut rooms: Vec<_> = preliminary
        .iter()
        .filter(|r| selected.contains(&r.id))
        .filter_map(|r| r.homeroom_id)
        .collect();
    rooms.sort_unstable();
    rooms.dedup();
    // Advisor replacement owns the parent room row; reading after acquiring its
    // share lock observes any committed assignment change before token checking.
    sqlx::query("SELECT id FROM homerooms WHERE id=ANY($1) ORDER BY id FOR SHARE")
        .bind(rooms)
        .fetch_all(&mut *connection)
        .await?;
    students_on_connection(connection, term, date).await
}
