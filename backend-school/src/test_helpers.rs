use sqlx::{postgres::PgPoolOptions, PgPool};
use std::env;
use std::process;
use std::sync::OnceLock;

static SCHEMA_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
static MIGRATION_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
static TEST_SCHEMA: OnceLock<String> = OnceLock::new();
static TEST_SCHEMA_READY: OnceLock<()> = OnceLock::new();

fn test_schema_name(process_id: u32) -> String {
    format!("schoolorbit_test_{process_id}")
}

fn set_search_path_sql(schema: &str) -> String {
    format!(r#"SET search_path TO "{schema}", public"#)
}

fn explicit_test_database_url() -> String {
    dotenvy::dotenv().ok();
    let database_url = env::var("TEST_DATABASE_URL")
        .expect("TEST_DATABASE_URL must be set for database-backed backend tests");

    direct_test_database_url(database_url)
}

fn direct_test_database_url(database_url: String) -> String {
    let scheme_end = database_url.find("://").map(|index| index + 3).unwrap_or(0);
    let authority_start = database_url[scheme_end..]
        .rfind('@')
        .map(|index| scheme_end + index + 1)
        .unwrap_or(scheme_end);
    let authority_end = database_url[authority_start..]
        .find(['/', '?'])
        .map(|index| authority_start + index)
        .unwrap_or(database_url.len());

    let authority = &database_url[authority_start..authority_end];
    if !authority.contains("-pooler.") {
        return database_url;
    }

    let direct_authority = authority.replace("-pooler.", ".");
    let mut direct_url = database_url;
    direct_url.replace_range(authority_start..authority_end, &direct_authority);
    direct_url
}

fn shared_test_schema() -> String {
    TEST_SCHEMA
        .get_or_init(|| test_schema_name(process::id()))
        .clone()
}

async fn ensure_test_schema(database_url: &str, schema: &str) {
    if TEST_SCHEMA_READY.get().is_some() {
        return;
    }

    let _guard = SCHEMA_LOCK.lock().await;
    if TEST_SCHEMA_READY.get().is_some() {
        return;
    }

    let admin_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(database_url)
        .await
        .expect("Failed to connect to TEST_DATABASE_URL for test schema setup");

    sqlx::query(&format!(r#"DROP SCHEMA IF EXISTS "{schema}" CASCADE"#))
        .execute(&admin_pool)
        .await
        .expect("Failed to reset isolated test schema");

    sqlx::query(&format!(r#"CREATE SCHEMA IF NOT EXISTS "{schema}""#))
        .execute(&admin_pool)
        .await
        .expect("Failed to create isolated test schema");

    admin_pool.close().await;
    let _ = TEST_SCHEMA_READY.set(());
}

/// Create a test database pool
pub async fn create_test_pool() -> PgPool {
    let database_url = explicit_test_database_url();
    let schema = shared_test_schema();
    ensure_test_schema(&database_url, &schema).await;
    let search_path_sql = set_search_path_sql(&schema);

    PgPoolOptions::new()
        .max_connections(5)
        .after_connect(move |connection, _metadata| {
            let search_path_sql = search_path_sql.clone();
            Box::pin(async move {
                sqlx::query(&search_path_sql).execute(connection).await?;
                Ok(())
            })
        })
        .connect(&database_url)
        .await
        .expect("Failed to connect to test database")
}

/// Run migrations on test database
pub async fn run_test_migrations(pool: &PgPool) {
    let _guard = MIGRATION_LOCK.lock().await;
    crate::db::migration::run_tenant_migrations(pool)
        .await
        .expect("Failed to run migrations");
}

/// Clean up test data
pub async fn cleanup_test_data(pool: &PgPool) {
    // Delete in reverse order of dependencies
    sqlx::query("DELETE FROM user_roles")
        .execute(pool)
        .await
        .ok();

    sqlx::query("DELETE FROM organization_permission_delegations")
        .execute(pool)
        .await
        .ok();

    sqlx::query("DELETE FROM organization_permission_grants")
        .execute(pool)
        .await
        .ok();

    sqlx::query("DELETE FROM organization_members")
        .execute(pool)
        .await
        .ok();

    sqlx::query("DELETE FROM users WHERE email LIKE '%test%'")
        .execute(pool)
        .await
        .ok();

    sqlx::query("DELETE FROM roles WHERE name LIKE '%test%'")
        .execute(pool)
        .await
        .ok();

    sqlx::query("DELETE FROM organization_units WHERE name LIKE '%test%'")
        .execute(pool)
        .await
        .ok();
}

/// Create a test user
pub async fn create_test_user(
    pool: &PgPool,
    email: &str,
    password: &str,
) -> Result<uuid::Uuid, sqlx::Error> {
    let password_hash = bcrypt::hash(password, 10).unwrap();

    let user_id: uuid::Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO users (username, email, password_hash, first_name, last_name, user_type, status)
        VALUES ($1, $1, $2, 'Test', 'User', 'staff', 'active')
        RETURNING id
        "#,
    )
    .bind(email)
    .bind(password_hash)
    .fetch_one(pool)
    .await?;

    Ok(user_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_name_is_stable_and_identifier_safe_for_process() {
        let schema = test_schema_name(12345);

        assert_eq!(schema, "schoolorbit_test_12345");
        assert!(schema
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_'));
    }

    #[test]
    fn search_path_sql_targets_isolated_schema_then_public() {
        assert_eq!(
            set_search_path_sql("schoolorbit_test_12345"),
            r#"SET search_path TO "schoolorbit_test_12345", public"#
        );
    }

    #[test]
    fn direct_test_database_url_removes_neon_pooler_host_marker() {
        let database_url = "postgresql://user:pass@ep-example-pooler.ap-southeast-1.aws.neon.tech/db?sslmode=require"
            .to_string();

        assert_eq!(
            direct_test_database_url(database_url),
            "postgresql://user:pass@ep-example.ap-southeast-1.aws.neon.tech/db?sslmode=require"
        );
    }

    #[test]
    fn direct_test_database_url_keeps_non_pooler_urls() {
        let database_url = "postgresql://user:pass@localhost/schoolorbit_test".to_string();

        assert_eq!(direct_test_database_url(database_url.clone()), database_url);
    }

    #[test]
    fn direct_test_database_url_only_rewrites_authority() {
        let database_url =
            "postgresql://user:pass-pooler.marker@ep-example-pooler.aws.neon.tech/db?tag=-pooler."
                .to_string();

        assert_eq!(
            direct_test_database_url(database_url),
            "postgresql://user:pass-pooler.marker@ep-example.aws.neon.tech/db?tag=-pooler."
        );
    }
}
