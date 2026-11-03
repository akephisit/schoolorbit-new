use crate::models::StudentRegistrationStatus;
use school_database::DbSession;
use school_errors::AppError;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

/// Lifecycle already authorized the school-wide declaration and owns the same
/// transaction. This provider owns the administrative assertion, never account
/// access, sessions or academic enrollment.
pub async fn apply_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    actor: Uuid,
    outcomes: &[(Uuid, StudentRegistrationStatus)],
) -> Result<(), AppError> {
    validate(outcomes)?;
    if outcomes.is_empty() {
        return Ok(());
    }
    let ids: Vec<_> = outcomes.iter().map(|row| row.0).collect();
    let statuses: Vec<_> = outcomes.iter().map(|row| row.1).collect();
    let updated = DbSession::new(&mut **tx).query(
        "students.registration_outcomes.apply_in_transaction",
        "UPDATE student_info info SET registration_status=outcome.status,
         registration_status_updated_at=CASE WHEN info.registration_status IS DISTINCT FROM outcome.status THEN now() ELSE info.registration_status_updated_at END,
         registration_status_updated_by=CASE WHEN info.registration_status IS DISTINCT FROM outcome.status THEN $3 ELSE info.registration_status_updated_by END
         FROM unnest($1::uuid[],$2::text[]) outcome(student_id,status)
         WHERE info.user_id=outcome.student_id",
    ).bind(ids).bind(statuses).bind(actor).execute().await?;
    if updated.rows_affected() != outcomes.len() as u64 {
        return Err(AppError::Conflict(
            "ข้อมูลทะเบียนนักเรียนไม่ครบ กรุณาตรวจสอบก่อนยืนยัน".into(),
        ));
    }
    Ok(())
}

fn validate(outcomes: &[(Uuid, StudentRegistrationStatus)]) -> Result<(), AppError> {
    if outcomes.len() > 10_000
        || outcomes.iter().any(|(id, status)| {
            id.is_nil()
                || !matches!(
                    status,
                    StudentRegistrationStatus::Graduated
                        | StudentRegistrationStatus::TransferredOut
                )
        })
        || outcomes
            .iter()
            .map(|row| row.0)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != outcomes.len()
    {
        return Err(AppError::ValidationError(
            "ผลทะเบียนนักเรียนไม่ถูกต้องหรือซ้ำ".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn registration_provider_only_accepts_unique_terminal_declarations() {
        let id = Uuid::new_v4();
        assert!(validate(&[(id, StudentRegistrationStatus::Graduated)]).is_ok());
        assert!(validate(&[(id, StudentRegistrationStatus::Studying)]).is_err());
        assert!(validate(&[
            (id, StudentRegistrationStatus::Graduated),
            (id, StudentRegistrationStatus::TransferredOut)
        ])
        .is_err());
    }
}
