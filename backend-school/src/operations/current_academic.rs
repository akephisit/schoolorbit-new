//! Read the sealed count/hash receipt for the current operational cutover.
use school_database::DbSession;
use school_errors::AppError;
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CurrentOperationsCheck {
    pub code: String,
    pub passed: bool,
    pub mismatch_count: i64,
}
pub struct CurrentOperationsAudit {
    pub completed: bool,
    pub checks: Vec<CurrentOperationsCheck>,
}
pub async fn read_on_connection(
    conn: &mut PgConnection,
) -> Result<CurrentOperationsAudit, AppError> {
    let row:Option<(bool,String,sqlx::types::Json<Vec<CurrentOperationsCheck>>)>=DbSession::new(conn).query_as("school.current_operations.cutover_receipt","SELECT passed,source_fingerprint,checks FROM academic_current_operations_migration_audit WHERE migration_version=157").fetch_optional().await?;
    let (passed, hash, sqlx::types::Json(checks)) = row.ok_or_else(|| {
        AppError::Conflict("Current operations cutover receipt is missing".into())
    })?;
    let mut codes = checks
        .iter()
        .map(|check| check.code.as_str())
        .collect::<Vec<_>>();
    codes.sort_unstable();
    let mut expected = vec![
        "CURRENT_DELIVERY_GRAPH_RECONCILED",
        "CURRENT_TEACHERS_RECONCILED",
        "CURRENT_TIMETABLE_IDS_RECONCILED",
        "CURRENT_FACTS_PRESERVED",
        "CURRENT_SUPERVISION_PRESERVED",
        "CURRENT_CATALOG_PINS_PRESERVED",
        "CURRENT_LEGACY_SCHEMA_REMOVED",
        "CURRENT_SCOPE_GUARDS_ENABLED",
    ];
    expected.sort_unstable();
    let completed = passed
        && hash.len() == 64
        && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
        && codes == expected
        && checks
            .iter()
            .all(|check| check.passed && check.mismatch_count == 0);
    Ok(CurrentOperationsAudit { completed, checks })
}
