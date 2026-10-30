use std::collections::BTreeMap;

use school_database::DbSession;
use school_errors::AppError;
use serde::Deserialize;
use sqlx::{PgPool, types::Json};

use crate::models::ReconciliationCheck;

pub const CONTEXT_CUTOVER_VERSION: i64 = 147;
#[derive(Debug, Clone)]
pub struct AcademicContextCutoverAuditStatus {
    pub completed: bool,
    pub checks: Vec<ReconciliationCheck>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RetainedFactDigest {
    row_count: i64,
    sha256: String,
}

fn check(code: &str, source: i64, target: i64, passed: bool) -> ReconciliationCheck {
    ReconciliationCheck {
        code: code.into(),
        source_count: source,
        target_count: target,
        passed,
    }
}
fn valid_evidence(facts: &BTreeMap<String, RetainedFactDigest>) -> bool {
    facts.contains_key("academic_years")
        && facts.contains_key("academic_terms")
        && facts.values().all(|fact| {
            fact.row_count >= 0
                && fact.sha256.len() == 64
                && fact
                    .sha256
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })
}
pub async fn read_academic_context_cutover_audit(
    pool: &PgPool,
) -> Result<AcademicContextCutoverAuditStatus, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_core.context_reconciliation.read",
    )
    .await?;
    let audit:Option<(Json<BTreeMap<String,RetainedFactDigest>>,i64,i64,i64)>=DbSession::new(&mut *connection).query_as("academic_core.context_reconciliation.evidence",
        "SELECT retained_facts,mapped_calendar_targets,materialized_attendance_days,scheduled_behavior_resets FROM academic_context_cutover_audit WHERE singleton").fetch_optional().await?;
    let (columns,scopes,defaults,legacy,protected):(i64,i64,i64,i64,bool)=DbSession::new(&mut *connection).query_as("academic_core.context_reconciliation.current_invariants",r#"
        SELECT
          (SELECT count(*) FROM information_schema.columns WHERE table_schema=current_schema() AND
            ((table_name='academic_years' AND column_name IN('start_date','end_date')) OR
             (table_name='academic_terms' AND column_name IN('start_date','planned_end_date','closed_on')))),
          (SELECT count(*) FROM calendar_event_targets target LEFT JOIN homerooms room ON room.id=target.homeroom_id WHERE (target.academic_year_id IS NULL AND (target.grade_level_id IS NOT NULL OR target.homeroom_id IS NOT NULL)) OR (target.homeroom_id IS NOT NULL AND (room.id IS NULL OR room.academic_year_id IS DISTINCT FROM target.academic_year_id))),
          (SELECT count(*) FROM academic_context_settings settings WHERE
            (settings.default_academic_year_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM academic_years year WHERE year.id=settings.default_academic_year_id AND year.is_visible))
            OR (settings.default_academic_term_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM academic_terms term WHERE term.id=settings.default_academic_term_id AND term.academic_year_id=settings.default_academic_year_id AND term.is_visible)))
            +CASE WHEN (SELECT count(*) FROM academic_context_settings)=1 THEN 0 ELSE 1 END,
          (SELECT count(*) FROM pg_proc function_record JOIN pg_namespace namespace ON namespace.oid=function_record.pronamespace WHERE namespace.nspname=current_schema() AND function_record.proname IN('academic_assert_term_within_year','academic_validate_offering_availability')),
          EXISTS(SELECT 1 FROM pg_trigger WHERE tgrelid='academic_context_cutover_audit'::regclass AND tgname='academic_context_cutover_audit_immutable' AND tgenabled='O')
    "#).fetch_one().await?;
    let evidence = audit.as_ref().is_some_and(|(facts, mapped, days, resets)| {
        protected && valid_evidence(&facts.0) && *mapped >= 0 && *days >= 0 && *resets >= 0
    });
    let checks = vec![
        check(
            "ACADEMIC_CONTEXT_CUTOVER_EVIDENCE",
            1,
            i64::from(evidence),
            evidence,
        ),
        check("ACADEMIC_CONTEXT_RETIRED_COLUMNS", 0, columns, columns == 0),
        check(
            "ACADEMIC_CONTEXT_EXPLICIT_CALENDAR_SCOPE",
            0,
            scopes,
            scopes == 0,
        ),
        check(
            "ACADEMIC_CONTEXT_VISIBLE_DEFAULTS",
            0,
            defaults,
            defaults == 0,
        ),
        check(
            "ACADEMIC_CONTEXT_LEGACY_OWNER_REMOVED",
            0,
            legacy,
            legacy == 0,
        ),
    ];
    Ok(AcademicContextCutoverAuditStatus {
        completed: checks.iter().all(|check| check.passed),
        checks,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn evidence_requires_both_identity_owners_valid_counts_and_real_digest_shapes() {
        let mut facts = BTreeMap::new();
        assert!(!valid_evidence(&facts));
        facts.insert(
            "academic_years".into(),
            RetainedFactDigest {
                row_count: 0,
                sha256: "a".repeat(64),
            },
        );
        facts.insert(
            "academic_terms".into(),
            RetainedFactDigest {
                row_count: 2,
                sha256: "0".repeat(64),
            },
        );
        assert!(valid_evidence(&facts));
        facts.get_mut("academic_terms").unwrap().row_count = -1;
        assert!(!valid_evidence(&facts));
    }
}
