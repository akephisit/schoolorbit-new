use super::super::models::{AcademicTermStatus, AcademicYearStatus};
use crate::error::AppError;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

// Two-int advisory keys occupy a different namespace from existing bigint entity locks.
pub(crate) const TRANSITION_NAMESPACE: i32 = 0x534F_4143;
pub(crate) const TRANSITION_KEY: i32 = 1;

#[derive(Debug, Clone, Copy, sqlx::FromRow)]
pub(crate) struct AcademicWriteState {
    pub year_status: AcademicYearStatus,
    pub term_status: AcademicTermStatus,
}

impl AcademicWriteState {
    pub fn is_writable(&self) -> bool {
        !matches!(
            self.year_status,
            AcademicYearStatus::Closed | AcademicYearStatus::Archived
        ) && !matches!(
            self.term_status,
            AcademicTermStatus::Closed | AcademicTermStatus::Cancelled
        )
    }

    pub fn require_writable(&self) -> Result<(), AppError> {
        if self.is_writable() {
            Ok(())
        } else {
            Err(AppError::Conflict(
                "ปีหรือภาคเรียนนี้ปิดแล้วหรือยกเลิกแล้ว ดูข้อมูลเดิมได้ แต่แก้ไขผ่านงานปกติไม่ได้".into(),
            ))
        }
    }
}

/// Source writers (including dedicated corrections) coordinate before entity locks.
pub(crate) async fn lock_transition_shared(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<(), AppError> {
    sqlx::query("SELECT pg_advisory_xact_lock_shared($1,$2)")
        .bind(TRANSITION_NAMESPACE)
        .bind(TRANSITION_KEY)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// Lifecycle takes this before readiness reads, never after accepting a snapshot.
#[cfg(test)]
pub(crate) async fn lock_transition(tx: &mut Transaction<'_, Postgres>) -> Result<(), AppError> {
    sqlx::query("SELECT pg_advisory_xact_lock($1,$2)")
        .bind(TRANSITION_NAMESPACE)
        .bind(TRANSITION_KEY)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// Lock ordering is tenant transition, year, term, then the caller's entities.
/// Read-with-lazy-initialization callers may inspect this state without creating
/// any missing configuration when the context is closed.
pub(crate) async fn lock_context(
    tx: &mut Transaction<'_, Postgres>,
    year: Uuid,
    term: Uuid,
) -> Result<AcademicWriteState, AppError> {
    lock_context_with_term_mode(tx, year, term, false).await
}

async fn lock_context_with_term_mode(
    tx: &mut Transaction<'_, Postgres>,
    year: Uuid,
    term: Uuid,
    exclusive_term: bool,
) -> Result<AcademicWriteState, AppError> {
    lock_transition_shared(tx).await?;
    let year_status = sqlx::query_scalar("SELECT status FROM academic_years WHERE id=$1 FOR SHARE")
        .bind(year)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบปีการศึกษา".into()))?;
    let term_query = if exclusive_term {
        "SELECT status FROM academic_terms WHERE id=$1 AND academic_year_id=$2 FOR UPDATE"
    } else {
        "SELECT status FROM academic_terms WHERE id=$1 AND academic_year_id=$2 FOR SHARE"
    };
    let term_status = sqlx::query_scalar(term_query)
        .bind(term)
        .bind(year)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(|| AppError::ValidationError("ภาคเรียนไม่อยู่ในปีการศึกษาที่เลือก".into()))?;
    Ok(AcademicWriteState {
        year_status,
        term_status,
    })
}

pub(crate) async fn require_term_write(
    tx: &mut Transaction<'_, Postgres>,
    year: Uuid,
    term: Uuid,
) -> Result<(), AppError> {
    lock_context(tx, year, term).await?.require_writable()
}

/// Acquire the write mode initially: never upgrade a shared term lock after
/// taking domain locks, since two concurrent upgrades can deadlock.
pub(crate) async fn require_term_write_exclusive(
    tx: &mut Transaction<'_, Postgres>,
    year: Uuid,
    term: Uuid,
) -> Result<(), AppError> {
    lock_context_with_term_mode(tx, year, term, true)
        .await?
        .require_writable()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closing_is_writable_but_closed_and_cancelled_are_not() {
        assert!(AcademicWriteState {
            year_status: AcademicYearStatus::Closing,
            term_status: AcademicTermStatus::Closing
        }
        .is_writable());
        for year_status in [AcademicYearStatus::Closed, AcademicYearStatus::Archived] {
            assert!(!AcademicWriteState {
                year_status,
                term_status: AcademicTermStatus::Active
            }
            .is_writable());
        }
        for term_status in [AcademicTermStatus::Closed, AcademicTermStatus::Cancelled] {
            assert!(!AcademicWriteState {
                year_status: AcademicYearStatus::Active,
                term_status
            }
            .is_writable());
        }
    }
}
