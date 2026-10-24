use crate::models::FeatureToggle;
use school_errors::AppError;
use sqlx::PgPool;
use uuid::Uuid;

pub async fn list_features(pool: &PgPool) -> Result<Vec<FeatureToggle>, AppError> {
    let mut connection =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "navigation.list_features")
            .await?;
    list_features_on_connection(&mut connection).await
}

pub async fn list_features_on_connection(
    connection: &mut sqlx::PgConnection,
) -> Result<Vec<FeatureToggle>, AppError> {
    school_database::DbSession::new(&mut *connection).query_as::<FeatureToggle>("navigation.services.feature_toggle_service.list_features_on_connection",
        "SELECT id, code, name, name_en, module, is_enabled FROM feature_toggles ORDER BY module, name"
    )
    .fetch_all().await
    .map_err(|e| AppError::InternalServerError(format!("Failed to fetch features: {}", e)))
}

pub async fn get_feature(pool: &PgPool, id: Uuid) -> Result<FeatureToggle, AppError> {
    let mut connection =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "navigation.get_feature").await?;
    get_feature_on_connection(&mut connection, id).await
}

pub async fn get_feature_on_connection(
    connection: &mut sqlx::PgConnection,
    id: Uuid,
) -> Result<FeatureToggle, AppError> {
    school_database::DbSession::new(&mut *connection)
        .query_as::<FeatureToggle>(
            "navigation.services.feature_toggle_service.get_feature_on_connection",
            "SELECT id, code, name, name_en, module, is_enabled FROM feature_toggles WHERE id = $1",
        )
        .bind(id)
        .fetch_optional()
        .await
        .map_err(|e| AppError::InternalServerError(format!("Database error: {}", e)))?
        .ok_or(AppError::NotFound("Feature toggle not found".to_string()))
}

pub async fn update_feature(
    pool: &PgPool,
    id: Uuid,
    is_enabled: Option<bool>,
) -> Result<FeatureToggle, AppError> {
    let mut connection =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "navigation.update_feature")
            .await?;
    update_feature_on_connection(&mut connection, id, is_enabled).await
}

pub async fn update_feature_on_connection(
    connection: &mut sqlx::PgConnection,
    id: Uuid,
    is_enabled: Option<bool>,
) -> Result<FeatureToggle, AppError> {
    let mut query = feature_update_set_clause(is_enabled);
    query.push_str(" WHERE id = $1 RETURNING id, code, name, name_en, module, is_enabled");

    school_database::DbSession::new(&mut *connection)
        .query_as::<FeatureToggle>(
            "navigation.services.feature_toggle_service.update_feature_on_connection",
            sqlx::AssertSqlSafe(query),
        )
        .bind(id)
        .fetch_optional()
        .await
        .map_err(|e| AppError::InternalServerError(format!("Failed to update feature: {}", e)))?
        .ok_or(AppError::NotFound("Feature toggle not found".to_string()))
}

pub async fn toggle_feature(pool: &PgPool, id: Uuid) -> Result<FeatureToggle, AppError> {
    let mut connection =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "navigation.toggle_feature")
            .await?;
    toggle_feature_on_connection(&mut connection, id).await
}

pub async fn toggle_feature_on_connection(
    connection: &mut sqlx::PgConnection,
    id: Uuid,
) -> Result<FeatureToggle, AppError> {
    school_database::DbSession::new(&mut *connection)
        .query_as::<FeatureToggle>(
            "navigation.services.feature_toggle_service.toggle_feature_on_connection",
            "UPDATE feature_toggles SET is_enabled = NOT is_enabled, updated_at = NOW()
         WHERE id = $1 RETURNING id, code, name, name_en, module, is_enabled",
        )
        .bind(id)
        .fetch_optional()
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
