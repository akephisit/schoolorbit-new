use crate::error::AppError;
use crate::modules::system::models::ProvisionRequest;
use sqlx::postgres::PgPoolOptions;

pub struct ProvisionOutcome {
    pub school_id: String,
    pub admin_username: String,
}

fn teacher_username_from_sequence(sequence: i64) -> String {
    format!("T{sequence:04}")
}

pub async fn provision_tenant(payload: ProvisionRequest) -> Result<ProvisionOutcome, AppError> {
    tracing::info!(
        school_id = %payload.school_id,
        subdomain = %payload.subdomain,
        admin_username = ?payload.admin_username,
        "Provisioning tenant database"
    );

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(std::time::Duration::from_secs(30))
        .connect(&payload.db_connection_string)
        .await
        .map_err(|error| {
            tracing::error!("Failed to connect to tenant database: {}", error);
            AppError::InternalServerError(format!("Database connection failed: {}", error))
        })?;

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .map_err(|error| {
            tracing::error!("Tenant migration failed: {}", error);
            AppError::InternalServerError(format!("Migration failed: {}", error))
        })?;

    if let Err(error) = crate::utils::permission_sync::sync_permissions(&pool).await {
        tracing::warn!("Failed to sync permissions during provisioning: {}", error);
    }

    let admin_role_id = sqlx::query_scalar::<_, uuid::Uuid>(
        r#"
        SELECT id FROM roles WHERE code = 'ADMIN'
        "#,
    )
    .fetch_one(&pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to find admin role: {}", error);
        AppError::InternalServerError(format!(
            "Failed to find admin role (migrations may not have run): {}",
            error
        ))
    })?;

    let mut tx = pool.begin().await.map_err(|error| {
        tracing::error!("Failed to start provisioning transaction: {}", error);
        AppError::InternalServerError("Failed to start transaction".to_string())
    })?;

    let password_hash =
        bcrypt::hash(&payload.admin_password, bcrypt::DEFAULT_COST).map_err(|error| {
            tracing::error!("Password hashing failed during provisioning: {}", error);
            AppError::InternalServerError("Password hashing failed".to_string())
        })?;

    let next_num: i64 = sqlx::query_scalar(
        r#"SELECT MIN(n)::bigint FROM generate_series(1, 9999) AS n
           WHERE NOT EXISTS (
               SELECT 1 FROM users WHERE username = 'T' || LPAD(n::text, 4, '0')
           )"#,
    )
    .fetch_one(&pool)
    .await
    .unwrap_or(Some(1))
    .unwrap_or(1);
    let username = teacher_username_from_sequence(next_num);

    let user_id = sqlx::query_scalar::<_, uuid::Uuid>(
        r#"
        INSERT INTO users (username, national_id, national_id_hash, password_hash, title, first_name, last_name, user_type, status)
        VALUES ($1, NULL, NULL, $2, $3, $4, $5, $6, $7)
        ON CONFLICT (username) DO UPDATE SET 
            password_hash = EXCLUDED.password_hash
        RETURNING id
        "#,
    )
    .bind(&username)
    .bind(&password_hash)
    .bind(&payload.admin_title)
    .bind(&payload.admin_first_name)
    .bind(&payload.admin_last_name)
    .bind("staff")
    .bind("active")
    .fetch_one(&mut *tx)
    .await
    .map_err(|error| {
        tracing::error!("Failed to create admin user: {}", error);
        AppError::InternalServerError(format!("Failed to create admin user: {}", error))
    })?;

    if let Err(error) = sqlx::query(
        r#"
        INSERT INTO user_roles (user_id, role_id, is_primary, started_at)
        VALUES ($1, $2, $3, CURRENT_DATE)
        ON CONFLICT (user_id, role_id, organization_unit_id, started_at) DO NOTHING
        "#,
    )
    .bind(user_id)
    .bind(admin_role_id)
    .bind(true)
    .execute(&mut *tx)
    .await
    {
        tracing::warn!("Failed to assign admin role: {}", error);
        if let Err(rollback_error) = tx.rollback().await {
            tracing::warn!(
                "Provisioning transaction rollback failed: {}",
                rollback_error
            );
        }
        return Err(AppError::InternalServerError(format!(
            "Failed to assign admin role: {}",
            error
        )));
    }

    tx.commit().await.map_err(|error| {
        tracing::error!("Failed to commit provisioning transaction: {}", error);
        AppError::InternalServerError("Failed to commit transaction".to_string())
    })?;

    tracing::info!(
        school_id = %payload.school_id,
        admin_username = %username,
        "Tenant provisioning completed"
    );

    Ok(ProvisionOutcome {
        school_id: payload.school_id,
        admin_username: username,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn teacher_username_from_sequence_pads_to_four_digits() {
        assert_eq!(teacher_username_from_sequence(1), "T0001");
        assert_eq!(teacher_username_from_sequence(42), "T0042");
        assert_eq!(teacher_username_from_sequence(9999), "T9999");
    }

    #[test]
    fn teacher_username_from_sequence_keeps_longer_numbers() {
        assert_eq!(teacher_username_from_sequence(12000), "T12000");
    }
}
