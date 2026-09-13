use super::super::models::{
    AcademicYearStatus, StudentAcademicYearStatus, TermTransitionAction, TermTransitionOutcome,
    TermTransitionRequest,
};
use crate::{
    error::AppError,
    middleware::permission::ActorContext,
    modules::academic::lifecycle::{models::LifecycleSeverity, services as lifecycle_services},
    permissions::registry::codes,
};
use sqlx::{types::Json, PgPool};
use uuid::Uuid;

const ACTION: &str = "activate";

/// Atomically opens a ready term and, when it is the first term of a planning
/// year, the year plus its planned enrollment and currently eligible placements.
pub(crate) async fn activate_term(
    pool: &PgPool,
    actor: &ActorContext,
    term_id: Uuid,
    request: TermTransitionRequest,
) -> Result<TermTransitionOutcome, AppError> {
    actor.require_permission(codes::ACADEMIC_LIFECYCLE_READ_SCHOOL)?;
    actor.require_permission(codes::ACADEMIC_LIFECYCLE_ACTIVATE_SCHOOL)?;
    if request.action != TermTransitionAction::Activate {
        return Err(AppError::ValidationError(
            "คำสั่งนี้รองรับเฉพาะการเริ่มใช้ภาคเรียน".into(),
        ));
    }
    validate_activation_shape(term_id, &request)?;
    let request_checksum = lifecycle_services::checksum(&(actor.user_id, term_id, &request))?;
    let mut tx = pool.begin().await?;
    super::lifecycle_guard::lock_transition(&mut tx).await?;

    let receipt: Option<(Uuid, String, String, Json<TermTransitionOutcome>)> = sqlx::query_as(
        "SELECT actor_user_id,action,request_checksum,outcome
         FROM academic_term_transition_receipts WHERE request_id=$1",
    )
    .bind(request.request_id)
    .fetch_optional(&mut *tx)
    .await?;
    if let Some((owner, action, checksum, outcome)) = receipt {
        if owner != actor.user_id || action != ACTION || checksum != request_checksum {
            return Err(AppError::Conflict(
                "คำขอนี้เคยใช้กับข้อมูล การดำเนินการ หรือผู้ดำเนินการอื่นแล้ว".into(),
            ));
        }
        tx.commit().await?;
        return Ok(outcome.0);
    }

    // The tenant transition lock prevents every participating academic writer
    // from changing the discovered set before the ordered entity locks below.
    let discovered = super::activation_context::read_activation_state(
        &mut tx,
        request.academic_year_id,
        term_id,
    )
    .await?;
    let mut year_ids = vec![request.academic_year_id];
    if let Some(predecessor) = &discovered.predecessor {
        year_ids.push(predecessor.academic_year_id);
    }
    year_ids.sort_unstable();
    year_ids.dedup();
    let locked_years: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM academic_years WHERE id=ANY($1) ORDER BY id FOR UPDATE")
            .bind(&year_ids)
            .fetch_all(&mut *tx)
            .await?;
    if locked_years != year_ids {
        return Err(AppError::Conflict(
            "บริบทปีการศึกษาเปลี่ยนแล้ว กรุณาตรวจสอบใหม่".into(),
        ));
    }
    sqlx::query(
        "SELECT id FROM academic_terms WHERE academic_year_id=ANY($1)
         ORDER BY academic_year_id,sequence_no,id FOR UPDATE",
    )
    .bind(&year_ids)
    .fetch_all(&mut *tx)
    .await?;

    let (workspace, state) = lifecycle_services::activation_evidence_in_transaction(
        &mut tx,
        actor,
        request.academic_year_id,
        term_id,
    )
    .await?;
    super::term_transitions::validate_request(
        &request,
        workspace.context.year_start_date,
        workspace.context.year_end_date,
        workspace.context.term_start_date,
    )?;
    if workspace.context.year_row_version != request.expected_year_version
        || workspace.context.term_row_version != request.expected_term_version
        || workspace.source_checksum != request.readiness_checksum
    {
        return Err(AppError::Conflict(
            "ข้อมูลภาคเรียนหรือความพร้อมเปิดใช้เปลี่ยนแล้ว กรุณาตรวจสอบใหม่".into(),
        ));
    }
    if !workspace.can_activate
        || workspace
            .findings
            .iter()
            .any(|finding| finding.severity == LifecycleSeverity::Blocking)
    {
        return Err(AppError::Conflict("ยังมีข้อมูลที่ต้องแก้ไขก่อนเริ่มใช้ภาคเรียน".into()));
    }

    let mut activated_students = 0_u64;
    let mut activated_placements = 0_u64;
    if workspace.opens_year {
        let student_ids: Vec<_> = state
            .students
            .iter()
            .filter(|student| student.status == StudentAcademicYearStatus::Planned)
            .map(|student| student.id)
            .collect();
        let placement_ids: Vec<_> = state
            .placements
            .iter()
            .filter(|placement| placement.eligible && placement.reference_valid)
            .map(|placement| placement.id)
            .collect();
        activated_students = sqlx::query(
            "UPDATE student_academic_years SET status='active',row_version=row_version+1,updated_at=now()
             WHERE academic_year_id=$1 AND id=ANY($2) AND status='planned'",
        )
        .bind(request.academic_year_id)
        .bind(&student_ids)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        activated_placements = sqlx::query(
            "UPDATE homeroom_placements SET status='current',row_version=row_version+1,updated_at=now()
             WHERE academic_year_id=$1 AND id=ANY($2) AND status='planned'",
        )
        .bind(request.academic_year_id)
        .bind(&placement_ids)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if activated_students != student_ids.len() as u64
            || activated_placements != placement_ids.len() as u64
        {
            return Err(AppError::Conflict(
                "ข้อมูลนักเรียนหรือห้องประจำชั้นเปลี่ยนแล้ว กรุณาตรวจสอบใหม่".into(),
            ));
        }
    }

    let expected_year_statuses: &[AcademicYearStatus] = if workspace.opens_year {
        &[AcademicYearStatus::Planning, AcademicYearStatus::Ready]
    } else {
        &[AcademicYearStatus::Active]
    };
    let year_update = sqlx::query(
        "UPDATE academic_years SET status='active',row_version=row_version+1,updated_at=now()
         WHERE id=$1 AND row_version=$2 AND status=ANY($3)",
    )
    .bind(request.academic_year_id)
    .bind(request.expected_year_version)
    .bind(expected_year_statuses)
    .execute(&mut *tx)
    .await?;
    if year_update.rows_affected() != 1 {
        return Err(AppError::Conflict(
            "สถานะปีการศึกษาเปลี่ยนแล้ว กรุณาตรวจสอบใหม่".into(),
        ));
    }
    let term_update = sqlx::query(
        "UPDATE academic_terms SET status='active',closed_on=NULL,row_version=row_version+1,updated_at=now()
         WHERE id=$1 AND academic_year_id=$2 AND row_version=$3 AND status='ready'",
    )
    .bind(term_id)
    .bind(request.academic_year_id)
    .bind(request.expected_term_version)
    .execute(&mut *tx)
    .await?;
    if term_update.rows_affected() != 1 {
        return Err(AppError::Conflict(
            "สถานะภาคเรียนเปลี่ยนแล้ว กรุณาตรวจสอบใหม่".into(),
        ));
    }

    let outcome = TermTransitionOutcome {
        request_id: request.request_id,
        action: TermTransitionAction::Activate,
        context: super::term_transitions::read_context(&mut tx, request.academic_year_id, term_id)
            .await?,
        completed_at: chrono::Utc::now(),
    };
    sqlx::query(
        "INSERT INTO academic_term_transition_receipts
         (request_id,academic_year_id,academic_term_id,actor_user_id,action,request_checksum,accepted_readiness,outcome)
         VALUES($1,$2,$3,$4,'activate',$5,$6,$7)",
    )
    .bind(request.request_id)
    .bind(request.academic_year_id)
    .bind(term_id)
    .bind(actor.user_id)
    .bind(&request_checksum)
    .bind(Json(&workspace))
    .bind(Json(&outcome))
    .execute(&mut *tx)
    .await?;
    if workspace.opens_year {
        sqlx::query(
            "INSERT INTO academic_year_transition_receipts
             (request_id,academic_year_id,actor_user_id,action,request_checksum,accepted_readiness,outcome)
             VALUES($1,$2,$3,'activate',$4,$5,$6)",
        )
        .bind(request.request_id)
        .bind(request.academic_year_id)
        .bind(actor.user_id)
        .bind(&request_checksum)
        .bind(Json(&workspace))
        .bind(Json(&outcome))
        .execute(&mut *tx)
        .await?;
    }
    super::years_terms::append_audit(
        &mut tx,
        "academic.term.activate",
        "academic_term",
        term_id,
        Some(request.academic_year_id),
        Some(term_id),
        actor.user_id,
        serde_json::json!({
            "request": request,
            "outcome": outcome,
            "openedYear": workspace.opens_year,
            "activatedStudents": activated_students,
            "activatedPlacements": activated_placements,
        }),
    )
    .await?;
    tx.commit().await?;
    Ok(outcome)
}

fn validate_activation_shape(
    term_id: Uuid,
    request: &TermTransitionRequest,
) -> Result<(), AppError> {
    let checksum_is_valid = request.readiness_checksum.len() == 64
        && request
            .readiness_checksum
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
    if term_id.is_nil()
        || request.request_id.is_nil()
        || request.academic_year_id.is_nil()
        || request.expected_year_version <= 0
        || request.expected_term_version <= 0
        || !checksum_is_valid
    {
        return Err(AppError::ValidationError(
            "ข้อมูลอ้างอิงหรือรุ่นความพร้อมเปิดใช้ไม่ถูกต้อง".into(),
        ));
    }
    if request.closed_on.is_some()
        || request.reason.is_some()
        || !request.acknowledged_warning_codes.is_empty()
    {
        return Err(AppError::ValidationError(
            "การเปิดภาคเรียนไม่รับวันที่ปิด เหตุผล หรือการยืนยันคำเตือน".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> TermTransitionRequest {
        TermTransitionRequest {
            academic_year_id: Uuid::new_v4(),
            request_id: Uuid::new_v4(),
            action: TermTransitionAction::Activate,
            expected_year_version: 1,
            expected_term_version: 1,
            readiness_checksum: "a".repeat(64),
            acknowledged_warning_codes: vec![],
            closed_on: None,
            reason: None,
        }
    }

    #[test]
    fn activation_accepts_only_a_complete_opening_preview_reference() {
        let term = Uuid::new_v4();
        assert!(validate_activation_shape(term, &request()).is_ok());
        let mut invalid = request();
        invalid.reason = Some("unrelated".into());
        assert!(validate_activation_shape(term, &invalid).is_err());
        invalid = request();
        invalid.acknowledged_warning_codes = vec!["warning".into()];
        assert!(validate_activation_shape(term, &invalid).is_err());
        invalid = request();
        invalid.readiness_checksum = "A".repeat(64);
        assert!(validate_activation_shape(term, &invalid).is_err());
        assert!(validate_activation_shape(Uuid::nil(), &request()).is_err());
    }
}
