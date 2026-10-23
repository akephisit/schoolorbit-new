use std::process::{Command, Output};

fn startup(configure: impl FnOnce(&mut Command)) -> Output {
    let directory = std::env::temp_dir().join(format!("school-startup-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&directory).unwrap();
    std::fs::write(directory.join(".env"), "").unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_backend-school"));
    // Empty owned .env stops dotenv discovery; no inherited provider configuration is read.
    command
        .current_dir(&directory)
        .env_clear()
        .env("LOG_FORMAT", "json");
    configure(&mut command);
    let output = command.output();
    std::fs::remove_dir_all(directory).unwrap();
    output.unwrap()
}

fn failure(output: Output, expected: &[&str]) {
    assert_eq!(output.status.code(), Some(1));
    let diagnostic = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    for value in expected {
        assert!(
            diagnostic.contains(value),
            "missing diagnostic {value}: {diagnostic}"
        );
    }
    assert!(
        !diagnostic.contains("panicked"),
        "startup must return an error: {diagnostic}"
    );
    assert!(
        !diagnostic.contains("synthetic-private-marker"),
        "startup diagnostic leaked untrusted input"
    );
}

#[test]
fn missing_required_startup_configuration_returns_an_explicit_error() {
    failure(
        startup(|_| {}),
        &["environment", "BACKEND_ADMIN_URL", "missing"],
    );
    failure(
        startup(|command| {
            command.env("BACKEND_ADMIN_URL", "http://127.0.0.1:1");
        }),
        &["environment", "INTERNAL_API_SECRET", "missing"],
    );
}

#[test]
fn malformed_startup_configuration_identifies_the_setting_without_echoing_its_value() {
    failure(
        startup(|command| {
            command
                .env("BACKEND_ADMIN_URL", "http://127.0.0.1:1")
                .env("INTERNAL_API_SECRET", "synthetic-private-marker")
                .env(
                    "BACKEND_ADMIN_REQUEST_TIMEOUT_MS",
                    "synthetic-private-marker",
                );
        }),
        &[
            "admin_client_configuration",
            "BACKEND_ADMIN_REQUEST_TIMEOUT_MS",
            "must be a valid integer",
        ],
    );
}

#[cfg(unix)]
#[test]
fn non_unicode_required_environment_is_reported_without_the_raw_secret() {
    use std::os::unix::ffi::OsStringExt;
    let secret = std::ffi::OsString::from_vec(b"synthetic-private-marker\xff".to_vec());
    failure(
        startup(|command| {
            command
                .env("BACKEND_ADMIN_URL", "http://127.0.0.1:1")
                .env("INTERNAL_API_SECRET", secret);
        }),
        &["environment", "INTERNAL_API_SECRET", "invalid_unicode"],
    );
}

#[cfg(not(feature = "api-docs"))]
#[test]
fn production_export_refuses_before_startup_configuration_is_read() {
    let output = startup(|command| {
        command.arg("export-openapi");
    });
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("--features api-docs"));
}

#[cfg(feature = "api-docs")]
#[test]
fn documentation_export_works_before_startup_configuration_is_read() {
    let output = startup(|command| {
        command.arg("export-openapi");
    });
    assert!(output.status.success());
    let document: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(document["openapi"], "3.1.0");
}

#[test]
fn invalid_export_arguments_keep_the_cli_usage_exit_code() {
    let output = startup(|command| {
        command.args(["export-openapi", "unexpected"]);
    });
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("usage: backend-school export-openapi")
    );
}

#[cfg(unix)]
#[test]
fn non_unicode_arguments_return_an_error_without_echoing_the_raw_value() {
    use std::os::unix::ffi::OsStringExt;
    let argument = std::ffi::OsString::from_vec(b"synthetic-private-marker\xff".to_vec());
    failure(
        startup(|command| {
            command.arg(argument);
        }),
        &["arguments", "invalid_unicode"],
    );
}
