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
            Ok(options) => options.options([
                ("default_transaction_read_only", "on"),
                ("statement_timeout", "10000"),
            ]),
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
    async fn personnel_preflight_prevents_any_tenant_migration_on_failure() {
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
    async fn personnel_preflight_reads_actual_version() {
        let pool = school_test_db::create_named_test_pool("personnel_preflight_version").await;
        school_test_db::run_test_migrations(&pool).await;
        let report = read_personnel_preflight(&pool).await.unwrap();
        assert_eq!(report.migration_version, 82);
        let status = personnel_cutover_status(Some(&pool), 82).await;
        assert!(status.passed);
    }
    #[tokio::test]
    async fn personnel_status_requires_current_audit() {
        let pool = school_test_db::create_named_test_pool("personnel_status_audit").await;
        school_test_db::run_test_migrations(&pool).await;
        sqlx::query("DELETE FROM staff_personnel_cutover_audit")
            .execute(&pool)
            .await
            .unwrap();
        let status = personnel_cutover_status(Some(&pool), 82).await;
        assert!(!status.passed);
        assert!(!status.checks.is_empty());
        assert!(!personnel_cutover_status(None, 82).await.passed);
    }
}
