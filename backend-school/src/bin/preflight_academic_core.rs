use backend_school::academic_core_preflight::{
    run_academic_core_preflight, AcademicCorePreflightReport,
};
use chrono::{Local, NaiveDate};
use sqlx::postgres::PgPoolOptions;
use std::{env, io::Write, process, time::Duration};

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
enum CliError {
    #[error("ACADEMIC_CORE_PREFLIGHT_CONFIG_INVALID")]
    ConfigInvalid,
    #[error("ACADEMIC_CORE_PREFLIGHT_CONNECTION_FAILED")]
    ConnectionFailed,
    #[error("ACADEMIC_CORE_PREFLIGHT_EXECUTION_FAILED")]
    ExecutionFailed,
    #[error("ACADEMIC_CORE_PREFLIGHT_OUTPUT_FAILED")]
    OutputFailed,
}

fn validate_schema_name(schema: &str, allow_public: bool) -> Result<(), CliError> {
    if schema.is_empty()
        || !schema
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return Err(CliError::ConfigInvalid);
    }
    if schema == "public" && !allow_public {
        return Err(CliError::ConfigInvalid);
    }
    Ok(())
}

fn parse_cutover_date(value: Option<&str>) -> Result<NaiveDate, CliError> {
    match value {
        Some(value) => {
            NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| CliError::ConfigInvalid)
        }
        None => Ok(Local::now().date_naive()),
    }
}

fn allow_public_schema() -> bool {
    env::var("PREFLIGHT_SCHEMA_ALLOW_PUBLIC").is_ok_and(|value| value == "1")
}

async fn execute() -> Result<AcademicCorePreflightReport, CliError> {
    if env::args_os().len() != 1 {
        return Err(CliError::ConfigInvalid);
    }

    dotenvy::dotenv().ok();
    let database_url =
        env::var("PREFLIGHT_SCHEMA_DATABASE_URL").map_err(|_| CliError::ConfigInvalid)?;
    let schema = env::var("PREFLIGHT_SCHEMA_NAME").map_err(|_| CliError::ConfigInvalid)?;
    validate_schema_name(&schema, allow_public_schema())?;
    let cutover_date = parse_cutover_date(env::var("PREFLIGHT_CUTOVER_DATE").ok().as_deref())?;

    let search_path_sql = format!(r#"SET search_path TO "{schema}", public"#);
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(30))
        .after_connect(move |connection, _metadata| {
            let search_path_sql = search_path_sql.clone();
            Box::pin(async move {
                sqlx::query(&search_path_sql).execute(connection).await?;
                Ok(())
            })
        })
        .connect(&database_url)
        .await
        .map_err(|_| CliError::ConnectionFailed)?;

    let result = run_academic_core_preflight(&pool, &schema, cutover_date)
        .await
        .map_err(|_| CliError::ExecutionFailed);
    pool.close().await;
    result
}

fn write_report(report: &AcademicCorePreflightReport) -> Result<(), CliError> {
    let mut stdout = std::io::stdout().lock();
    serde_json::to_writer(&mut stdout, report).map_err(|_| CliError::OutputFailed)?;
    stdout
        .write_all(b"\n")
        .map_err(|_| CliError::OutputFailed)?;
    Ok(())
}

fn write_bounded_error(error: &CliError) {
    let mut stderr = std::io::stderr().lock();
    let _ = writeln!(stderr, "{error}");
}

#[tokio::main]
async fn main() {
    match execute().await {
        Ok(report) => {
            let can_cut_over = report.can_cut_over;
            if let Err(error) = write_report(&report) {
                write_bounded_error(&error);
                process::exit(1);
            }
            if !can_cut_over {
                process::exit(2);
            }
        }
        Err(error) => {
            write_bounded_error(&error);
            process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_cutover_date, validate_schema_name};
    use chrono::NaiveDate;

    #[test]
    fn schema_guard_rejects_public_without_explicit_opt_in() {
        assert!(validate_schema_name("public", false).is_err());
        assert!(validate_schema_name("public", true).is_ok());
    }

    #[test]
    fn schema_guard_rejects_non_ascii_identifiers() {
        assert!(validate_schema_name("", true).is_err());
        assert!(validate_schema_name("tenant-name", true).is_err());
        assert!(validate_schema_name("โรงเรียน", true).is_err());
    }

    #[test]
    fn cutover_date_parser_is_strict_iso_date() {
        assert_eq!(
            parse_cutover_date(Some("2025-08-23")),
            Ok(NaiveDate::from_ymd_opt(2025, 8, 23).expect("test date must be valid"))
        );
        assert!(parse_cutover_date(Some("23/08/2025")).is_err());
    }
}
