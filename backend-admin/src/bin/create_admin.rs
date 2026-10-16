//! Explicit central-operator bootstrap and cutover readiness. Never prints secrets.
use backend_admin::{
    db::init_admin_pool,
    portal::auth::{
        self, bootstrap_operator, consume_token, portal_origin, send_link, OperatorBootstrap,
    },
};

fn mail_settings() -> Result<(), String> {
    portal_origin().map_err(|_| "Portal domain is not configured")?;
    for name in ["RESEND_API_KEY", "RESEND_FROM"] {
        if std::env::var(name).map_or(true, |value| value.trim().is_empty()) {
            return Err(format!("{name} must be configured before account cutover"));
        }
    }
    Ok(())
}

fn bootstrap_settings() -> Result<OperatorBootstrap, String> {
    let email = std::env::var("ADMIN_BOOTSTRAP_EMAIL").map_err(|_| {
        "Configure the ADMIN_BOOTSTRAP_EMAIL Actions secret before the first release"
    })?;
    let password = std::env::var("ADMIN_BOOTSTRAP_PASSWORD").map_err(|_| {
        "Configure the ADMIN_BOOTSTRAP_PASSWORD Actions secret before the first release"
    })?;
    let name = std::env::var("ADMIN_BOOTSTRAP_NAME").unwrap_or_else(|_| "SchoolOrbit".into());
    OperatorBootstrap::new(&email, password, &name)
        .map_err(|_| "Invalid operator bootstrap settings".into())
}

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
        "--check-deploy-inputs" => {
            mail_settings()?;
            if !auth::has_verified_operator(&pool)
                .await
                .map_err(|_| "Operator readiness unavailable")?
            {
                auth::check_operator_bootstrap(&pool, &bootstrap_settings()?)
                    .await
                    .map_err(|_| "Operator bootstrap identity refused")?;
            }
            println!("Admin release inputs passed; no account changes made.");
        }
        "--prepare-release" => {
            mail_settings()?;
            if auth::has_verified_operator(&pool)
                .await
                .map_err(|_| "Operator readiness unavailable")?
            {
                println!("Verified operator preserved; no bootstrap required.");
            } else {
                let email = auth::ensure_operator_bootstrap(&pool, bootstrap_settings()?)
                    .await
                    .map_err(|_| "Operator bootstrap refused")?;
                send_link(&pool, &email, "verify")
                    .await
                    .map_err(|_| "Operator preserved but email delivery failed; retry Pipeline")?;
                println!("Operator prepared; confirm the verification email after this release opens the portal.");
            }
        }
        "--check-release-ready" => {
            mail_settings()?;
            if !auth::has_verified_operator(&pool)
                .await
                .map_err(|_| "Operator readiness unavailable")?
            {
                let input = bootstrap_settings()?;
                if !auth::pending_operator_ready(&pool, &input.email)
                    .await
                    .map_err(|_| "Operator readiness unavailable")?
                {
                    return Err("An operator and an active verification link are required before opening the portal".into());
                }
            }
            println!(
                "Admin release account readiness passed; login still requires verified email."
            );
        }
        "--check-ready" => {
            mail_settings()?;
            let exists: bool =
                sqlx::query_scalar("SELECT to_regclass('platform_accounts') IS NOT NULL")
                    .fetch_one(&pool)
                    .await
                    .map_err(|_| "Account readiness unavailable")?;
            if !exists {
                return Err(
                    "Account schema is not prepared; use the normal Pipeline release".into(),
                );
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
            println!("Operator created; verify its email before logging in.");
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
