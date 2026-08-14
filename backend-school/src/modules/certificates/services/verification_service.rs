use std::net::IpAddr;

use chrono::{DateTime, Duration, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{FromRow, PgPool};
use subtle::ConstantTimeEq;
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::{
    error::AppError,
    modules::certificates::models::{
        CertificateStatus, ManualCertificateVerificationRequest, PublicCertificateVerificationData,
        QrCertificateVerificationRequest,
    },
    modules::certificates::verification_limiter::CertificateVerificationLimiter,
    utils::field_encryption,
};

use super::import_validation::{normalize_display_text, normalize_name_for_match};

const GENERIC_NOT_FOUND_MESSAGE: &str = "ไม่พบข้อมูลที่ตรงกัน";
const RECEIPT_VERSION: u8 = 1;
const PUBLIC_RENDER_ACTION: &str = "public_render";
const RECEIPT_TTL_MINUTES: i64 = 5;
const MAX_CERTIFICATE_NUMBER_LENGTH: usize = 64;
const MAX_NAME_LENGTH: usize = 100;
const MAX_PROOF_LENGTH: usize = 256;

#[derive(FromRow)]
struct PublicVerificationRow {
    id: Uuid,
    status: String,
    certificate_number: String,
    title_snapshot: Option<String>,
    first_name_snapshot: String,
    last_name_snapshot: String,
    campaign_name: String,
    academic_year_value: i32,
    template_name_snapshot: String,
    activity_item_snapshot: Option<String>,
    award_or_role_snapshot: Option<String>,
    issue_date: NaiveDate,
    school_name_snapshot: String,
    replacement_certificate_number: Option<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RenderReceiptPayload {
    version: u8,
    tenant_id: Uuid,
    certificate_id: Uuid,
    action: String,
    expires_at: DateTime<Utc>,
}

pub enum CertificateVerificationAttempt {
    Manual(ManualCertificateVerificationRequest),
    Qr(QrCertificateVerificationRequest),
}

impl CertificateVerificationAttempt {
    fn certificate_number(&self) -> &str {
        match self {
            Self::Manual(request) => &request.certificate_number,
            Self::Qr(request) => &request.certificate_number,
        }
    }
}

pub async fn verify_rate_limited(
    pool: &PgPool,
    tenant_id: Uuid,
    client_ip: IpAddr,
    limiter: &CertificateVerificationLimiter,
    attempt: CertificateVerificationAttempt,
) -> Result<PublicCertificateVerificationData, AppError> {
    let target_number = normalize_display_text(attempt.certificate_number());
    let target = CertificateVerificationLimiter::target_digest(&target_number);
    limiter.begin_attempt(tenant_id, client_ip, target)?;

    match verify(pool, tenant_id, attempt).await {
        Ok(data) => {
            limiter.record_success(tenant_id, client_ip, target);
            Ok(data)
        }
        Err(error) => {
            if is_generic_not_found(&error) {
                limiter.record_failure(tenant_id, client_ip, target)?;
            }
            Err(error)
        }
    }
}

pub async fn verify(
    pool: &PgPool,
    tenant_id: Uuid,
    attempt: CertificateVerificationAttempt,
) -> Result<PublicCertificateVerificationData, AppError> {
    let row = match attempt {
        CertificateVerificationAttempt::Manual(request) => {
            let certificate_number = normalize_certificate_number(&request.certificate_number)?;
            let first_name = normalize_name_input(&request.first_name)?;
            let last_name = normalize_name_input(&request.last_name)?;
            let row = load_by_number(pool, &certificate_number)
                .await?
                .ok_or_else(generic_not_found)?;
            if !names_match(&first_name, &last_name, &row) {
                return Err(generic_not_found());
            }
            row
        }
        CertificateVerificationAttempt::Qr(request) => {
            let certificate_number = normalize_certificate_number(&request.certificate_number)?;
            if request.proof.is_empty() || request.proof.chars().count() > MAX_PROOF_LENGTH {
                return Err(generic_not_found());
            }
            let proof_hash = field_encryption::hash_for_search_with_domain(
                "certificate-qr-proof-v1",
                &request.proof,
            )
            .map_err(verification_crypto_error)?;
            load_by_number_and_proof(pool, &certificate_number, &proof_hash)
                .await?
                .ok_or_else(generic_not_found)?
        }
    };

    public_data(row, tenant_id)
}

fn normalize_certificate_number(value: &str) -> Result<String, AppError> {
    let normalized = normalize_display_text(value);
    if normalized.is_empty() || normalized.chars().count() > MAX_CERTIFICATE_NUMBER_LENGTH {
        return Err(generic_not_found());
    }
    Ok(normalized)
}

fn normalize_name_input(value: &str) -> Result<String, AppError> {
    let normalized = normalize_name_for_match(value);
    if normalized.is_empty() || normalized.chars().count() > MAX_NAME_LENGTH {
        return Err(generic_not_found());
    }
    Ok(normalized)
}

async fn load_by_number(
    pool: &PgPool,
    certificate_number: &str,
) -> Result<Option<PublicVerificationRow>, AppError> {
    verification_query(certificate_number, None)
        .fetch_optional(pool)
        .await
        .map_err(AppError::from)
}

async fn load_by_number_and_proof(
    pool: &PgPool,
    certificate_number: &str,
    proof_hash: &str,
) -> Result<Option<PublicVerificationRow>, AppError> {
    verification_query(certificate_number, Some(proof_hash))
        .fetch_optional(pool)
        .await
        .map_err(AppError::from)
}

fn verification_query<'a>(
    certificate_number: &'a str,
    proof_hash: Option<&'a str>,
) -> sqlx::query::QueryAs<'a, sqlx::Postgres, PublicVerificationRow, sqlx::postgres::PgArguments> {
    let query = sqlx::query_as::<_, PublicVerificationRow>(
        "SELECT certificate.id, certificate.status, certificate.certificate_number,
                certificate.title_snapshot, certificate.first_name_snapshot,
                certificate.last_name_snapshot, campaign.name AS campaign_name,
                certificate.academic_year_value, certificate.template_name_snapshot,
                certificate.activity_item_snapshot, certificate.award_or_role_snapshot,
                certificate.issue_date, certificate.school_name_snapshot,
                replacement.certificate_number AS replacement_certificate_number
         FROM certificates certificate
         JOIN certificate_campaigns campaign ON campaign.id = certificate.campaign_id
         LEFT JOIN certificates replacement
           ON replacement.id = certificate.replaced_by_certificate_id
         WHERE certificate.certificate_number = $1
           AND ($2::text IS NULL OR certificate.qr_proof_hash = $2)",
    )
    .bind(certificate_number);
    query.bind(proof_hash)
}

fn names_match(
    supplied_first_name: &str,
    supplied_last_name: &str,
    row: &PublicVerificationRow,
) -> bool {
    let supplied_first = Sha256::digest(supplied_first_name.as_bytes());
    let supplied_last = Sha256::digest(supplied_last_name.as_bytes());
    let stored_first =
        Sha256::digest(normalize_name_for_match(&row.first_name_snapshot).as_bytes());
    let stored_last = Sha256::digest(normalize_name_for_match(&row.last_name_snapshot).as_bytes());
    bool::from(supplied_first.ct_eq(&stored_first) & supplied_last.ct_eq(&stored_last))
}

fn public_data(
    row: PublicVerificationRow,
    tenant_id: Uuid,
) -> Result<PublicCertificateVerificationData, AppError> {
    let status = match row.status.as_str() {
        "issued" => CertificateStatus::Issued,
        "revoked" => CertificateStatus::Revoked,
        _ => {
            return Err(AppError::InternalServerError(
                "certificate_public_status_invalid".to_string(),
            ));
        }
    };
    let (receipt, receipt_expires_at) = if status == CertificateStatus::Issued {
        let expires_at = Utc::now() + Duration::minutes(RECEIPT_TTL_MINUTES);
        (
            Some(create_public_render_receipt(tenant_id, row.id, expires_at)?),
            Some(expires_at),
        )
    } else {
        (None, None)
    };
    Ok(PublicCertificateVerificationData {
        status,
        certificate_number: row.certificate_number,
        title: row.title_snapshot,
        first_name: row.first_name_snapshot,
        last_name: row.last_name_snapshot,
        campaign_name: row.campaign_name,
        academic_year: row.academic_year_value,
        template_name: row.template_name_snapshot,
        activity_item: row.activity_item_snapshot,
        award_or_role: row.award_or_role_snapshot,
        issue_date: row.issue_date,
        issuer_school_name: row.school_name_snapshot,
        replacement_certificate_number: row.replacement_certificate_number,
        receipt,
        receipt_expires_at,
    })
}

fn create_public_render_receipt(
    tenant_id: Uuid,
    certificate_id: Uuid,
    expires_at: DateTime<Utc>,
) -> Result<String, AppError> {
    encode_receipt_payload(&RenderReceiptPayload {
        version: RECEIPT_VERSION,
        tenant_id,
        certificate_id,
        action: PUBLIC_RENDER_ACTION.to_string(),
        expires_at,
    })
}

fn encode_receipt_payload(payload: &RenderReceiptPayload) -> Result<String, AppError> {
    let plaintext = Zeroizing::new(
        serde_json::to_string(payload).map_err(|_| verification_crypto_error(String::new()))?,
    );
    field_encryption::encrypt(&plaintext).map_err(verification_crypto_error)
}

pub(super) fn validate_public_render_receipt(
    receipt: &str,
    tenant_id: Uuid,
    now: DateTime<Utc>,
) -> Result<Uuid, AppError> {
    if receipt.is_empty() || receipt.chars().count() > 2_048 {
        return Err(generic_not_found());
    }
    let plaintext =
        Zeroizing::new(field_encryption::decrypt(receipt).map_err(|_| generic_not_found())?);
    let payload = serde_json::from_str::<RenderReceiptPayload>(&plaintext)
        .map_err(|_| generic_not_found())?;
    if payload.version != RECEIPT_VERSION
        || payload.tenant_id != tenant_id
        || payload.action != PUBLIC_RENDER_ACTION
        || payload.expires_at <= now
    {
        return Err(generic_not_found());
    }
    Ok(payload.certificate_id)
}

fn generic_not_found() -> AppError {
    AppError::NotFound(GENERIC_NOT_FOUND_MESSAGE.to_string())
}

fn is_generic_not_found(error: &AppError) -> bool {
    matches!(error, AppError::NotFound(message) if message == GENERIC_NOT_FOUND_MESSAGE)
}

fn verification_crypto_error(_error: String) -> AppError {
    AppError::InternalServerError("certificate_public_verification_cryptography_failed".to_string())
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;

    #[test]
    fn public_render_receipt_rejects_expiry_tenant_action_and_tampering() {
        let _crypto_guard = crate::utils::field_encryption::test_env_lock();
        std::env::set_var(
            "ENCRYPTION_KEY",
            "certificate-public-receipt-encryption-test-key",
        );
        let now = Utc.with_ymd_and_hms(2026, 8, 15, 3, 0, 0).unwrap();
        let tenant_id = Uuid::new_v4();
        let certificate_id = Uuid::new_v4();
        let receipt = create_public_render_receipt(
            tenant_id,
            certificate_id,
            now + chrono::Duration::minutes(5),
        )
        .unwrap();

        assert_eq!(
            validate_public_render_receipt(&receipt, tenant_id, now).unwrap(),
            certificate_id
        );

        let wrong_action = encode_receipt_payload(&RenderReceiptPayload {
            version: 1,
            tenant_id,
            certificate_id,
            action: "another_action".to_string(),
            expires_at: now + chrono::Duration::minutes(5),
        })
        .unwrap();
        let mut tampered = receipt.clone().into_bytes();
        let middle = tampered.len() / 2;
        tampered[middle] = if tampered[middle] == b'A' { b'B' } else { b'A' };
        let tampered = String::from_utf8(tampered).unwrap();

        for invalid in [
            validate_public_render_receipt(&receipt, Uuid::new_v4(), now),
            validate_public_render_receipt(&receipt, tenant_id, now + chrono::Duration::minutes(6)),
            validate_public_render_receipt(&wrong_action, tenant_id, now),
            validate_public_render_receipt(&tampered, tenant_id, now),
        ] {
            let error = invalid.unwrap_err();
            assert_eq!(error.status_code(), axum::http::StatusCode::NOT_FOUND);
            assert_eq!(error.public_message(), "ไม่พบข้อมูลที่ตรงกัน");
        }
    }
}
