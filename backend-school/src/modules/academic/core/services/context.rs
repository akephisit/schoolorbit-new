use sqlx::{types::Json, PgPool};
use uuid::Uuid;

use crate::error::AppError;

use super::super::models::{AcademicContextOptions, AcademicTermOption, AcademicYearOption};

pub async fn list_options(pool: &PgPool) -> Result<AcademicContextOptions, AppError> {
    let (years, terms, active_academic_year_id, active_academic_term_id): (
        Json<Vec<AcademicYearOption>>,
        Json<Vec<AcademicTermOption>>,
        Option<Uuid>,
        Option<Uuid>,
    ) = sqlx::query_as(
        r#"
        SELECT
            COALESCE((
                SELECT jsonb_agg(
                    jsonb_build_object(
                        'id', year.id,
                        'year', year.year,
                        'name', year.name,
                        'startDate', year.start_date,
                        'endDate', year.end_date,
                        'status', year.status
                    ) ORDER BY year.year DESC, year.start_date DESC, year.id
                )
                FROM academic_years year
            ), '[]'::jsonb),
            COALESCE((
                SELECT jsonb_agg(
                    jsonb_build_object(
                        'id', term.id,
                        'academicYearId', term.academic_year_id,
                        'sequence', term.sequence_no,
                        'code', term.code,
                        'name', term.name,
                        'termType', term.term_type,
                        'startDate', term.start_date,
                        'endDate', term.end_date,
                        'includedInYearResult', term.included_in_year_result,
                        'blocksYearClosure', term.blocks_year_closure,
                        'status', term.status
                    ) ORDER BY year.year DESC, term.sequence_no, term.start_date, term.id
                )
                FROM academic_terms term
                JOIN academic_years year ON year.id = term.academic_year_id
            ), '[]'::jsonb),
            (SELECT year.id FROM academic_years year WHERE year.status = 'active'),
            (SELECT term.id FROM academic_terms term WHERE term.status = 'active')
        "#,
    )
    .fetch_one(pool)
    .await
    .map_err(|error| {
        tracing::error!(reason = "academic_context_options_query_failed", database_error = %error);
        AppError::InternalServerError("ไม่สามารถโหลดตัวเลือกปีและภาคเรียนได้".to_string())
    })?;

    Ok(AcademicContextOptions {
        years: years.0,
        terms: terms.0,
        active_academic_year_id,
        active_academic_term_id,
    })
}
