//! Database-per-tenant provisioning. Resource markers authorize retry and cleanup.
use reqwest::Url;
use sqlx::ConnectOptions;
use sqlx::{postgres::PgConnectOptions, Connection, PgConnection};
use std::str::FromStr;
use std::time::Duration;
use uuid::Uuid;

pub fn tenant_role(tenant: Uuid, purpose: &str) -> Result<String, String> {
    let suffix = match purpose {
        "migration" => "owner",
        "runtime" => "runtime",
        _ => return Err("Invalid role purpose".into()),
    };
    Ok(format!("so_{}_{suffix}", tenant.simple()))
}

fn ident(value: &str) -> Result<String, String> {
    if value.is_empty()
        || value.len() > 63
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return Err("Invalid database resource identifier".into());
    }
    Ok(format!("\"{value}\""))
}

fn db_error(error: sqlx::Error) -> String {
    tracing::warn!(database_code = ?error.as_database_error().and_then(|e| e.code()), "PostgreSQL provisioning operation failed");
    "PostgreSQL provisioning operation failed".into()
}

pub struct PostgresClient {
    options: PgConnectOptions,
    template: Url,
}

impl PostgresClient {
    pub fn new() -> Result<Self, String> {
        let provisioning = std::env::var("PG_PROVISIONING_DATABASE_URL")
            .map_err(|_| "PG_PROVISIONING_DATABASE_URL is required".to_string())?;
        let template = std::env::var("PG_TENANT_DATABASE_URL")
            .map_err(|_| "PG_TENANT_DATABASE_URL is required".to_string())?;
        Self::from_config(&provisioning, &template)
    }

    pub fn from_config(provisioning: &str, template: &str) -> Result<Self, String> {
        let template =
            Url::parse(template).map_err(|_| "Invalid tenant connection template".to_string())?;
        if !matches!(template.scheme(), "postgres" | "postgresql") || template.host_str().is_none()
        {
            return Err("Invalid tenant connection template".into());
        }
        let options = sqlx::postgres::PgConnectOptions::from_str(provisioning)
            .map_err(|_| "Invalid provisioning connection".to_string())?
            .disable_statement_logging();
        provisioning_budget()?;
        Ok(Self { options, template })
    }

    pub fn connection_string(
        &self,
        tenant: Uuid,
        db_name: &str,
        purpose: &str,
        password: &str,
    ) -> Result<String, String> {
        ident(db_name)?;
        let role = tenant_role(tenant, purpose)?;
        let mut url = self.template.clone();
        url.set_username(&role)
            .map_err(|_| "Invalid tenant role".to_string())?;
        url.set_password(Some(password))
            .map_err(|_| "Invalid tenant password".to_string())?;
        url.set_path(&format!("/{db_name}"));
        Ok(url.to_string())
    }

    async fn role_owned(
        &self,
        connection: &mut PgConnection,
        tenant: Uuid,
        purpose: &str,
    ) -> Result<bool, String> {
        let role = tenant_role(tenant, purpose)?;
        let marker = format!("schoolorbit:{tenant}:{purpose}");
        let actual: Option<Option<String>> = sqlx::query_scalar(
            "SELECT shobj_description(oid, 'pg_authid') FROM pg_roles WHERE rolname = $1",
        )
        .bind(role)
        .fetch_optional(&mut *connection)
        .await
        .map_err(db_error)?;
        match actual {
            None => Ok(false),
            Some(Some(value)) if value == marker => Ok(true),
            _ => Err("Database role ownership mismatch".into()),
        }
    }

    pub async fn create_database(
        &self,
        tenant: Uuid,
        db_name: &str,
        owner_password: &str,
        runtime_password: &str,
    ) -> Result<(), String> {
        let db = ident(db_name)?;
        let owner = tenant_role(tenant, "migration")?;
        let runtime = tenant_role(tenant, "runtime")?;
        // A stable school record with encrypted passwords exists before this operation.
        // Serialize the whole nontransactional CREATE DATABASE operation across processes.
        let _permit = provisioning_permit().await?;
        let mut connection = PgConnection::connect_with(&self.options)
            .await
            .map_err(db_error)?;
        sqlx::query("SELECT pg_advisory_lock(hashtextextended($1, 0))")
            .bind(db_name)
            .execute(&mut connection)
            .await
            .map_err(db_error)?;
        let outcome = self
            .create_locked(
                &mut connection,
                tenant,
                db_name,
                owner_password,
                runtime_password,
            )
            .await;
        // This connection must not return a held session lock to the pool after cancellation.
        outcome?;
        let set_owner = format!("SET ROLE {}", ident(&owner)?);
        sqlx::raw_sql(sqlx::AssertSqlSafe(set_owner))
            .execute(&mut connection)
            .await
            .map_err(db_error)?;
        let statement = format!(
            "REVOKE ALL ON DATABASE {db} FROM PUBLIC; GRANT CONNECT ON DATABASE {db} TO {}, {}",
            ident(&owner)?,
            ident(&runtime)?
        );
        sqlx::raw_sql(sqlx::AssertSqlSafe(statement))
            .execute(&mut connection)
            .await
            .map_err(db_error)?;
        sqlx::Connection::close(connection)
            .await
            .map_err(db_error)?;
        Ok(())
    }

    async fn create_locked(
        &self,
        connection: &mut PgConnection,
        tenant: Uuid,
        db_name: &str,
        owner_password: &str,
        runtime_password: &str,
    ) -> Result<(), String> {
        let owner = tenant_role(tenant, "migration")?;
        let runtime = tenant_role(tenant, "runtime")?;
        let owner_owned = self.role_owned(connection, tenant, "migration").await?;
        let runtime_owned = self.role_owned(connection, tenant, "runtime").await?;
        let mut tx = sqlx::Connection::begin(&mut *connection)
            .await
            .map_err(db_error)?;
        for (role, purpose, password, exists) in [
            (&owner, "migration", owner_password, owner_owned),
            (&runtime, "runtime", runtime_password, runtime_owned),
        ] {
            if !exists {
                // Only generated hexadecimal passwords may enter utility SQL; utility logging is disabled.
                if password.len() != 64 || !password.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return Err("Invalid generated database password".into());
                }
                let marker = format!("schoolorbit:{tenant}:{purpose}");
                let role = ident(role)?;
                let statement = format!("CREATE ROLE {role} LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS PASSWORD '{password}'; COMMENT ON ROLE {role} IS '{marker}'");
                sqlx::raw_sql(sqlx::AssertSqlSafe(statement))
                    .execute(&mut *tx)
                    .await
                    .map_err(db_error)?;
            }
        }
        let provisioner: String = sqlx::query_scalar("SELECT current_user::text")
            .fetch_one(&mut *tx)
            .await
            .map_err(db_error)?;
        let statement = format!(
            "GRANT {} TO {} WITH SET TRUE, INHERIT FALSE",
            ident(&owner)?,
            ident(&provisioner)?
        );
        sqlx::raw_sql(sqlx::AssertSqlSafe(statement))
            .execute(&mut *tx)
            .await
            .map_err(db_error)?;
        tx.commit().await.map_err(db_error)?;
        let existing: Option<String> = sqlx::query_scalar(
            "SELECT pg_get_userbyid(datdba)::text FROM pg_database WHERE datname = $1",
        )
        .bind(db_name)
        .fetch_optional(&mut *connection)
        .await
        .map_err(db_error)?;
        match existing {
            Some(actual) if actual == owner && owner_owned => (),
            Some(_) => return Err("Database ownership mismatch; refusing adoption".into()),
            None => {
                let statement = format!(
                "CREATE DATABASE {} OWNER {} TEMPLATE template0 ENCODING 'UTF8' LOCALE 'C.UTF-8' LOCALE_PROVIDER builtin BUILTIN_LOCALE 'C.UTF-8'",
                    ident(db_name)?,
                    ident(&owner)?
                );
                sqlx::raw_sql(sqlx::AssertSqlSafe(statement))
                    .execute(&mut *connection)
                    .await
                    .map_err(db_error)?;
            }
        }
        Ok(())
    }

    pub async fn delete_database(&self, tenant: Uuid, db_name: &str) -> Result<(), String> {
        ident(db_name)?;
        let _permit = provisioning_permit().await?;
        let mut owned_connection = PgConnection::connect_with(&self.options)
            .await
            .map_err(db_error)?;
        let connection = &mut owned_connection;
        sqlx::query("SELECT pg_advisory_lock(hashtextextended($1, 0))")
            .bind(db_name)
            .execute(&mut *connection)
            .await
            .map_err(db_error)?;
        let owner_owned = self.role_owned(connection, tenant, "migration").await?;
        let runtime_owned = self.role_owned(connection, tenant, "runtime").await?;
        let owner = tenant_role(tenant, "migration")?;
        let existing: Option<String> = sqlx::query_scalar(
            "SELECT pg_get_userbyid(datdba)::text FROM pg_database WHERE datname = $1",
        )
        .bind(db_name)
        .fetch_optional(&mut *connection)
        .await
        .map_err(db_error)?;
        if let Some(actual) = existing {
            if !owner_owned || actual != owner {
                return Err("Database ownership mismatch; refusing deletion".into());
            }
            let set_owner = format!("SET ROLE {}", ident(&owner)?);
            sqlx::raw_sql(sqlx::AssertSqlSafe(set_owner))
                .execute(&mut *connection)
                .await
                .map_err(db_error)?;
            let statement = format!("DROP DATABASE {} WITH (FORCE)", ident(db_name)?);
            sqlx::raw_sql(sqlx::AssertSqlSafe(statement))
                .execute(&mut *connection)
                .await
                .map_err(db_error)?;
        }
        sqlx::query("RESET ROLE")
            .execute(&mut *connection)
            .await
            .map_err(db_error)?;
        for (purpose, exists) in [("runtime", runtime_owned), ("migration", owner_owned)] {
            if exists {
                let statement = format!("DROP ROLE {}", ident(&tenant_role(tenant, purpose)?)?);
                sqlx::raw_sql(sqlx::AssertSqlSafe(statement))
                    .execute(&mut *connection)
                    .await
                    .map_err(db_error)?;
            }
        }
        Ok(())
    }
}

static PROVISIONING_BUDGET: std::sync::OnceLock<
    Result<std::sync::Arc<tokio::sync::Semaphore>, String>,
> = std::sync::OnceLock::new();
fn provisioning_budget() -> Result<std::sync::Arc<tokio::sync::Semaphore>, String> {
    PROVISIONING_BUDGET
        .get_or_init(|| {
            let count =
                std::env::var("PG_PROVISIONING_MAX_CONNECTIONS").map_or(Ok(2u32), |raw| {
                    raw.parse::<u32>().map_err(|_| {
                        "PG_PROVISIONING_MAX_CONNECTIONS must be an integer".to_string()
                    })
                })?;
            if !(1..=100).contains(&count) {
                return Err("PG_PROVISIONING_MAX_CONNECTIONS outside supported range".into());
            }
            Ok(std::sync::Arc::new(tokio::sync::Semaphore::new(
                count as usize,
            )))
        })
        .clone()
}
async fn provisioning_permit() -> Result<tokio::sync::OwnedSemaphorePermit, String> {
    tokio::time::timeout(
        Duration::from_secs(5),
        provisioning_budget()?.acquire_owned(),
    )
    .await
    .map_err(|_| "Provisioning connection budget timed out")?
    .map_err(|_| "Provisioning connection budget closed".into())
}
