use super::super::models::*;
use super::promotion_runs::RUN_COLUMNS;
use crate::{error::AppError, middleware::permission::ActorContext};
use crate::{
    modules::academic::{
        core::services::{lifecycle_guard, promotion_students},
        results,
    },
    permissions::registry::codes,
};
use sqlx::types::Json;
use sqlx::PgPool;
use uuid::Uuid;

pub(super) const ITEM_COLUMNS:&str="id,run_id,source_year_id,target_year_id,student_academic_year_id,student_id,source_grade_level_id,source_study_program_id,source_row_version,annual_revision_id,source_checksum,recommendation,existing_target_student_year_id,decision,reviewed_by,reviewed_at,status,row_version,created_at,updated_at";

pub async fn calculate_run(
    pool: &PgPool,
    actor: &ActorContext,
    run_id: Uuid,
    input: CalculatePromotionRunInput,
) -> Result<PromotionRunCalculation, AppError> {
    actor.require_all_permissions(&[
        codes::ACADEMIC_PROMOTION_READ_SCHOOL,
        codes::ACADEMIC_PROMOTION_MANAGE_SCHOOL,
    ])?;
    if run_id.is_nil() || input.request_id.is_nil() || input.row_version < 1 {
        return Err(AppError::ValidationError(
            "รอบ รหัสคำขอ หรือรุ่นรายการไม่ถูกต้อง".into(),
        ));
    }
    let request_checksum = super::checksum(&(actor.user_id, run_id, "calculate", &input))?;
    let mut tx = pool.begin().await?;
    lifecycle_guard::lock_transition(&mut tx).await?;
    if let Some(outcome) = super::promotion_runs::replay_command(
        &mut tx,
        input.request_id,
        "calculate",
        &request_checksum,
    )
    .await?
    {
        tx.commit().await?;
        return Ok(outcome);
    }
    let run: PromotionRun = sqlx::query_as(&format!(
        "SELECT {RUN_COLUMNS} FROM academic_promotion_runs WHERE id=$1"
    ))
    .bind(run_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบรอบเลื่อนชั้น".into()))?;
    if run.row_version != input.row_version
        || matches!(
            run.status,
            PromotionRunStatus::Completed | PromotionRunStatus::Executing
        )
    {
        return Err(AppError::Conflict(
            "รอบมีการเปลี่ยนแปลง กำลังดำเนินการ หรือเสร็จแล้ว กรุณาโหลดข้อมูลล่าสุด".into(),
        ));
    }
    promotion_students::validate_run_years(&mut tx, run.source_year_id, run.target_year_id).await?;
    // Identity is immutable. Keep entity locks after the ordered year locks.
    sqlx::query("SELECT id FROM academic_promotion_runs WHERE id=$1 FOR UPDATE")
        .bind(run_id)
        .execute(&mut *tx)
        .await?;
    let Json(rules): Json<Vec<PromotionRuleInput>> =
        sqlx::query_scalar("SELECT rules FROM academic_promotion_policy_versions WHERE id=$1")
            .bind(run.policy_id)
            .fetch_one(&mut *tx)
            .await?;
    let rules: std::collections::BTreeMap<_, _> = rules
        .iter()
        .map(|rule| ((rule.from_grade_level_id, rule.from_study_program_id), rule))
        .collect();
    let coverage = results::services::annual_closure_coverage(&mut tx, run.source_year_id).await?;
    let retained:Vec<Uuid>=sqlx::query_scalar("SELECT student_academic_year_id FROM academic_promotion_run_items WHERE run_id=$1 ORDER BY student_academic_year_id LIMIT 10001")
        .bind(run_id).fetch_all(&mut *tx).await?;
    let students: Vec<Uuid> = coverage
        .students
        .iter()
        .map(|s| s.student_academic_year_id)
        .chain(retained)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    if students.is_empty() || students.len() > 10000 {
        return Err(AppError::ValidationError(
            "รอบเลื่อนชั้นต้องมีนักเรียนต้นทาง 1–10000 คน".into(),
        ));
    }
    for ids in students.chunks(500) {
        let sources = promotion_students::read_promotion_students(
            &mut tx,
            run.source_year_id,
            run.target_year_id,
            ids,
        )
        .await?;
        let mut annual =
            results::services::promotion_annual_sources(&mut tx, run.source_year_id, ids).await?;
        let mut prepared = Vec::with_capacity(sources.len());
        for source in sources {
            let evidence = annual
                .remove(&source.student_academic_year_id)
                .ok_or_else(|| {
                    AppError::InternalServerError("ชุดผลรายปีไม่ครบนักเรียนที่ตรวจสอบ".into())
                })?;
            let rule = rules
                .get(&(source.grade_level_id, source.study_program_id))
                .copied();
            let recommendation =
                super::promotion_recommendation::recommend(rule, evidence.as_ref())?;
            let source_checksum = evidence_checksum(run.policy_id, &source, evidence.as_ref())?;
            prepared.push(PreparedItem {
                student_academic_year_id: source.student_academic_year_id,
                student_id: source.student_id,
                source_grade_level_id: source.grade_level_id,
                source_study_program_id: source.study_program_id,
                source_row_version: source.row_version,
                annual_revision_id: evidence.as_ref().map(|row| row.id),
                source_checksum,
                recommendation,
                existing_target_student_year_id: source.existing_target_student_year_id,
            });
        }
        sqlx::query(
            "INSERT INTO academic_promotion_run_items(run_id,source_year_id,target_year_id,student_academic_year_id,student_id,source_grade_level_id,source_study_program_id,source_row_version,annual_revision_id,source_checksum,recommendation,existing_target_student_year_id)
             SELECT $1,$2,$3,item.student_academic_year_id,item.student_id,item.source_grade_level_id,item.source_study_program_id,item.source_row_version,item.annual_revision_id,item.source_checksum,item.recommendation,item.existing_target_student_year_id
             FROM jsonb_to_recordset($4) AS item(student_academic_year_id uuid,student_id uuid,source_grade_level_id uuid,source_study_program_id uuid,source_row_version bigint,annual_revision_id uuid,source_checksum text,recommendation jsonb,existing_target_student_year_id uuid)
             ON CONFLICT(run_id,student_academic_year_id) DO UPDATE SET
             source_grade_level_id=EXCLUDED.source_grade_level_id,source_study_program_id=EXCLUDED.source_study_program_id,
             source_row_version=EXCLUDED.source_row_version,annual_revision_id=EXCLUDED.annual_revision_id,
             source_checksum=EXCLUDED.source_checksum,recommendation=EXCLUDED.recommendation,
             existing_target_student_year_id=EXCLUDED.existing_target_student_year_id,decision=NULL,reviewed_by=NULL,reviewed_at=NULL,
             status='calculated',row_version=academic_promotion_run_items.row_version+1,updated_at=now()
             WHERE academic_promotion_run_items.status<>'executed'"
        ).bind(run.id).bind(run.source_year_id).bind(run.target_year_id).bind(Json(prepared)).execute(&mut *tx).await?;
    }
    let run:PromotionRun=sqlx::query_as(&format!("UPDATE academic_promotion_runs SET status='calculated',row_version=row_version+1,reviewed_by=NULL,reviewed_at=NULL,approved_by=NULL,approved_at=NULL,approval_id=NULL,updated_at=now() WHERE id=$1 RETURNING {RUN_COLUMNS}"))
        .bind(run_id).fetch_one(&mut *tx).await?;
    let items=sqlx::query_as(&format!("SELECT {ITEM_COLUMNS} FROM academic_promotion_run_items WHERE run_id=$1 ORDER BY student_academic_year_id LIMIT 10001"))
        .bind(run_id).fetch_all(&mut *tx).await?;
    let outcome = PromotionRunCalculation { run, items };
    sqlx::query("INSERT INTO academic_audit_events(event_code,entity_type,entity_id,actor_user_id,payload) VALUES ('promotion_run.calculated','promotion_run',$1,$2,$3)")
        .bind(run_id).bind(actor.user_id).bind(Json(serde_json::json!({"itemCount":outcome.items.len(),"rowVersion":outcome.run.row_version}))).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO academic_promotion_run_commands(request_id,run_id,actor_user_id,action,request_checksum,outcome) VALUES ($1,$2,$3,'calculate',$4,$5)")
        .bind(input.request_id).bind(run_id).bind(actor.user_id).bind(request_checksum).bind(Json(&outcome)).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(outcome)
}

#[derive(serde::Serialize)]
struct PreparedItem {
    student_academic_year_id: Uuid,
    student_id: Uuid,
    source_grade_level_id: Uuid,
    source_study_program_id: Uuid,
    source_row_version: i64,
    annual_revision_id: Option<Uuid>,
    source_checksum: String,
    recommendation: PromotionRecommendation,
    existing_target_student_year_id: Option<Uuid>,
}

pub(super) fn evidence_checksum(
    policy: Uuid,
    source: &promotion_students::PromotionStudentContext,
    annual: Option<&results::models::AnnualResultRevision>,
) -> Result<String, AppError> {
    // Display labels are not academic evidence. Enrollment identity/version,
    // target ownership and the exact current annual revision are.
    super::checksum(&(
        policy,
        source.student_academic_year_id,
        source.student_id,
        source.grade_level_id,
        source.study_program_id,
        source.status,
        source.row_version,
        source.existing_target_student_year_id,
        source.existing_target_row_version,
        annual.map(|row| {
            (
                row.id,
                row.revision,
                &row.snapshot.source_checksum,
                row.is_current,
            )
        }),
    ))
}

#[cfg(test)]
#[path = "promotion_calculation_tests.rs"]
mod tests;
