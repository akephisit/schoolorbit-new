pub(crate) async fn pending_term_work(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    year: uuid::Uuid,
    term: uuid::Uuid,
) -> Result<Vec<crate::modules::academic::lifecycle::models::PendingTermWork>, crate::error::AppError>
{
    Ok(sqlx::query_as(
        "SELECT id, updated_at::text || ':' || status AS revision, false AS blocks_closure
         FROM supervision_observations WHERE academic_year_id=$1 AND academic_term_id=$2
         AND status NOT IN ('completed','cancelled') ORDER BY id",
    )
    .bind(year)
    .bind(term)
    .fetch_all(&mut **tx)
    .await?)
}

use std::collections::BTreeSet;

use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::academic::core::services::lifecycle_guard;

pub(super) type SupervisionAcademicContext = (Uuid, Option<Uuid>);

fn ordered_contexts(contexts: &[SupervisionAcademicContext]) -> (Vec<Uuid>, Vec<(Uuid, Uuid)>) {
    let years: BTreeSet<_> = contexts.iter().map(|context| context.0).collect();
    let terms: BTreeSet<_> = contexts
        .iter()
        .filter_map(|(year, term)| term.map(|term| (*year, term)))
        .collect();
    (years.into_iter().collect(), terms.into_iter().collect())
}

pub(super) async fn require_context_writes(
    tx: &mut Transaction<'_, Postgres>,
    contexts: &[SupervisionAcademicContext],
) -> Result<(), AppError> {
    let (years, terms) = ordered_contexts(contexts);
    for year in years {
        lifecycle_guard::require_year_write_exclusive(tx, year).await?;
    }
    for (year, term) in terms {
        lifecycle_guard::require_term_write(tx, year, term).await?;
    }
    Ok(())
}

pub(super) async fn require_cycle_write(
    tx: &mut Transaction<'_, Postgres>,
    cycle: Uuid,
    target: Option<SupervisionAcademicContext>,
) -> Result<SupervisionAcademicContext, AppError> {
    lifecycle_guard::lock_transition_shared(tx).await?;
    let before: SupervisionAcademicContext = sqlx::query_as(
        "SELECT academic_year_id,academic_term_id FROM supervision_cycles WHERE id=$1",
    )
    .bind(cycle)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบรอบนิเทศ".into()))?;
    let contexts = target.map_or_else(|| vec![before], |target| vec![before, target]);
    require_context_writes(tx, &contexts).await?;
    let current: SupervisionAcademicContext = sqlx::query_as(
        "SELECT academic_year_id,academic_term_id FROM supervision_cycles WHERE id=$1 FOR UPDATE",
    )
    .bind(cycle)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบรอบนิเทศ".into()))?;
    if current != before {
        return Err(AppError::Conflict(
            "รอบนิเทศถูกย้ายปีหรือภาคเรียนแล้ว กรุณาโหลดข้อมูลใหม่".into(),
        ));
    }
    Ok(current)
}

pub(super) async fn require_observation_write(
    tx: &mut Transaction<'_, Postgres>,
    observation: Uuid,
) -> Result<(), AppError> {
    lifecycle_guard::lock_transition_shared(tx).await?;
    let before: (Uuid, Uuid, Uuid) = sqlx::query_as(
        "SELECT academic_year_id,academic_term_id,cycle_id FROM supervision_observations WHERE id=$1",
    )
    .bind(observation)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบรายการนิเทศ".into()))?;
    let cycle = require_cycle_write(tx, before.2, Some((before.0, Some(before.1)))).await?;
    if cycle.0 != before.0 || cycle.1.is_some_and(|term| term != before.1) {
        return Err(AppError::Conflict(
            "ปีหรือภาคเรียนของรายการนิเทศไม่ตรงกับรอบนิเทศ".into(),
        ));
    }
    let current: (Uuid, Uuid, Uuid) = sqlx::query_as(
        "SELECT academic_year_id,academic_term_id,cycle_id FROM supervision_observations WHERE id=$1 FOR UPDATE",
    )
    .bind(observation)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบรายการนิเทศ".into()))?;
    if current != before {
        return Err(AppError::Conflict(
            "รายการนิเทศเปลี่ยนแล้ว กรุณาโหลดข้อมูลใหม่".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_ordering_deduplicates_year_wide_and_term_scoped_owners() {
        let (years, terms) = ordered_contexts(&[
            (Uuid::from_u128(2), Some(Uuid::from_u128(12))),
            (Uuid::from_u128(1), None),
            (Uuid::from_u128(2), None),
            (Uuid::from_u128(1), Some(Uuid::from_u128(11))),
            (Uuid::from_u128(2), Some(Uuid::from_u128(12))),
        ]);
        assert_eq!(years, vec![Uuid::from_u128(1), Uuid::from_u128(2)]);
        assert_eq!(
            terms,
            vec![
                (Uuid::from_u128(1), Uuid::from_u128(11)),
                (Uuid::from_u128(2), Uuid::from_u128(12))
            ]
        );
    }
}
