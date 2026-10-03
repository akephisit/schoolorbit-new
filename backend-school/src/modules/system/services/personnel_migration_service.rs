//! Explicit migration boundary: these connections never invoke tenant auto-migration.
use school_errors::AppError;
use school_staff::services::personnel_cutover_service::{
    read_personnel_cutover_audit, read_personnel_preflight, PersonnelCheck,
    PERSONNEL_MIGRATION_VERSION,
};
use school_tenancy::ActiveSchool;
use serde::Serialize;
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    PgPool,
};
use std::{future::Future, str::FromStr, time::Duration};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonnelSchoolPreflight {
    pub subdomain: String,
    pub migration_version: i64,
    pub passed: bool,
    pub checks: Vec<PersonnelCheck>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonnelTenantPreflight {
    pub total_schools: usize,
    pub passed: bool,
    pub schools: Vec<PersonnelSchoolPreflight>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonnelCutoverStatus {
    pub status: String,
    pub migration_version: i64,
    pub passed: bool,
    pub checks: Vec<PersonnelCheck>,
}
fn failed_check(code: &str) -> Vec<PersonnelCheck> {
    vec![PersonnelCheck {
        code: code.into(),
        passed: false,
        count: 1,
    }]
}
fn failed_school(subdomain: &str, code: &str) -> PersonnelSchoolPreflight {
    PersonnelSchoolPreflight {
        subdomain: subdomain.into(),
        migration_version: 0,
        passed: false,
        checks: failed_check(code),
    }
}

fn personnel_preflight_options(mut options: PgConnectOptions) -> PgConnectOptions {
    // Registry URLs serve ordinary runtime traffic through Neon's transaction pooler.
    // The explicit migration preflight needs a direct session for read-only startup settings.
    let host = options.get_host();
    if host.ends_with(".neon.tech") {
        if let Some((endpoint, suffix)) = host.split_once('.') {
            if let Some(endpoint) = endpoint.strip_suffix("-pooler") {
                let direct_host = format!("{endpoint}.{suffix}");
                options = options.host(&direct_host);
            }
        }
    }
    options.options([
        ("default_transaction_read_only", "on"),
        ("statement_timeout", "10000"),
    ])
}

pub async fn preflight_personnel_tenants(
    schools: &[ActiveSchool],
) -> Result<PersonnelTenantPreflight, AppError> {
    let mut results = Vec::with_capacity(schools.len());
    for school in schools {
        let Some(url) = school
            .db_connection_string
            .as_deref()
            .filter(|url| !url.is_empty())
        else {
            results.push(failed_school(
                &school.subdomain,
                "PERSONNEL_CONNECTION_MISSING",
            ));
            continue;
        };
        let options = match PgConnectOptions::from_str(url) {
            Ok(options) => personnel_preflight_options(options),
            Err(_) => {
                results.push(failed_school(
                    &school.subdomain,
                    "PERSONNEL_CONNECTION_INVALID",
                ));
                continue;
            }
        };
        let pool = match PgPoolOptions::new()
            .max_connections(1)
            .acquire_timeout(Duration::from_secs(10))
            .connect_with(options)
            .await
        {
            Ok(pool) => pool,
            Err(_) => {
                results.push(failed_school(
                    &school.subdomain,
                    "PERSONNEL_CONNECTION_UNAVAILABLE",
                ));
                continue;
            }
        };
        let result = match read_personnel_preflight(&pool).await {
            Ok(report) => PersonnelSchoolPreflight {
                subdomain: school.subdomain.clone(),
                migration_version: report.migration_version,
                passed: report.passed,
                checks: report.checks,
            },
            Err(_) => failed_school(&school.subdomain, "PERSONNEL_PREFLIGHT_QUERY_FAILED"),
        };
        pool.close().await;
        results.push(result);
    }
    let passed = !results.is_empty()
        && results.iter().all(|r| {
            r.passed && !r.checks.is_empty() && r.checks.iter().all(|c| c.passed && c.count >= 0)
        });
    Ok(PersonnelTenantPreflight {
        total_schools: schools.len(),
        passed,
        schools: results,
    })
}
pub async fn after_personnel_preflight<T>(
    report: &PersonnelTenantPreflight,
    migration: impl Future<Output = T>,
) -> Result<T, AppError> {
    if !report.passed
        || report.total_schools == 0
        || report.schools.len() != report.total_schools
        || report.schools.iter().any(|school| {
            !school.passed
                || school.checks.is_empty()
                || school
                    .checks
                    .iter()
                    .any(|check| !check.passed || check.count < 0)
        })
    {
        return Err(AppError::Conflict(
            "PERSONNEL_PREFLIGHT_FAILED: migration was not started".into(),
        ));
    }
    Ok(migration.await)
}
pub async fn personnel_cutover_status(
    pool: Option<&PgPool>,
    current_version: i32,
) -> PersonnelCutoverStatus {
    let report = if current_version >= PERSONNEL_MIGRATION_VERSION as i32 {
        if let Some(pool) = pool {
            read_personnel_cutover_audit(pool).await.ok()
        } else {
            None
        }
    } else {
        None
    };
    match report {
        Some(report) => PersonnelCutoverStatus {
            status: if report.passed {
                "cutoverCompleted"
            } else {
                "failed"
            }
            .into(),
            migration_version: PERSONNEL_MIGRATION_VERSION,
            passed: report.passed,
            checks: report.checks,
        },
        None => PersonnelCutoverStatus {
            status: if current_version < PERSONNEL_MIGRATION_VERSION as i32 {
                "cutoverPending"
            } else {
                "failed"
            }
            .into(),
            migration_version: PERSONNEL_MIGRATION_VERSION,
            passed: false,
            checks: failed_check("PERSONNEL_AUDIT_UNAVAILABLE"),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "Requires the explicitly supplied disposable Neon rehearsal endpoint"]
    async fn personnel_preflight_neon_connection() {
        let url = std::env::var("PERSONNEL_PREFLIGHT_NEON_DATABASE_URL")
            .expect("disposable rehearsal connection required");
        let endpoint = std::env::var("PERSONNEL_PREFLIGHT_NEON_ENDPOINT")
            .expect("disposable rehearsal endpoint required");
        let options = PgConnectOptions::from_str(&url).expect("valid rehearsal connection");
        assert!(endpoint.starts_with("ep-"));
        assert_eq!(options.get_host(), endpoint);
        let mut schools = Vec::new();
        for pooled in [false, true] {
            let host = if pooled {
                let (label, suffix) = endpoint.split_once('.').unwrap();
                format!("{label}-pooler.{suffix}")
            } else {
                options.get_host().to_owned()
            };
            let probe = PgPoolOptions::new()
                .max_connections(1)
                .connect_with(personnel_preflight_options(options.clone().host(&host)))
                .await;
            match probe {
                Ok(pool) => {
                    assert_read_only_connection(&pool).await;
                    pool.close().await;
                }
                Err(error) => {
                    let unsupported = error
                        .as_database_error()
                        .is_some_and(|e| e.message().contains("unsupported startup parameter"));
                    panic!("PERSONNEL_PROVIDER_CONNECTION_FAILED pooled={pooled} unsupported_startup={unsupported} database_code={:?}",
                        error.as_database_error().and_then(|e| e.code()).map(|c| c.into_owned()));
                }
            }
            let mut connection = url::Url::parse(&url).unwrap();
            connection.set_host(Some(&host)).unwrap();
            schools.push(ActiveSchool {
                subdomain: format!("rehearsal-{pooled}"),
                db_connection_string: Some(connection.to_string()),
                migration_version: None,
                migration_status: None,
                last_migrated_at: None,
                migration_error: None,
            });
        }
        let report = preflight_personnel_tenants(&schools).await.unwrap();
        assert!(report.passed, "{report:?}");
        assert_eq!(report.total_schools, 2);
    }

    async fn assert_read_only_connection(pool: &PgPool) {
        let read_only: String = sqlx::query_scalar("SHOW default_transaction_read_only")
            .fetch_one(pool)
            .await
            .unwrap();
        assert_eq!(read_only, "on");
        let timeout: String = sqlx::query_scalar("SHOW statement_timeout")
            .fetch_one(pool)
            .await
            .unwrap();
        assert_eq!(timeout, "10s");
        let error = sqlx::query("CREATE TABLE personnel_read_only_must_never_exist (id int)")
            .execute(pool)
            .await
            .unwrap_err();
        assert_eq!(
            error.as_database_error().and_then(|e| e.code()).as_deref(),
            Some("25006")
        );
    }

    #[test]
    fn personnel_preflight_uses_direct_neon_host_without_changing_database_or_user() {
        for (url, expected_host) in [
            ("postgres://user-pooler.example:synthetic@ep-example-pooler.ap-southeast-1.aws.neon.tech/campus-pooler.db?sslmode=require", "ep-example.ap-southeast-1.aws.neon.tech"),
            ("postgres://user-pooler.example:synthetic@ep-example.ap-southeast-1.aws.neon.tech/campus-pooler.db?sslmode=require", "ep-example.ap-southeast-1.aws.neon.tech"),
            ("postgres://user-pooler.example:synthetic@custom-pooler.example/campus-pooler.db?sslmode=require", "custom-pooler.example"),
            ("postgres://user-pooler.example:synthetic@127.0.0.1/campus-pooler.db?sslmode=require", "127.0.0.1"),
        ] {
            let options = personnel_preflight_options(PgConnectOptions::from_str(url).unwrap());
            assert_eq!(options.get_host(), expected_host);
            assert_eq!(options.get_username(), "user-pooler.example");
            assert_eq!(options.get_database(), Some("campus-pooler.db"));
        }
    }

    #[tokio::test]
    async fn personnel_preflight_connection_rejects_writes() {
        let url = std::env::var("TEST_DATABASE_URL").expect("local test database required");
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect_with(personnel_preflight_options(
                PgConnectOptions::from_str(&url).unwrap(),
            ))
            .await
            .unwrap();
        assert_read_only_connection(&pool).await;
        pool.close().await;
    }

    #[tokio::test]
    async fn personnel_simplification_prevents_any_tenant_migration_on_failure() {
        let report = PersonnelTenantPreflight {
            total_schools: 1,
            passed: false,
            schools: vec![],
        };
        let calls = std::sync::atomic::AtomicUsize::new(0);
        let result = after_personnel_preflight(&report, async {
            calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        })
        .await;
        assert!(result.is_err());
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    }
    #[tokio::test]
    async fn personnel_simplification_preflight_reads_actual_version() {
        let pool = school_test_db::create_named_test_pool("personnel_preflight_version").await;
        school_test_db::run_test_migrations(&pool).await;
        let report = read_personnel_preflight(&pool).await.unwrap();
        assert_eq!(report.migration_version, PERSONNEL_MIGRATION_VERSION);
        let status =
            personnel_cutover_status(Some(&pool), PERSONNEL_MIGRATION_VERSION as i32).await;
        assert!(status.passed);
        assert_eq!(status.migration_version, PERSONNEL_MIGRATION_VERSION);
        assert_eq!(status.checks.len(), 7);
    }
    #[tokio::test]
    async fn personnel_simplification_status_requires_current_audit() {
        let pool = school_test_db::create_named_test_pool("personnel_status_audit").await;
        school_test_db::run_test_migrations(&pool).await;
        sqlx::query("DELETE FROM staff_personnel_simplification_audit")
            .execute(&pool)
            .await
            .unwrap();
        let status =
            personnel_cutover_status(Some(&pool), PERSONNEL_MIGRATION_VERSION as i32).await;
        assert!(!status.passed);
        assert!(!status.checks.is_empty());
        assert!(
            !personnel_cutover_status(None, PERSONNEL_MIGRATION_VERSION as i32)
                .await
                .passed
        );
        assert!(!personnel_cutover_status(Some(&pool), 83).await.passed);
    }
}
