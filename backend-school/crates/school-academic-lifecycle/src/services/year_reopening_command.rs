use super::super::models::{YearReopeningOutcome, YearReopeningRequest};
use school_academic_core::services::{lifecycle_guard, student_years, year_commands, years_terms};
use school_authorization::ActorContext;
use school_errors::AppError;
use school_permissions::registry::codes;
use sqlx::{types::Json, PgPool};
use uuid::Uuid;

pub async fn reopen_year(
    pool: &PgPool,
    actor: &ActorContext,
    year: Uuid,
    input: YearReopeningRequest,
) -> Result<YearReopeningOutcome, AppError> {
    actor.require_permission(codes::ACADEMIC_LIFECYCLE_READ_SCHOOL)?;
    actor.require_permission(codes::ACADEMIC_LIFECYCLE_REOPEN_SCHOOL)?;
    if year.is_nil()
        || input.request_id.is_nil()
        || input.expected_year_version <= 0
        || input.source_checksum.len() != 64
        || !input
            .source_checksum
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return Err(AppError::ValidationError(
            "ข้อมูลอ้างอิงหรือรุ่นความพร้อมไม่ถูกต้อง".into(),
        ));
    }
    let reason = input.reason.trim();
    if reason.is_empty()
        || reason.chars().count() > 1000
        || student_years::contains_thirteen_digit_run(reason)
    {
        return Err(AppError::ValidationError(
            "กรุณาระบุเหตุผลไม่เกิน 1,000 ตัวอักษร โดยไม่ใส่เลขประจำตัวประชาชน".into(),
        ));
    }
    let checksum = super::checksum(&(actor.user_id, year, &input))?;
    let mut tx = pool.begin().await?;
    lifecycle_guard::lock_transition(&mut tx).await?;
    if let Some(outcome) = year_commands::replay_year_command(
        &mut tx,
        input.request_id,
        actor.user_id,
        "reopen",
        &checksum,
    )
    .await?
    {
        tx.commit().await?;
        return Ok(outcome);
    }
    sqlx::query("SELECT id FROM academic_years WHERE id=$1 FOR UPDATE")
        .bind(year)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบปีการศึกษา".into()))?;
    let workspace = super::reopening_workspace_in_transaction(&mut tx, actor, year).await?;
    if workspace.context.row_version != input.expected_year_version
        || workspace.source_checksum != input.source_checksum
    {
        return Err(AppError::Conflict(
            "ข้อมูลปีการศึกษาหรือเงื่อนไขการเปิดกลับเปลี่ยนแล้ว กรุณาตรวจสอบใหม่".into(),
        ));
    }
    if !workspace.can_reopen {
        return Err(AppError::Conflict(
            "ยังมีข้อมูลที่ไม่อนุญาตให้เปิดปีการศึกษาเก่ากลับ".into(),
        ));
    }
    sqlx::query("UPDATE academic_years SET status='closing',row_version=row_version+1,updated_at=now() WHERE id=$1")
        .bind(year).execute(&mut *tx).await?;
    let outcome = YearReopeningOutcome {
        request_id: input.request_id,
        context: super::year_transitions::read_context(&mut tx, year).await?,
        completed_at: chrono::Utc::now(),
    };
    sqlx::query("INSERT INTO academic_year_transition_receipts(request_id,academic_year_id,actor_user_id,action,request_checksum,accepted_readiness,outcome) VALUES($1,$2,$3,'reopen',$4,$5,$6)")
        .bind(input.request_id).bind(year).bind(actor.user_id).bind(checksum)
        .bind(Json(&workspace)).bind(Json(&outcome)).execute(&mut *tx).await?;
    years_terms::append_audit(
        &mut tx,
        "academic.year.reopen",
        "academic_year",
        year,
        Some(year),
        None,
        actor.user_id,
        (&input, &outcome),
    )
    .await?;
    tx.commit().await?;
    Ok(outcome)
}
