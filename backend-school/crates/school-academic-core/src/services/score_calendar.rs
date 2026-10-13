//! Academic calendar boundaries consumed by score-cycle scheduling.
use chrono::NaiveDate;
use school_errors::AppError;
use sqlx::PgConnection;

pub async fn reset_boundaries(
    connection: &mut PgConnection,
) -> Result<(Vec<NaiveDate>, Vec<NaiveDate>), AppError> {
    let terms: Vec<NaiveDate> = sqlx::query_scalar(
        "SELECT DISTINCT COALESCE(t.closed_on,t.planned_end_date,y.end_date) \
         FROM academic_terms t JOIN academic_years y ON y.id=t.academic_year_id \
         WHERE t.status IN('active','closing','closed') AND y.status IN('active','closing','closed','archived') ORDER BY 1 LIMIT 1001",
    )
    .fetch_all(&mut *connection)
    .await?;
    let years: Vec<NaiveDate> = sqlx::query_scalar(
        "SELECT DISTINCT end_date FROM academic_years WHERE status IN('active','closing','closed','archived') ORDER BY end_date LIMIT 1001",
    )
    .fetch_all(connection)
    .await?;
    validate_boundary_count(terms.len(), years.len())?;
    Ok((terms, years))
}

fn validate_boundary_count(terms: usize, years: usize) -> Result<(), AppError> {
    if terms > 1000 || years > 1000 {
        return Err(AppError::ValidationError(
            "ปฏิทินปีการศึกษามีขนาดเกินขอบเขตงานรีเซ็ตคะแนน".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn excessive_term_or_year_boundaries_are_not_silently_truncated() {
        assert!(validate_boundary_count(0, 0).is_ok());
        assert!(validate_boundary_count(1000, 1000).is_ok());
        assert!(validate_boundary_count(1001, 10).is_err());
        assert!(validate_boundary_count(10, 1001).is_err());
    }
}
