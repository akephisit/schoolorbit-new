use super::{aggregate_preview::aggregate_in_transaction, *};
use crate::{
    middleware::permission::ActorContext,
    policies::academic_aggregate_access_policy::{require_aggregate_lock, require_aggregate_read},
};
use chrono::{DateTime, Utc};
use sqlx::{types::Json, PgPool};
use uuid::Uuid;

#[derive(sqlx::FromRow)]
struct RevisionRow {
    id: Uuid,
    revision: i64,
    snapshot: Json<TermAggregatePreview>,
    official_gpa: Option<String>,
    hold_reason: Option<String>,
    locked_by: Uuid,
    locked_at: DateTime<Utc>,
    request_checksum: String,
}

impl RevisionRow {
    fn wire(self, is_current: bool) -> TermAggregateRevision {
        TermAggregateRevision {
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

fn hold_reason(input: &AggregateLockInput, required: bool) -> Result<Option<String>, AppError> {
    let reason = input
        .hold_reason
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if required != reason.is_some() || reason.is_some_and(|value| value.chars().count() > 1000) {
        return Err(AppError::ValidationError(
            "ระบุเหตุผลไม่เกิน 1000 ตัวอักษรเฉพาะเมื่อมีผลที่ต้องพิจารณาค้างไว้".into(),
        ));
    }
    Ok(reason.map(str::to_owned))
}

fn write_error(error: sqlx::Error) -> AppError {
    if error
        .as_database_error()
        .and_then(|error| error.code())
        .is_some_and(|code| code == "40001" || code == "23505" || code == "40P01")
    {
        conflict()
    } else {
        error.into()
    }
}

pub async fn lock_term_aggregate(
    pool: &PgPool,
    actor: &ActorContext,
    context: &ResultContext,
    student: Uuid,
    input: AggregateLockInput,
) -> Result<TermAggregateRevision, AppError> {
    require_aggregate_lock(actor)?;
    if input.expected_revision.is_some_and(|value| value < 1) || input.source_checksum.len() != 64 {
        return Err(AppError::ValidationError(
            "Invalid aggregate revision or source checksum".into(),
        ));
    }
    let request_checksum = hash(&(actor.user_id, context, student, &input))?;
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL SERIALIZABLE")
        .execute(&mut *tx)
        .await?;
    // Concurrent revision writers conflict rather than accepting two snapshots of the
    // same expected revision. Source corrections remain append-only and detectable.
    let previous: Option<RevisionRow> = sqlx::query_as("SELECT id,revision,snapshot,official_gpa::text,hold_reason,locked_by,locked_at,request_checksum FROM academic_term_aggregate_revisions WHERE request_id=$1").bind(input.request_id).fetch_optional(&mut *tx).await?;
    if let Some(previous) = previous {
        if previous.request_checksum != request_checksum {
            return Err(conflict());
        }
        let current = aggregate_in_transaction(
            &mut tx,
            student,
            &AggregatePreviewQuery {
                academic_year_id: context.academic_year_id,
                academic_term_id: context.academic_term_id,
                policy_id: input.policy_id,
            },
        )
        .await?;
        let latest: i64 = sqlx::query_scalar("SELECT max(revision) FROM academic_term_aggregate_revisions WHERE student_academic_year_id=$1 AND academic_term_id=$2").bind(student).bind(context.academic_term_id).fetch_one(&mut *tx).await?;
        let is_current = previous.revision == latest
            && previous.snapshot.source_checksum == current.source_checksum;
        tx.commit().await.map_err(write_error)?;
        return Ok(previous.wire(is_current));
    }
    let latest: Option<i64> = sqlx::query_scalar("SELECT max(revision) FROM academic_term_aggregate_revisions WHERE student_academic_year_id=$1 AND academic_term_id=$2").bind(student).bind(context.academic_term_id).fetch_one(&mut *tx).await?;
    check_version(input.expected_revision, latest)?;
    let preview = aggregate_in_transaction(
        &mut tx,
        student,
        &AggregatePreviewQuery {
            academic_year_id: context.academic_year_id,
            academic_term_id: context.academic_term_id,
            policy_id: input.policy_id,
        },
    )
    .await?;
    if !preview.can_lock || preview.source_checksum != input.source_checksum {
        return Err(conflict());
    }
    let hold = hold_reason(&input, !preview.hold_findings.is_empty())?;
    let official_gpa = if hold.is_none() {
        preview
            .results
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
        .ok_or_else(|| AppError::ValidationError("Aggregate revision limit exceeded".into()))?;
    let row: RevisionRow = sqlx::query_as("INSERT INTO academic_term_aggregate_revisions (student_academic_year_id,academic_year_id,academic_term_id,revision,policy_id,source_checksum,request_id,request_checksum,snapshot,official_gpa,hold_reason,locked_by) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12) RETURNING id,revision,snapshot,official_gpa::text,hold_reason,locked_by,locked_at,request_checksum")
        .bind(student).bind(context.academic_year_id).bind(context.academic_term_id).bind(revision).bind(input.policy_id).bind(&preview.source_checksum).bind(input.request_id).bind(request_checksum).bind(Json(&preview)).bind(official_gpa).bind(hold).bind(actor.user_id).fetch_one(&mut *tx).await.map_err(write_error)?;
    tx.commit().await.map_err(write_error)?;
    Ok(row.wire(true))
}

pub async fn list_term_aggregate_revisions(
    pool: &PgPool,
    actor: &ActorContext,
    context: &ResultContext,
    student: Uuid,
) -> Result<Vec<TermAggregateRevision>, AppError> {
    require_aggregate_read(actor)?;
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    validate_context(&mut tx, context).await?;
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM student_academic_years WHERE id=$1 AND academic_year_id=$2)",
    )
    .bind(student)
    .bind(context.academic_year_id)
    .fetch_one(&mut *tx)
    .await?;
    if !exists {
        return Err(AppError::NotFound(
            "Student academic year not found in selected context".into(),
        ));
    }
    let rows: Vec<RevisionRow> = sqlx::query_as("SELECT id,revision,snapshot,official_gpa::text,hold_reason,locked_by,locked_at,request_checksum FROM academic_term_aggregate_revisions WHERE student_academic_year_id=$1 AND academic_term_id=$2 AND academic_year_id=$3 ORDER BY revision DESC LIMIT 101").bind(student).bind(context.academic_term_id).bind(context.academic_year_id).fetch_all(&mut *tx).await?;
    if rows.len() > 100 {
        return Err(AppError::ValidationError(
            "Aggregate history exceeds 100 revisions".into(),
        ));
    }
    let mut history = Vec::with_capacity(rows.len());
    for (index, row) in rows.into_iter().enumerate() {
        let is_current = if index == 0 {
            let current = aggregate_in_transaction(
                &mut tx,
                student,
                &AggregatePreviewQuery {
                    academic_year_id: context.academic_year_id,
                    academic_term_id: context.academic_term_id,
                    policy_id: row.snapshot.policy.id,
                },
            )
            .await?;
            current.source_checksum == row.snapshot.source_checksum
        } else {
            false
        };
        history.push(row.wire(is_current));
    }
    tx.commit().await?;
    Ok(history)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hold_reason_cannot_hide_missing_review_or_be_added_to_complete_results() {
        let mut input = AggregateLockInput {
            policy_id: Uuid::new_v4(),
            source_checksum: "a".repeat(64),
            expected_revision: None,
            request_id: Uuid::new_v4(),
            hold_reason: None,
        };
        assert!(hold_reason(&input, true).is_err());
        input.hold_reason = Some("  Reviewed incomplete outcome  ".into());
        assert_eq!(
            hold_reason(&input, true).unwrap().as_deref(),
            Some("Reviewed incomplete outcome")
        );
        assert!(hold_reason(&input, false).is_err());
    }
}
