use super::super::models::{
    AcademicYearStatus, YearLifecycleContext, YearTermLifecycleState, YearTransitionAction,
    YearTransitionOutcome, YearTransitionRequest,
};
use crate::{error::AppError, middleware::permission::ActorContext, permissions::registry::codes};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

pub(crate) async fn read_context(
    tx: &mut Transaction<'_, Postgres>,
    year: Uuid,
) -> Result<YearLifecycleContext, AppError> {
    sqlx::query_as("SELECT id AS academic_year_id,year,name,start_date,end_date,status,row_version FROM academic_years WHERE id=$1")
        .bind(year).fetch_optional(&mut **tx).await?.ok_or_else(||AppError::NotFound("ไม่พบปีการศึกษา".into()))
}

pub(crate) async fn read_terms(
    tx: &mut Transaction<'_, Postgres>,
    year: Uuid,
) -> Result<Vec<YearTermLifecycleState>, AppError> {
    let terms:Vec<YearTermLifecycleState>=sqlx::query_as("SELECT id AS academic_term_id,name,sequence_no AS sequence,status,included_in_year_result,blocks_year_closure,row_version FROM academic_terms WHERE academic_year_id=$1 ORDER BY sequence_no,id LIMIT 101")
        .bind(year).fetch_all(&mut **tx).await?;
    if terms.len() > 100 {
        return Err(AppError::ValidationError(
            "ปีการศึกษามีภาคเรียนเกิน 100 รายการ".into(),
        ));
    }
    Ok(terms)
}

pub(crate) fn action_permission(action: YearTransitionAction) -> &'static str {
    match action {
        YearTransitionAction::Close => codes::ACADEMIC_LIFECYCLE_CLOSE_SCHOOL,
        _ => codes::ACADEMIC_LIFECYCLE_MANAGE_SCHOOL,
    }
}

pub(crate) fn target_state(
    state: AcademicYearStatus,
    action: YearTransitionAction,
) -> Result<AcademicYearStatus, AppError> {
    use AcademicYearStatus as State;
    use YearTransitionAction as Action;
    match (state, action) {
        (State::Active, Action::BeginClosing) => Ok(State::Closing),
        (State::Closing, Action::CancelClosing) => Ok(State::Active),
        (State::Closing, Action::Close) => Ok(State::Closed),
        _ => Err(AppError::Conflict(
            "สถานะปีการศึกษาไม่รองรับการดำเนินการนี้".into(),
        )),
    }
}

pub async fn transition_year(
    pool: &PgPool,
    actor: &ActorContext,
    year: Uuid,
    input: YearTransitionRequest,
) -> Result<YearTransitionOutcome, AppError> {
    use crate::modules::academic::lifecycle::{models::LifecycleSeverity, services};
    use sqlx::types::Json;
    actor.require_permission(codes::ACADEMIC_LIFECYCLE_READ_SCHOOL)?;
    actor.require_permission(action_permission(input.action))?;
    if year.is_nil()
        || input.request_id.is_nil()
        || input.expected_year_version <= 0
        || input.readiness_checksum.len() != 64
        || !input
            .readiness_checksum
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return Err(AppError::ValidationError(
            "ข้อมูลอ้างอิงหรือรุ่นความพร้อมไม่ถูกต้อง".into(),
        ));
    }
    let action = match input.action {
        YearTransitionAction::BeginClosing => "begin_closing",
        YearTransitionAction::CancelClosing => "cancel_closing",
        YearTransitionAction::Close => "close",
    };
    let checksum = services::checksum(&(actor.user_id, year, &input))?;
    let mut tx = pool.begin().await?;
    super::lifecycle_guard::lock_transition(&mut tx).await?;
    if let Some(outcome) = super::year_commands::replay_year_command(
        &mut tx,
        input.request_id,
        actor.user_id,
        action,
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
    sqlx::query("SELECT id FROM academic_terms WHERE academic_year_id=$1 ORDER BY id FOR UPDATE")
        .bind(year)
        .fetch_all(&mut *tx)
        .await?;
    let workspace = services::year_workspace_in_transaction(&mut tx, actor, year).await?;
    if workspace.context.row_version != input.expected_year_version
        || workspace.source_checksum != input.readiness_checksum
    {
        return Err(AppError::Conflict(
            "ข้อมูลปีการศึกษาหรือความพร้อมเปลี่ยนแล้ว กรุณาตรวจสอบใหม่".into(),
        ));
    }
    let target = target_state(workspace.context.status, input.action)?;
    if input.action == YearTransitionAction::Close {
        if !workspace.can_close {
            return Err(AppError::Conflict("ยังมีข้อมูลที่ต้องแก้ไขก่อนปิดปีการศึกษา".into()));
        }
        let mut warnings: Vec<_> = workspace
            .findings
            .iter()
            .filter(|row| row.severity == LifecycleSeverity::Warning)
            .map(|row| row.code.clone())
            .collect();
        let mut acknowledged = input.acknowledged_warning_codes.clone();
        warnings.sort();
        acknowledged.sort();
        if warnings != acknowledged {
            return Err(AppError::Conflict(
                "ต้องยืนยันคำเตือนปัจจุบันให้ครบก่อนปิดปีการศึกษา".into(),
            ));
        }
    } else if !input.acknowledged_warning_codes.is_empty() {
        return Err(AppError::ValidationError(
            "การยืนยันคำเตือนใช้เฉพาะการปิดปีการศึกษา".into(),
        ));
    }
    sqlx::query("UPDATE academic_years SET status=$2,row_version=row_version+1,updated_at=now() WHERE id=$1").bind(year).bind(target).execute(&mut *tx).await?;
    let outcome = YearTransitionOutcome {
        request_id: input.request_id,
        action: input.action,
        context: read_context(&mut tx, year).await?,
        completed_at: chrono::Utc::now(),
    };
    sqlx::query("INSERT INTO academic_year_transition_receipts(request_id,academic_year_id,actor_user_id,action,request_checksum,accepted_readiness,outcome) VALUES($1,$2,$3,$4,$5,$6,$7)").bind(input.request_id).bind(year).bind(actor.user_id).bind(action).bind(checksum).bind(Json(&workspace)).bind(Json(&outcome)).execute(&mut *tx).await?;
    super::years_terms::append_audit(
        &mut tx,
        &format!("academic.year.{action}"),
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

#[cfg(test)]
mod tests {
    use super::*;
    use AcademicYearStatus as State;
    use YearTransitionAction as Action;

    #[test]
    fn year_lifecycle_state_table_only_allows_explicit_closure_actions() {
        for state in [
            State::Planning,
            State::Ready,
            State::Active,
            State::Closing,
            State::Closed,
            State::Archived,
        ] {
            for action in [Action::BeginClosing, Action::CancelClosing, Action::Close] {
                let wanted = match (state, action) {
                    (State::Active, Action::BeginClosing) => Some(State::Closing),
                    (State::Closing, Action::CancelClosing) => Some(State::Active),
                    (State::Closing, Action::Close) => Some(State::Closed),
                    _ => None,
                };
                assert_eq!(
                    target_state(state, action).ok(),
                    wanted,
                    "{state:?} {action:?}"
                );
            }
        }
    }
}
