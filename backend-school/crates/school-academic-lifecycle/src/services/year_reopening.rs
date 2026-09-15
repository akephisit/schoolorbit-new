use crate::models::{LifecycleFinding, LifecycleSeverity, YearReopeningWorkspace};
use school_academic_core::{models::AcademicYearStatus, services::lifecycle_context};
use school_authorization::ActorContext;
use school_errors::AppError;
use school_permissions::registry::codes;
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
    let state = lifecycle_context::read_year_recovery_state(tx, year).await?;
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
