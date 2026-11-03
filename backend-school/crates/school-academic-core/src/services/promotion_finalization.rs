use crate::models::PromotionDestinationOutcome;
use chrono::NaiveDate;
use school_database::DbSession;
use school_errors::AppError;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

/// Finish the source history after a closed-year manual decision. The caller
/// retains the tenant transition lock, audit and immutable confirmation receipt.
pub async fn finish_source(
    tx: &mut Transaction<'_, Postgres>,
    actor: Uuid,
    source_year: Uuid,
    source_student_year: Uuid,
    effective_on: NaiveDate,
    outcome: PromotionDestinationOutcome,
) -> Result<i64, AppError> {
    let closed: bool = DbSession::new(&mut **tx)
        .scalar(
            "academic_core.promotion_finalization.finish_source",
            "SELECT EXISTS(SELECT 1 FROM academic_years WHERE id=$1 AND status='closed')",
        )
        .bind(source_year)
        .fetch_one()
        .await?;
    if !closed {
        return Err(AppError::Conflict(
            "ต้องปิดปีต้นทางก่อนยืนยันรายชื่อนักเรียนปีใหม่".into(),
        ));
    }
    let status = final_status(outcome);
    if let Some(status) = status {
        let invalid: bool = DbSession::new(&mut **tx).scalar(
            "academic_core.promotion_finalization.finish_source",
            "SELECT EXISTS(SELECT 1 FROM homeroom_placements WHERE student_academic_year_id=$1 AND status='current' AND start_date>$2)",
        ).bind(source_student_year).bind(effective_on).fetch_one().await?;
        if invalid {
            return Err(AppError::Conflict("วันสิ้นสุดต้องไม่อยู่ก่อนวันเริ่มห้องปัจจุบัน".into()));
        }
        DbSession::new(&mut **tx).query(
            "academic_core.promotion_finalization.finish_source",
            "UPDATE homeroom_placements SET status=CASE WHEN start_date>$2 THEN 'removed' ELSE 'ended' END,
             end_date=CASE WHEN start_date>$2 THEN end_date ELSE $2 END,
             row_version=row_version+1,updated_at=now()
             WHERE student_academic_year_id=$1 AND status IN ('current','planned')",
        ).bind(source_student_year).bind(effective_on).execute().await?;
        DbSession::new(&mut **tx).query(
            "academic_core.promotion_finalization.finish_source",
            "UPDATE student_academic_years SET status=$2,row_version=row_version+1,updated_at=now()
             WHERE id=$1 AND academic_year_id=$3 AND status IS DISTINCT FROM $2",
        ).bind(source_student_year).bind(status).bind(source_year).execute().await?;
        super::years_terms::append_audit(
            tx,
            "student_academic_year.rollover_finalized",
            "student_academic_year",
            source_student_year,
            Some(source_year),
            None,
            actor,
            serde_json::json!({"outcome":outcome,"effectiveOn":effective_on}),
        )
        .await?;
    }
    DbSession::new(&mut **tx)
        .scalar(
            "academic_core.promotion_finalization.finish_source",
            "SELECT row_version FROM student_academic_years WHERE id=$1 AND academic_year_id=$2",
        )
        .bind(source_student_year)
        .bind(source_year)
        .fetch_one()
        .await
        .map_err(Into::into)
}

fn final_status(outcome: PromotionDestinationOutcome) -> Option<&'static str> {
    match outcome {
        PromotionDestinationOutcome::Hold => None,
        PromotionDestinationOutcome::Graduate => Some("graduated"),
        PromotionDestinationOutcome::TransferOut => Some("withdrawn"),
        _ => Some("completed"),
    }
}

/// The frozen identities supplied by Lifecycle's existing accepted receipts.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PreparedPromotionTarget {
    pub student_id: Uuid,
    pub target_student_year_id: Uuid,
    pub target_placement_id: Option<Uuid>,
    pub starts_on: NaiveDate,
    pub grade_level_id: Option<Uuid>,
    pub study_program_id: Option<Uuid>,
    pub homeroom_id: Option<Uuid>,
}

/// Verify exact prepared annual/room identities without N+1 reads or row locks.
/// Caller supplies a stable read-only snapshot or the tenant transition lock.
pub async fn verify_prepared_targets(
    tx: &mut Transaction<'_, Postgres>,
    target_year: Uuid,
    targets: &[PreparedPromotionTarget],
) -> Result<(), AppError> {
    if targets.is_empty()
        || targets.len() > 500
        || targets
            .iter()
            .map(|t| t.target_student_year_id)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != targets.len()
    {
        return Err(AppError::ValidationError(
            "ตรวจปลายทางที่เตรียมไว้ได้ครั้งละ 1–500 คนโดยไม่ซ้ำ".into(),
        ));
    }
    let invalid:i64=DbSession::new(&mut **tx).scalar("academic_core.promotion_finalization.verify_prepared_targets",
        "SELECT count(*) FROM jsonb_to_recordset($1) expected(student_id uuid,target_student_year_id uuid,target_placement_id uuid,starts_on date,grade_level_id uuid,study_program_id uuid,homeroom_id uuid)
         WHERE NOT EXISTS(SELECT 1 FROM student_academic_years student_year
           WHERE student_year.id=expected.target_student_year_id AND student_year.student_id=expected.student_id AND student_year.academic_year_id=$2
             AND student_year.status='planned' AND student_year.grade_level_id=expected.grade_level_id AND student_year.study_program_id=expected.study_program_id
             AND ((expected.target_placement_id IS NULL AND expected.homeroom_id IS NULL AND NOT EXISTS(SELECT 1 FROM homeroom_placements p WHERE p.student_academic_year_id=student_year.id AND p.status IN ('planned','current')))
               OR EXISTS(SELECT 1 FROM homeroom_placements p WHERE p.id=expected.target_placement_id AND p.student_academic_year_id=student_year.id
                 AND p.homeroom_id=expected.homeroom_id AND p.start_date=expected.starts_on AND p.status='planned')))")
        .bind(sqlx::types::Json(targets)).bind(target_year).fetch_one().await?;
    if invalid > 0 {
        return Err(AppError::Conflict(
            "ปลายทางที่เตรียมไว้เปลี่ยนแล้ว กรุณาตรวจสอบประวัติก่อนยืนยัน".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_explicit_hold_leaves_source_enrollment_unfinished() {
        assert_eq!(final_status(PromotionDestinationOutcome::Hold), None);
        assert_eq!(
            final_status(PromotionDestinationOutcome::Promote),
            Some("completed")
        );
        assert_eq!(
            final_status(PromotionDestinationOutcome::Repeat),
            Some("completed")
        );
        assert_eq!(
            final_status(PromotionDestinationOutcome::Graduate),
            Some("graduated")
        );
        assert_eq!(
            final_status(PromotionDestinationOutcome::TransferOut),
            Some("withdrawn")
        );
    }
}
