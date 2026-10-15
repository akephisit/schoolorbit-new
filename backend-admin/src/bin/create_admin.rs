//! Explicit central-operator bootstrap and cutover readiness. Never prints secrets.
use backend_admin::{
    db::init_admin_pool,
    portal::auth::{bootstrap_operator, consume_token, portal_origin, send_link},
};

#[tokio::main]
async fn main() -> Result<(), String> {
    dotenv::dotenv().ok();
    let url = std::env::var("ADMIN_MIGRATION_DATABASE_URL")
        .map_err(|_| "ADMIN_MIGRATION_DATABASE_URL is required")?;
    let pool = init_admin_pool(&url)
        .await
        .map_err(|_| "Bootstrap database unavailable")?;
    let mode = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "--bootstrap".into());
    match mode.as_str() {
        "--check-ready" => {
            portal_origin().map_err(|_| "Portal domain is not configured")?;
            for name in ["RESEND_API_KEY", "RESEND_FROM"] {
                if std::env::var(name).map_or(true, |value| value.trim().is_empty()) {
                    return Err(format!("{name} must be configured before account cutover"));
                }
            }
            let exists: bool =
                sqlx::query_scalar("SELECT to_regclass('platform_accounts') IS NOT NULL")
                    .fetch_one(&pool)
                    .await
                    .map_err(|_| "Account readiness unavailable")?;
            if !exists {
                return Err("Prepare the reviewed central-account cutover under maintenance before deploying; schools must remain preserved".into());
            }
            let ready: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM platform_accounts WHERE role='operator' AND verified_at IS NOT NULL)")
                .fetch_one(&pool).await.map_err(|_| "Operator readiness unavailable")?;
            if !ready {
                return Err("A verified operator is required before deployment".into());
            }
            println!("Central account cutover readiness passed.");
        }
        "--bootstrap" => {
            let email = std::env::var("ADMIN_BOOTSTRAP_EMAIL")
                .map_err(|_| "ADMIN_BOOTSTRAP_EMAIL is required")?;
            let password = std::env::var("ADMIN_BOOTSTRAP_PASSWORD")
                .map_err(|_| "ADMIN_BOOTSTRAP_PASSWORD is required")?;
            let name = std::env::var("ADMIN_BOOTSTRAP_NAME")
                .map_err(|_| "ADMIN_BOOTSTRAP_NAME is required")?;
            bootstrap_operator(&pool, &email, password, &name)
                .await
                .map_err(|_| "Operator bootstrap refused")?;
            send_link(&pool, &email, "verify").await.map_err(|_| {
                "Operator created but email delivery failed; use --resend-verification"
            })?;
            println!("Operator created; verify its email before deployment.");
        }
        "--resend-verification" => {
            let email = std::env::var("ADMIN_BOOTSTRAP_EMAIL")
                .map_err(|_| "ADMIN_BOOTSTRAP_EMAIL is required")?;
            let email = backend_admin::portal::auth::normalize_email(&email)
                .map_err(|_| "Invalid operator email")?;
            send_link(&pool, &email, "verify")
                .await
                .map_err(|_| "Verification delivery unavailable")?;
            println!("Verification delivery requested.");
        }
        "--verify-email" => {
            let token = std::env::var("ADMIN_BOOTSTRAP_TOKEN")
                .map_err(|_| "ADMIN_BOOTSTRAP_TOKEN is required")?;
            consume_token(&pool, &token, None)
                .await
                .map_err(|_| "Verification token refused")?;
            println!("Email verified.");
        }
        _ => return Err("Unsupported operator command".into()),
    }
    pool.close().await;
    Ok(())
}
