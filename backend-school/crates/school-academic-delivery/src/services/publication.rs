use chrono::NaiveDate;
use school_database::DbSession;
use school_errors::AppError;
use sqlx::PgConnection;
use uuid::Uuid;

use crate::models::versions::{DeliveryPublicationContext, DeliveryVersion, PublicationDateRules};

pub const PUBLICATION_REVISIONS_VERSION: i64 = 151;
pub const PUBLICATION_REVISIONS_CHECK_CODES: [&str; 8] = [
    "delivery_publications_preserved",
    "downstream_sources_preserved",
    "offering_registry_preserved",
    "projection_integrity_reconciled",
    "publication_numbers_reconciled",
    "publication_receipts_preserved",
    "teacher_episodes_preserved",
    "timetable_publications_preserved",
];

#[derive(Debug)]
pub struct PublicationRevisionsAudit {
    pub completed: bool,
    pub checks: Vec<super::versions::DeliveryCutoverCheck>,
}

fn complete_evidence(passed: bool, checks: &[super::versions::DeliveryCutoverCheck]) -> bool {
    let codes = checks
        .iter()
        .map(|check| check.code.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    passed
        && checks.len() == PUBLICATION_REVISIONS_CHECK_CODES.len()
        && codes.into_iter().eq(PUBLICATION_REVISIONS_CHECK_CODES)
        && checks.iter().all(|check| check.passed && check.count == 0)
}

/// The immutable migration receipt describes reconciliation at cutover, not evolving live counts.
pub async fn read_revisions_audit_on_connection(
    connection: &mut PgConnection,
) -> Result<PublicationRevisionsAudit, AppError> {
    let row: Option<(bool,sqlx::types::Json<Vec<super::versions::DeliveryCutoverCheck>>)> = DbSession::new(connection)
        .query_as("academic_delivery.services.publication.revisions_audit","SELECT passed,checks FROM academic_publication_revisions_audit WHERE migration_version=$1")
        .bind(PUBLICATION_REVISIONS_VERSION).fetch_optional().await?;
    match row {
        Some((passed, checks)) => Ok(PublicationRevisionsAudit {
            completed: complete_evidence(passed, &checks.0),
            checks: checks.0,
        }),
        None => Ok(PublicationRevisionsAudit {
            completed: false,
            checks: vec![],
        }),
    }
}

pub async fn read_revisions_audit(
    pool: &sqlx::PgPool,
) -> Result<PublicationRevisionsAudit, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_delivery.services.publication.revisions_audit.checkout",
    )
    .await?;
    read_revisions_audit_on_connection(&mut connection).await
}

pub async fn date_rules_on_connection(
    connection: &mut PgConnection,
    term: Uuid,
) -> Result<PublicationDateRules, AppError> {
    let active: bool = DbSession::new(connection)
        .scalar(
            "academic_delivery.services.publication.date_rules",
            "SELECT status='active' FROM academic_terms WHERE id=$1",
        )
        .bind(term)
        .fetch_optional()
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบภาคเรียน".into()))?;
    let today = super::bangkok_today();
    Ok(PublicationDateRules {
        today,
        minimum_effective_from: active.then_some(today),
    })
}

pub fn context(version: &DeliveryVersion) -> Result<DeliveryPublicationContext, AppError> {
    if version.status != crate::models::versions::DeliveryVersionStatus::Published {
        return Err(AppError::Conflict("รุ่นเปิดสอนยังไม่ได้เผยแพร่".into()));
    }
    let publication_no = version
        .publication_no
        .ok_or_else(|| AppError::Conflict("รุ่นเปิดสอนยังไม่ได้เผยแพร่".into()))?;
    let effective_from = version
        .effective_from
        .ok_or_else(|| AppError::Conflict("รุ่นเปิดสอนไม่มีวันที่เริ่มใช้".into()))?;
    Ok(DeliveryPublicationContext {
        id: version.id,
        publication_no,
        effective_from,
        effective_until: version.effective_until,
        published_at: version.published_at,
        is_superseded: version.is_superseded,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn checks() -> Vec<super::super::versions::DeliveryCutoverCheck> {
        PUBLICATION_REVISIONS_CHECK_CODES
            .iter()
            .map(|code| super::super::versions::DeliveryCutoverCheck {
                code: (*code).into(),
                count: 0,
                passed: true,
            })
            .collect()
    }
    #[test]
    fn preservation_requires_every_exact_successful_check_without_duplicate_codes() {
        let mut evidence = checks();
        assert!(complete_evidence(true, &evidence));
        evidence.reverse();
        assert!(complete_evidence(true, &evidence));
        assert!(!complete_evidence(false, &evidence));
        evidence[0].count = 1;
        assert!(!complete_evidence(true, &evidence));
        evidence[0].count = 0;
        evidence[0].code = evidence[1].code.clone();
        assert!(!complete_evidence(true, &evidence));
        assert!(!complete_evidence(true, &[]));
    }
}

pub async fn context_on_connection(
    connection: &mut PgConnection,
    id: Uuid,
) -> Result<DeliveryPublicationContext, AppError> {
    context(&super::versions::get_version_on_connection(connection, id).await?)
}

/// Select the date's winner, independently of the latest planning publication.
pub async fn applicable_on_connection(
    connection: &mut PgConnection,
    term: Uuid,
    date: NaiveDate,
) -> Result<Option<DeliveryPublicationContext>, AppError> {
    let id: Option<Uuid> = DbSession::new(&mut *connection).scalar("academic_delivery.services.publication.applicable", "SELECT id FROM academic_delivery_versions WHERE academic_term_id=$1 AND status='published' AND effective_from<=$2 ORDER BY publication_no DESC LIMIT 1")
        .bind(term).bind(date).fetch_optional().await?;
    match id {
        Some(id) => Ok(Some(context_on_connection(connection, id).await?)),
        None => Ok(None),
    }
}
