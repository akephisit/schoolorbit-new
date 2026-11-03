use crate::models::*;
use crate::ports::{ExternalLifecyclePort, RegistrationOutcome};
use school_academic_core::services::{
    lifecycle_guard, promotion_execution, promotion_finalization,
};
use school_authorization::ActorContext;
use school_database::DbSession;
use school_errors::AppError;
use school_permissions::registry::codes;
use sqlx::{types::Json, PgPool, Postgres, Transaction};
use uuid::Uuid;

pub async fn confirm(
    external: &dyn ExternalLifecyclePort,
    pool: &PgPool,
    actor: &ActorContext,
    mut input: ConfirmYearRolloverInput,
) -> Result<YearRolloverOutcome, AppError> {
    actor.require_all_permissions(&[
        codes::ACADEMIC_PROMOTION_READ_SCHOOL,
        codes::ACADEMIC_PROMOTION_EXECUTE_SCHOOL,
    ])?;
    super::year_rollover::validate_input(&input.preview)?;
    if input.request_id.is_nil() || !valid_checksum(&input.source_checksum) {
        return Err(AppError::ValidationError(
            "ระบุรหัสคำขอและรุ่นตัวอย่างให้ถูกต้อง".into(),
        ));
    }
    input
        .preview
        .decisions
        .sort_by_key(|row| row.student_academic_year_id);
    let checksum = super::checksum(&(actor.user_id, "confirm", &input))?;
    let mut tx = school_tenancy::TenantPool::begin_observed(
        pool,
        "academic_lifecycle.year_rollover_command.confirm",
    )
    .await?;
    lifecycle_guard::lock_transition(&mut tx).await?;
    if let Some(outcome) = replay(&mut tx, actor.user_id, &input, &checksum).await? {
        tx.commit().await?;
        return Ok(outcome);
    }
    let preview = super::year_rollover::preview_in_transaction(&mut tx, &input.preview).await?;
    if preview.source_checksum != input.source_checksum {
        return Err(AppError::Conflict(
            "รายชื่อนักเรียนหรือปลายทางเปลี่ยนแล้ว กรุณาตรวจตัวอย่างใหม่".into(),
        ));
    }
    if !preview.can_confirm {
        return Err(AppError::Conflict(
            "ยังมีรายการที่ต้องตรวจสอบก่อนยืนยันรายชื่อนักเรียนปีใหม่".into(),
        ));
    }
    let sources = super::year_rollover::source_contexts(&mut tx, &input.preview).await?;
    let accepted = super::year_rollover::accepted_destinations(
        &mut tx,
        input.preview.source_year_id,
        input.preview.target_year_id,
    )
    .await?;
    let run_id = Uuid::now_v7();
    let confirmation_id = Uuid::now_v7();
    DbSession::new(&mut *tx).query(
        "academic_lifecycle.year_rollover_command.confirm",
        "INSERT INTO academic_promotion_runs(id,source_year_id,target_year_id,target_placement_starts_on,policy_id,request_id,request_checksum,created_by)
         VALUES($1,$2,$3,$4,NULL,$5,$6,$7)",
    ).bind(run_id).bind(input.preview.source_year_id).bind(input.preview.target_year_id).bind(input.preview.target_placement_starts_on)
        .bind(input.request_id).bind(&checksum).bind(actor.user_id).execute().await?;
    // This is the real single staff confirmation. No policy/result evidence or
    // separate approval/review action is manufactured for the ledger.
    DbSession::new(&mut *tx).query(
        "academic_lifecycle.year_rollover_command.confirm",
        "INSERT INTO academic_promotion_run_approvals(id,run_id,approved_run_version,source_checksum,intent,approved_by)
         VALUES($1,$2,1,$3,$4,$5)",
    ).bind(confirmation_id).bind(run_id).bind(&input.source_checksum).bind(Json(&preview)).bind(actor.user_id).execute().await?;
    DbSession::new(&mut *tx).query(
        "academic_lifecycle.year_rollover_command.confirm",
        "UPDATE academic_promotion_runs SET status='executing',approval_id=$2,approved_by=$3,approved_at=now() WHERE id=$1",
    ).bind(run_id).bind(confirmation_id).bind(actor.user_id).execute().await?;
    let mut items = Vec::with_capacity(sources.len());
    let mut registration = vec![];
    let mut roster_ids = vec![];
    for (source, declared) in sources.iter().zip(&input.preview.decisions) {
        if source.student_academic_year_id != declared.student_academic_year_id {
            return Err(AppError::Conflict("ต้องยืนยันรายชื่อนักเรียนครบตามตัวอย่าง".into()));
        }
        let item_id = Uuid::now_v7();
        let source_checksum = super::checksum(&(
            source,
            &declared.decision,
            input.preview.outcome_effective_on,
            input.preview.target_placement_starts_on,
        ))?;
        DbSession::new(&mut *tx).query(
            "academic_lifecycle.year_rollover_command.confirm",
            "INSERT INTO academic_promotion_run_items(id,run_id,source_year_id,target_year_id,target_placement_starts_on,student_academic_year_id,student_id,
             source_grade_level_id,source_study_program_id,source_row_version,annual_revision_id,source_checksum,recommendation,
             existing_target_student_year_id,decision,reviewed_by,reviewed_at,status)
             VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,NULL,$11,NULL,$12,$13,$14,now(),'reviewed')",
        ).bind(item_id).bind(run_id).bind(input.preview.source_year_id).bind(input.preview.target_year_id)
            .bind(input.preview.target_placement_starts_on).bind(source.student_academic_year_id).bind(source.student_id)
            .bind(source.grade_level_id).bind(source.study_program_id).bind(source.row_version).bind(source_checksum)
            .bind(source.existing_target_student_year_id).bind(Json(&declared.decision)).bind(actor.user_id).execute().await?;
        let core_decision = declared.decision.core_command();
        let mut result = if let Some(target) = source.existing_target_student_year_id {
            let prior = accepted
                .get(&source.student_academic_year_id)
                .ok_or_else(|| AppError::Conflict("ไม่พบหลักฐานปลายทางที่เตรียมไว้".into()))?;
            // The same-transaction preview verified exact accepted identities under
            // the exclusive tenant transition lock shared by ordinary writers.
            promotion_execution::PromotionWriteOutcome {
                target_student_year_id: Some(target),
                target_placement_id: prior.target_placement_id,
                source_row_version: source.row_version,
            }
        } else {
            promotion_execution::execute_decision(
                &mut tx,
                actor.user_id,
                input.preview.source_year_id,
                input.preview.target_year_id,
                Some(input.preview.target_placement_starts_on),
                source,
                &core_decision,
            )
            .await?
        };
        result.source_row_version = promotion_finalization::finish_source(
            &mut tx,
            actor.user_id,
            input.preview.source_year_id,
            source.student_academic_year_id,
            input.preview.outcome_effective_on,
            core_decision.outcome,
        )
        .await?;
        match declared.decision.outcome {
            PromotionDecisionOutcome::Graduate => {
                registration.push((source.student_id, RegistrationOutcome::Graduated))
            }
            PromotionDecisionOutcome::TransferOut => {
                registration.push((source.student_id, RegistrationOutcome::TransferredOut))
            }
            _ => {}
        }
        roster_ids.push(source.student_academic_year_id);
        roster_ids.extend(result.target_student_year_id);
        items.push((item_id, source.student_id, result));
    }
    external
        .apply_registration_outcomes(&mut tx, actor.user_id, &registration)
        .await?;
    roster_ids.sort_unstable();
    roster_ids.dedup();
    for batch in roster_ids.chunks(500) {
        school_academic_delivery::services::roster_tracking::sync_students(
            &mut tx,
            actor.user_id,
            batch,
        )
        .await?;
    }
    for (index, batch) in items.chunks(100).enumerate() {
        let batch_id = Uuid::new_v5(
            &input.request_id,
            format!("manual-rollover-batch:{index}").as_bytes(),
        );
        let ids: Vec<_> = batch.iter().map(|row| row.0).collect();
        DbSession::new(&mut *tx).query(
            "academic_lifecycle.year_rollover_command.confirm",
            "INSERT INTO academic_promotion_execution_batches(request_id,run_id,approval_id,actor_user_id,request_checksum,item_ids) VALUES($1,$2,$3,$4,$5,$6)",
        ).bind(batch_id).bind(run_id).bind(confirmation_id).bind(actor.user_id).bind(&checksum).bind(&ids).execute().await?;
        for (item_id, student_id, result) in batch {
            DbSession::new(&mut *tx).query(
                "academic_lifecycle.year_rollover_command.confirm",
                "INSERT INTO academic_promotion_execution_receipts(item_id,run_id,request_id,approval_id,source_year_id,target_year_id,student_id,
                 target_student_year_id,target_placement_id,source_row_version,executed_by) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)",
            ).bind(item_id).bind(run_id).bind(batch_id).bind(confirmation_id).bind(input.preview.source_year_id).bind(input.preview.target_year_id)
                .bind(student_id).bind(result.target_student_year_id).bind(result.target_placement_id).bind(result.source_row_version).bind(actor.user_id).execute().await?;
        }
        DbSession::new(&mut *tx).query("academic_lifecycle.year_rollover_command.confirm",
            "UPDATE academic_promotion_run_items SET status='executed',row_version=row_version+1,updated_at=now() WHERE id=ANY($1)",
        ).bind(ids).execute().await?;
    }
    DbSession::new(&mut *tx).query("academic_lifecycle.year_rollover_command.confirm",
        "UPDATE academic_promotion_runs SET status='completed',row_version=row_version+1,executed_by=$2,executed_at=now(),updated_at=now() WHERE id=$1",
    ).bind(run_id).bind(actor.user_id).execute().await?;
    let receipts=DbSession::new(&mut *tx).query_as("academic_lifecycle.year_rollover_command.confirm",
        "SELECT item_id,run_id,request_id,approval_id,target_student_year_id,target_placement_id,source_row_version,executed_by,executed_at FROM academic_promotion_execution_receipts WHERE run_id=$1 ORDER BY item_id",
    ).bind(run_id).fetch_all().await?;
    let outcome = YearRolloverOutcome {
        request_id: input.request_id,
        run_id,
        source_year_id: input.preview.source_year_id,
        target_year_id: input.preview.target_year_id,
        prepared: true,
        activated: false,
        student_count: items.len(),
        receipts,
        completed_at: chrono::Utc::now(),
    };
    DbSession::new(&mut *tx).query("academic_lifecycle.year_rollover_command.confirm",
        "INSERT INTO academic_promotion_run_commands(request_id,run_id,actor_user_id,action,request_checksum,outcome) VALUES($1,$2,$3,'confirm',$4,$5)",
    ).bind(input.request_id).bind(run_id).bind(actor.user_id).bind(checksum).bind(Json(&outcome)).execute().await?;
    school_academic_core::services::years_terms::append_audit(
        &mut tx,
        "promotion_run.manual_confirmed",
        "promotion_run",
        run_id,
        Some(input.preview.source_year_id),
        None,
        actor.user_id,
        (&input, &outcome),
    )
    .await?;
    tx.commit().await?;
    Ok(outcome)
}

async fn replay(
    tx: &mut Transaction<'_, Postgres>,
    actor: Uuid,
    input: &ConfirmYearRolloverInput,
    checksum: &str,
) -> Result<Option<YearRolloverOutcome>, AppError> {
    let previous:Option<(Uuid,String,String)>=DbSession::new(&mut **tx).query_as(
        "academic_lifecycle.year_rollover_command.replay",
        "SELECT actor_user_id,action,request_checksum FROM academic_promotion_run_commands WHERE request_id=$1",
    ).bind(input.request_id).fetch_optional().await?;
    if let Some((owner, action, original)) = previous {
        if owner != actor || action != "confirm" || original != checksum {
            return Err(AppError::Conflict(
                "คำขอนี้เคยใช้กับข้อมูลหรือผู้ดำเนินการอื่นแล้ว".into(),
            ));
        }
        let Json(outcome) = DbSession::new(&mut **tx)
            .scalar::<Json<YearRolloverOutcome>>(
                "academic_lifecycle.year_rollover_command.replay",
                "SELECT outcome FROM academic_promotion_run_commands WHERE request_id=$1",
            )
            .bind(input.request_id)
            .fetch_one()
            .await?;
        return Ok(Some(outcome));
    }
    let reserved:bool=DbSession::new(&mut **tx).scalar("academic_lifecycle.year_rollover_command.replay",
        "SELECT EXISTS(SELECT 1 FROM academic_promotion_runs WHERE request_id=$1) OR EXISTS(SELECT 1 FROM academic_promotion_execution_batches WHERE request_id=$1)",
    ).bind(input.request_id).fetch_one().await?;
    if reserved {
        return Err(AppError::Conflict("รหัสคำขอนี้ใช้กับประวัติการเลื่อนชั้นแล้ว".into()));
    }
    Ok(None)
}

fn valid_checksum(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn confirmation_requires_the_exact_lowercase_sha256_snapshot() {
        assert!(valid_checksum(&"a".repeat(64)));
        assert!(!valid_checksum(&"A".repeat(64)));
        assert!(!valid_checksum("stale"));
    }
}
