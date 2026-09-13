use super::super::models::{YearRecoveryState, YearReopeningOutcome, YearReopeningRequest};
use crate::{error::AppError, middleware::permission::ActorContext, permissions::registry::codes};
use sqlx::{types::Json, PgPool, Postgres, Transaction};
use uuid::Uuid;

pub async fn reopen_year(
    pool: &PgPool,
    actor: &ActorContext,
    year: Uuid,
    input: YearReopeningRequest,
) -> Result<YearReopeningOutcome, AppError> {
    use crate::modules::academic::lifecycle::services;
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
        || super::student_years::contains_thirteen_digit_run(reason)
    {
        return Err(AppError::ValidationError(
            "กรุณาระบุเหตุผลไม่เกิน 1,000 ตัวอักษร โดยไม่ใส่เลขประจำตัวประชาชน".into(),
        ));
    }
    let checksum = services::checksum(&(actor.user_id, year, &input))?;
    let mut tx = pool.begin().await?;
    super::lifecycle_guard::lock_transition(&mut tx).await?;
    if let Some(outcome) = super::year_commands::replay_year_command(
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
    let workspace = services::reopening_workspace_in_transaction(&mut tx, actor, year).await?;
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
    super::years_terms::append_audit(
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

pub(crate) async fn recovery_state(
    tx: &mut Transaction<'_, Postgres>,
    year: Uuid,
) -> Result<YearRecoveryState, AppError> {
    let context = super::year_transitions::read_context(tx, year).await?;
    let (running_years, successor_years, running_terms): (i64, i64, i64) = sqlx::query_as(
        "SELECT
         (SELECT count(*) FROM academic_years WHERE id<>$1 AND status IN ('active','closing')),
         (SELECT count(*) FROM academic_years successor WHERE successor.id<>$1 AND successor.start_date>$2 AND
          (successor.status IN ('active','closing','closed','archived') OR EXISTS(
           SELECT 1 FROM academic_year_transition_receipts receipt WHERE receipt.academic_year_id=successor.id AND receipt.action='activate'))),
         (SELECT count(*) FROM academic_terms WHERE status IN ('active','closing'))",
    ).bind(year).bind(context.start_date).fetch_one(&mut **tx).await?;
    Ok(YearRecoveryState {
        context,
        running_years,
        successor_years,
        running_terms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::academic::{core, cutover_test_support::apply_migrations_through};

    #[tokio::test]
    async fn year_reopening_core_evidence_keeps_successor_activation_history() {
        let pool = core::services_tests::prepare_core_fixture("year_reopening_core_history").await;
        apply_migrations_through(&pool, 76).await.unwrap();
        let year: Uuid = sqlx::query_scalar("SELECT id FROM academic_years WHERE status='active'")
            .fetch_one(&pool)
            .await
            .unwrap();
        let target: Uuid = sqlx::query_scalar("INSERT INTO academic_years(year,name,start_date,end_date,school_days,status) SELECT max(year)+1,'E2E-LIFECYCLE-recovery-history',max(end_date)+1,max(end_date)+366,'MON','planning' FROM academic_years RETURNING id").fetch_one(&pool).await.unwrap();
        let actor: Uuid =
            sqlx::query_scalar("SELECT id FROM users WHERE user_type='staff' ORDER BY id LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        // A historical accepted activation still blocks destructive reopening,
        // even if an independently repaired year row no longer says active.
        sqlx::query("INSERT INTO academic_year_transition_receipts(request_id,academic_year_id,actor_user_id,action,request_checksum,accepted_readiness,outcome) VALUES($1,$2,$3,'activate',$4,'{}','{}')").bind(Uuid::new_v4()).bind(target).bind(actor).bind("a".repeat(64)).execute(&pool).await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        let state = recovery_state(&mut tx, year).await.unwrap();
        assert_eq!(state.context.academic_year_id, year);
        assert_eq!(state.successor_years, 1);
        assert!(state.running_terms > 0);
    }
}
