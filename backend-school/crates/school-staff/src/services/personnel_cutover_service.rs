//! Migration-only preflight and durable cutover evidence; never a profile read fallback.
use school_errors::AppError;
use serde::{Deserialize, Serialize};
use sqlx::{types::Json, PgPool};

pub const PERSONNEL_MIGRATION_VERSION: i64 = 81;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonnelCheck {
    pub code: String,
    pub passed: bool,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonnelPreflightReport {
    pub migration_version: i64,
    pub passed: bool,
    pub checks: Vec<PersonnelCheck>,
}

pub type PersonnelCutoverAudit = PersonnelPreflightReport;

fn report(migration_version: i64, checks: Vec<PersonnelCheck>) -> PersonnelPreflightReport {
    PersonnelPreflightReport {
        migration_version,
        passed: !checks.is_empty() && checks.iter().all(|check| check.passed && check.count >= 0),
        checks,
    }
}

fn zero_check(code: &str, count: i64) -> PersonnelCheck {
    PersonnelCheck {
        code: code.to_string(),
        passed: count == 0,
        count,
    }
}

// Kept only for version-gated release preflight. DB tests prove agreement with migration 081.
const LEGACY_EDUCATION_ALIASES: &[&str] = &[
    "primary",
    "ประถมศึกษา",
    "lower_secondary",
    "มัธยมศึกษาตอนต้น",
    "ม.3",
    "upper_secondary",
    "มัธยมศึกษาตอนปลาย",
    "ม.6",
    "vocational_certificate",
    "ปวช.",
    "ประกาศนียบัตรวิชาชีพ",
    "higher_vocational",
    "ปวส.",
    "ประกาศนียบัตรวิชาชีพชั้นสูง",
    "diploma",
    "อนุปริญญา",
    "bachelor",
    "ปริญญาตรี",
    "ป.ตรี",
    "bachelor's degree",
    "bachelor degree",
    "master",
    "ปริญญาโท",
    "ป.โท",
    "master's degree",
    "master degree",
    "doctorate",
    "ปริญญาเอก",
    "ป.เอก",
    "doctoral degree",
    "phd",
    "ph.d.",
    "other",
    "อื่น ๆ",
    "อื่นๆ",
];

pub async fn read_personnel_preflight(pool: &PgPool) -> Result<PersonnelPreflightReport, AppError> {
    let (has_history, has_info, has_users): (bool, bool, bool) = sqlx::query_as(
        "SELECT to_regclass('_sqlx_migrations') IS NOT NULL, to_regclass('staff_info') IS NOT NULL, to_regclass('users') IS NOT NULL",
    ).fetch_one(pool).await?;
    if !has_history {
        return Ok(report(
            0,
            vec![zero_check(
                "PERSONNEL_EMPTY_DATABASE",
                i64::from(has_info || has_users),
            )],
        ));
    }
    let (version, failed): (i64, i64) = sqlx::query_as(
        "SELECT coalesce(max(version) FILTER (WHERE success), 0)::bigint, count(*) FILTER (WHERE NOT success) FROM _sqlx_migrations",
    ).fetch_one(pool).await?;
    if version >= PERSONNEL_MIGRATION_VERSION {
        let audit = read_personnel_cutover_audit(pool).await?;
        let mut checks = audit.checks;
        checks.push(zero_check("PERSONNEL_MIGRATION_HISTORY_VALID", failed));
        return Ok(report(version, checks));
    }
    if !has_info {
        return Ok(report(
            version,
            vec![zero_check("PERSONNEL_SOURCE_SCHEMA_MISSING", 1)],
        ));
    }
    let (unmapped, invalid): (i64, i64) = sqlx::query_as(
        r#"WITH source AS (
          SELECT nullif(lower(btrim(regexp_replace(education_level, '[[:space:]]+', ' ', 'g'))), '') AS degree,
                 btrim(regexp_replace(major, '[[:space:]]+', ' ', 'g')) AS major,
                 btrim(regexp_replace(university, '[[:space:]]+', ' ', 'g')) AS university
          FROM staff_info
        ) SELECT count(*) FILTER (WHERE degree IS NOT NULL AND NOT (degree = ANY($1::text[]))),
                 count(*) FILTER (WHERE major ~ '[[:cntrl:]]' OR university ~ '[[:cntrl:]]') FROM source"#,
    ).bind(LEGACY_EDUCATION_ALIASES).fetch_one(pool).await?;
    Ok(report(
        version,
        vec![
            zero_check("PERSONNEL_MIGRATION_HISTORY_VALID", failed),
            zero_check("PERSONNEL_EDUCATION_UNMAPPED", unmapped),
            zero_check("PERSONNEL_REFERENCE_INVALID", invalid),
        ],
    ))
}

pub async fn read_personnel_cutover_audit(
    pool: &PgPool,
) -> Result<PersonnelCutoverAudit, AppError> {
    let has_audit: bool =
        sqlx::query_scalar("SELECT to_regclass('staff_personnel_cutover_audit') IS NOT NULL")
            .fetch_one(pool)
            .await?;
    if !has_audit {
        return Ok(report(
            PERSONNEL_MIGRATION_VERSION,
            vec![zero_check("PERSONNEL_AUDIT_UNAVAILABLE", 1)],
        ));
    }
    let stored: Option<(bool, Json<Vec<PersonnelCheck>>)> = sqlx::query_as(
        "SELECT passed, checks FROM staff_personnel_cutover_audit WHERE migration_version = $1",
    )
    .bind(PERSONNEL_MIGRATION_VERSION)
    .fetch_optional(pool)
    .await?;
    let Some((passed, Json(mut checks))) = stored else {
        return Ok(report(
            PERSONNEL_MIGRATION_VERSION,
            vec![zero_check("PERSONNEL_AUDIT_UNAVAILABLE", 1)],
        ));
    };
    let required = [
        "PERSONNEL_ROWS_PRESERVED",
        "PERSONNEL_RELATIONSHIPS_PRESERVED",
        "PERSONNEL_EDUCATION_MAPPED",
        "PERSONNEL_REFERENCES_MAPPED",
    ];
    let valid = passed
        && checks.len() == required.len()
        && required
            .iter()
            .all(|code| checks.iter().any(|check| check.code == *code));
    checks.push(zero_check(
        "PERSONNEL_RECONCILIATION_REQUIRED",
        i64::from(!valid),
    ));
    let (missing_columns, legacy_columns, missing_constraints): (i64, i64, i64) = sqlx::query_as(
        r#"SELECT
          (SELECT count(*) FROM (VALUES ('job_position_id'), ('academic_rank'), ('major_id'), ('university_id')) required(name)
           WHERE NOT EXISTS (SELECT 1 FROM information_schema.columns c WHERE c.table_schema = current_schema() AND c.table_name = 'staff_info' AND c.column_name = required.name)),
          (SELECT count(*) FROM information_schema.columns c WHERE c.table_schema = current_schema() AND c.table_name = 'staff_info' AND c.column_name IN ('major', 'university')),
          (SELECT count(*) FROM (VALUES ('staff_info_position_reference_fkey'), ('staff_info_major_reference_fkey'), ('staff_info_university_reference_fkey'), ('staff_info_education_code_check'), ('staff_info_academic_rank_check')) required(name)
           WHERE NOT EXISTS (SELECT 1 FROM pg_constraint c JOIN pg_class t ON t.oid = c.conrelid JOIN pg_namespace n ON n.oid = t.relnamespace WHERE n.nspname = current_schema() AND t.relname = 'staff_info' AND c.conname = required.name))"#,
    ).fetch_one(pool).await?;
    checks.extend([
        zero_check("PERSONNEL_CANONICAL_COLUMNS_REQUIRED", missing_columns),
        zero_check("PERSONNEL_LEGACY_COLUMNS_REMOVED", legacy_columns),
        zero_check(
            "PERSONNEL_CANONICAL_CONSTRAINTS_REQUIRED",
            missing_constraints,
        ),
    ]);
    Ok(report(PERSONNEL_MIGRATION_VERSION, checks))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn personnel_report_never_accepts_empty_or_failed_checks() {
        assert!(!report(81, vec![]).passed);
        assert!(!report(81, vec![zero_check("PERSONNEL_TEST", 1)]).passed);
        assert!(report(81, vec![zero_check("PERSONNEL_TEST", 0)]).passed);
    }
}
