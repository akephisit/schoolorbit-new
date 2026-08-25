use crate::error::AppError;
use serde::{Deserialize, Serialize};
use sqlx::{types::Json, PgPool};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReconciliationCheck {
    pub code: String,
    pub passed: bool,
    pub source_count: i64,
    pub target_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcademicCoreCleanupAuditStatus {
    pub migration_version: i64,
    pub completed: bool,
    pub checks: Vec<ReconciliationCheck>,
}

pub const PHASE_B_MIGRATION_VERSION: i64 = 45;
pub const CLEANUP_MAPPING_VERSION: &str = "academic-core-v1-cleanup";

const CLEANUP_COUNT_KEYS: [&str; 5] = [
    "legacyRelationsRemoved",
    "legacyColumnsRemoved",
    "legacyPermissionDefinitionsRemoved",
    "legacyPermissionGrantsRemoved",
    "targetRowsRetained",
];

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CleanupAuditCounts {
    legacy_relations_removed: i64,
    legacy_columns_removed: i64,
    legacy_permission_definitions_removed: i64,
    legacy_permission_grants_removed: i64,
    target_rows_retained: i64,
}

fn check(code: &str, source_count: i64, target_count: i64, passed: bool) -> ReconciliationCheck {
    ReconciliationCheck {
        code: code.to_string(),
        passed,
        source_count,
        target_count,
    }
}

fn checksum_is_valid(value: &str) -> bool {
    let value = value.trim();
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub async fn read_academic_core_cleanup_audit(
    pool: &PgPool,
) -> Result<AcademicCoreCleanupAuditStatus, AppError> {
    let audit: Option<(
        String,
        Json<CleanupAuditCounts>,
        Json<CleanupAuditCounts>,
        String,
        String,
    )> = sqlx::query_as(
        r#"SELECT mapping_algorithm_version, source_counts, target_counts,
                  source_checksum::text, target_checksum::text
           FROM academic_core_cutover_audits
           WHERE migration_version = $1"#,
    )
    .bind(PHASE_B_MIGRATION_VERSION)
    .fetch_optional(pool)
    .await?;

    let Some((mapping_version, source_counts, target_counts, source_checksum, target_checksum)) =
        audit
    else {
        return Ok(AcademicCoreCleanupAuditStatus {
            migration_version: PHASE_B_MIGRATION_VERSION,
            completed: false,
            checks: vec![check("ACADEMIC_CORE_CLEANUP_AUDIT_MISSING", 1, 0, false)],
        });
    };

    let counts_passed =
        mapping_version == CLEANUP_MAPPING_VERSION && source_counts.0 == target_counts.0;

    let checksums_passed = checksum_is_valid(&source_checksum)
        && checksum_is_valid(&target_checksum)
        && source_checksum.trim() == target_checksum.trim();
    let checks = vec![
        check(
            "ACADEMIC_CORE_CLEANUP_AUDIT_COUNTS",
            CLEANUP_COUNT_KEYS.len() as i64,
            CLEANUP_COUNT_KEYS.len() as i64,
            counts_passed,
        ),
        check(
            "ACADEMIC_CORE_CLEANUP_AUDIT_CHECKSUMS",
            1,
            i64::from(checksums_passed),
            checksums_passed,
        ),
    ];

    Ok(AcademicCoreCleanupAuditStatus {
        migration_version: PHASE_B_MIGRATION_VERSION,
        completed: checks.iter().all(|entry| entry.passed),
        checks,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleanup_checksum_requires_exact_hex_digest() {
        assert!(checksum_is_valid(&"a".repeat(64)));
        assert!(!checksum_is_valid(&"z".repeat(64)));
        assert!(!checksum_is_valid(&"a".repeat(63)));
    }
}
