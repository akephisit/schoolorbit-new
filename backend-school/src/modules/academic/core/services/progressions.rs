use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;

use super::super::models::{
    GradeProgressionInput, GradeProgressionSet, ReplaceGradeProgressionsRequest,
};
use super::parse_row_version;
use super::years_terms::append_audit;

pub async fn list(pool: &PgPool) -> Result<GradeProgressionSet, AppError> {
    let row_version: i64 =
        sqlx::query_scalar("SELECT row_version FROM grade_level_progression_sets WHERE id = 1")
            .fetch_one(pool)
            .await?;
    let progressions = sqlx::query_as(
        r#"
        SELECT id, from_grade_level_id, to_grade_level_id, transition_kind,
               curriculum_id, is_active, created_at, updated_at
        FROM grade_level_progressions
        ORDER BY from_grade_level_id, curriculum_id NULLS FIRST, transition_kind, to_grade_level_id
        "#,
    )
    .fetch_all(pool)
    .await?;
    Ok(GradeProgressionSet {
        row_version,
        progressions,
    })
}

pub async fn replace(
    pool: &PgPool,
    actor_user_id: Uuid,
    request: ReplaceGradeProgressionsRequest,
) -> Result<GradeProgressionSet, AppError> {
    parse_row_version(request.row_version)?;
    validate_input_duplicates(&request.progressions)?;
    let mut transaction = pool.begin().await?;
    let next_row_version: Option<i64> = sqlx::query_scalar(
        r#"
        UPDATE grade_level_progression_sets
        SET row_version = row_version + 1, updated_at = now()
        WHERE id = 1 AND row_version = $1
        RETURNING row_version
        "#,
    )
    .bind(request.row_version)
    .fetch_optional(&mut *transaction)
    .await?;
    let next_row_version = next_row_version
        .ok_or_else(|| AppError::Conflict("กฎการเลื่อนระดับชั้นถูกแก้ไขโดยผู้ใช้อื่นแล้ว".to_string()))?;
    for progression in &request.progressions {
        validate_scope(&mut transaction, progression).await?;
    }
    sqlx::query("DELETE FROM grade_level_progressions")
        .execute(&mut *transaction)
        .await?;
    for progression in request.progressions {
        sqlx::query(
            r#"
            INSERT INTO grade_level_progressions (
                id, from_grade_level_id, to_grade_level_id, transition_kind,
                curriculum_id, is_active
            ) VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(progression.from_grade_level_id)
        .bind(progression.to_grade_level_id)
        .bind(progression.transition_kind)
        .bind(progression.curriculum_id)
        .bind(progression.is_active)
        .execute(&mut *transaction)
        .await?;
    }
    append_audit(
        &mut transaction,
        "grade_progressions.replaced",
        "grade_progressions",
        Uuid::nil(),
        None,
        None,
        actor_user_id,
        serde_json::json!({"rowVersion": next_row_version}),
    )
    .await?;
    transaction.commit().await?;
    list(pool).await
}

fn validate_input_duplicates(progressions: &[GradeProgressionInput]) -> Result<(), AppError> {
    let mut seen = std::collections::HashSet::new();
    for progression in progressions {
        let key = (
            progression.from_grade_level_id,
            progression.to_grade_level_id,
            progression.transition_kind as u8,
            progression.curriculum_id,
        );
        if !seen.insert(key) {
            return Err(AppError::ValidationError("กฎการเลื่อนระดับชั้นซ้ำกัน".to_string()));
        }
        if progression.transition_kind == super::super::models::GradeProgressionKind::Graduate
            && progression.to_grade_level_id.is_some()
        {
            return Err(AppError::ValidationError(
                "กฎจบการศึกษาต้องไม่มีระดับชั้นปลายทาง".to_string(),
            ));
        }
    }
    Ok(())
}

async fn validate_scope(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    progression: &GradeProgressionInput,
) -> Result<(), AppError> {
    let grade_ids = [
        Some(progression.from_grade_level_id),
        progression.to_grade_level_id,
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    let grade_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM grade_levels WHERE id = ANY($1)")
            .bind(&grade_ids)
            .fetch_one(&mut **transaction)
            .await?;
    if grade_count != grade_ids.len() as i64 {
        return Err(AppError::ValidationError(
            "ระดับชั้นในกฎการเลื่อนไม่ถูกต้อง".to_string(),
        ));
    }
    if let Some(curriculum_id) = progression.curriculum_id {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM curricula WHERE id = $1 AND is_active IS TRUE)",
        )
        .bind(curriculum_id)
        .fetch_one(&mut **transaction)
        .await?;
        if !exists {
            return Err(AppError::ValidationError(
                "หลักสูตรในกฎการเลื่อนไม่ถูกต้อง".to_string(),
            ));
        }
    }
    Ok(())
}
