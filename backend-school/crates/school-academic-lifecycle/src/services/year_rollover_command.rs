use crate::models::*;
use crate::ports::{ExternalLifecyclePort, RegistrationOutcome};
use school_academic_core::services::{lifecycle_guard, promotion_batch};
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
    if sources.len() != input.preview.decisions.len() {
        return Err(AppError::Conflict("ต้องยืนยันรายชื่อนักเรียนครบตามตัวอย่าง".into()));
    }
    let mut items = Vec::with_capacity(sources.len());
    let mut declarations = Vec::with_capacity(sources.len());
    let mut registration = vec![];
    for (source, declared) in sources.iter().zip(&input.preview.decisions) {
        if source.student_academic_year_id != declared.student_academic_year_id {
            return Err(AppError::Conflict("ต้องยืนยันรายชื่อนักเรียนครบตามตัวอย่าง".into()));
        }
        items.push(ManualItem {
            id: Uuid::now_v7(),
            student_academic_year_id: source.student_academic_year_id,
            student_id: source.student_id,
            source_grade_level_id: source.grade_level_id,
            source_study_program_id: source.study_program_id,
            source_row_version: source.row_version,
            source_checksum: super::checksum(&(
                source,
                &declared.decision,
                input.preview.outcome_effective_on,
                input.preview.target_placement_starts_on,
            ))?,
            existing_target_student_year_id: source.existing_target_student_year_id,
            decision: declared.decision.clone(),
        });
        declarations.push(promotion_batch::PromotionBatchDeclaration {
            source: source.clone(),
            decision: declared.decision.core_command(),
            original_target_placement_id: accepted
                .get(&source.student_academic_year_id)
                .and_then(|prior| prior.target_placement_id),
        });
        match declared.decision.outcome {
            PromotionDecisionOutcome::Graduate => {
                registration.push((source.student_id, RegistrationOutcome::Graduated))
            }
            PromotionDecisionOutcome::TransferOut => {
                registration.push((source.student_id, RegistrationOutcome::TransferredOut))
            }
            _ => {}
        }
    }
    for batch in items.chunks(500) {
        DbSession::new(&mut *tx).query("academic_lifecycle.year_rollover_command.insert_items",
            "INSERT INTO academic_promotion_run_items(id,run_id,source_year_id,target_year_id,target_placement_starts_on,student_academic_year_id,student_id,
             source_grade_level_id,source_study_program_id,source_row_version,annual_revision_id,source_checksum,recommendation,existing_target_student_year_id,decision,reviewed_by,reviewed_at,status)
             SELECT row.id,$2,$3,$4,$5,row.student_academic_year_id,row.student_id,row.source_grade_level_id,row.source_study_program_id,row.source_row_version,
               NULL,row.source_checksum,NULL,row.existing_target_student_year_id,row.decision,$6,now(),'reviewed'
             FROM jsonb_to_recordset($1) row(id uuid,student_academic_year_id uuid,student_id uuid,source_grade_level_id uuid,source_study_program_id uuid,
               source_row_version bigint,source_checksum text,existing_target_student_year_id uuid,decision jsonb)")
            .bind(Json(batch)).bind(run_id).bind(input.preview.source_year_id).bind(input.preview.target_year_id)
            .bind(input.preview.target_placement_starts_on).bind(actor.user_id).execute().await?;
    }
    let mut written = vec![];
    for batch in declarations.chunks(500) {
        written.extend(
            promotion_batch::apply_declarations(
                &mut tx,
                actor.user_id,
                input.preview.source_year_id,
                input.preview.target_year_id,
                input.preview.outcome_effective_on,
                input.preview.target_placement_starts_on,
                batch,
            )
            .await?,
        );
    }
    external
        .apply_registration_outcomes(&mut tx, actor.user_id, &registration)
        .await?;
    let mut roster_ids: Vec<_> = written
        .iter()
        .flat_map(|row| {
            std::iter::once(row.student_academic_year_id).chain(row.write.target_student_year_id)
        })
        .collect();
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
    let written: std::collections::BTreeMap<_, _> = written
        .into_iter()
        .map(|row| (row.student_academic_year_id, row))
        .collect();
    let mut batches = vec![];
    let mut receipt_rows = vec![];
    for (index, batch) in items.chunks(100).enumerate() {
        let batch_id = Uuid::new_v5(
            &input.request_id,
            format!("manual-rollover-batch:{index}").as_bytes(),
        );
        batches.push(ManualBatch {
            request_id: batch_id,
            item_ids: batch.iter().map(|row| row.id).collect(),
        });
        for item in batch {
            let result = written
                .get(&item.student_academic_year_id)
                .ok_or_else(|| AppError::Conflict("ผลเตรียมปีใหม่ไม่ครบตามรายชื่อ".into()))?;
            receipt_rows.push(ManualReceipt {
                item_id: item.id,
                request_id: batch_id,
                student_id: item.student_id,
                target_student_year_id: result.write.target_student_year_id,
                target_placement_id: result.write.target_placement_id,
                source_row_version: result.write.source_row_version,
            });
        }
    }
    DbSession::new(&mut *tx).query("academic_lifecycle.year_rollover_command.insert_batches",
        "INSERT INTO academic_promotion_execution_batches(request_id,run_id,approval_id,actor_user_id,request_checksum,item_ids)
         SELECT request_id,$2,$3,$4,$5,item_ids FROM jsonb_to_recordset($1) row(request_id uuid,item_ids uuid[])")
        .bind(Json(&batches)).bind(run_id).bind(confirmation_id).bind(actor.user_id).bind(&checksum).execute().await?;
    for batch in receipt_rows.chunks(500) {
        DbSession::new(&mut *tx).query("academic_lifecycle.year_rollover_command.insert_receipts",
            "INSERT INTO academic_promotion_execution_receipts(item_id,run_id,request_id,approval_id,source_year_id,target_year_id,student_id,target_student_year_id,target_placement_id,source_row_version,executed_by)
             SELECT item_id,$2,request_id,$3,$4,$5,student_id,target_student_year_id,target_placement_id,source_row_version,$6
             FROM jsonb_to_recordset($1) row(item_id uuid,request_id uuid,student_id uuid,target_student_year_id uuid,target_placement_id uuid,source_row_version bigint)")
            .bind(Json(batch)).bind(run_id).bind(confirmation_id).bind(input.preview.source_year_id).bind(input.preview.target_year_id).bind(actor.user_id).execute().await?;
    }
    DbSession::new(&mut *tx).query("academic_lifecycle.year_rollover_command.complete_items",
        "UPDATE academic_promotion_run_items SET status='executed',row_version=row_version+1,updated_at=now() WHERE run_id=$1")
        .bind(run_id).execute().await?;
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

#[derive(serde::Serialize)]
struct ManualItem {
    id: Uuid,
    student_academic_year_id: Uuid,
    student_id: Uuid,
    source_grade_level_id: Uuid,
    source_study_program_id: Uuid,
    source_row_version: i64,
    source_checksum: String,
    existing_target_student_year_id: Option<Uuid>,
    decision: PromotionDecisionInput,
}
#[derive(serde::Serialize)]
struct ManualBatch {
    request_id: Uuid,
    item_ids: Vec<Uuid>,
}
#[derive(serde::Serialize)]
struct ManualReceipt {
    item_id: Uuid,
    request_id: Uuid,
    student_id: Uuid,
    target_student_year_id: Option<Uuid>,
    target_placement_id: Option<Uuid>,
    source_row_version: i64,
}
