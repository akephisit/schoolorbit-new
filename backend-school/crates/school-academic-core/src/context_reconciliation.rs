use std::collections::BTreeMap;

use school_database::DbSession;
use school_errors::AppError;
use serde::Deserialize;
use sqlx::{types::Json, PgConnection, PgPool};

use crate::models::ReconciliationCheck;

pub const CONTEXT_CUTOVER_VERSION: i64 = 149;
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReceiptRecoveryEvidence {
    protocol_version: i64,
    source_version: i64,
    receipt_count: i64,
    recovered_receipt_count: i64,
    original_sha256: String,
    reconstructed_sha256: String,
    #[serde(deserialize_with = "required_nullable")]
    projected_sha256: Option<String>,
    final_sha256: String,
    native148_receipt_sha256: String,
    native148_receipt_count: i64,
    native148_link_sha256: String,
    #[serde(deserialize_with = "required_nullable")]
    nonreceipt_sha256: Option<String>,
    #[serde(deserialize_with = "required_nullable")]
    legacy_dates_unbound: Option<bool>,
    original_column_preserved: bool,
    phase_proof_passed: bool,
}

fn required_nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

fn digest_shape(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_receipt_recovery(
    proof: &ReceiptRecoveryEvidence,
    native: &RetainedFactDigest,
    native_link: &str,
    nonreceipt: &str,
) -> bool {
    if proof.protocol_version != 1
        || proof.receipt_count < 0
        || proof.recovered_receipt_count < 0
        || proof.recovered_receipt_count > proof.receipt_count
        || !proof.original_column_preserved
        || !proof.phase_proof_passed
        || proof.original_sha256 != proof.reconstructed_sha256
        || proof.original_sha256 != proof.final_sha256
        || proof.native148_receipt_count != native.row_count
        || proof.native148_receipt_sha256 != native.sha256
        || proof.native148_link_sha256 != native_link
        || ![
            &proof.original_sha256,
            &proof.reconstructed_sha256,
            &proof.final_sha256,
            &proof.native148_receipt_sha256,
            &proof.native148_link_sha256,
        ]
        .into_iter()
        .all(|value| digest_shape(value))
    {
        return false;
    }
    match proof.source_version {
        147 => {
            proof.receipt_count == proof.native148_receipt_count
                && proof.projected_sha256.as_deref() == Some(native.sha256.as_str())
                && proof.nonreceipt_sha256.as_deref() == Some(nonreceipt)
                && proof.legacy_dates_unbound == Some(true)
        }
        148 => {
            proof.recovered_receipt_count == 0
                && proof.projected_sha256.is_none()
                && proof.nonreceipt_sha256.is_none()
                && proof.legacy_dates_unbound.is_none()
        }
        _ => false,
    }
}

async fn receipt_recovery_evidence(
    connection: &mut PgConnection,
    native: Option<&RetainedFactDigest>,
) -> Result<bool, AppError> {
    let (present,protected,staging): (bool,bool,bool) = DbSession::new(connection).query_as(
        "academic_core.context_reconciliation.receipt_recovery_schema",
        "SELECT to_regclass(format('%I.academic_context_receipt_recovery_audit',current_schema())) IS NOT NULL,EXISTS(SELECT 1 FROM pg_trigger WHERE tgrelid=to_regclass(format('%I.academic_context_receipt_recovery_audit',current_schema())) AND tgname='academic_context_receipt_recovery_audit_immutable' AND tgenabled='O' AND tgfoid=to_regprocedure(format('%I.academic_reject_official_result_mutation()',current_schema()))),EXISTS(SELECT 1 FROM pg_attribute WHERE attrelid='learning_delivery_apply_runs'::regclass AND NOT attisdropped AND starts_with(attname,'academic149_')) OR EXISTS(SELECT 1 FROM pg_class relation WHERE relation.relnamespace IN(current_schema()::regnamespace,pg_my_temp_schema()) AND starts_with(relation.relname,'academic149_')) OR EXISTS(SELECT 1 FROM pg_proc function_record WHERE function_record.pronamespace IN(current_schema()::regnamespace,pg_my_temp_schema()) AND starts_with(function_record.proname,'academic149_')) OR EXISTS(SELECT 1 FROM pg_type type_record WHERE type_record.typnamespace IN(current_schema()::regnamespace,pg_my_temp_schema()) AND starts_with(type_record.typname,'academic149_'))",
    ).fetch_one().await?;
    if !present || !protected || staging {
        return Ok(false);
    }
    let raw = DbSession::new(connection)
        .scalar::<Json<ReceiptRecoveryEvidence>>(
            "academic_core.context_reconciliation.receipt_recovery_evidence",
            "SELECT evidence FROM academic_context_receipt_recovery_audit WHERE singleton",
        )
        .fetch_optional()
        .await;
    let proof = match raw {
        Ok(value) => value.map(|value| value.0),
        Err(sqlx::Error::ColumnDecode { .. } | sqlx::Error::Decode(_)) => None,
        Err(error) => return Err(error.into()),
    };
    // This hashes only immutable facts/counters, never checked_at or current receipts.
    let link: Option<(String,String)> = DbSession::new(connection).query_as(
        "academic_core.context_reconciliation.receipt_recovery_link",
        "SELECT encode(sha256(convert_to(jsonb_build_object('retainedFacts',retained_facts,'mappedCalendarTargets',mapped_calendar_targets,'materializedAttendanceDays',materialized_attendance_days,'scheduledBehaviorResets',scheduled_behavior_resets)::text,'UTF8')),'hex'),encode(sha256(convert_to((retained_facts-'learning_delivery_apply_runs')::text,'UTF8')),'hex') FROM academic_context_cutover_audit WHERE singleton",
    ).fetch_optional().await?;
    Ok(match (proof, native, link) {
        (Some(proof), Some(native), Some((link, nonreceipt))) => {
            valid_receipt_recovery(&proof, native, &link, &nonreceipt)
        }
        _ => false,
    })
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
    let receipt_recovery = receipt_recovery_evidence(
        &mut connection,
        audit
            .as_ref()
            .and_then(|(facts, _, _, _)| facts.0.get("learning_delivery_apply_runs")),
    )
    .await?;
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
        check(
            "ACADEMIC_CONTEXT_RECEIPT_RECOVERY_EVIDENCE",
            1,
            i64::from(receipt_recovery),
            receipt_recovery,
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
    fn recovery_evidence_requires_all_fifteen_keys_including_explicit_nulls() {
        let value = serde_json::json!({"protocolVersion":1,"sourceVersion":148,"receiptCount":2,"recoveredReceiptCount":0,"originalSha256":"a".repeat(64),"reconstructedSha256":"a".repeat(64),"projectedSha256":null,"finalSha256":"a".repeat(64),"native148ReceiptSha256":"b".repeat(64),"native148ReceiptCount":1,"native148LinkSha256":"c".repeat(64),"nonreceiptSha256":null,"legacyDatesUnbound":null,"originalColumnPreserved":true,"phaseProofPassed":true});
        let proof: ReceiptRecoveryEvidence = serde_json::from_value(value.clone()).unwrap();
        let native = RetainedFactDigest {
            row_count: 1,
            sha256: "b".repeat(64),
        };
        assert!(valid_receipt_recovery(
            &proof,
            &native,
            &"c".repeat(64),
            &"d".repeat(64)
        ));
        for key in value.as_object().unwrap().keys() {
            let mut missing = value.clone();
            missing.as_object_mut().unwrap().remove(key);
            assert!(
                serde_json::from_value::<ReceiptRecoveryEvidence>(missing).is_err(),
                "accepted missing key {key}"
            );
            let mut wrong_type = value.clone();
            wrong_type[key] = serde_json::json!(["invalid"]);
            assert!(
                serde_json::from_value::<ReceiptRecoveryEvidence>(wrong_type).is_err(),
                "accepted wrong type for {key}"
            );
        }
        let mut unknown = value;
        unknown["privatePayload"] = serde_json::json!("private-sentinel");
        assert!(serde_json::from_value::<ReceiptRecoveryEvidence>(unknown).is_err());
    }
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
