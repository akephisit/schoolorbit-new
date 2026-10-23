use std::{
    ffi::OsStr,
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

const PRIVATE_MARKER: &str = "synthetic-private-startup-marker";
static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct StartupProcess {
    command: Command,
    directory: PathBuf,
}

impl StartupProcess {
    fn new() -> Self {
        // An owned empty dotenv file stops parent-directory discovery; the clean
        // environment prevents developer credentials from changing the tested boundary.
        let directory = std::env::temp_dir().join(format!(
            "schoolorbit-admin-startup-{}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&directory).expect("create owned startup test directory");
        fs::write(directory.join(".env"), "").expect("create owned empty startup dotenv file");
        let mut command = Command::new(env!("CARGO_BIN_EXE_backend-admin"));
        command
            .current_dir(&directory)
            .env_clear()
            .env("RUST_LOG", "info");
        Self { command, directory }
    }

    fn valid_credentials(&mut self) {
        self.command
            .env("DATABASE_CREDENTIALS_KEY", "ab".repeat(32));
    }

    fn valid_provisioning(&mut self) {
        self.valid_credentials();
        // Configuration parsing only; these tests never connect to this address.
        self.command.env(
            "PG_PROVISIONING_DATABASE_URL",
            "postgresql://fixture:fixture@127.0.0.1/fixture",
        );
        self.command.env(
            "PG_TENANT_DATABASE_URL",
            "postgresql://fixture:fixture@127.0.0.1/fixture",
        );
    }

    fn fails(&mut self, stage: &str, reason: &str, variable: Option<&str>) -> String {
        let Output {
            status,
            stdout,
            stderr,
        } = self.command.output().expect("run Admin startup subprocess");
        let diagnostic = format!(
            "{}{}",
            String::from_utf8_lossy(&stdout),
            String::from_utf8_lossy(&stderr)
        );
        assert_eq!(status.code(), Some(1), "{diagnostic}");
        assert!(diagnostic.contains("Admin startup failed"), "{diagnostic}");
        assert!(diagnostic.contains(stage), "{diagnostic}");
        assert!(diagnostic.contains(reason), "{diagnostic}");
        if let Some(variable) = variable {
            assert!(diagnostic.contains(variable), "{diagnostic}");
        }
        assert!(
            !diagnostic.contains(PRIVATE_MARKER),
            "startup leaked a synthetic private marker"
        );
        assert!(!diagnostic.contains("panicked at"), "{diagnostic}");
        diagnostic
    }

    fn fails_before_logging(&mut self, reason: &str) {
        let output = self.command.output().expect("run Admin logging subprocess");
        let diagnostic = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.status.code(), Some(1), "{diagnostic}");
        assert!(diagnostic.contains("Error: StartupError"), "{diagnostic}");
        assert!(diagnostic.contains("logging"), "{diagnostic}");
        assert!(diagnostic.contains(reason), "{diagnostic}");
        assert!(diagnostic.contains("RUST_LOG"), "{diagnostic}");
        assert!(diagnostic.contains("Set RUST_LOG"), "{diagnostic}");
        assert!(
            !diagnostic.contains(PRIVATE_MARKER),
            "logging initialization leaked a synthetic private marker"
        );
        assert!(!diagnostic.contains("panicked at"), "{diagnostic}");
    }
}

impl Drop for StartupProcess {
    fn drop(&mut self) {
        fs::remove_file(self.directory.join(".env")).expect("remove owned startup dotenv file");
        fs::remove_dir(&self.directory).expect("remove owned startup test directory");
    }
}

#[test]
fn missing_runtime_and_migration_configuration_fail_with_actionable_diagnostics() {
    StartupProcess::new().fails(
        "configuration",
        "missing_variable",
        Some("DATABASE_CREDENTIALS_KEY"),
    );

    let mut migration = StartupProcess::new();
    migration.command.arg("--migrate-admin");
    migration.fails(
        "configuration",
        "missing_variable",
        Some("ADMIN_MIGRATION_DATABASE_URL"),
    );

    let mut provisioning = StartupProcess::new();
    provisioning.valid_credentials();
    provisioning.fails(
        "configuration",
        "missing_variable",
        Some("PG_PROVISIONING_DATABASE_URL"),
    );

    let mut runtime = StartupProcess::new();
    runtime.valid_provisioning();
    runtime.fails("configuration", "missing_variable", Some("DATABASE_URL"));
}

#[test]
fn unsupported_operation_reports_usage_without_echoing_arguments() {
    let mut startup = StartupProcess::new();
    startup.command.arg(format!("--{PRIVATE_MARKER}"));
    let diagnostic = startup.fails("arguments", "unsupported_operation", None);
    assert!(diagnostic.contains("--migrate-admin"));
}

#[test]
fn malformed_log_filter_fails_safely_before_logger_initialization() {
    let mut startup = StartupProcess::new();
    startup
        .command
        .env("RUST_LOG", format!("{PRIVATE_MARKER}=invalid"));
    startup.fails_before_logging("invalid_filter_directive");
}

#[test]
fn unset_and_empty_log_filters_preserve_default_error_diagnostics() {
    let mut unset = StartupProcess::new();
    unset.command.env_remove("RUST_LOG");
    unset.fails(
        "configuration",
        "missing_variable",
        Some("DATABASE_CREDENTIALS_KEY"),
    );

    let mut empty = StartupProcess::new();
    empty.command.env("RUST_LOG", "");
    empty.fails(
        "configuration",
        "missing_variable",
        Some("DATABASE_CREDENTIALS_KEY"),
    );
}

#[test]
fn invalid_configuration_redacts_encryption_keys_and_connection_values() {
    let mut encryption = StartupProcess::new();
    encryption
        .command
        .env("DATABASE_CREDENTIALS_KEY", PRIVATE_MARKER);
    encryption.fails(
        "configuration",
        "invalid_encryption_key",
        Some("DATABASE_CREDENTIALS_KEY"),
    );

    let mut provisioning = StartupProcess::new();
    provisioning.valid_provisioning();
    provisioning
        .command
        .env("PG_PROVISIONING_DATABASE_URL", PRIVATE_MARKER);
    provisioning.fails(
        "configuration",
        "invalid_database_url",
        Some("PG_PROVISIONING_DATABASE_URL"),
    );

    let mut runtime = StartupProcess::new();
    runtime.valid_provisioning();
    runtime.command.env("DATABASE_URL", PRIVATE_MARKER);
    runtime.fails(
        "database_connection",
        "invalid_database_configuration",
        Some("DATABASE_URL"),
    );

    let mut budget = StartupProcess::new();
    budget.valid_provisioning();
    budget.command.env(
        "DATABASE_URL",
        "postgresql://fixture:fixture@127.0.0.1/fixture",
    );
    budget
        .command
        .env("ADMIN_POOL_MAX_CONNECTIONS", PRIVATE_MARKER);
    budget.fails(
        "configuration",
        "invalid_integer",
        Some("ADMIN_POOL_MAX_CONNECTIONS"),
    );
}

#[cfg(unix)]
#[test]
fn invalid_unicode_arguments_and_environment_fail_without_panics_or_values() {
    use std::os::unix::ffi::OsStrExt;

    let mut bytes = PRIVATE_MARKER.as_bytes().to_vec();
    bytes.push(0xff);
    let invalid = OsStr::from_bytes(&bytes);
    let mut arguments = StartupProcess::new();
    arguments.command.arg(invalid);
    arguments.fails("arguments", "invalid_unicode", None);

    let mut environment = StartupProcess::new();
    environment.command.env("DATABASE_CREDENTIALS_KEY", invalid);
    environment.fails(
        "configuration",
        "invalid_unicode",
        Some("DATABASE_CREDENTIALS_KEY"),
    );

    let mut logging = StartupProcess::new();
    logging.command.env("RUST_LOG", invalid);
    logging.fails_before_logging("invalid_unicode");
}
