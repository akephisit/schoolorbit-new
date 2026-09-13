use crate::{
    error::AppError,
    middleware::permission::ActorContext,
    modules::academic::{
        core::{models::AcademicYearStatus, services::year_reopening},
        lifecycle::models::{LifecycleFinding, LifecycleSeverity, YearReopeningWorkspace},
    },
    permissions::registry::codes,
};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

pub async fn get_year_reopening_workspace(
    pool: &PgPool,
    actor: &ActorContext,
    year: Uuid,
) -> Result<YearReopeningWorkspace, AppError> {
    actor.require_permission(codes::ACADEMIC_LIFECYCLE_READ_SCHOOL)?;
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let workspace = reopening_workspace_in_transaction(&mut tx, actor, year).await?;
    tx.commit().await?;
    Ok(workspace)
}

pub(crate) async fn reopening_workspace_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    year: Uuid,
) -> Result<YearReopeningWorkspace, AppError> {
    let state = year_reopening::recovery_state(tx, year).await?;
    let (executed, executing): (i64, i64) = sqlx::query_as(
        "SELECT
         (SELECT count(*) FROM academic_promotion_execution_receipts WHERE source_year_id=$1),
         (SELECT count(*) FROM academic_promotion_runs WHERE source_year_id=$1 AND status='executing')",
    ).bind(year).fetch_one(&mut **tx).await?;
    let source_checksum = super::checksum(&(&state, executed, executing))?;
    let mut findings = Vec::new();
    let promotion_url = actor
        .has_permission(codes::ACADEMIC_PROMOTION_READ_SCHOOL)
        .then(|| format!("/staff/academic/promotion?academicYearId={year}"));
    for (code, count, message, url) in [
        (
            "year.reopen_state",
            i64::from(state.context.status != AcademicYearStatus::Closed),
            "เปิดกลับได้เฉพาะปีที่ปิดแล้ว และจะกลับสู่สถานะกำลังตรวจปิดปี",
            None,
        ),
        (
            "year.reopen_running",
            state.running_years + state.running_terms,
            "มีปีหรือภาคเรียนกำลังใช้งาน จึงยังเปิดปีเก่ากลับไม่ได้",
            None,
        ),
        (
            "year.reopen_successor",
            state.successor_years,
            "มีปีถัดไปที่เริ่มใช้งานแล้ว ให้ใช้การแก้ผลเฉพาะรายการแทน",
            None,
        ),
        (
            "year.reopen_promotion",
            executed,
            "มีการดำเนินการเลื่อนชั้นจากปีนี้แล้ว ต้องแก้ผลและตรวจผลกระทบเฉพาะรายการ",
            promotion_url.clone(),
        ),
        (
            "year.reopen_execution_pending",
            executing,
            "มีรอบเลื่อนชั้นกำลังดำเนินการ ต้องจัดการรอบนั้นก่อน",
            promotion_url,
        ),
    ] {
        if count > 0 {
            findings.push(LifecycleFinding {
                code: code.into(),
                severity: LifecycleSeverity::Blocking,
                count: usize::try_from(count)
                    .map_err(|_| AppError::InternalServerError("จำนวนข้อมูลอ้างอิงเกินขอบเขต".into()))?,
                message: message.into(),
                resolution_url: url,
            });
        }
    }
    findings.sort_by(|a, b| a.code.cmp(&b.code));
    Ok(YearReopeningWorkspace {
        context: state.context,
        can_reopen: findings.is_empty(),
        findings,
        source_checksum,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn year_reopening_inflight_execution_blocks_before_any_receipt_exists() {
        let (pool, manager, calc, run) =
            super::super::promotion_execution::tests::started_run_fixture(
                "year_reopening_inflight",
            )
            .await;
        let year = calc.run.source_year_id;
        super::super::year_reopening_tests::set_closed(&pool, year).await;
        let reader = ActorContext {
            user_id: manager.user_id,
            permissions: vec![codes::ACADEMIC_LIFECYCLE_READ_SCHOOL.into()],
        };
        let pending = get_year_reopening_workspace(&pool, &reader, year)
            .await
            .unwrap();
        assert!(!pending.can_reopen);
        assert!(pending
            .findings
            .iter()
            .any(|finding| finding.code == "year.reopen_execution_pending"));
        sqlx::query("UPDATE academic_promotion_runs SET status='failed',row_version=row_version+1 WHERE id=$1").bind(run.id).execute(&pool).await.unwrap();
        let failed = get_year_reopening_workspace(&pool, &reader, year)
            .await
            .unwrap();
        assert!(
            failed.can_reopen,
            "a failed attempt without committed items does not invent a completed dependency"
        );
        assert_ne!(failed.source_checksum, pending.source_checksum);
    }

    #[tokio::test]
    async fn year_reopening_completed_hold_preserves_a_blocking_dependency() {
        use crate::modules::academic::lifecycle::models::{
            ExecutePromotionRunInput, PromotionDecisionOutcome,
        };
        let (pool, executor, _, _, calc, approved) =
            super::super::promotion_execution::tests::approved_fixture(
                "year_reopening_executed",
                PromotionDecisionOutcome::Hold,
            )
            .await;
        super::super::execute_run(
            &pool,
            &executor,
            calc.run.id,
            ExecutePromotionRunInput {
                request_id: Uuid::new_v4(),
                row_version: approved.row_version,
                limit: 1,
            },
        )
        .await
        .unwrap();
        let year = calc.run.source_year_id;
        super::super::year_reopening_tests::set_closed(&pool, year).await;
        let reader = ActorContext {
            user_id: executor.user_id,
            permissions: vec![codes::ACADEMIC_LIFECYCLE_READ_SCHOOL.into()],
        };
        let workspace = get_year_reopening_workspace(&pool, &reader, year)
            .await
            .unwrap();
        assert!(!workspace.can_reopen);
        let finding = workspace
            .findings
            .iter()
            .find(|finding| finding.code == "year.reopen_promotion")
            .unwrap();
        assert_eq!(finding.count, 1);
        assert!(finding.resolution_url.is_none());
        let office = ActorContext {
            user_id: reader.user_id,
            permissions: vec![
                codes::ACADEMIC_LIFECYCLE_READ_SCHOOL.into(),
                codes::ACADEMIC_PROMOTION_READ_SCHOOL.into(),
            ],
        };
        let office_view = get_year_reopening_workspace(&pool, &office, year)
            .await
            .unwrap();
        assert_eq!(workspace.source_checksum, office_view.source_checksum);
        assert!(office_view
            .findings
            .iter()
            .find(|finding| finding.code == "year.reopen_promotion")
            .unwrap()
            .resolution_url
            .is_some());
    }
}
