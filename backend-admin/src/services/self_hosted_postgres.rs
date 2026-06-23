use sqlx::postgres::PgPoolOptions;
use std::env;
use std::time::Duration;

#[derive(Clone, PartialEq, Eq)]
pub struct SelfHostedPostgresConfig {
    pub admin_url: String,
    pub app_host: String,
    pub app_port: u16,
    pub tenant_user: String,
    pub tenant_password: String,
    pub sslmode: String,
}

#[derive(Clone, PartialEq, Eq)]
pub struct ProvisionedDatabase {
    pub database_name: String,
    pub connection_string: String,
}

#[derive(Clone)]
pub struct SelfHostedPostgresProvisioner {
    config: SelfHostedPostgresConfig,
}

impl SelfHostedPostgresProvisioner {
    pub fn new(config: SelfHostedPostgresConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &SelfHostedPostgresConfig {
        &self.config
    }

    pub fn from_env() -> Result<Self, String> {
        let app_port = parse_port(&optional_env_or("SELF_HOSTED_POSTGRES_APP_PORT", "5432"))?;

        Ok(Self::new(SelfHostedPostgresConfig {
            admin_url: required_env("SELF_HOSTED_POSTGRES_ADMIN_URL")?,
            app_host: optional_env_or("SELF_HOSTED_POSTGRES_APP_HOST", "postgres"),
            app_port,
            tenant_user: required_env("SELF_HOSTED_POSTGRES_TENANT_USER")?,
            tenant_password: required_env("SELF_HOSTED_POSTGRES_TENANT_PASSWORD")?,
            sslmode: optional_env_or("SELF_HOSTED_POSTGRES_SSLMODE", "disable"),
        }))
    }

    pub async fn create_database(
        &self,
        database_name: &str,
    ) -> Result<ProvisionedDatabase, String> {
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .acquire_timeout(Duration::from_secs(30))
            .connect(&self.config.admin_url)
            .await
            .map_err(|error| {
                format!(
                    "Failed to connect to self-hosted PostgreSQL admin connection: {}",
                    error
                )
            })?;

        let create_sql = format!(
            "CREATE DATABASE {} OWNER {}",
            quote_pg_identifier(database_name),
            quote_pg_identifier(&self.config.tenant_user)
        );
        let create_result = sqlx::query(&create_sql).execute(&pool).await;
        pool.close().await;

        match create_result {
            Ok(_) => {}
            Err(sqlx::Error::Database(error)) if error.code().as_deref() == Some("42P04") => {
                return Err(format!(
                    "Tenant database '{}' already exists",
                    database_name
                ));
            }
            Err(error) => {
                return Err(format!(
                    "Failed to create self-hosted tenant database '{}': {}",
                    database_name, error
                ));
            }
        }

        let connection_string = self.config.connection_string_for_database(database_name);
        wait_for_database_connectable(&connection_string).await?;

        Ok(ProvisionedDatabase {
            database_name: database_name.to_string(),
            connection_string,
        })
    }

    pub async fn drop_database_by_name(&self, database_name: &str) -> Result<(), String> {
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .acquire_timeout(Duration::from_secs(30))
            .connect(&self.config.admin_url)
            .await
            .map_err(|error| {
                format!(
                    "Failed to connect to self-hosted PostgreSQL admin connection: {}",
                    error
                )
            })?;

        let terminate_result = sqlx::query(
            "SELECT pg_terminate_backend(pid)
             FROM pg_stat_activity
             WHERE datname = $1
               AND pid <> pg_backend_pid()",
        )
        .bind(database_name)
        .execute(&pool)
        .await;

        if let Err(error) = terminate_result {
            pool.close().await;
            return Err(format!(
                "Failed to terminate active connections for self-hosted database '{}': {}",
                database_name, error
            ));
        }

        let drop_sql = format!(
            "DROP DATABASE IF EXISTS {} WITH (FORCE)",
            quote_pg_identifier(database_name)
        );
        let drop_result = sqlx::query(&drop_sql).execute(&pool).await;
        pool.close().await;

        drop_result.map_err(|error| {
            format!(
                "Failed to drop self-hosted database '{}': {}",
                database_name, error
            )
        })?;

        Ok(())
    }
}

impl SelfHostedPostgresConfig {
    pub fn connection_string_for_database(&self, database_name: &str) -> String {
        format!(
            "postgresql://{}:{}@{}:{}/{}?sslmode={}",
            self.tenant_user,
            self.tenant_password,
            self.app_host,
            self.app_port,
            database_name,
            self.sslmode
        )
    }
}

fn required_env(key: &str) -> Result<String, String> {
    env::var(key)
        .map(|value| value.trim().to_string())
        .ok()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{} must be set", key))
}

fn optional_env_or(key: &str, default: &str) -> String {
    env::var(key)
        .map(|value| value.trim().to_string())
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default.to_string())
}

fn parse_port(value: &str) -> Result<u16, String> {
    value.parse::<u16>().map_err(|_| {
        format!(
            "SELF_HOSTED_POSTGRES_APP_PORT must be a valid u16, got '{}'",
            value
        )
    })
}

fn quote_pg_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

async fn wait_for_database_connectable(connection_string: &str) -> Result<(), String> {
    const MAX_ATTEMPTS: u8 = 30;
    const RETRY_DELAY: Duration = Duration::from_secs(2);

    for attempt in 1..=MAX_ATTEMPTS {
        let connect_result = PgPoolOptions::new()
            .max_connections(1)
            .acquire_timeout(Duration::from_secs(5))
            .connect(connection_string)
            .await;

        if let Ok(pool) = connect_result {
            let ping_result = sqlx::query("SELECT 1").execute(&pool).await;
            pool.close().await;

            if ping_result.is_ok() {
                return Ok(());
            }
        }

        if attempt < MAX_ATTEMPTS {
            tokio::time::sleep(RETRY_DELAY).await;
        }
    }

    Err(format!(
        "Tenant database did not become connectable after {} attempts",
        MAX_ATTEMPTS
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    fn clear_self_host_env() {
        for key in [
            "SELF_HOSTED_POSTGRES_ADMIN_URL",
            "SELF_HOSTED_POSTGRES_APP_HOST",
            "SELF_HOSTED_POSTGRES_APP_PORT",
            "SELF_HOSTED_POSTGRES_TENANT_USER",
            "SELF_HOSTED_POSTGRES_TENANT_PASSWORD",
            "SELF_HOSTED_POSTGRES_SSLMODE",
        ] {
            env::remove_var(key);
        }
    }

    #[test]
    fn parse_port_accepts_u16_values() {
        assert_eq!(parse_port("5432").unwrap(), 5432);
        assert_eq!(parse_port("65535").unwrap(), 65535);
    }

    #[test]
    fn parse_port_rejects_invalid_and_out_of_range_values() {
        assert!(parse_port("not-a-port").unwrap_err().contains("u16"));
        assert!(parse_port("70000").unwrap_err().contains("u16"));
    }

    #[test]
    fn quote_pg_identifier_escapes_double_quotes() {
        assert_eq!(
            quote_pg_identifier("schoolorbit_sandbox"),
            "\"schoolorbit_sandbox\""
        );
        assert_eq!(quote_pg_identifier("school\"name"), "\"school\"\"name\"");
    }

    #[test]
    fn connection_string_uses_configured_host_port_and_sslmode() {
        let config = SelfHostedPostgresConfig {
            admin_url: "postgresql://provisioner:secret@postgres:5432/postgres".to_string(),
            app_host: "postgres.internal".to_string(),
            app_port: 5433,
            tenant_user: "schoolorbit_tenant_owner".to_string(),
            tenant_password: "tenant-secret".to_string(),
            sslmode: "require".to_string(),
        };

        let url = config.connection_string_for_database("schoolorbit_sandbox");

        assert_eq!(
            url,
            "postgresql://schoolorbit_tenant_owner:tenant-secret@postgres.internal:5433/schoolorbit_sandbox?sslmode=require"
        );
    }

    #[test]
    fn from_env_loads_self_hosted_config() {
        let _guard = env_lock();
        clear_self_host_env();
        env::set_var(
            "SELF_HOSTED_POSTGRES_ADMIN_URL",
            "postgresql://provisioner:secret@postgres:5432/postgres",
        );
        env::set_var("SELF_HOSTED_POSTGRES_APP_HOST", "postgres.internal");
        env::set_var("SELF_HOSTED_POSTGRES_APP_PORT", "5433");
        env::set_var(
            "SELF_HOSTED_POSTGRES_TENANT_USER",
            "schoolorbit_tenant_owner",
        );
        env::set_var("SELF_HOSTED_POSTGRES_TENANT_PASSWORD", "tenant-secret");
        env::set_var("SELF_HOSTED_POSTGRES_SSLMODE", "require");

        let provisioner = SelfHostedPostgresProvisioner::from_env().unwrap();

        assert_eq!(provisioner.config().app_host, "postgres.internal");
        assert_eq!(provisioner.config().app_port, 5433);
        assert_eq!(provisioner.config().tenant_user, "schoolorbit_tenant_owner");
        assert_eq!(provisioner.config().sslmode, "require");

        clear_self_host_env();
    }
}
