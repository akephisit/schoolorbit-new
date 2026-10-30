use school_database::DbSession;
use school_errors::AppError;
use sqlx::{types::Json, PgConnection, PgPool};
use uuid::Uuid;

use crate::models::{AcademicContextOptions, AcademicTermOption, AcademicYearOption};

pub async fn list_options(pool: &PgPool) -> Result<AcademicContextOptions, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_core.services.context.list_options",
    )
    .await?;
    list_options_on_connection(&mut connection).await
}

pub async fn list_options_on_connection(
    connection: &mut PgConnection,
) -> Result<AcademicContextOptions, AppError> {
    scoped_options(connection, 0, None).await
}

pub async fn list_public_options(pool: &PgPool) -> Result<AcademicContextOptions, AppError> {
    list_options(pool).await
}

pub async fn list_public_options_on_connection(
    connection: &mut PgConnection,
) -> Result<AcademicContextOptions, AppError> {
    list_options_on_connection(connection).await
}

pub async fn list_options_for_student(
    pool: &PgPool,
    student_id: Uuid,
) -> Result<AcademicContextOptions, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_core.services.context.list_options_for_student",
    )
    .await?;
    list_options_for_student_on_connection(&mut connection, student_id).await
}

pub async fn list_options_for_student_on_connection(
    connection: &mut PgConnection,
    student_id: Uuid,
) -> Result<AcademicContextOptions, AppError> {
    scoped_options(connection, 1, Some(student_id)).await
}

pub async fn list_options_for_parent(
    pool: &PgPool,
    parent_id: Uuid,
) -> Result<AcademicContextOptions, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_core.services.context.list_options_for_parent",
    )
    .await?;
    list_options_for_parent_on_connection(&mut connection, parent_id).await
}

pub async fn list_options_for_parent_on_connection(
    connection: &mut PgConnection,
    parent_id: Uuid,
) -> Result<AcademicContextOptions, AppError> {
    scoped_options(connection, 2, Some(parent_id)).await
}

// One statement keeps visibility, relationships and defaults in the same snapshot.
// Scope values are internal constants; request IDs never select their own access scope.
async fn scoped_options(
    connection: &mut PgConnection,
    scope: i16,
    actor: Option<Uuid>,
) -> Result<AcademicContextOptions, AppError> {
    let (years, terms, default_academic_year_id, default_academic_term_id):
        (Json<Vec<AcademicYearOption>>, Json<Vec<AcademicTermOption>>, Option<Uuid>, Option<Uuid>) =
        DbSession::new(connection).query_as("academic_core.services.context.scoped_options", r#"
        WITH allowed_years AS (
            SELECT year.* FROM academic_years year
            WHERE year.is_visible AND ($1::smallint=0 OR EXISTS (
                SELECT 1 FROM student_academic_years enrollment
                WHERE enrollment.academic_year_id=year.id AND (
                    ($1=1 AND enrollment.student_id=$2) OR ($1=2 AND EXISTS (
                        SELECT 1 FROM student_parents link JOIN users student
                          ON student.id=link.student_user_id AND student.user_type='student' AND student.status='active'
                        WHERE link.parent_user_id=$2 AND link.student_user_id=enrollment.student_id
                    ))
                )
            ))
        ), allowed_terms AS (
            SELECT term.* FROM academic_terms term JOIN allowed_years year ON year.id=term.academic_year_id
            WHERE term.is_visible
        )
        SELECT
          COALESCE((SELECT jsonb_agg(jsonb_build_object(
            'id',year.id,'year',year.year,'name',year.name,'isVisible',year.is_visible,'status',year.status
          ) ORDER BY year.year DESC,year.id) FROM allowed_years year),'[]'::jsonb),
          COALESCE((SELECT jsonb_agg(jsonb_build_object(
            'id',term.id,'academicYearId',term.academic_year_id,'sequence',term.sequence_no,
            'code',term.code,'name',term.name,'termType',term.term_type,'isVisible',term.is_visible,
            'includedInYearResult',term.included_in_year_result,'blocksYearClosure',term.blocks_year_closure,'status',term.status
          ) ORDER BY year.year DESC,term.sequence_no,term.id) FROM allowed_terms term
            JOIN allowed_years year ON year.id=term.academic_year_id),'[]'::jsonb),
          (SELECT year.id FROM allowed_years year JOIN academic_context_settings settings
             ON settings.default_academic_year_id=year.id WHERE settings.singleton),
          (SELECT term.id FROM allowed_terms term JOIN academic_context_settings settings
             ON settings.default_academic_term_id=term.id AND settings.default_academic_year_id=term.academic_year_id
             WHERE settings.singleton)
        "#).bind(scope).bind(actor).fetch_one().await?;
    Ok(AcademicContextOptions {
        years: years.0,
        terms: terms.0,
        default_academic_year_id,
        default_academic_term_id,
    })
}
