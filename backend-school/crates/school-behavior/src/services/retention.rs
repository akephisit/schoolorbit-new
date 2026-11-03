//! Lifecycle-scheduled detail retention reuses Behavior's reconciliation and chunk jobs.
#[cfg(test)]
#[path = "retention_tests.rs"]
mod tests;
use super::{cleanup, shared};
use crate::models::*;
use school_database::DbSession;
use school_errors::AppError;
use sqlx::{types::Json, PgConnection};
use uuid::Uuid;

pub async fn entry_count(connection: &mut PgConnection, term: Uuid) -> Result<i64, AppError> {
    Ok(DbSession::new(connection)
        .scalar("behavior.retention.entry_count", "SELECT count(*) FROM behavior_entries e JOIN behavior_daily_totals t ON t.id=e.total_id WHERE t.academic_term_id=$1")
        .bind(term).fetch_one().await?)
}

/// Caller holds Attendance module, academic transition and term locks first.
/// The request ID belongs to the durable Lifecycle job and replays its original result.
pub async fn retain_in_transaction(
    connection: &mut PgConnection,
    actor: Uuid,
    request: Uuid,
    term: Uuid,
) -> Result<BehaviorCleanupOutcome, AppError> {
    shared::raw_lock(connection).await?;
    let existing: Option<(Uuid, Uuid, Json<BehaviorCleanupOutcome>)> = DbSession::new(connection)
        .query_as("behavior.retention.replay", "SELECT actor_id,academic_term_id,outcome FROM behavior_retention_receipts WHERE request_id=$1")
        .bind(request).fetch_optional().await?;
    if let Some((original_actor, original_term, Json(outcome))) = existing {
        if original_actor != actor || original_term != term {
            return Err(AppError::Conflict("คำขอเก็บสรุปคะแนนถูกใช้กับข้อมูลอื่นแล้ว".into()));
        }
        return if outcome.status == "pending" {
            cleanup::job_on_connection(connection, outcome.id).await
        } else {
            Ok(outcome)
        };
    }
    shared::lock(connection).await?;
    let scope = BehaviorCleanupScope {
        start: None,
        end: None,
        academic_year_id: None,
        academic_term_id: Some(term),
        sides: vec![BehaviorSide::Merit, BehaviorSide::Deduction],
        sources: vec![
            BehaviorSource::Manual,
            BehaviorSource::Arrival,
            BehaviorSource::Lesson,
            BehaviorSource::Special,
        ],
    };
    let snapshot = cleanup::snapshot(connection, &scope, false).await?;
    let outcome = cleanup::apply(
        connection,
        Some(actor),
        &scope,
        BehaviorCleanupMode::DetailsOnly,
        snapshot,
        "เก็บยอดสรุปตามนโยบายอายุรายละเอียดของโรงเรียน",
        false,
    )
    .await?;
    DbSession::new(connection)
        .query("behavior.retention.accept", "INSERT INTO behavior_retention_receipts(request_id,academic_term_id,actor_id,outcome) VALUES($1,$2,$3,$4)")
        .bind(request).bind(term).bind(actor).bind(Json(&outcome)).execute().await?;
    Ok(outcome)
}
