//! Closure obligations come from accepted dated promises, independently of
//! whether Attendance has materialized the corresponding day or session.
use chrono::NaiveDateTime;
use school_academic_core::ports::PendingTermWork;
use school_database::DbSession;
use school_errors::AppError;
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::{types::Json, PgConnection};
use uuid::Uuid;

/// Reads live teaching promises in the caller's snapshot and lifecycle locks.
/// `None` includes every explicitly owned term in the selected year, including
/// optional terms which have not started. A weekly board alone is never work.
pub async fn pending_on_connection(
    connection: &mut PgConnection,
    year: Uuid,
    term: Option<Uuid>,
) -> Result<Vec<PendingTermWork>, AppError> {
    // Read acceptance wall time after transition locks, not transaction-start
    // time. One cutoff keeps all bounded pages on the same interval boundary.
    let cutoff: NaiveDateTime = DbSession::new(connection)
        .scalar(
            "academic_timetable.dated_obligations.cutoff",
            "SELECT clock_timestamp() AT TIME ZONE 'Asia/Bangkok'",
        )
        .fetch_one()
        .await?;
    let mut cursor: Option<Uuid> = None;
    let mut pending = Vec::new();
    loop {
        let rows: Vec<(Uuid, Json<Value>)> = DbSession::new(connection)
            .query_as(
                "academic_timetable.dated_obligations.pending",
                "SELECT duty.id,to_jsonb(duty) FROM academic_teaching_coverage_duties duty
                 WHERE duty.academic_year_id=$1
                   AND ($2::uuid IS NULL OR duty.academic_term_id=$2)
                   AND duty.state IN ('reserved','approved') AND duty.kind<>'absence'
                   AND duty.occurrence_date >= $3::timestamp::date
                   AND duty.occurrence_date+duty.end_time > $3
                   AND ($4::uuid IS NULL OR duty.id>$4)
                 ORDER BY duty.id LIMIT 256",
            )
            .bind(year)
            .bind(term)
            .bind(cutoff)
            .bind(cursor)
            .fetch_all()
            .await?;
        let page_len = rows.len();
        for (id, Json(source)) in rows {
            cursor = Some(id);
            pending.push(PendingTermWork {
                id,
                revision: source_revision(&source)?,
                blocks_closure: true,
            });
        }
        if page_len < 256 {
            return Ok(pending);
        }
    }
}

fn source_revision(source: &Value) -> Result<String, AppError> {
    // Bind the exact duty, plan revision, state, interval and immutable source /
    // taught-content snapshots; replacing one promise cannot preserve a preview
    // merely because its count and parent request are unchanged.
    let bytes = serde_json::to_vec(source)
        .map_err(|_| AppError::InternalServerError("ไม่สามารถสร้างรุ่นภาระสอนรายวันได้".into()))?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn obligation_revision_binds_exact_source_and_live_state() {
        let source = json!({"id": "duty-a", "request_id": "leave-a", "plan_revision": 1,
            "state": "reserved", "occurrence_date": "2026-11-20", "end_time": "10:00",
            "source_snapshot": {"sourceRevision": 1}, "teaching_content": {"subjectId": "math"}});
        let original = source_revision(&source).unwrap();
        assert_eq!(original, source_revision(&source).unwrap());
        for (key, value) in [
            ("id", json!("duty-b")),
            ("request_id", json!("leave-b")),
            ("plan_revision", json!(2)),
            ("state", json!("approved")),
            ("occurrence_date", json!("2026-11-21")),
            ("end_time", json!("11:00")),
            ("source_snapshot", json!({"sourceRevision": 2})),
            ("teaching_content", json!({"subjectId": "english"})),
        ] {
            let mut changed = source.clone();
            changed[key] = value;
            assert_ne!(original, source_revision(&changed).unwrap(), "{key}");
        }
    }
}
