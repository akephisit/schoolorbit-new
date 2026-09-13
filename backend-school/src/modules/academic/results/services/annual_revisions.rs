use super::aggregate_revisions::write_error;
use super::*;
use crate::middleware::permission::ActorContext;
use crate::policies::academic_aggregate_access_policy::{
    require_aggregate_lock, require_aggregate_read,
};
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

pub(crate) async fn annual_closure_coverage(
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

pub(crate) async fn require_term_without_annual_results(
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
    crate::policies::academic_aggregate_access_policy::require_aggregate_read(actor)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        modules::academic::{
            cutover_test_support::apply_migrations_through, results::services_tests::fixture,
        },
        permissions::registry::codes,
    };

    #[tokio::test]
    async fn annual_revision_preserves_zero_replays_and_invalidates_after_source_correction() {
        let (pool, actor, context, student) =
            crate::modules::academic::results::aggregate_revision_tests::ready_aggregate_fixture(
                "annual_revision_history",
            )
            .await;
        apply_migrations_through(&pool, 69).await.unwrap();
        sqlx::query(
            "UPDATE academic_terms SET included_in_year_result=(id=$2) WHERE academic_year_id=$1",
        )
        .bind(context.academic_year_id)
        .bind(context.academic_term_id)
        .execute(&pool)
        .await
        .unwrap();
        let policy = create_aggregate_policy(
            &pool,
            &actor,
            AggregatePolicyInput {
                name: "Reviewed annual source".into(),
                passing_grade: "1".into(),
                minimum_learner_level: 1,
                allow_reviewed_holds: false,
            },
        )
        .await
        .unwrap();
        let query = AggregatePreviewQuery {
            academic_year_id: context.academic_year_id,
            academic_term_id: context.academic_term_id,
            policy_id: policy.id,
        };
        let term_preview = preview_aggregate(&pool, &actor, student, &query)
            .await
            .unwrap();
        let term = lock_term_aggregate(
            &pool,
            &actor,
            &context,
            student,
            AggregateLockInput {
                policy_id: policy.id,
                source_checksum: term_preview.source_checksum.clone(),
                expected_revision: None,
                request_id: Uuid::new_v4(),
                hold_reason: None,
            },
        )
        .await
        .unwrap();
        let preview = preview_annual(&pool, &actor, context.academic_year_id, student)
            .await
            .unwrap();
        assert!(preview.can_lock && !preview.needs_hold);
        assert_eq!(preview.terms.len(), 1);
        assert_eq!(preview.terms[0].revision.as_ref().unwrap().id, term.id);
        assert_eq!(preview.totals.provisional_gpa.as_deref(), Some("0.00"));
        let input = AnnualLockInput {
            expected_revision: None,
            source_checksum: preview.source_checksum,
            request_id: Uuid::new_v4(),
            hold_reason: None,
        };
        let first = lock_annual(
            &pool,
            &actor,
            context.academic_year_id,
            student,
            input.clone(),
        )
        .await
        .unwrap();
        assert_eq!(first.revision, 1);
        assert_eq!(first.official_gpa.as_deref(), Some("0.00"));
        assert!(first.is_current);
        let mut dependency_tx = pool.begin().await.unwrap();
        assert!(matches!(
            require_term_without_annual_results(
                &mut dependency_tx,
                context.academic_year_id,
                context.academic_term_id
            )
            .await,
            Err(AppError::Conflict(_))
        ));
        assert!(require_term_without_annual_results(
            &mut dependency_tx,
            context.academic_year_id,
            Uuid::new_v4()
        )
        .await
        .is_ok());
        dependency_tx.rollback().await.unwrap();
        let retry = lock_annual(
            &pool,
            &actor,
            context.academic_year_id,
            student,
            input.clone(),
        )
        .await
        .unwrap();
        assert_eq!(retry.id, first.id);
        let count:i64 = sqlx::query_scalar("SELECT count(*) FROM academic_annual_result_term_sources WHERE annual_revision_id=$1 AND term_aggregate_revision_id=$2").bind(first.id).bind(term.id).fetch_one(&pool).await.unwrap();
        assert_eq!(count, 1);
        let mut changed = input.clone();
        changed.expected_revision = Some(1);
        assert!(matches!(
            lock_annual(&pool, &actor, context.academic_year_id, student, changed).await,
            Err(AppError::Conflict(_))
        ));
        let reader = ActorContext {
            user_id: actor.user_id,
            permissions: vec![
                codes::ACADEMIC_RESULT_READ_SCHOOL.into(),
                codes::ACADEMIC_LEARNER_EVALUATION_READ_SCHOOL.into(),
            ],
        };
        assert!(matches!(
            lock_annual(&pool, &reader, context.academic_year_id, student, input).await,
            Err(AppError::Forbidden(_))
        ));
        assert!(
            sqlx::query("DELETE FROM academic_annual_result_revisions WHERE id=$1")
                .bind(first.id)
                .execute(&pool)
                .await
                .is_err()
        );
        assert!(sqlx::query(
            "DELETE FROM academic_annual_result_term_sources WHERE annual_revision_id=$1"
        )
        .bind(first.id)
        .execute(&pool)
        .await
        .is_err());
        let course = &term_preview.results.courses[0];
        correct_result(
            &pool,
            &actor,
            &context,
            ResultCorrectionInput::Course {
                course_result_id: course.result_id.unwrap(),
                outcome: CourseOfficialOutcome::Numeric,
                numeric_grade: Some("4".into()),
                expected_effective_version: 1,
            },
        )
        .await
        .unwrap();
        let stale = preview_annual(&pool, &reader, context.academic_year_id, student)
            .await
            .unwrap();
        assert!(!stale.can_lock && !stale.terms[0].is_current);
        let history = list_annual_revisions(&pool, &reader, context.academic_year_id, student)
            .await
            .unwrap();
        assert_eq!(history.len(), 1);
        assert!(!history[0].is_current);
        assert_eq!(history[0].official_gpa.as_deref(), Some("0.00"));
        correct_result(
            &pool,
            &actor,
            &context,
            ResultCorrectionInput::Course {
                course_result_id: course.result_id.unwrap(),
                outcome: CourseOfficialOutcome::Incomplete,
                numeric_grade: None,
                expected_effective_version: 2,
            },
        )
        .await
        .unwrap();
        let held_policy = create_aggregate_policy(
            &pool,
            &actor,
            AggregatePolicyInput {
                name: "Reviewed holds".into(),
                passing_grade: "1".into(),
                minimum_learner_level: 1,
                allow_reviewed_holds: true,
            },
        )
        .await
        .unwrap();
        let held_preview = preview_aggregate(
            &pool,
            &actor,
            student,
            &AggregatePreviewQuery {
                academic_year_id: context.academic_year_id,
                academic_term_id: context.academic_term_id,
                policy_id: held_policy.id,
            },
        )
        .await
        .unwrap();
        assert!(held_preview.can_lock && !held_preview.hold_findings.is_empty());
        lock_term_aggregate(
            &pool,
            &actor,
            &context,
            student,
            AggregateLockInput {
                policy_id: held_policy.id,
                source_checksum: held_preview.source_checksum,
                expected_revision: Some(1),
                request_id: Uuid::new_v4(),
                hold_reason: Some("Reviewed incomplete source".into()),
            },
        )
        .await
        .unwrap();
        let annual = preview_annual(&pool, &actor, context.academic_year_id, student)
            .await
            .unwrap();
        assert!(annual.can_lock && annual.needs_hold);
        assert_eq!(annual.totals.exceptional_result_count, 1);
        let mut held_input = AnnualLockInput {
            expected_revision: Some(1),
            source_checksum: annual.source_checksum,
            request_id: Uuid::new_v4(),
            hold_reason: None,
        };
        assert!(matches!(
            lock_annual(
                &pool,
                &actor,
                context.academic_year_id,
                student,
                held_input.clone()
            )
            .await,
            Err(AppError::ValidationError(_))
        ));
        held_input.hold_reason = Some("Reviewed annual hold".into());
        let held = lock_annual(&pool, &actor, context.academic_year_id, student, held_input)
            .await
            .unwrap();
        assert_eq!(held.revision, 2);
        assert!(held.official_gpa.is_none() && held.is_current);
        let history = list_annual_revisions(&pool, &reader, context.academic_year_id, student)
            .await
            .unwrap();
        assert_eq!(history.len(), 2);
        assert!(history[0].is_current && !history[1].is_current);
        assert_eq!(history[1].official_gpa.as_deref(), Some("0.00"));
        let mut coverage_tx = pool.begin().await.unwrap();
        let coverage = annual_closure_coverage(&mut coverage_tx, context.academic_year_id)
            .await
            .unwrap();
        let covered = coverage
            .students
            .iter()
            .find(|row| row.student_academic_year_id == student)
            .unwrap();
        assert_eq!(covered.revision_id, Some(held.id));
        assert!(covered.is_current && covered.hold_reason.is_some());
        coverage_tx.rollback().await.unwrap();
        // The public Core command must enforce the owner guard too, not only
        // its direct helper. Other terms are still unstarted in this fixture.
        sqlx::query("UPDATE academic_terms SET status=CASE WHEN id=$2 THEN 'closed' ELSE 'cancelled' END,closed_on=CASE WHEN id=$2 THEN start_date ELSE NULL END WHERE academic_year_id=$1")
            .bind(context.academic_year_id).bind(context.academic_term_id).execute(&pool).await.unwrap();
        let mut lifecycle_actor = actor.clone();
        lifecycle_actor.permissions.extend([
            codes::ACADEMIC_LIFECYCLE_READ_SCHOOL.into(),
            codes::ACADEMIC_LIFECYCLE_REOPEN_SCHOOL.into(),
        ]);
        let workspace = crate::modules::academic::lifecycle::services::get_workspace(
            &pool,
            &lifecycle_actor,
            context.academic_year_id,
            context.academic_term_id,
        )
        .await
        .unwrap();
        let reopen = crate::modules::academic::core::models::TermTransitionRequest {
            academic_year_id: context.academic_year_id,
            request_id: Uuid::new_v4(),
            action: crate::modules::academic::core::models::TermTransitionAction::Reopen,
            expected_year_version: workspace.context.year_row_version,
            expected_term_version: workspace.context.term_row_version,
            readiness_checksum: workspace.source_checksum,
            acknowledged_warning_codes: vec![],
            closed_on: None,
            reason: Some("Requested source review".into()),
        };
        assert!(matches!(
            crate::modules::academic::core::services::term_transitions::transition_term(
                &pool,
                &lifecycle_actor,
                context.academic_term_id,
                reopen
            )
            .await,
            Err(AppError::Conflict(_))
        ));
    }

    #[tokio::test]
    async fn annual_preview_requires_current_applicable_term_revisions_and_exact_student_context() {
        let (pool, actor, context, group) = fixture("annual_preview_missing").await;
        apply_migrations_through(&pool, 69).await.unwrap();
        let student:Uuid=sqlx::query_scalar("SELECT student_academic_year_id FROM learning_group_students WHERE learning_group_id=$1 AND membership_status='active' ORDER BY student_academic_year_id LIMIT 1").bind(group).fetch_one(&pool).await.unwrap();
        let office = ActorContext {
            user_id: actor.user_id,
            permissions: vec![
                codes::ACADEMIC_RESULT_READ_SCHOOL.into(),
                codes::ACADEMIC_LEARNER_EVALUATION_READ_SCHOOL.into(),
            ],
        };
        let preview = preview_annual(&pool, &office, context.academic_year_id, student)
            .await
            .unwrap();
        assert!(!preview.terms.is_empty());
        assert!(!preview.can_lock);
        assert!(preview
            .terms
            .iter()
            .any(|term| term.academic_term_id == context.academic_term_id
                && term.revision.is_none()
                && !term.is_current));
        assert!(preview.totals.provisional_gpa.is_none());
        assert!(!preview.totals.coverage_complete);
        let mut coverage_tx = pool.begin().await.unwrap();
        let coverage = annual_closure_coverage(&mut coverage_tx, context.academic_year_id)
            .await
            .unwrap();
        assert!(!coverage.ready);
        assert!(coverage
            .students
            .iter()
            .any(|row| row.student_academic_year_id == student
                && row.revision_id.is_none()
                && !row.is_current));
        assert!(annual_closure_coverage(&mut coverage_tx, Uuid::new_v4())
            .await
            .is_err());
        coverage_tx.rollback().await.unwrap();
        assert!(preview_annual(&pool, &office, Uuid::new_v4(), student)
            .await
            .is_err());
        assert!(
            preview_annual(&pool, &office, context.academic_year_id, Uuid::new_v4())
                .await
                .is_err()
        );
        let teacher = ActorContext {
            user_id: actor.user_id,
            permissions: vec![
                codes::ACADEMIC_RESULT_READ_ASSIGNED.into(),
                codes::ACADEMIC_LEARNER_EVALUATION_READ_ASSIGNED.into(),
            ],
        };
        assert!(matches!(
            preview_annual(&pool, &teacher, context.academic_year_id, student).await,
            Err(AppError::Forbidden(_))
        ));
        // Only applicable included, non-cancelled terms can become annual sources.
        sqlx::query(
            "UPDATE academic_terms SET included_in_year_result=false WHERE academic_year_id=$1",
        )
        .bind(context.academic_year_id)
        .execute(&pool)
        .await
        .unwrap();
        let excluded = preview_annual(&pool, &office, context.academic_year_id, student)
            .await
            .unwrap();
        assert!(excluded.terms.is_empty() && !excluded.can_lock);
    }

    #[tokio::test]
    async fn annual_preview_ignores_nonparticipant_summer_but_requires_participant_results() {
        let (pool, actor, context, group) = fixture("annual_preview_summer").await;
        apply_migrations_through(&pool, 69).await.unwrap();
        let student:Uuid=sqlx::query_scalar("SELECT student_academic_year_id FROM learning_group_students WHERE learning_group_id=$1 AND membership_status='active' LIMIT 1").bind(group).fetch_one(&pool).await.unwrap();
        let office = ActorContext {
            user_id: actor.user_id,
            permissions: vec![
                codes::ACADEMIC_RESULT_READ_SCHOOL.into(),
                codes::ACADEMIC_LEARNER_EVALUATION_READ_SCHOOL.into(),
            ],
        };
        sqlx::query("UPDATE academic_terms SET included_in_year_result=(id=$2),term_type='summer' WHERE academic_year_id=$1").bind(context.academic_year_id).bind(context.academic_term_id).execute(&pool).await.unwrap();
        let empty:Uuid=sqlx::query_scalar("INSERT INTO academic_terms(academic_year_id,sequence_no,code,name,term_type,start_date,bell_schedule_id,status,included_in_year_result,blocks_year_closure) SELECT academic_year_id,(SELECT max(sequence_no)+1 FROM academic_terms WHERE academic_year_id=$1),'empty-summer','Empty summer','summer',start_date,bell_schedule_id,'planning',true,true FROM academic_terms WHERE id=$2 RETURNING id").bind(context.academic_year_id).bind(context.academic_term_id).fetch_one(&pool).await.unwrap();
        let preview = preview_annual(&pool, &office, context.academic_year_id, student)
            .await
            .unwrap();
        assert_eq!(preview.terms.len(), 1);
        assert_eq!(preview.terms[0].academic_term_id, context.academic_term_id);
        assert!(preview
            .terms
            .iter()
            .all(|term| term.academic_term_id != empty));
        assert!(!preview.can_lock);
        sqlx::query("UPDATE academic_terms SET status='cancelled' WHERE id=$1")
            .bind(context.academic_term_id)
            .execute(&pool)
            .await
            .unwrap();
        let cancelled = preview_annual(&pool, &office, context.academic_year_id, student)
            .await
            .unwrap();
        assert!(cancelled.terms.is_empty() && !cancelled.can_lock);
    }
}
