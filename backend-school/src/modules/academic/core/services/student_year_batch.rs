use crate::error::AppError;
use sqlx::{Postgres, Transaction};
use std::collections::BTreeSet;
use uuid::Uuid;

fn validate_ids(students: &[Uuid]) -> Result<(), AppError> {
    if students.len() > 500 || students.iter().collect::<BTreeSet<_>>().len() != students.len() {
        return Err(AppError::ValidationError(
            "Student batch must contain at most 500 distinct student-year IDs".into(),
        ));
    }
    Ok(())
}

/// Internal read provider: callers own authorization and the surrounding snapshot.
pub(crate) async fn validate_student_year_batch(
    tx: &mut Transaction<'_, Postgres>,
    year: Uuid,
    students: &[Uuid],
) -> Result<(), AppError> {
    validate_ids(students)?;
    if students.is_empty() {
        return Ok(());
    }
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM student_academic_years WHERE academic_year_id=$1 AND id=ANY($2)",
    )
    .bind(year)
    .bind(students)
    .fetch_one(&mut **tx)
    .await?;
    if count != students.len() as i64 {
        return Err(AppError::NotFound(
            "Student academic year not found in selected context".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn student_batch_bounds_are_checked_before_querying() {
        assert!(validate_ids(&[]).is_ok());
        let id = Uuid::new_v4();
        assert!(validate_ids(&[id]).is_ok());
        assert!(validate_ids(&[id, id]).is_err());
        let ids: Vec<_> = (0..501).map(|_| Uuid::new_v4()).collect();
        assert!(validate_ids(&ids[..500]).is_ok());
        assert!(validate_ids(&ids).is_err());
    }
}
