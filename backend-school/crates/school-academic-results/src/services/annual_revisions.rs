use super::aggregate_revisions::write_error;
use super::*;
use crate::policy::aggregate::{require_aggregate_lock, require_aggregate_read};
use school_authorization::ActorContext;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(sqlx::FromRow)]
struct LatestAnnualRevision {
    student_academic_year_id: Uuid,
    id: Uuid,
    revision: i64,
    source_checksum: String,
    hold_reason: Option<String>,
}

pub async fn annual_closure_coverage(
    tx: &mut Transaction<'_, Postgres>,
    year: Uuid,
) -> Result<AnnualClosureCoverage, AppError> {
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM academic_years WHERE id=$1)")
            .bind(year)
            .fetch_one(&mut **tx)
            .await?;
    if !exists {
        return Err(AppError::NotFound("Academic year not found".into()));
    }
    let expected:Vec<Uuid>=sqlx::query_scalar(
        "SELECT id FROM student_academic_years WHERE academic_year_id=$1 AND status IN ('active','completed')
         UNION SELECT student_academic_year_id FROM learning_group_students WHERE academic_year_id=$1 AND membership_status='active'
         UNION SELECT student_academic_year_id FROM academic_course_results WHERE academic_year_id=$1
         UNION SELECT student_academic_year_id FROM academic_activity_results WHERE academic_year_id=$1
         UNION SELECT student_academic_year_id FROM academic_term_aggregate_revisions WHERE academic_year_id=$1
         UNION SELECT student_academic_year_id FROM academic_annual_result_revisions WHERE academic_year_id=$1
         ORDER BY 1 LIMIT 10001"
    ).bind(year).fetch_all(&mut **tx).await?;
    if expected.len() > 10000 {
        return Err(AppError::ValidationError(
            "Annual coverage exceeds 10000 students".into(),
        ));
    }
    let rows:Vec<LatestAnnualRevision>=sqlx::query_as("SELECT DISTINCT ON(student_academic_year_id) student_academic_year_id,id,revision,source_checksum,hold_reason FROM academic_annual_result_revisions WHERE academic_year_id=$1 AND student_academic_year_id=ANY($2) ORDER BY student_academic_year_id,revision DESC").bind(year).bind(&expected).fetch_all(&mut **tx).await?;
    let ids: Vec<Uuid> = rows
        .iter()
        .map(|row| row.student_academic_year_id)
        .collect();
    let mut latest: std::collections::BTreeMap<Uuid, LatestAnnualRevision> = rows
        .into_iter()
        .map(|row| (row.student_academic_year_id, row))
        .collect();
    let mut current = std::collections::BTreeMap::new();
    for batch in ids.chunks(500) {
        for (student, preview) in annual_students_in_transaction(tx, year, batch).await? {
            current.insert(
                student,
                (
                    preview.source_checksum,
                    preview.can_lock,
                    preview.needs_hold,
                ),
            );
        }
    }
    let students: Vec<AnnualClosureStudent> = expected
        .into_iter()
        .map(|student| {
            let revision = latest.remove(&student);
            let is_current = match (&revision, current.remove(&student)) {
                (Some(row), Some((checksum, can_lock, needs_hold))) => {
                    can_lock
                        && row.source_checksum == checksum
                        && row.hold_reason.is_some() == needs_hold
                }
                _ => false,
            };
            AnnualClosureStudent {
                student_academic_year_id: student,
                revision_id: revision.as_ref().map(|row| row.id),
                revision: revision.as_ref().map(|row| row.revision),
                is_current,
                hold_reason: revision.and_then(|row| row.hold_reason),
            }
        })
        .collect();
    let ready = !students.is_empty() && students.iter().all(|row| row.is_current);
    let source_checksum = hash(&(year, &students))?;
    Ok(AnnualClosureCoverage {
        students,
        ready,
        source_checksum,
    })
}

pub async fn require_term_without_annual_results(
    tx: &mut Transaction<'_, Postgres>,
    year: Uuid,
    term: Uuid,
) -> Result<(), AppError> {
    let exists:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM academic_annual_result_term_sources WHERE academic_year_id=$1 AND academic_term_id=$2)").bind(year).bind(term).fetch_one(&mut **tx).await?;
    if exists {
        return Err(AppError::Conflict(
            "เปิดภาคเรียนกลับไม่ได้ เพราะมีผลรายปีอ้างอิงอยู่ ให้ใช้การแก้ผลการเรียนและสรุปรุ่นใหม่".into(),
        ));
    }
    Ok(())
}

#[derive(sqlx::FromRow)]
struct AnnualRevisionRow {
    id: Uuid,
    revision: i64,
    snapshot: sqlx::types::Json<AnnualResultPreview>,
    official_gpa: Option<String>,
    hold_reason: Option<String>,
    locked_by: Uuid,
    locked_at: chrono::DateTime<chrono::Utc>,
    request_checksum: String,
}

impl AnnualRevisionRow {
    fn wire(self, is_current: bool) -> AnnualResultRevision {
        AnnualResultRevision {
            id: self.id,
            revision: self.revision,
            snapshot: self.snapshot.0,
            official_gpa: self.official_gpa,
            hold_reason: self.hold_reason,
            locked_by: self.locked_by,
            locked_at: self.locked_at,
            is_current,
        }
    }
}

pub async fn lock_annual(
    pool: &PgPool,
    actor: &ActorContext,
    year: Uuid,
    student: Uuid,
    input: AnnualLockInput,
) -> Result<AnnualResultRevision, AppError> {
    require_aggregate_lock(actor)?;
    if input.request_id.is_nil()
        || input.expected_revision.is_some_and(|value| value < 1)
        || input.source_checksum.len() != 64
        || !input
            .source_checksum
            .bytes()
            .all(|value| value.is_ascii_digit() || (b'a'..=b'f').contains(&value))
    {
        return Err(AppError::ValidationError(
            "Invalid annual revision, request ID or source checksum".into(),
        ));
    }
    let request_checksum = hash(&(actor.user_id, year, student, &input))?;
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL SERIALIZABLE")
        .execute(&mut *tx)
        .await?;
    lifecycle_guard::lock_transition_shared(&mut tx).await?;
    let previous:Option<AnnualRevisionRow>=sqlx::query_as("SELECT id,revision,snapshot,official_gpa::text,hold_reason,locked_by,locked_at,request_checksum FROM academic_annual_result_revisions WHERE request_id=$1").bind(input.request_id).fetch_optional(&mut *tx).await?;
    if previous
        .as_ref()
        .is_some_and(|row| row.request_checksum != request_checksum)
    {
        return Err(conflict());
    }
    let latest:Option<i64>=sqlx::query_scalar("SELECT max(revision) FROM academic_annual_result_revisions WHERE academic_year_id=$1 AND student_academic_year_id=$2").bind(year).bind(student).fetch_one(&mut *tx).await?;
    let preview = annual_in_transaction(&mut tx, year, student).await?;
    if let Some(previous) = previous {
        let current = Some(previous.revision) == latest
            && preview.can_lock
            && previous.snapshot.source_checksum == preview.source_checksum;
        tx.commit().await.map_err(write_error)?;
        return Ok(previous.wire(current));
    }
    check_version(input.expected_revision, latest)?;
    if !preview.can_lock || preview.source_checksum != input.source_checksum {
        return Err(conflict());
    }
    let reason = input
        .hold_reason
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if preview.needs_hold != reason.is_some()
        || reason.is_some_and(|value| value.chars().count() > 1000)
    {
        return Err(AppError::ValidationError(
            "ระบุเหตุผลไม่เกิน 1000 ตัวอักษรเฉพาะเมื่อมีผลที่ต้องพิจารณาค้างไว้".into(),
        ));
    }
    let official_gpa = if reason.is_none() {
        preview
            .totals
            .provisional_gpa
            .as_deref()
            .map(decimal)
            .transpose()?
    } else {
        None
    };
    let revision = latest
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| AppError::ValidationError("Annual revision limit exceeded".into()))?;
    let row:AnnualRevisionRow=sqlx::query_as("INSERT INTO academic_annual_result_revisions(student_academic_year_id,academic_year_id,revision,source_checksum,request_id,request_checksum,snapshot,official_gpa,hold_reason,locked_by) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10) RETURNING id,revision,snapshot,official_gpa::text,hold_reason,locked_by,locked_at,request_checksum")
        .bind(student).bind(year).bind(revision).bind(&preview.source_checksum).bind(input.request_id).bind(request_checksum).bind(sqlx::types::Json(&preview)).bind(official_gpa).bind(reason).bind(actor.user_id).fetch_one(&mut *tx).await.map_err(write_error)?;
    let mut term_ids = Vec::with_capacity(preview.terms.len());
    let mut revision_ids = Vec::with_capacity(preview.terms.len());
    for term in &preview.terms {
        let source = term.revision.as_ref().ok_or_else(conflict)?;
        term_ids.push(term.academic_term_id);
        revision_ids.push(source.id);
    }
    sqlx::query("INSERT INTO academic_annual_result_term_sources(annual_revision_id,academic_year_id,academic_term_id,student_academic_year_id,term_aggregate_revision_id) SELECT $1,$2,source.term_id,$3,source.revision_id FROM unnest($4::uuid[],$5::uuid[]) AS source(term_id,revision_id)")
        .bind(row.id).bind(year).bind(student).bind(term_ids).bind(revision_ids).execute(&mut *tx).await.map_err(write_error)?;
    tx.commit().await.map_err(write_error)?;
    Ok(row.wire(true))
}

pub async fn list_annual_revisions(
    pool: &PgPool,
    actor: &ActorContext,
    year: Uuid,
    student: Uuid,
) -> Result<Vec<AnnualResultRevision>, AppError> {
    require_aggregate_read(actor)?;
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let preview = annual_in_transaction(&mut tx, year, student).await?;
    let rows:Vec<AnnualRevisionRow>=sqlx::query_as("SELECT id,revision,snapshot,official_gpa::text,hold_reason,locked_by,locked_at,request_checksum FROM academic_annual_result_revisions WHERE academic_year_id=$1 AND student_academic_year_id=$2 ORDER BY revision DESC LIMIT 101").bind(year).bind(student).fetch_all(&mut *tx).await?;
    if rows.len() > 100 {
        return Err(AppError::ValidationError(
            "Annual history exceeds 100 revisions".into(),
        ));
    }
    let history = rows
        .into_iter()
        .enumerate()
        .map(|(index, row)| {
            let current = index == 0
                && preview.can_lock
                && row.snapshot.source_checksum == preview.source_checksum;
            row.wire(current)
        })
        .collect();
    tx.commit().await?;
    Ok(history)
}

#[derive(Clone, sqlx::FromRow)]
struct IncludedTerm {
    id: Uuid,
    name: String,
    sequence: i32,
}

#[derive(sqlx::FromRow)]
struct TermRevisionRow {
    id: Uuid,
    revision: i64,
    snapshot: sqlx::types::Json<TermAggregatePreview>,
    official_gpa: Option<String>,
    hold_reason: Option<String>,
    locked_by: Uuid,
    locked_at: chrono::DateTime<chrono::Utc>,
}

pub async fn preview_annual(
    pool: &PgPool,
    actor: &ActorContext,
    year: Uuid,
    student: Uuid,
) -> Result<AnnualResultPreview, AppError> {
    crate::policy::aggregate::require_aggregate_read(actor)?;
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let preview = annual_in_transaction(&mut tx, year, student).await?;
    tx.commit().await?;
    Ok(preview)
}

async fn annual_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    year: Uuid,
    student: Uuid,
) -> Result<AnnualResultPreview, AppError> {
    annual_students_in_transaction(tx, year, &[student])
        .await?
        .remove(&student)
        .ok_or_else(|| {
            AppError::InternalServerError("Annual batch omitted a validated student".into())
        })
}

pub(super) async fn annual_students_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    year: Uuid,
    students: &[Uuid],
) -> Result<std::collections::BTreeMap<Uuid, AnnualResultPreview>, AppError> {
    let distinct: std::collections::BTreeSet<Uuid> = students.iter().copied().collect();
    if students.is_empty() || students.len() > 500 || distinct.len() != students.len() {
        return Err(AppError::ValidationError(
            "Select 1–500 distinct students for annual results".into(),
        ));
    }
    let found: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM student_academic_years WHERE id=ANY($1) AND academic_year_id=$2",
    )
    .bind(students)
    .bind(year)
    .fetch_all(&mut **tx)
    .await?;
    if found.len() != students.len() {
        return Err(AppError::NotFound(
            "Student academic year not found in selected context".into(),
        ));
    }
    let included: Vec<IncludedTerm> = sqlx::query_as("SELECT id,name,sequence_no AS sequence FROM academic_terms WHERE academic_year_id=$1 AND included_in_year_result AND status<>'cancelled' ORDER BY sequence_no,id LIMIT 101")
        .bind(year).fetch_all(&mut **tx).await?;
    if included.len() > 100 {
        return Err(AppError::ValidationError(
            "Annual result exceeds 100 terms".into(),
        ));
    }
    let mut applicable: std::collections::BTreeMap<Uuid, Vec<(IncludedTerm, TermClosureStudent)>> =
        students
            .iter()
            .map(|student| (*student, Vec::new()))
            .collect();
    for term in included {
        let context = ResultContext {
            academic_year_id: year,
            academic_term_id: term.id,
        };
        let coverage =
            closure_coverage::term_closure_coverage_for_students(tx, &context, students).await?;
        for source in coverage.students {
            let sources = applicable
                .get_mut(&source.student_academic_year_id)
                .ok_or_else(|| {
                    AppError::InternalServerError(
                        "Annual coverage returned an unexpected student".into(),
                    )
                })?;
            sources.push((term.clone(), source));
        }
    }
    let ids: Vec<Uuid> = applicable
        .values()
        .flatten()
        .filter_map(|(_, row)| row.revision_id)
        .collect();
    let rows: Vec<TermRevisionRow> = sqlx::query_as("SELECT id,revision,snapshot,official_gpa::text,hold_reason,locked_by,locked_at FROM academic_term_aggregate_revisions WHERE id=ANY($1) AND academic_year_id=$2 AND student_academic_year_id=ANY($3)")
        .bind(&ids).bind(year).bind(students).fetch_all(&mut **tx).await?;
    let mut revisions: std::collections::BTreeMap<Uuid, TermRevisionRow> =
        rows.into_iter().map(|row| (row.id, row)).collect();
    let mut previews = std::collections::BTreeMap::new();
    for (student, sources) in applicable {
        let mut terms = Vec::with_capacity(sources.len());
        for (term, source) in sources {
            let revision = source
                .revision_id
                .and_then(|id| revisions.remove(&id))
                .map(|row| TermAggregateRevision {
                    id: row.id,
                    revision: row.revision,
                    snapshot: row.snapshot.0,
                    official_gpa: row.official_gpa,
                    hold_reason: row.hold_reason,
                    locked_by: row.locked_by,
                    locked_at: row.locked_at,
                    is_current: source.is_current,
                });
            terms.push(AnnualTermSource {
                academic_term_id: term.id,
                term_name: term.name,
                sequence: term.sequence,
                is_current: source.is_current,
                revision,
            });
        }
        previews.insert(student, build_annual_preview(year, student, terms)?);
    }
    Ok(previews)
}

fn build_annual_preview(
    year: Uuid,
    student: Uuid,
    terms: Vec<AnnualTermSource>,
) -> Result<AnnualResultPreview, AppError> {
    let source_totals: Vec<CourseCreditTotals> = terms
        .iter()
        .filter_map(|term| {
            term.revision
                .as_ref()
                .map(|row| row.snapshot.results.totals.clone())
        })
        .collect();
    let mut totals = annual_calculation::sum_annual_credit_totals(&source_totals)?;
    let sources_complete = !terms.is_empty()
        && terms
            .iter()
            .all(|term| term.is_current && term.revision.is_some());
    if !sources_complete {
        totals.coverage_complete = false;
        totals.all_outcomes_numeric = false;
    }
    let needs_hold = terms.iter().any(|term| {
        term.revision
            .as_ref()
            .is_some_and(|row| row.hold_reason.is_some())
    });
    let can_lock = sources_complete && totals.coverage_complete;
    let source_checksum = hash(&(year, student, &terms, &totals))?;
    Ok(AnnualResultPreview {
        academic_year_id: year,
        student_academic_year_id: student,
        terms,
        totals,
        can_lock,
        needs_hold,
        source_checksum,
    })
}
