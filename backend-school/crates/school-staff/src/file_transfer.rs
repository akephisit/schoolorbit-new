//! Ephemeral staff discovery, explicit pairing and host-only WebRTC signaling.
pub mod http;
pub mod models;
pub mod runtime;

/// Resolve current active staff identity using the canonical display-name projection.
pub async fn staff_display_name(
    pool: &sqlx::PgPool,
    user_id: uuid::Uuid,
) -> Result<String, school_errors::AppError> {
    school_database::DbSession::new(&mut *school_tenancy::TenantPool::observed_checkout_pool(pool, "staff.file_transfer.staff_display_name").await?)
        .scalar("staff.file_transfer.staff_display_name", "SELECT display_name FROM user_display_names WHERE id=$1 AND user_type='staff' AND status='active'")
        .bind(user_id).fetch_optional().await?
        .ok_or_else(|| school_errors::AppError::Forbidden("Active staff account required".into()))
}
