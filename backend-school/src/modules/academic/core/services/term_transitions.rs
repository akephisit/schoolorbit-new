use super::super::models::TermTransitionRequest;
use super::super::models::{AcademicTermStatus, AcademicYearStatus, TermTransitionAction};
use super::super::models::{TermLifecycleContext, TermTransitionOutcome};
use crate::error::AppError;
use crate::{middleware::permission::ActorContext, permissions::registry::codes};
use chrono::NaiveDate;
use sqlx::{types::Json, Postgres, Transaction};
use uuid::Uuid;

pub(crate) fn action_permission(action: TermTransitionAction) -> &'static str {
    match action {
        TermTransitionAction::Close => codes::ACADEMIC_LIFECYCLE_CLOSE_SCHOOL,
        TermTransitionAction::Reopen => codes::ACADEMIC_LIFECYCLE_REOPEN_SCHOOL,
        TermTransitionAction::Activate => codes::ACADEMIC_LIFECYCLE_ACTIVATE_SCHOOL,
        _ => codes::ACADEMIC_LIFECYCLE_MANAGE_SCHOOL,
    }
}

pub(crate) async fn read_context(
    tx: &mut Transaction<'_, Postgres>,
    year: Uuid,
    term: Uuid,
) -> Result<TermLifecycleContext, AppError> {
    sqlx::query_as(
        "SELECT year.id AS academic_year_id,term.id AS academic_term_id,year.name AS year_name,
         term.name AS term_name,year.status AS year_status,term.status AS term_status,
         year.row_version AS year_row_version,term.row_version AS term_row_version,
         year.start_date AS year_start_date,year.end_date AS year_end_date,term.start_date AS term_start_date,
         term.planned_end_date,term.closed_on,term.sequence_no AS sequence,term.bell_schedule_id,
         term.included_in_year_result,term.blocks_year_closure
         FROM academic_terms term JOIN academic_years year ON year.id=term.academic_year_id
         WHERE year.id=$1 AND term.id=$2",
    ).bind(year).bind(term).fetch_optional(&mut **tx).await?
        .ok_or_else(|| AppError::NotFound("ไม่พบภาคเรียนในปีการศึกษาที่เลือก".into()))
}

pub async fn transition_term(
    pool: &sqlx::PgPool,
    actor: &ActorContext,
    term: Uuid,
    request: TermTransitionRequest,
) -> Result<TermTransitionOutcome, AppError> {
    if request.action == TermTransitionAction::Activate {
        return super::activation::activate_term(pool, actor, term, request).await;
    }
    use crate::modules::academic::lifecycle::{models::LifecycleSeverity, services};
    actor.require_permission(codes::ACADEMIC_LIFECYCLE_READ_SCHOOL)?;
    actor.require_permission(action_permission(request.action))?;
    let request_checksum = services::checksum(&(actor.user_id, term, &request))?;
    let mut tx = pool.begin().await?;
    // Acquire before any entity lock or readiness read. Ordinary writers use
    // the shared side of this same lock, including audited corrections.
    super::lifecycle_guard::lock_transition(&mut tx).await?;
    let receipt:Option<(Uuid,String,Json<TermTransitionOutcome>)>=sqlx::query_as(
        "SELECT actor_user_id,request_checksum,outcome FROM academic_term_transition_receipts WHERE request_id=$1",
    ).bind(request.request_id).fetch_optional(&mut *tx).await?;
    if let Some((owner, checksum, outcome)) = receipt {
        if owner != actor.user_id || checksum != request_checksum {
            return Err(AppError::Conflict(
                "คำขอนี้เคยใช้กับข้อมูลหรือผู้ดำเนินการอื่นแล้ว".into(),
            ));
        }
        tx.commit().await?;
        return Ok(outcome.0);
    }
    sqlx::query("SELECT id FROM academic_years WHERE id=$1 FOR UPDATE")
        .bind(request.academic_year_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบปีการศึกษา".into()))?;
    sqlx::query("SELECT id FROM academic_terms WHERE id=$1 AND academic_year_id=$2 FOR UPDATE")
        .bind(term)
        .bind(request.academic_year_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบภาคเรียนในปีการศึกษาที่เลือก".into()))?;
    let workspace =
        services::workspace_in_transaction(&mut tx, actor, request.academic_year_id, term).await?;
    let before = &workspace.context;
    validate_request(
        &request,
        before.year_start_date,
        before.year_end_date,
        before.term_start_date,
    )?;
    if before.year_row_version != request.expected_year_version
        || before.term_row_version != request.expected_term_version
        || workspace.source_checksum != request.readiness_checksum
    {
        return Err(AppError::Conflict(
            "ข้อมูลภาคเรียนหรือความพร้อมเปลี่ยนแล้ว กรุณาตรวจสอบใหม่".into(),
        ));
    }
    let target = target_state(before.term_status, before.year_status, request.action)?;
    if request.action == TermTransitionAction::Close {
        if !workspace.coverage.ready
            || workspace
                .findings
                .iter()
                .any(|finding| finding.severity == LifecycleSeverity::Blocking)
        {
            return Err(AppError::Conflict("ยังมีข้อมูลที่ต้องแก้ไขก่อนปิดภาคเรียน".into()));
        }
        let mut warnings: Vec<_> = workspace
            .findings
            .iter()
            .filter(|finding| finding.severity == LifecycleSeverity::Warning)
            .map(|finding| finding.code.clone())
            .collect();
        warnings.sort();
        let mut acknowledged = request.acknowledged_warning_codes.clone();
        acknowledged.sort();
        if warnings != acknowledged {
            return Err(AppError::Conflict(
                "ต้องยืนยันคำเตือนปัจจุบันให้ครบก่อนปิดภาคเรียน".into(),
            ));
        }
    } else if !request.acknowledged_warning_codes.is_empty() {
        return Err(AppError::ValidationError(
            "การยืนยันคำเตือนใช้เฉพาะการปิดภาคเรียน".into(),
        ));
    }
    if request.action == TermTransitionAction::Reopen {
        crate::modules::academic::results::services::require_term_without_annual_results(
            &mut tx,
            request.academic_year_id,
            term,
        )
        .await?;
        let successor:bool=sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM academic_terms successor JOIN academic_years owner ON owner.id=successor.academic_year_id
             WHERE successor.status IN ('active','closing','closed') AND
             ((successor.academic_year_id=$1 AND successor.sequence_no>$2) OR owner.start_date>$3))",
        ).bind(request.academic_year_id).bind(before.sequence).bind(before.year_start_date).fetch_one(&mut *tx).await?;
        let running:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM academic_terms WHERE id<>$1 AND status IN ('active','closing'))")
            .bind(term).fetch_one(&mut *tx).await?;
        if successor || running {
            return Err(AppError::Conflict(
                "เปิดกลับไม่ได้ เพราะมีภาคเรียนถัดไปเริ่มใช้แล้วหรือมีภาคเรียนอื่นกำลังใช้งาน".into(),
            ));
        }
    }
    sqlx::query("UPDATE academic_terms SET status=$2,closed_on=CASE WHEN $2='closed' THEN $3 WHEN $2='closing' THEN NULL ELSE closed_on END,row_version=row_version+1,updated_at=now() WHERE id=$1")
        .bind(term).bind(target).bind(request.closed_on).execute(&mut *tx).await?;
    sqlx::query("UPDATE academic_years SET row_version=row_version+1,updated_at=now() WHERE id=$1")
        .bind(request.academic_year_id)
        .execute(&mut *tx)
        .await?;
    let outcome = TermTransitionOutcome {
        request_id: request.request_id,
        action: request.action,
        context: read_context(&mut tx, request.academic_year_id, term).await?,
        completed_at: chrono::Utc::now(),
    };
    let action = match request.action {
        TermTransitionAction::MarkReady => "mark_ready",
        TermTransitionAction::BeginClosing => "begin_closing",
        TermTransitionAction::CancelClosing => "cancel_closing",
        TermTransitionAction::Close => "close",
        TermTransitionAction::Reopen => "reopen",
        TermTransitionAction::Cancel => "cancel",
        TermTransitionAction::Activate => "activate",
    };
    sqlx::query("INSERT INTO academic_term_transition_receipts(request_id,academic_year_id,academic_term_id,actor_user_id,action,request_checksum,accepted_readiness,outcome) VALUES($1,$2,$3,$4,$5,$6,$7,$8)")
        .bind(request.request_id).bind(request.academic_year_id).bind(term).bind(actor.user_id).bind(action)
        .bind(request_checksum).bind(Json(&workspace)).bind(Json(&outcome)).execute(&mut *tx).await?;
    super::years_terms::append_audit(
        &mut tx,
        &format!("academic.term.{action}"),
        "academic_term",
        term,
        Some(request.academic_year_id),
        Some(term),
        actor.user_id,
        (&request, &outcome),
    )
    .await?;
    tx.commit().await?;
    Ok(outcome)
}

pub(crate) fn validate_request(
    request: &TermTransitionRequest,
    year_start: NaiveDate,
    year_end: NaiveDate,
    term_start: NaiveDate,
) -> Result<(), AppError> {
    if request.request_id.is_nil()
        || request.academic_year_id.is_nil()
        || request.expected_year_version <= 0
        || request.expected_term_version <= 0
        || request.readiness_checksum.len() != 64
        || !request
            .readiness_checksum
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(AppError::ValidationError(
            "ข้อมูลอ้างอิงหรือรุ่นความพร้อมไม่ถูกต้อง".into(),
        ));
    }
    if request.action == TermTransitionAction::Close {
        let date = request
            .closed_on
            .ok_or_else(|| AppError::ValidationError("กรุณาระบุวันที่ปิดภาคเรียนจริง".into()))?;
        if date < term_start || date < year_start || date > year_end {
            return Err(AppError::ValidationError(
                "วันที่ปิดต้องไม่ก่อนเริ่มภาคเรียนและอยู่ในปีการศึกษา".into(),
            ));
        }
    } else if request.closed_on.is_some() {
        return Err(AppError::ValidationError("วันที่ปิดใช้เฉพาะการปิดภาคเรียน".into()));
    }
    if request
        .reason
        .as_ref()
        .is_some_and(|reason| reason.trim().is_empty() || reason.chars().count() > 1000)
        || (request.action == TermTransitionAction::Reopen && request.reason.is_none())
        || (request.action != TermTransitionAction::Reopen && request.reason.is_some())
    {
        return Err(AppError::ValidationError(
            "กรุณาระบุเหตุผลที่มีความยาวไม่เกิน 1000 ตัวอักษร".into(),
        ));
    }
    let unique: std::collections::BTreeSet<_> = request.acknowledged_warning_codes.iter().collect();
    if unique.len() != request.acknowledged_warning_codes.len() || unique.len() > 30 {
        return Err(AppError::ValidationError(
            "รายการยืนยันคำเตือนซ้ำหรือมากเกินไป".into(),
        ));
    }
    if request.action != TermTransitionAction::Close
        && !request.acknowledged_warning_codes.is_empty()
    {
        return Err(AppError::ValidationError(
            "การยืนยันคำเตือนใช้เฉพาะการปิดภาคเรียน".into(),
        ));
    }
    Ok(())
}

pub(crate) fn target_state(
    current: AcademicTermStatus,
    year: AcademicYearStatus,
    action: TermTransitionAction,
) -> Result<AcademicTermStatus, AppError> {
    use AcademicTermStatus as T;
    use AcademicYearStatus as Y;
    use TermTransitionAction as A;
    if matches!(year, Y::Closed | Y::Archived) {
        return Err(AppError::Conflict(
            "ปีการศึกษาปิดแล้ว ไม่สามารถเปลี่ยนสถานะภาคเรียนได้".into(),
        ));
    }
    match (current, action) {
        (T::Planning, A::MarkReady) => Ok(T::Ready),
        (T::Planning, A::Cancel) => Ok(T::Cancelled),
        (T::Active, A::BeginClosing) => Ok(T::Closing),
        (T::Closing, A::CancelClosing) => Ok(T::Active),
        (T::Closing, A::Close) => Ok(T::Closed),
        (T::Closed, A::Reopen) => Ok(T::Closing),
        _ => Err(AppError::Conflict(
            "สถานะภาคเรียนหรือปีการศึกษาไม่รองรับการดำเนินการนี้".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn term_transition_request_requires_precise_context_date_and_reopening_reason() {
        let start = NaiveDate::from_ymd_opt(2026, 5, 1).unwrap();
        let end = NaiveDate::from_ymd_opt(2027, 4, 30).unwrap();
        let term_start = NaiveDate::from_ymd_opt(2026, 5, 16).unwrap();
        let mut request = TermTransitionRequest {
            academic_year_id: uuid::Uuid::new_v4(),
            request_id: uuid::Uuid::new_v4(),
            action: TermTransitionAction::Close,
            expected_year_version: 1,
            expected_term_version: 2,
            readiness_checksum: "a".repeat(64),
            acknowledged_warning_codes: vec![],
            closed_on: Some(term_start),
            reason: None,
        };
        assert!(validate_request(&request, start, end, term_start).is_ok());
        for date in [None, Some(start), Some(end.succ_opt().unwrap())] {
            request.closed_on = date;
            assert!(validate_request(&request, start, end, term_start).is_err());
        }
        request.closed_on = Some(end);
        assert!(validate_request(&request, start, end, term_start).is_ok());
        request.action = TermTransitionAction::Reopen;
        request.closed_on = None;
        assert!(validate_request(&request, start, end, term_start).is_err());
        request.reason = Some("ตรวจสอบผลแก้ไข".into());
        assert!(validate_request(&request, start, end, term_start).is_ok());
        request.reason = Some(" ".into());
        assert!(validate_request(&request, start, end, term_start).is_err());
        request.reason = Some("ก".repeat(1001));
        assert!(validate_request(&request, start, end, term_start).is_err());
        request.action = TermTransitionAction::BeginClosing;
        request.reason = None;
        assert!(validate_request(&request, start, end, term_start).is_ok());
        request.expected_term_version = 0;
        assert!(validate_request(&request, start, end, term_start).is_err());
        request.expected_term_version = 1;
        request.expected_year_version = -1;
        assert!(validate_request(&request, start, end, term_start).is_err());
        request.expected_year_version = 1;
        request.readiness_checksum = "invalid".into();
        assert!(validate_request(&request, start, end, term_start).is_err());
        request.readiness_checksum = "a".repeat(64);
        request.request_id = uuid::Uuid::nil();
        assert!(validate_request(&request, start, end, term_start).is_err());
        request.request_id = uuid::Uuid::new_v4();
        request.action = TermTransitionAction::Activate;
        request.reason = Some("ไม่เกี่ยวกับการเปิดภาคเรียน".into());
        assert!(validate_request(&request, start, end, term_start).is_err());
        request.reason = None;
        request.acknowledged_warning_codes = vec!["opening.warning".into()];
        assert!(validate_request(&request, start, end, term_start).is_err());
    }

    #[test]
    fn term_transition_actions_have_exact_predecessors_and_never_write_closed_years() {
        use AcademicTermStatus as T;
        use AcademicYearStatus as Y;
        use TermTransitionAction as A;
        let transitions = [
            (T::Planning, A::MarkReady, T::Ready),
            (T::Planning, A::Cancel, T::Cancelled),
            (T::Active, A::BeginClosing, T::Closing),
            (T::Closing, A::CancelClosing, T::Active),
            (T::Closing, A::Close, T::Closed),
            (T::Closed, A::Reopen, T::Closing),
        ];
        for (source, action, target) in transitions {
            assert_eq!(target_state(source, Y::Active, action).unwrap(), target);
            for invalid in [
                T::Planning,
                T::Ready,
                T::Active,
                T::Closing,
                T::Closed,
                T::Cancelled,
            ] {
                if invalid != source {
                    assert!(target_state(invalid, Y::Active, action).is_err());
                }
            }
            for year in [Y::Closed, Y::Archived] {
                assert!(target_state(source, year, action).is_err());
            }
        }
        for year in [Y::Planning, Y::Ready, Y::Active, Y::Closing] {
            assert!(target_state(T::Ready, year, A::Activate).is_err());
        }
    }
}
