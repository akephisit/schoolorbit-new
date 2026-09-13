use super::promotion_students::PromotionStudentContext;
use crate::{error::AppError, modules::academic::lifecycle::models::PromotionDecisionInput};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PromotionWriteOutcome {
    pub target_student_year_id: Option<Uuid>,
    pub target_placement_id: Option<Uuid>,
    pub source_row_version: i64,
}

/// Internal Core command. Lifecycle owns permission, approved evidence and the
/// immutable receipt, in this same transaction under the tenant transition lock.
pub(crate) async fn execute_decision(
    tx: &mut Transaction<'_, Postgres>,
    actor: Uuid,
    source_year: Uuid,
    target_year: Uuid,
    source: &PromotionStudentContext,
    decision: &PromotionDecisionInput,
) -> Result<PromotionWriteOutcome, AppError> {
    use crate::modules::academic::lifecycle::models::PromotionDecisionOutcome as Outcome;
    super::promotion_students::validate_run_years(tx, source_year, target_year).await?;
    let actual: Option<i64> = sqlx::query_scalar(
        "SELECT row_version FROM student_academic_years WHERE id=$1 AND academic_year_id=$2 AND student_id=$3 FOR UPDATE",
    ).bind(source.student_academic_year_id).bind(source_year).bind(source.student_id)
        .fetch_optional(&mut **tx).await?;
    if actual != Some(source.row_version) {
        return Err(AppError::Conflict(
            "ข้อมูลนักเรียนต้นทางเปลี่ยนแล้ว ต้องตรวจและอนุมัติใหม่".into(),
        ));
    }
    let current = super::promotion_students::read_promotion_students(
        tx,
        source_year,
        target_year,
        &[source.student_academic_year_id],
    )
    .await?
    .into_iter()
    .next()
    .ok_or_else(|| AppError::NotFound("ไม่พบนักเรียนต้นทาง".into()))?;
    if current.grade_level_id != source.grade_level_id
        || current.study_program_id != source.study_program_id
        || current.status != source.status
    {
        return Err(AppError::Conflict(
            "สถานะ ชั้น หรือแผนการเรียนต้นทางเปลี่ยนแล้ว".into(),
        ));
    }
    super::promotion_targets::validate_destination(tx, &current, target_year, decision).await?;
    let mut result = PromotionWriteOutcome {
        target_student_year_id: None,
        target_placement_id: None,
        source_row_version: current.row_version,
    };
    match decision.outcome {
        Outcome::Hold => return Ok(result),
        Outcome::Graduate | Outcome::TransferOut => {
            result.source_row_version = sqlx::query_scalar(
                "UPDATE student_academic_years SET status=$1,row_version=row_version+1,updated_at=now() WHERE id=$2 RETURNING row_version",
            ).bind(if decision.outcome == Outcome::Graduate { "graduated" } else { "withdrawn" })
                .bind(current.student_academic_year_id).fetch_one(&mut **tx).await?;
        }
        Outcome::Promote | Outcome::Repeat | Outcome::Conditional => {
            let grade = decision
                .target_grade_level_id
                .ok_or_else(|| AppError::ValidationError("ระบุชั้นปลายทาง".into()))?;
            let program = decision
                .target_study_program_id
                .ok_or_else(|| AppError::ValidationError("ระบุแผนการเรียนปลายทาง".into()))?;
            if let Some(room) = decision.target_homeroom_id {
                let capacity: Option<i32> =
                    sqlx::query_scalar("SELECT capacity FROM homerooms WHERE id=$1 FOR UPDATE")
                        .bind(room)
                        .fetch_one(&mut **tx)
                        .await?;
                let occupied: i64 = sqlx::query_scalar("SELECT count(DISTINCT student_academic_year_id) FROM homeroom_placements WHERE homeroom_id=$1 AND academic_year_id=$2 AND status IN ('planned','current')")
                    .bind(room).bind(target_year).fetch_one(&mut **tx).await?;
                if capacity.is_some_and(|maximum| occupied >= i64::from(maximum)) {
                    return Err(AppError::Conflict(
                        "ห้องปลายทางเต็มแล้ว กรุณาตรวจห้องก่อนดำเนินการ".into(),
                    ));
                }
            }
            let id = Uuid::new_v4();
            sqlx::query("INSERT INTO student_academic_years(id,student_id,academic_year_id,grade_level_id,study_program_id,status) VALUES($1,$2,$3,$4,$5,'planned')")
                .bind(id).bind(current.student_id).bind(target_year).bind(grade).bind(program).execute(&mut **tx).await?;
            result.target_student_year_id = Some(id);
            if let Some(room) = decision.target_homeroom_id {
                let placement = Uuid::new_v4();
                sqlx::query("INSERT INTO homeroom_placements(id,student_academic_year_id,academic_year_id,homeroom_id,start_date,status,enrollment_type) SELECT $1,$2,$3,$4,start_date,'planned','promotion' FROM academic_years WHERE id=$3")
                    .bind(placement).bind(id).bind(target_year).bind(room).execute(&mut **tx).await?;
                result.target_placement_id = Some(placement);
            }
        }
    }
    super::years_terms::append_audit(
        tx,
        "student_academic_year.promotion_applied",
        "student_academic_year",
        current.student_academic_year_id,
        Some(source_year),
        None,
        actor,
        &result,
    )
    .await?;
    Ok(result)
}

#[cfg(test)]
#[path = "promotion_execution_tests.rs"]
mod tests;
