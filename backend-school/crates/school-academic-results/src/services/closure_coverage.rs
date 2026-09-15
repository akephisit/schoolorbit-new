use super::*;
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(sqlx::FromRow)]
struct LatestRevision {
    student_academic_year_id: Uuid,
    id: Uuid,
    revision: i64,
    policy_id: Uuid,
    source_checksum: String,
    hold_reason: Option<String>,
}

pub async fn term_closure_coverage_for_students(
    tx: &mut Transaction<'_, Postgres>,
    context: &ResultContext,
    students: &[Uuid],
) -> Result<TermClosureCoverage, AppError> {
    coverage_in_transaction(tx, context, Some(students)).await
}

/// The lifecycle caller owns school authorization and snapshot isolation.
pub async fn term_closure_coverage(
    tx: &mut Transaction<'_, Postgres>,
    context: &ResultContext,
) -> Result<TermClosureCoverage, AppError> {
    coverage_in_transaction(tx, context, None).await
}

async fn coverage_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    context: &ResultContext,
    selected: Option<&[Uuid]>,
) -> Result<TermClosureCoverage, AppError> {
    validate_context(tx, context).await?;
    // Regular/custom terms include the year cohort so unconfigured students do
    // not disappear. Summer/remedial terms cover their actual participants.
    // Retained results/revisions preserve coverage after withdrawal.
    let mut expected: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM student_academic_years WHERE academic_year_id=$1 AND status IN ('active','completed')
         AND EXISTS(SELECT 1 FROM academic_terms WHERE id=$2 AND academic_year_id=$1 AND term_type NOT IN ('summer','remedial'))
         UNION SELECT student_academic_year_id FROM learning_group_students WHERE academic_year_id=$1 AND academic_term_id=$2 AND membership_status='active'
         UNION SELECT student_academic_year_id FROM academic_course_results WHERE academic_year_id=$1 AND academic_term_id=$2
         UNION SELECT student_academic_year_id FROM academic_activity_results WHERE academic_year_id=$1 AND academic_term_id=$2
         UNION SELECT student_academic_year_id FROM academic_term_aggregate_revisions WHERE academic_year_id=$1 AND academic_term_id=$2
         ORDER BY 1 LIMIT 10001",
    ).bind(context.academic_year_id).bind(context.academic_term_id).fetch_all(&mut **tx).await?;
    if expected.len() > 10000 {
        return Err(AppError::ValidationError(
            "ผลสรุปภาคเรียนเกินขอบเขต 10000 คน".into(),
        ));
    }
    if let Some(selected) = selected {
        let selected: std::collections::BTreeSet<Uuid> = selected.iter().copied().collect();
        expected.retain(|id| selected.contains(id));
    }
    let revisions: Vec<LatestRevision> = sqlx::query_as(
        "SELECT DISTINCT ON (student_academic_year_id) student_academic_year_id,id,revision,policy_id,source_checksum,hold_reason
         FROM academic_term_aggregate_revisions WHERE academic_year_id=$1 AND academic_term_id=$2
           AND student_academic_year_id=ANY($3)
         ORDER BY student_academic_year_id,revision DESC",
    ).bind(context.academic_year_id).bind(context.academic_term_id).bind(&expected).fetch_all(&mut **tx).await?;
    let mut policies: BTreeMap<Uuid, Vec<Uuid>> = BTreeMap::new();
    let mut latest = BTreeMap::new();
    for revision in revisions {
        policies
            .entry(revision.policy_id)
            .or_default()
            .push(revision.student_academic_year_id);
        latest.insert(revision.student_academic_year_id, revision);
    }
    let mut current = BTreeMap::new();
    for (policy_id, students) in policies {
        let query = AggregatePreviewQuery {
            academic_year_id: context.academic_year_id,
            academic_term_id: context.academic_term_id,
            policy_id,
        };
        for batch in students.chunks(500) {
            current.extend(
                aggregate_preview::aggregate_students_in_transaction(tx, batch, &query).await?,
            );
        }
    }
    let mut students = Vec::with_capacity(expected.len());
    for student in expected {
        let revision = latest.remove(&student);
        let preview = current.remove(&student);
        let is_current = match (&revision, &preview) {
            (Some(revision), Some(preview)) => {
                preview.can_lock
                    && revision.source_checksum == preview.source_checksum
                    && preview.hold_findings.is_empty() == revision.hold_reason.is_none()
            }
            _ => false,
        };
        students.push(TermClosureStudent {
            student_academic_year_id: student,
            revision_id: revision.as_ref().map(|row| row.id),
            revision: revision.as_ref().map(|row| row.revision),
            policy_id: revision.as_ref().map(|row| row.policy_id),
            is_current,
            blockers: preview
                .as_ref()
                .map(|row| row.blockers.clone())
                .unwrap_or_default(),
            hold_reason: revision.and_then(|row| row.hold_reason),
            current_source_checksum: preview.map(|row| row.source_checksum),
        });
    }
    let ready = !students.is_empty() && students.iter().all(|student| student.is_current);
    let source_checksum = hash(&(context, &students))?;
    Ok(TermClosureCoverage {
        students,
        ready,
        source_checksum,
    })
}
