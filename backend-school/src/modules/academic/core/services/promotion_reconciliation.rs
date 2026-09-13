use super::promotion_students::PromotionStudentContext;
use crate::{
    error::AppError,
    modules::academic::lifecycle::models::{PromotionDecisionInput, PromotionDecisionOutcome},
};
use chrono::NaiveDate;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PromotionReconciliationOutcome {
    pub target_student_year_id: Option<Uuid>,
    pub target_placement_id: Option<Uuid>,
    pub source_row_version: i64,
}

pub(crate) async fn reconcile_decision(
    tx: &mut Transaction<'_, Postgres>,
    actor_user_id: Uuid,
    source_year_id: Uuid,
    target_year_id: Uuid,
    source_student_year_id: Uuid,
    owned_target_student_year_id: Option<Uuid>,
    owned_target_placement_id: Option<Uuid>,
    decision: &PromotionDecisionInput,
) -> Result<PromotionReconciliationOutcome, AppError> {
    super::promotion_students::validate_run_years(tx, source_year_id, target_year_id).await?;
    let mut sources = super::promotion_students::read_promotion_students(
        tx,
        source_year_id,
        target_year_id,
        &[source_student_year_id],
    )
    .await?;
    let source = sources
        .pop()
        .ok_or_else(|| AppError::NotFound("ไม่พบนักเรียนต้นทาง".into()))?;
    super::promotion_targets::validate_reconciliation_destination(
        tx,
        &source,
        target_year_id,
        decision,
        owned_target_student_year_id,
    )
    .await?;
    lock_owned_rows(
        tx,
        &source,
        target_year_id,
        owned_target_student_year_id,
        owned_target_placement_id,
    )
    .await?;
    let creates_target = matches!(
        decision.outcome,
        PromotionDecisionOutcome::Promote
            | PromotionDecisionOutcome::Repeat
            | PromotionDecisionOutcome::Conditional
    );
    let source_status = match decision.outcome {
        PromotionDecisionOutcome::Graduate => "graduated",
        PromotionDecisionOutcome::TransferOut => "withdrawn",
        _ => "active",
    };
    let source_row_version: i64 = sqlx::query_scalar(
        "UPDATE student_academic_years SET status=$1,row_version=row_version+1,updated_at=now()
         WHERE id=$2 RETURNING row_version",
    )
    .bind(source_status)
    .bind(source.student_academic_year_id)
    .fetch_one(&mut **tx)
    .await?;
    let (target_student_year_id, target_placement_id) = if creates_target {
        let target_student_year_id = upsert_target_student_year(
            tx,
            target_year_id,
            &source,
            owned_target_student_year_id,
            decision,
        )
        .await?;
        let target_placement_id = reconcile_placement(
            tx,
            target_year_id,
            target_student_year_id,
            owned_target_placement_id,
            decision.target_homeroom_id,
        )
        .await?;
        (Some(target_student_year_id), target_placement_id)
    } else {
        if let Some(placement_id) = owned_target_placement_id {
            end_placement(tx, placement_id).await?;
        }
        if let Some(target_id) = owned_target_student_year_id {
            sqlx::query(
                "UPDATE student_academic_years SET status='withdrawn',row_version=row_version+1,updated_at=now() WHERE id=$1",
            )
            .bind(target_id)
            .execute(&mut **tx)
            .await?;
        }
        (None, None)
    };
    let outcome = PromotionReconciliationOutcome {
        target_student_year_id,
        target_placement_id,
        source_row_version,
    };
    super::years_terms::append_audit(
        tx,
        "student_academic_year.promotion_reconciled",
        "student_academic_year",
        source.student_academic_year_id,
        Some(source_year_id),
        None,
        actor_user_id,
        &outcome,
    )
    .await?;
    Ok(outcome)
}

async fn lock_owned_rows(
    tx: &mut Transaction<'_, Postgres>,
    source: &PromotionStudentContext,
    target_year_id: Uuid,
    target_student_year_id: Option<Uuid>,
    target_placement_id: Option<Uuid>,
) -> Result<(), AppError> {
    sqlx::query("SELECT id FROM student_academic_years WHERE id=$1 FOR UPDATE")
        .bind(source.student_academic_year_id)
        .execute(&mut **tx)
        .await?;
    if let Some(target_id) = target_student_year_id {
        let matches: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM student_academic_years
             WHERE id=$1 AND academic_year_id=$2 AND student_id=$3 FOR UPDATE)",
        )
        .bind(target_id)
        .bind(target_year_id)
        .bind(source.student_id)
        .fetch_one(&mut **tx)
        .await?;
        if !matches {
            return Err(AppError::Conflict(
                "ข้อมูลนักเรียนปีปลายทางเปลี่ยนหรือไม่ตรงกับหลักฐานเดิม".into(),
            ));
        }
    }
    if let Some(placement_id) = target_placement_id {
        let matches: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM homeroom_placements
             WHERE id=$1 AND student_academic_year_id=$2 AND academic_year_id=$3 FOR UPDATE)",
        )
        .bind(placement_id)
        .bind(target_student_year_id)
        .bind(target_year_id)
        .fetch_one(&mut **tx)
        .await?;
        if !matches {
            return Err(AppError::Conflict(
                "ห้องประจำชั้นปลายทางเปลี่ยนหรือไม่ตรงกับหลักฐานเดิม".into(),
            ));
        }
    }
    Ok(())
}

async fn upsert_target_student_year(
    tx: &mut Transaction<'_, Postgres>,
    target_year_id: Uuid,
    source: &PromotionStudentContext,
    owned_target_student_year_id: Option<Uuid>,
    decision: &PromotionDecisionInput,
) -> Result<Uuid, AppError> {
    let grade = decision
        .target_grade_level_id
        .ok_or_else(|| AppError::ValidationError("ระบุชั้นปลายทาง".into()))?;
    let program = decision
        .target_study_program_id
        .ok_or_else(|| AppError::ValidationError("ระบุแผนการเรียนปลายทาง".into()))?;
    if let Some(id) = owned_target_student_year_id {
        sqlx::query(
            "UPDATE student_academic_years
             SET grade_level_id=$1,study_program_id=$2,status='planned',row_version=row_version+1,updated_at=now()
             WHERE id=$3",
        )
        .bind(grade)
        .bind(program)
        .bind(id)
        .execute(&mut **tx)
        .await?;
        Ok(id)
    } else {
        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO student_academic_years(
                 id,student_id,academic_year_id,grade_level_id,study_program_id,status
             ) VALUES($1,$2,$3,$4,$5,'planned')",
        )
        .bind(id)
        .bind(source.student_id)
        .bind(target_year_id)
        .bind(grade)
        .bind(program)
        .execute(&mut **tx)
        .await?;
        Ok(id)
    }
}

async fn reconcile_placement(
    tx: &mut Transaction<'_, Postgres>,
    target_year_id: Uuid,
    target_student_year_id: Uuid,
    owned_target_placement_id: Option<Uuid>,
    target_homeroom_id: Option<Uuid>,
) -> Result<Option<Uuid>, AppError> {
    let Some(room_id) = target_homeroom_id else {
        if let Some(placement_id) = owned_target_placement_id {
            end_placement(tx, placement_id).await?;
        }
        return Ok(None);
    };
    let capacity: Option<i32> = sqlx::query_scalar(
        "SELECT capacity FROM homerooms WHERE id=$1 AND academic_year_id=$2 FOR UPDATE",
    )
    .bind(room_id)
    .bind(target_year_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::ValidationError("ไม่พบห้องปลายทางในปีที่เลือก".into()))?;
    let occupied: i64 = sqlx::query_scalar(
        "SELECT count(DISTINCT student_academic_year_id) FROM homeroom_placements
         WHERE homeroom_id=$1 AND academic_year_id=$2 AND status IN ('planned','current')
           AND student_academic_year_id<>$3",
    )
    .bind(room_id)
    .bind(target_year_id)
    .bind(target_student_year_id)
    .fetch_one(&mut **tx)
    .await?;
    if capacity.is_some_and(|maximum| occupied >= i64::from(maximum)) {
        return Err(AppError::Conflict(
            "ห้องปลายทางเต็มแล้ว กรุณาตรวจห้องก่อนปรับผล".into(),
        ));
    }
    if let Some(id) = owned_target_placement_id {
        sqlx::query(
            "UPDATE homeroom_placements
             SET homeroom_id=$1,status='planned',end_date=NULL,row_version=row_version+1,updated_at=now()
             WHERE id=$2",
        )
        .bind(room_id)
        .bind(id)
        .execute(&mut **tx)
        .await?;
        Ok(Some(id))
    } else {
        let (id, start_date): (Uuid, NaiveDate) =
            sqlx::query_as("SELECT gen_random_uuid(),start_date FROM academic_years WHERE id=$1")
                .bind(target_year_id)
                .fetch_one(&mut **tx)
                .await?;
        sqlx::query(
            "INSERT INTO homeroom_placements(
                 id,student_academic_year_id,academic_year_id,homeroom_id,start_date,status,enrollment_type
             ) VALUES($1,$2,$3,$4,$5,'planned','promotion_adjustment')",
        )
        .bind(id)
        .bind(target_student_year_id)
        .bind(target_year_id)
        .bind(room_id)
        .bind(start_date)
        .execute(&mut **tx)
        .await?;
        Ok(Some(id))
    }
}

async fn end_placement(
    tx: &mut Transaction<'_, Postgres>,
    placement_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE homeroom_placements
         SET status='ended',end_date=start_date,row_version=row_version+1,updated_at=now()
         WHERE id=$1",
    )
    .bind(placement_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

#[cfg(test)]
#[path = "promotion_reconciliation_tests.rs"]
mod tests;
