use crate::models::auth::User;
use crate::models::staff::UserPermissions;

/// Database-driven permission check using UserPermissions trait
/// This is the recommended way to check permissions in handlers
/// 
/// # Example
/// ```rust
/// use crate::middleware::permission::check_user_permission;
/// 
/// async fn my_handler(user: User, pool: &PgPool) -> Result<Response> {
///     if !check_user_permission(&user, &pool, "staff.edit").await? {
///         return Err(StatusCode::FORBIDDEN);
///     }
///     // ... rest of handler
/// }
/// ```
pub async fn check_user_permission(
    user: &User,
    pool: &sqlx::PgPool,
    permission: &str,
) -> Result<bool, sqlx::Error> {
    // Use the UserPermissions trait
    user.has_permission(pool, permission).await
}
