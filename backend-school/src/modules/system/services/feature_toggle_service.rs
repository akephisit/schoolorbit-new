use crate::error::AppError;
use crate::modules::menu::models::FeatureToggle;
use sqlx::PgPool;
use uuid::Uuid;

pub async fn list_features(pool: &PgPool) -> Result<Vec<FeatureToggle>, AppError> {
    sqlx::query_as::<_, FeatureToggle>(
        "SELECT id, code, name, name_en, module, is_enabled FROM feature_toggles ORDER BY module, name"
    )
    .fetch_all(pool).await
    .map_err(|e| AppError::InternalServerError(format!("Failed to fetch features: {}", e)))
}

pub async fn get_feature(pool: &PgPool, id: Uuid) -> Result<FeatureToggle, AppError> {
    sqlx::query_as::<_, FeatureToggle>(
        "SELECT id, code, name, name_en, module, is_enabled FROM feature_toggles WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::InternalServerError(format!("Database error: {}", e)))?
    .ok_or(AppError::NotFound("Feature toggle not found".to_string()))
}

pub async fn update_feature(
    pool: &PgPool,
    id: Uuid,
    is_enabled: Option<bool>,
) -> Result<FeatureToggle, AppError> {
    let mut query = feature_update_set_clause(is_enabled);
    query.push_str(" WHERE id = $1 RETURNING id, code, name, name_en, module, is_enabled");

    sqlx::query_as::<_, FeatureToggle>(&query)
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(|e| AppError::InternalServerError(format!("Failed to update feature: {}", e)))?
        .ok_or(AppError::NotFound("Feature toggle not found".to_string()))
}

pub async fn toggle_feature(pool: &PgPool, id: Uuid) -> Result<FeatureToggle, AppError> {
    sqlx::query_as::<_, FeatureToggle>(
        "UPDATE feature_toggles SET is_enabled = NOT is_enabled, updated_at = NOW()
         WHERE id = $1 RETURNING id, code, name, name_en, module, is_enabled",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::InternalServerError(format!("Failed to toggle feature: {}", e)))?
    .ok_or(AppError::NotFound("Feature toggle not found".to_string()))
}

fn feature_update_set_clause(is_enabled: Option<bool>) -> String {
    let mut query = String::from("UPDATE feature_toggles SET updated_at = NOW()");
    if let Some(enabled) = is_enabled {
        query.push_str(&format!(", is_enabled = {}", enabled));
    }
    query
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feature_update_set_clause_only_updates_timestamp_when_enabled_is_absent() {
        assert_eq!(
            feature_update_set_clause(None),
            "UPDATE feature_toggles SET updated_at = NOW()"
        );
    }

    #[test]
    fn feature_update_set_clause_includes_explicit_enabled_value() {
        assert_eq!(
            feature_update_set_clause(Some(true)),
            "UPDATE feature_toggles SET updated_at = NOW(), is_enabled = true"
        );
        assert_eq!(
            feature_update_set_clause(Some(false)),
            "UPDATE feature_toggles SET updated_at = NOW(), is_enabled = false"
        );
    }
}
