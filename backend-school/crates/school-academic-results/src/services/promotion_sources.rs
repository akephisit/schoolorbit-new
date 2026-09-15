use super::*;
use std::collections::BTreeMap;
use uuid::Uuid;

pub async fn promotion_annual_sources(
    tx: &mut Transaction<'_, Postgres>,
    year: Uuid,
    students: &[Uuid],
) -> Result<BTreeMap<Uuid, Option<AnnualResultRevision>>, AppError> {
    // The owner validates 1–500 distinct student-year IDs and exact year scope.
    // Caller holds a consistent read transaction or the lifecycle transition lock.
    let previews = annual_revisions::annual_students_in_transaction(tx, year, students).await?;
    let rows:Vec<StoredSource>=sqlx::query_as(
        "SELECT DISTINCT ON(student_academic_year_id) student_academic_year_id,id,revision,snapshot,official_gpa::text,hold_reason,locked_by,locked_at FROM academic_annual_result_revisions WHERE academic_year_id=$1 AND student_academic_year_id=ANY($2) ORDER BY student_academic_year_id,revision DESC"
    ).bind(year).bind(students).fetch_all(&mut **tx).await?;
    let mut sources: BTreeMap<_, _> = students.iter().map(|id| (*id, None)).collect();
    for row in rows {
        let preview = previews.get(&row.student_academic_year_id).ok_or_else(|| {
            AppError::InternalServerError("ผลรายปีไม่ตรงกับนักเรียนในชุดที่ตรวจสอบ".into())
        })?;
        let is_current = preview.can_lock
            && preview.source_checksum == row.snapshot.source_checksum
            && preview.needs_hold == row.hold_reason.is_some();
        sources.insert(
            row.student_academic_year_id,
            Some(AnnualResultRevision {
                id: row.id,
                revision: row.revision,
                snapshot: row.snapshot.0,
                official_gpa: row.official_gpa,
                hold_reason: row.hold_reason,
                locked_by: row.locked_by,
                locked_at: row.locked_at,
                is_current,
            }),
        );
    }
    Ok(sources)
}

#[derive(sqlx::FromRow)]
struct StoredSource {
    student_academic_year_id: Uuid,
    id: Uuid,
    revision: i64,
    snapshot: sqlx::types::Json<AnnualResultPreview>,
    official_gpa: Option<String>,
    hold_reason: Option<String>,
    locked_by: Uuid,
    locked_at: chrono::DateTime<chrono::Utc>,
}
