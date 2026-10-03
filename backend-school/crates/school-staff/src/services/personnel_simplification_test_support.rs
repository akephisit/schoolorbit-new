//! Test-only assertions for historical migration preservation and refusal fixtures.
use super::personnel_migration_test_support::{
    report, zero_check, PersonnelCheck, PersonnelCutoverAudit, PERSONNEL_MIGRATION_VERSION,
};
use school_errors::AppError;
use sqlx::{types::Json, PgPool};

const PRESERVATION_CODES: [&str; 4] = [
    "PERSONNEL_SIMPLIFICATION_STAFF_PRESERVED",
    "PERSONNEL_SIMPLIFICATION_POSITIONS_PRESERVED",
    "PERSONNEL_SIMPLIFICATION_EDUCATION_TEXT_PRESERVED",
    "PERSONNEL_SIMPLIFICATION_UNRELATED_FIELDS_PRESERVED",
];

fn valid_preservation_checks(checks: &[PersonnelCheck], staff_count: i64, positions: i64) -> bool {
    checks.len() == PRESERVATION_CODES.len()
        && PRESERVATION_CODES.iter().all(|code| {
            checks.iter().filter(|c| c.code == *code).count() == 1
                && checks.iter().any(|c| {
                    c.code == *code
                        && c.passed
                        && c.count
                            == if *code == PRESERVATION_CODES[1] {
                                positions
                            } else {
                                staff_count
                            }
                        && c.count >= 0
                })
        })
}

pub(super) async fn source_check(pool: &PgPool) -> Result<PersonnelCheck, AppError> {
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM staff_info i
         LEFT JOIN staff_reference_items p ON p.id=i.job_position_id AND p.kind='job_position'
         LEFT JOIN staff_reference_items m ON m.id=i.major_id AND m.kind='major'
         LEFT JOIN staff_reference_items u ON u.id=i.university_id AND u.kind='university'
         WHERE (i.job_position_id IS NOT NULL AND p.id IS NULL)
            OR (i.major_id IS NOT NULL AND m.id IS NULL)
            OR (i.university_id IS NOT NULL AND u.id IS NULL)
            OR m.name ~ '[[:cntrl:]]' OR u.name ~ '[[:cntrl:]]'",
    )
    .fetch_one(pool)
    .await?;
    Ok(zero_check("PERSONNEL_SIMPLIFICATION_SOURCE_INVALID", count))
}

async fn canonical_schema_errors(pool: &PgPool, completed: bool) -> Result<i64, AppError> {
    Ok(sqlx::query_scalar(r#"SELECT
          (SELECT count(*) FROM (VALUES
             ('staff_info','job_position_id','uuid',NULL::integer,true),
             ('staff_info','academic_rank','varchar',32,true),
             ('staff_info','education_level','varchar',100,true),
             ('staff_info','major','varchar',200,true),
             ('staff_info','university','varchar',200,true),
             ('staff_job_positions','id','uuid',NULL::integer,false),
             ('staff_job_positions','code','varchar',64,false),
             ('staff_job_positions','name','varchar',200,false),
             ('staff_job_positions','is_active','bool',NULL::integer,false),
             ('staff_job_positions','is_selectable','bool',NULL::integer,false),
             ('staff_job_positions','display_order','int4',NULL::integer,false),
             ('staff_job_positions','created_at','timestamptz',NULL::integer,false),
             ('staff_job_positions','updated_at','timestamptz',NULL::integer,false)
          ) required(table_name,column_name,udt_name,max_length,nullable)
           WHERE NOT EXISTS (SELECT 1 FROM information_schema.columns c
             WHERE c.table_schema=current_schema() AND c.table_name=required.table_name
               AND c.column_name=required.column_name AND c.udt_name=required.udt_name
               AND c.character_maximum_length IS NOT DISTINCT FROM required.max_length
               AND (c.is_nullable='YES')=required.nullable))
          + (SELECT count(*) FROM (VALUES
             ('staff_info','staff_info_job_position_fkey','f'),
             ('staff_info','staff_info_education_code_check','c'),
             ('staff_info','staff_info_academic_rank_check','c'),
             ('staff_info','staff_info_major_text_check','c'),
             ('staff_info','staff_info_university_text_check','c'),
             ('staff_job_positions','staff_job_positions_pkey','p'),
             ('staff_job_positions','staff_job_positions_code_key','u'),
             ('staff_job_positions','staff_job_positions_selection_check','c')
          ) required(table_name,constraint_name,kind)
           WHERE (required.constraint_name<>'staff_info_job_position_fkey' OR $1)
             AND NOT EXISTS (SELECT 1 FROM pg_constraint c
             WHERE c.conrelid=to_regclass(current_schema() || '.' || required.table_name)
               AND c.conname=required.constraint_name AND c.contype::text=required.kind AND c.convalidated))
          + CASE WHEN NOT $1 OR EXISTS (SELECT 1 FROM pg_constraint c
              WHERE c.conrelid=to_regclass(current_schema() || '.staff_info') AND c.conname='staff_info_job_position_fkey'
                AND c.confrelid=to_regclass(current_schema() || '.staff_job_positions')
                AND c.conkey=ARRAY[(SELECT attnum FROM pg_attribute WHERE attrelid=c.conrelid AND attname='job_position_id')]::smallint[]
                AND c.confkey=ARRAY[(SELECT attnum FROM pg_attribute WHERE attrelid=c.confrelid AND attname='id')]::smallint[])
            THEN 0 ELSE 1 END"#)
        .bind(completed).fetch_one(pool).await?)
}

pub(super) async fn read_audit(
    pool: &PgPool,
    version: i64,
) -> Result<PersonnelCutoverAudit, AppError> {
    let exists: bool = sqlx::query_scalar("SELECT to_regclass(current_schema() || '.staff_personnel_simplification_audit') IS NOT NULL")
        .fetch_one(pool).await?;
    if !exists {
        return Ok(report(
            PERSONNEL_MIGRATION_VERSION,
            vec![zero_check("PERSONNEL_SIMPLIFICATION_AUDIT_UNAVAILABLE", 1)],
        ));
    }
    let row: Option<(bool,bool,i64,i64,bool,bool,Json<Vec<PersonnelCheck>>)> = sqlx::query_as(
        "SELECT passed,cutover_completed,source_count,position_count,completed_at IS NOT NULL,
                source_fingerprint ~ '^[a-f0-9]{32}$' AND target_fingerprint ~ '^[a-f0-9]{32}$',checks
         FROM staff_personnel_simplification_audit WHERE migration_version=$1",
    ).bind(PERSONNEL_MIGRATION_VERSION).fetch_optional(pool).await?;
    let Some((
        passed,
        completed,
        staff_count,
        positions,
        has_time,
        valid_fingerprints,
        Json(mut checks),
    )) = row
    else {
        return Ok(report(
            PERSONNEL_MIGRATION_VERSION,
            vec![zero_check("PERSONNEL_SIMPLIFICATION_AUDIT_UNAVAILABLE", 1)],
        ));
    };
    let expanded = version < PERSONNEL_MIGRATION_VERSION;
    if !passed
        || !valid_fingerprints
        || !valid_preservation_checks(&checks, staff_count, positions)
        || completed == expanded
        || has_time == expanded
    {
        return Ok(report(
            PERSONNEL_MIGRATION_VERSION,
            vec![zero_check("PERSONNEL_SIMPLIFICATION_AUDIT_INVALID", 1)],
        ));
    }
    if expanded {
        checks.push(source_check(pool).await?);
        checks.push(zero_check(
            "PERSONNEL_SIMPLIFICATION_CANONICAL_SCHEMA_VALID",
            canonical_schema_errors(pool, false).await?,
        ));
        let fresh: bool = sqlx::query_scalar(
            "SELECT source_fingerprint=staff_personnel_simplification_source_fingerprint()
                AND target_fingerprint=md5(staff_personnel_simplification_snapshot(false)::text)
                AND staff_personnel_simplification_snapshot(true)=staff_personnel_simplification_snapshot(false)
                AND source_count=(SELECT count(*) FROM staff_info)
                AND position_count=(SELECT count(*) FROM staff_job_positions)
             FROM staff_personnel_simplification_audit WHERE migration_version=$1",
        ).bind(PERSONNEL_MIGRATION_VERSION).fetch_one(pool).await?;
        checks.push(zero_check(
            "PERSONNEL_SIMPLIFICATION_RECONCILIATION_REQUIRED",
            i64::from(!fresh),
        ));
        return Ok(report(PERSONNEL_MIGRATION_VERSION, checks));
    }
    let schema_errors = canonical_schema_errors(pool, true).await?;
    let (retired_owners,failed_history): (i64,i64) = sqlx::query_as(
        r#"SELECT
          (SELECT count(*) FROM information_schema.columns WHERE table_schema=current_schema() AND table_name='staff_info'
             AND column_name IN ('major_id','university_id','job_position_kind','major_kind','university_kind'))
          + CASE WHEN to_regclass(current_schema() || '.staff_reference_items') IS NULL THEN 0 ELSE 1 END
          + CASE WHEN to_regprocedure(current_schema() || '.staff_reference_display_name(text)') IS NULL THEN 0 ELSE 1 END
          + CASE WHEN to_regprocedure(current_schema() || '.staff_personnel_simplification_snapshot(boolean)') IS NULL THEN 0 ELSE 1 END
          + CASE WHEN to_regprocedure(current_schema() || '.staff_personnel_simplification_source_fingerprint()') IS NULL THEN 0 ELSE 1 END
          + CASE WHEN to_regprocedure(current_schema() || '.staff_personnel_simplification_schema_errors(boolean)') IS NULL THEN 0 ELSE 1 END,
          (SELECT count(*) FROM _sqlx_migrations WHERE NOT success)"#,
    ).fetch_one(pool).await?;
    checks.extend([
        zero_check(
            "PERSONNEL_SIMPLIFICATION_CANONICAL_SCHEMA_VALID",
            schema_errors,
        ),
        zero_check(
            "PERSONNEL_SIMPLIFICATION_RETIRED_OWNERS_REMOVED",
            retired_owners,
        ),
        zero_check("PERSONNEL_MIGRATION_HISTORY_VALID", failed_history),
    ]);
    Ok(report(PERSONNEL_MIGRATION_VERSION, checks))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn personnel_simplification_rejects_duplicate_and_wrong_count_evidence() {
        let mut checks = PRESERVATION_CODES
            .iter()
            .map(|code| PersonnelCheck {
                code: code.to_string(),
                passed: true,
                count: 0,
            })
            .collect::<Vec<_>>();
        assert!(valid_preservation_checks(&checks, 0, 0));
        checks[0].code = checks[1].code.clone();
        assert!(!valid_preservation_checks(&checks, 0, 0));
        checks[0].code = PRESERVATION_CODES[0].to_string();
        checks[0].count = 1;
        assert!(!valid_preservation_checks(&checks, 0, 0));
    }
}
