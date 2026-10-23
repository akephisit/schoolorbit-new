//! Only bounded, sanitized operational context may reach startup diagnostics.
use std::{env, io};
use tracing_subscriber::filter::{EnvFilter, LevelFilter};

#[derive(Debug)]
pub(crate) struct StartupError {
    stage: &'static str,
    reason: &'static str,
    variable: Option<&'static str>,
    database_code: Option<String>,
    io_kind: Option<io::ErrorKind>,
    action: &'static str,
}

impl StartupError {
    fn new(stage: &'static str, reason: &'static str, action: &'static str) -> Self {
        Self {
            stage,
            reason,
            variable: None,
            database_code: None,
            io_kind: None,
            action,
        }
    }

    fn configuration(variable: &'static str, reason: &'static str, action: &'static str) -> Self {
        let mut error = Self::new("configuration", reason, action);
        error.variable = Some(variable);
        error
    }

    fn environment(variable: &'static str, error: env::VarError) -> Self {
        let reason = match error {
            env::VarError::NotPresent => "missing_variable",
            env::VarError::NotUnicode(_) => "invalid_unicode",
        };
        Self::configuration(
            variable,
            reason,
            "Set the required environment variable to valid Unicode.",
        )
    }

    pub(crate) fn logging() -> Self {
        Self::new(
            "logging",
            "initialization_failed",
            "Check the tracing subscriber configuration.",
        )
    }

    fn logging_configuration(reason: &'static str) -> Self {
        let mut error = Self::new(
            "logging",
            reason,
            "Set RUST_LOG to valid Unicode tracing filter directives, or leave it unset for error-level logging.",
        );
        error.variable = Some("RUST_LOG");
        error
    }

    pub(crate) fn operation(reason: &'static str) -> Self {
        Self::new(
            "arguments",
            reason,
            "Run without arguments to serve, or use --migrate-admin for migration jobs.",
        )
    }

    pub(crate) fn credentials() -> Self {
        Self::configuration(
            "DATABASE_CREDENTIALS_KEY",
            "invalid_encryption_key",
            "Provide the existing 32-byte database credentials key encoded as hexadecimal.",
        )
    }

    pub(crate) fn provisioning(provider_reason: &str) -> Self {
        let (variable, reason, action) = match provider_reason {
            "Invalid tenant connection template" => (
                "PG_TENANT_DATABASE_URL",
                "invalid_database_url",
                "Provide a PostgreSQL tenant connection template with a host.",
            ),
            "Invalid provisioning connection" => (
                "PG_PROVISIONING_DATABASE_URL",
                "invalid_database_url",
                "Provide a valid PostgreSQL provisioning connection URL.",
            ),
            "PG_PROVISIONING_MAX_CONNECTIONS must be an integer" => (
                "PG_PROVISIONING_MAX_CONNECTIONS",
                "invalid_integer",
                "Set the provisioning connection limit to an integer from 1 through 100.",
            ),
            "PG_PROVISIONING_MAX_CONNECTIONS outside supported range" => (
                "PG_PROVISIONING_MAX_CONNECTIONS",
                "outside_supported_range",
                "Set the provisioning connection limit to an integer from 1 through 100.",
            ),
            _ => {
                return Self::new(
                    "provisioning_configuration",
                    "invalid_configuration",
                    "Check the PostgreSQL provisioning configuration.",
                )
            }
        };
        Self::configuration(variable, reason, action)
    }

    pub(crate) fn database(
        stage: &'static str,
        variable: &'static str,
        error: sqlx::Error,
    ) -> Self {
        if let sqlx::Error::Protocol(reason) = &error {
            match reason.as_str() {
                "ADMIN_POOL_MAX_CONNECTIONS must be an integer" => {
                    return Self::configuration(
                        "ADMIN_POOL_MAX_CONNECTIONS",
                        "invalid_integer",
                        "Set the Admin pool limit to an integer from 1 through 1000.",
                    )
                }
                "ADMIN_POOL_MAX_CONNECTIONS outside supported range" => {
                    return Self::configuration(
                        "ADMIN_POOL_MAX_CONNECTIONS",
                        "outside_supported_range",
                        "Set the Admin pool limit to an integer from 1 through 1000.",
                    )
                }
                _ => (),
            }
        }
        let reason = match &error {
            sqlx::Error::Configuration(_) => "invalid_database_configuration",
            sqlx::Error::Database(_) => "database_rejected_connection",
            sqlx::Error::Io(_) => "database_io_failed",
            sqlx::Error::Tls(_) => "database_tls_failed",
            sqlx::Error::Protocol(_) => "database_protocol_failed",
            sqlx::Error::PoolTimedOut => "database_connection_timed_out",
            sqlx::Error::PoolClosed => "database_pool_closed",
            _ => "database_connection_failed",
        };
        let mut diagnostic = Self::new(stage, reason, "Check the named database connection, credentials, availability, and pool configuration.");
        diagnostic.variable = Some(variable);
        if let sqlx::Error::Io(error) = &error {
            diagnostic.io_kind = Some(error.kind());
        }
        diagnostic.database_code = error
            .as_database_error()
            .and_then(|error| error.code())
            .filter(|code| {
                code.len() == 5
                    && code
                        .bytes()
                        .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
            })
            .map(|code| code.into_owned());
        diagnostic
    }

    pub(crate) fn schema(provider_reason: &str) -> Self {
        let reason = match provider_reason {
            "Admin migration history unavailable" => "migration_history_unavailable",
            "Admin schema does not match this release" => "schema_release_mismatch",
            _ => "schema_verification_failed",
        };
        Self::new(
            "schema_verification",
            reason,
            "Run the approved Admin migration job for this release before starting the service.",
        )
    }

    pub(crate) fn migration(provider_reason: &str) -> Self {
        let reason = match provider_reason {
            "Admin migration gate failed" => "migration_gate_failed",
            "Admin migration identity unavailable" => "migration_identity_unavailable",
            "Admin runtime grants failed" => "runtime_grants_failed",
            _ => "migration_reconciliation_failed",
        };
        Self::new("admin_migration", reason, "Check the migration owner connection and migration/reconciliation gate before retrying.")
    }

    pub(crate) fn io(stage: &'static str, error: io::Error) -> Self {
        let mut diagnostic = Self::new(
            stage,
            "io_failed",
            "Check the service listener and operating system network resources.",
        );
        diagnostic.io_kind = Some(error.kind());
        diagnostic
    }

    pub(crate) fn log(&self) {
        tracing::error!(stage = self.stage, reason = self.reason, variable = ?self.variable, database_code = ?self.database_code, io_kind = ?self.io_kind, action = self.action, "Admin startup failed");
    }
}

pub(crate) fn required_environment(variable: &'static str) -> Result<String, StartupError> {
    env::var(variable).map_err(|error| StartupError::environment(variable, error))
}

pub(crate) fn logging_filter() -> Result<EnvFilter, StartupError> {
    let directives = match env::var("RUST_LOG") {
        Ok(directives) => directives,
        Err(env::VarError::NotPresent) => String::new(),
        Err(env::VarError::NotUnicode(_)) => {
            return Err(StartupError::logging_configuration("invalid_unicode"));
        }
    };
    // Match from_default_env's error-level fallback without its lossy parser,
    // which prints rejected directives (including their raw values) to stderr.
    EnvFilter::builder()
        .with_default_directive(LevelFilter::ERROR.into())
        .parse(directives)
        .map_err(|_| StartupError::logging_configuration("invalid_filter_directive"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_error_payloads_cannot_reach_startup_diagnostics() {
        let secret = "synthetic-private-startup-marker";
        let failures = [
            StartupError::environment("DATABASE_URL", env::VarError::NotUnicode(secret.into())),
            StartupError::database(
                "database_connection",
                "DATABASE_URL",
                sqlx::Error::Protocol(secret.into()),
            ),
            StartupError::database(
                "database_connection",
                "DATABASE_URL",
                sqlx::Error::Io(io::Error::new(io::ErrorKind::PermissionDenied, secret)),
            ),
            StartupError::io(
                "listener_bind",
                io::Error::new(io::ErrorKind::AddrInUse, secret),
            ),
            StartupError::provisioning(secret),
            StartupError::schema(secret),
            StartupError::migration(secret),
        ];
        for failure in failures {
            assert!(!format!("{failure:?}").contains(secret));
        }
    }
}
