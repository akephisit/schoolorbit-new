use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, NaiveDate, Utc};
use sqlx::{FromRow, PgPool, Postgres, QueryBuilder, Transaction};
use uuid::Uuid;

use crate::{
    error::AppError,
    middleware::permission::ActorContext,
    modules::{
        certificates::models::{
            CandidateValidationCode, CandidateValidationStatus, CertificateCapabilities,
            CertificateElement, CertificateFontSource, CertificateIssueCandidateProblem,
            CertificateIssueCode, CertificateLayoutV1, CertificateNumber,
            CertificateReplacementCandidate, CertificateStatus, IssueCertificateOutcome,
            IssueCertificateRequest, IssuedCertificateDetail, IssuedCertificateListQuery,
            IssuedCertificateSummary, PageGeometry, RecipientType, RevokeCertificateRequest,
            RevokeCertificateResult,
        },
        school_fonts::models::SchoolFontStyle,
    },
    permissions::registry::codes,
    policies::certificate_access_policy::{require_owner_action, CertificateAction},
    scheduling::SCHOOL_TIMEZONE,
};

use super::{
    audit_service::{
        record_certificate_revocation_audit, record_issue_request_audit,
        CertificateIssueRequestAuditMetadata, CertificateRevocationAuditMetadata,
    },
    import_validation::{
        contains_thirteen_digit_run, is_forbidden_header, normalize_display_text,
        normalize_name_for_match, variable_catalog,
    },
    layout::validate_layout,
    proof::generate_certificate_proof,
};

const MAX_CERTIFICATES_PER_REQUEST: usize = 5_000;
const MAX_ACTIVITY_SEQUENCE: i32 = 9_999;
const MAX_CERTIFICATE_SEQUENCE: i32 = 999_999;

#[derive(Debug, FromRow)]
struct IssueRunRow {
    id: Uuid,
    request_id: Uuid,
    campaign_id: Uuid,
    idempotency_key: Uuid,
    outcome: String,
    first_certificate_sequence: Option<i32>,
    last_certificate_sequence: Option<i32>,
    issue_codes: Vec<String>,
}

#[derive(Debug, FromRow)]
struct IssueRequestRow {
    id: Uuid,
    campaign_id: Uuid,
    status: String,
}

#[derive(Debug, FromRow)]
struct CertificateAccessRow {
    owner_organization_unit_id: Option<Uuid>,
}

#[derive(Debug, FromRow)]
struct IssueRunProblemRow {
    candidate_id: Uuid,
    issue_codes: Vec<String>,
}

#[derive(Debug, FromRow)]
struct CampaignRow {
    id: Uuid,
    academic_year_id: Uuid,
    academic_year_value: i32,
    owner_organization_unit_id: Option<Uuid>,
    name: String,
    event_date: NaiveDate,
    status: String,
    activity_sequence: Option<i32>,
    next_certificate_sequence: i32,
}

#[derive(Debug, FromRow)]
struct CandidateRow {
    id: Uuid,
    campaign_id: Uuid,
    template_id: Option<Uuid>,
    recipient_type: String,
    matched_user_id: Option<Uuid>,
    lookup_student_id: Option<String>,
    lookup_staff_username: Option<String>,
    imported_title: Option<String>,
    imported_first_name: String,
    imported_last_name: String,
    account_title: Option<String>,
    account_first_name: Option<String>,
    account_last_name: Option<String>,
    selected_name_source: Option<String>,
    activity_item: Option<String>,
    award_or_role: Option<String>,
    custom_values: sqlx::types::Json<BTreeMap<String, String>>,
    match_status: String,
    validation_status: String,
    validation_codes: Vec<String>,
    replacement_for_certificate_id: Option<Uuid>,
    issued_certificate_id: Option<Uuid>,
    deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, FromRow)]
struct AccountRow {
    id: Uuid,
    username: Option<String>,
    student_id: Option<String>,
    user_type: String,
    status: String,
    title: Option<String>,
    first_name: String,
    last_name: String,
}

#[derive(Debug, FromRow)]
struct TemplateRow {
    id: Uuid,
    campaign_id: Uuid,
    name: String,
    background_file_id: Option<Uuid>,
    background_lifecycle_status: Option<String>,
    crop_box_width: Option<f64>,
    crop_box_height: Option<f64>,
    page_rotation: Option<i16>,
    allowed_recipient_types: Vec<String>,
    layout: sqlx::types::Json<CertificateLayoutV1>,
    is_active: bool,
}

#[derive(Debug, FromRow)]
struct AssetRow {
    id: Uuid,
    template_id: Uuid,
    kind: String,
    lifecycle_status: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ExpectedAsset {
    Image,
}

#[derive(Debug, FromRow)]
struct SchoolFontRow {
    id: Uuid,
    font_family: String,
    font_weight: i16,
    font_style: String,
    purpose_code: String,
    visibility: String,
    lifecycle_status: String,
    retention_class: String,
    storage_status: String,
    scan_status: String,
}

#[derive(Debug, FromRow)]
struct CertificateOutcomeRow {
    id: Uuid,
    campaign_id: Uuid,
    campaign_name: String,
    owner_organization_unit_id: Option<Uuid>,
    owner_organization_unit_name: Option<String>,
    template_id: Uuid,
    template_name: String,
    academic_year_id: Uuid,
    academic_year_value: i32,
    activity_sequence: i32,
    certificate_sequence: i32,
    certificate_number: String,
    recipient_type: String,
    title_snapshot: Option<String>,
    first_name_snapshot: String,
    last_name_snapshot: String,
    activity_item_snapshot: Option<String>,
    award_or_role_snapshot: Option<String>,
    issue_date: NaiveDate,
    status: String,
    replacement_for_certificate_id: Option<Uuid>,
    replaced_by_certificate_id: Option<Uuid>,
    replacement_candidate_id: Option<Uuid>,
    created_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct CertificateRevokeRow {
    id: Uuid,
    campaign_id: Uuid,
    template_id: Uuid,
    recipient_type: String,
    user_id: Option<Uuid>,
    title_snapshot: Option<String>,
    first_name_snapshot: String,
    last_name_snapshot: String,
    activity_item_snapshot: Option<String>,
    award_or_role_snapshot: Option<String>,
    custom_values_snapshot: sqlx::types::Json<BTreeMap<String, String>>,
    status: String,
}

#[derive(Debug, FromRow)]
struct ReplacementTemplateRow {
    allowed_recipient_types: Vec<String>,
    is_active: bool,
    background_is_ready: bool,
}

#[derive(Debug, FromRow)]
struct CertificateDetailRow {
    id: Uuid,
    campaign_id: Uuid,
    campaign_name: String,
    owner_organization_unit_id: Option<Uuid>,
    owner_organization_unit_name: Option<String>,
    template_id: Uuid,
    template_name: String,
    academic_year_id: Uuid,
    academic_year_value: i32,
    activity_sequence: i32,
    certificate_sequence: i32,
    certificate_number: String,
    recipient_type: String,
    title_snapshot: Option<String>,
    first_name_snapshot: String,
    last_name_snapshot: String,
    activity_item_snapshot: Option<String>,
    award_or_role_snapshot: Option<String>,
    issue_date: NaiveDate,
    status: String,
    replacement_for_certificate_id: Option<Uuid>,
    replaced_by_certificate_id: Option<Uuid>,
    replacement_candidate_id: Option<Uuid>,
    created_at: DateTime<Utc>,
    issue_run_id: Uuid,
    custom_values_snapshot: sqlx::types::Json<BTreeMap<String, String>>,
    school_name_snapshot: String,
    owner_organization_unit_name_snapshot: Option<String>,
    revoked_by: Option<Uuid>,
    revoked_at: Option<DateTime<Utc>>,
    revocation_reason: Option<String>,
    updated_at: DateTime<Utc>,
}

pub async fn issue_request(
    pool: &PgPool,
    actor: &ActorContext,
    school_name: String,
    request_id: Uuid,
    request: IssueCertificateRequest,
) -> Result<IssueCertificateOutcome, AppError> {
    actor.require_permission(codes::CERTIFICATE_ISSUE_SCHOOL)?;
    let school_name = normalize_school_name(&school_name)?;
    let campaign_id = fetch_request_campaign_id(pool, request_id).await?;
    let mut tx = pool.begin().await.map_err(issuance_db_error)?;

    let mut campaign = lock_campaign(&mut tx, campaign_id).await?;
    require_campaign_not_purging(&campaign.status)?;
    lock_issue_command(&mut tx, request_id).await?;
    if let Some(outcome) =
        load_matching_run_outcome(&mut tx, actor, request_id, request.idempotency_key).await?
    {
        tx.commit().await.map_err(issuance_db_error)?;
        return Ok(outcome);
    }

    let issue_request = lock_request(&mut tx, request_id).await?;
    if issue_request.status != "reviewing" {
        return Err(AppError::Conflict(
            "ออกเลขได้เฉพาะคำขอที่กำลังตรวจสอบ".to_string(),
        ));
    }
    if issue_request.campaign_id != campaign.id {
        return Err(AppError::InternalServerError(
            "certificate_issue_request_campaign_invalid".to_string(),
        ));
    }
    let candidate_ids = lock_request_items(&mut tx, request_id).await?;
    if candidate_ids.is_empty() || candidate_ids.len() > MAX_CERTIFICATES_PER_REQUEST {
        return Err(AppError::Conflict("คำขอมีจำนวนรายการไม่ถูกต้อง".to_string()));
    }
    let candidates = lock_candidates(&mut tx, &candidate_ids).await?;
    if candidates.len() != candidate_ids.len()
        || candidates
            .iter()
            .any(|candidate| candidate.campaign_id != campaign.id)
    {
        return Err(AppError::InternalServerError(
            "certificate_issue_request_items_invalid".to_string(),
        ));
    }
    let template_ids = candidates
        .iter()
        .filter_map(|candidate| candidate.template_id)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let templates = lock_templates(&mut tx, &template_ids).await?;
    let owner_name = lock_owner(&mut tx, campaign.owner_organization_unit_id).await?;

    // A missing student ID or staff username has no row to lock. This short table lock
    // serializes issuance with identity creation and update so converted external rows are
    // checked against the authoritative state at one transaction point.
    sqlx::query("LOCK TABLE users, student_info IN SHARE MODE")
        .execute(&mut *tx)
        .await
        .map_err(issuance_db_error)?;
    let accounts = load_accounts(&mut tx, &candidates).await?;
    let assets = lock_assets(&mut tx, &templates).await?;
    let school_fonts = lock_school_fonts(&mut tx, &templates).await?;
    let custom_headers = load_custom_headers(&mut tx, campaign.id).await?;
    let catalog = variable_catalog(&custom_headers).map_err(|_| {
        AppError::InternalServerError("certificate_variable_catalog_invalid".to_string())
    })?;

    let problems = revalidate(
        &campaign,
        owner_name.as_ref(),
        &candidates,
        &templates,
        &accounts,
        &assets,
        &school_fonts,
        &catalog,
    );
    if !problems.is_empty() {
        let issue_codes = problems
            .values()
            .flatten()
            .copied()
            .collect::<BTreeSet<_>>();
        let run_id = persist_returned_outcome(
            &mut tx,
            actor.user_id,
            &issue_request,
            request.idempotency_key,
            &candidates,
            &accounts,
            &problems,
            &issue_codes,
        )
        .await?;
        let outcome = IssueCertificateOutcome::Returned {
            issue_run_id: run_id,
            request_id,
            campaign_id: campaign.id,
            issue_codes: issue_codes.into_iter().collect(),
            candidate_problems: problems
                .into_iter()
                .map(
                    |(candidate_id, issue_codes)| CertificateIssueCandidateProblem {
                        candidate_id,
                        issue_codes: issue_codes.into_iter().collect(),
                    },
                )
                .collect(),
        };
        tx.commit().await.map_err(issuance_db_error)?;
        return Ok(outcome);
    }

    let activity_sequence = allocate_activity_sequence(&mut tx, &mut campaign).await?;
    let (first_sequence, last_sequence) =
        allocate_certificate_range(&mut tx, &mut campaign, candidates.len()).await?;
    let issue_date = Utc::now().with_timezone(&SCHOOL_TIMEZONE).date_naive();
    let issue_run_id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO certificate_issue_runs (
             request_id, idempotency_key, issued_by, outcome, issued_count,
             first_certificate_sequence, last_certificate_sequence
         )
         VALUES ($1, $2, $3, 'issued', $4, $5, $6)
         RETURNING id",
    )
    .bind(request_id)
    .bind(request.idempotency_key)
    .bind(actor.user_id)
    .bind(i32::try_from(candidates.len()).map_err(|_| {
        AppError::InternalServerError("certificate_issue_count_overflow".to_string())
    })?)
    .bind(first_sequence)
    .bind(last_sequence)
    .fetch_one(&mut *tx)
    .await
    .map_err(issuance_db_error)?;

    let template_map = templates
        .iter()
        .map(|template| (template.id, template))
        .collect::<BTreeMap<_, _>>();
    let owner_snapshot = owner_name
        .as_ref()
        .map(|(_, name)| name.clone())
        .unwrap_or_else(|| school_name.clone());
    for (offset, candidate) in candidates.iter().enumerate() {
        let sequence = first_sequence
            .checked_add(i32::try_from(offset).map_err(|_| {
                AppError::InternalServerError("certificate_sequence_overflow".to_string())
            })?)
            .ok_or_else(|| {
                AppError::InternalServerError("certificate_sequence_overflow".to_string())
            })?;
        let number = CertificateNumber::new(
            campaign.academic_year_value,
            u32::try_from(activity_sequence).map_err(|_| {
                AppError::InternalServerError("certificate_activity_sequence_invalid".to_string())
            })?,
            u32::try_from(sequence).map_err(|_| {
                AppError::InternalServerError("certificate_sequence_invalid".to_string())
            })?,
        )
        .map_err(|_| AppError::Conflict("เลขเกียรติบัตรเกินขอบเขตที่รองรับ".to_string()))?;
        let check_digit = number.check_digit().ok_or_else(|| {
            AppError::InternalServerError("certificate_check_digit_invalid".to_string())
        })?;
        let template_id = candidate.template_id.ok_or_else(|| {
            AppError::InternalServerError("certificate_candidate_template_missing".to_string())
        })?;
        let template = template_map.get(&template_id).ok_or_else(|| {
            AppError::InternalServerError("certificate_template_missing".to_string())
        })?;
        let (title, first_name, last_name) = selected_name(candidate)?;
        let recipient_type = RecipientType::parse(&candidate.recipient_type).ok_or_else(|| {
            AppError::InternalServerError("certificate_recipient_type_invalid".to_string())
        })?;
        let proof = generate_certificate_proof()?;
        let certificate_id = Uuid::new_v4();

        sqlx::query(
            "INSERT INTO certificates (
                 id, campaign_id, template_id, candidate_id, issue_run_id,
                 academic_year_id, academic_year_value, activity_sequence,
                 certificate_sequence, check_digit, certificate_number, recipient_type,
                 user_id, title_snapshot, first_name_snapshot, last_name_snapshot,
                 template_name_snapshot, activity_item_snapshot, award_or_role_snapshot,
                 custom_values_snapshot, school_name_snapshot,
                 owner_organization_unit_name_snapshot, issue_date,
                 qr_proof_encrypted, qr_proof_hash, replacement_for_certificate_id
             )
             VALUES (
                 $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12,
                 $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24, $25, $26
             )",
        )
        .bind(certificate_id)
        .bind(campaign.id)
        .bind(template.id)
        .bind(candidate.id)
        .bind(issue_run_id)
        .bind(campaign.academic_year_id)
        .bind(campaign.academic_year_value)
        .bind(activity_sequence)
        .bind(sequence)
        .bind(i16::from(check_digit))
        .bind(number.as_str())
        .bind(recipient_type.as_str())
        .bind(candidate.matched_user_id)
        .bind(title)
        .bind(first_name)
        .bind(last_name)
        .bind(&template.name)
        .bind(&candidate.activity_item)
        .bind(&candidate.award_or_role)
        .bind(sqlx::types::Json(candidate.custom_values.0.clone()))
        .bind(&school_name)
        .bind(&owner_snapshot)
        .bind(issue_date)
        .bind(proof.encrypted())
        .bind(proof.hash())
        .bind(candidate.replacement_for_certificate_id)
        .execute(&mut *tx)
        .await
        .map_err(issuance_db_error)?;

        if let Some(replacement_for_id) = candidate.replacement_for_certificate_id {
            let linked = sqlx::query(
                "UPDATE certificates
                 SET replaced_by_certificate_id = $2, updated_at = clock_timestamp()
                 WHERE id = $1 AND status = 'revoked' AND replaced_by_certificate_id IS NULL",
            )
            .bind(replacement_for_id)
            .bind(certificate_id)
            .execute(&mut *tx)
            .await
            .map_err(issuance_db_error)?;
            if linked.rows_affected() != 1 {
                return Err(AppError::Conflict(
                    "ใบเดิมไม่พร้อมเชื่อมกับเกียรติบัตรทดแทน".to_string(),
                ));
            }
        }

        sqlx::query(
            "UPDATE certificate_candidates
             SET issued_certificate_id = $2, lookup_student_id = NULL,
                 lookup_staff_username = NULL, updated_by = $3,
                 updated_at = clock_timestamp()
             WHERE id = $1 AND issued_certificate_id IS NULL",
        )
        .bind(candidate.id)
        .bind(certificate_id)
        .bind(actor.user_id)
        .execute(&mut *tx)
        .await
        .map_err(issuance_db_error)?;
    }

    sqlx::query(
        "UPDATE certificate_issue_requests
         SET status = 'issued', issued_at = clock_timestamp(), issue_codes = ARRAY[]::text[],
             updated_at = clock_timestamp()
         WHERE id = $1",
    )
    .bind(request_id)
    .execute(&mut *tx)
    .await
    .map_err(issuance_db_error)?;
    record_issue_request_audit(
        &mut tx,
        actor.user_id,
        "issue",
        CertificateIssueRequestAuditMetadata {
            campaign_id: campaign.id,
            request_id,
            from_status: Some("reviewing".to_string()),
            to_status: "issued".to_string(),
            item_count: candidates.len().try_into().unwrap_or(u32::MAX),
            issue_codes: Vec::new(),
        },
    )
    .await?;

    let run = fetch_issue_run(&mut tx, request_id).await?.ok_or_else(|| {
        AppError::InternalServerError("certificate_issue_run_missing".to_string())
    })?;
    let outcome = load_run_outcome(&mut tx, actor, run).await?;
    tx.commit().await.map_err(issuance_db_error)?;
    Ok(outcome)
}

pub async fn replay_issue_request(
    pool: &PgPool,
    actor: &ActorContext,
    request_id: Uuid,
    idempotency_key: Uuid,
) -> Result<Option<IssueCertificateOutcome>, AppError> {
    actor.require_permission(codes::CERTIFICATE_ISSUE_SCHOOL)?;
    let campaign_id = fetch_request_campaign_id(pool, request_id).await?;
    let mut tx = pool.begin().await.map_err(issuance_db_error)?;
    let campaign = lock_campaign(&mut tx, campaign_id).await?;
    require_campaign_not_purging(&campaign.status)?;
    lock_issue_command(&mut tx, request_id).await?;
    let outcome = load_matching_run_outcome(&mut tx, actor, request_id, idempotency_key).await?;
    tx.commit().await.map_err(issuance_db_error)?;
    Ok(outcome)
}

pub async fn revoke_certificate(
    pool: &PgPool,
    actor: &ActorContext,
    certificate_id: Uuid,
    request: RevokeCertificateRequest,
) -> Result<RevokeCertificateResult, AppError> {
    actor.require_permission(codes::CERTIFICATE_REVOKE_SCHOOL)?;
    let reason = normalize_revocation_reason(&request.reason)?;
    let campaign_id = fetch_certificate_campaign_id(pool, certificate_id).await?;
    let mut tx = pool.begin().await.map_err(issuance_db_error)?;
    let campaign = lock_campaign(&mut tx, campaign_id).await?;
    require_campaign_not_purging(&campaign.status)?;
    let certificate = sqlx::query_as::<_, CertificateRevokeRow>(
        "SELECT id, campaign_id, template_id, recipient_type, user_id,
                title_snapshot, first_name_snapshot, last_name_snapshot,
                activity_item_snapshot, award_or_role_snapshot,
                custom_values_snapshot, status
         FROM certificates
         WHERE id = $1
         FOR UPDATE",
    )
    .bind(certificate_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(issuance_db_error)?
    .ok_or_else(|| AppError::NotFound("ไม่พบเกียรติบัตร".to_string()))?;
    if certificate.campaign_id != campaign.id {
        return Err(AppError::InternalServerError(
            "certificate_campaign_reference_invalid".to_string(),
        ));
    }
    if certificate.status != "issued" {
        return Err(AppError::Conflict("เกียรติบัตรนี้ถูกเพิกถอนไปแล้ว".to_string()));
    }

    sqlx::query(
        "UPDATE certificates
         SET status = 'revoked', revoked_by = $2, revoked_at = clock_timestamp(),
             revocation_reason = $3, updated_at = clock_timestamp()
         WHERE id = $1",
    )
    .bind(certificate.id)
    .bind(actor.user_id)
    .bind(reason)
    .execute(&mut *tx)
    .await
    .map_err(issuance_db_error)?;

    let replacement_candidate = if request.create_replacement_candidate {
        Some(create_replacement_candidate(&mut tx, actor.user_id, &certificate).await?)
    } else {
        None
    };
    record_certificate_revocation_audit(
        &mut tx,
        actor.user_id,
        CertificateRevocationAuditMetadata {
            campaign_id: certificate.campaign_id,
            certificate_id: certificate.id,
            replacement_candidate_id: replacement_candidate.as_ref().map(|candidate| candidate.id),
        },
    )
    .await?;
    let detail = load_certificate_detail_tx(&mut tx, actor, certificate.id).await?;
    tx.commit().await.map_err(issuance_db_error)?;
    Ok(RevokeCertificateResult {
        certificate: detail,
        replacement_candidate,
    })
}

pub async fn list_campaign_certificates(
    pool: &PgPool,
    actor: &ActorContext,
    campaign_id: Uuid,
    query: IssuedCertificateListQuery,
) -> Result<Vec<IssuedCertificateSummary>, AppError> {
    let owner_organization_unit_id = sqlx::query_scalar::<_, Option<Uuid>>(
        "SELECT owner_organization_unit_id
         FROM certificate_campaigns
         WHERE id = $1 AND status <> 'purging'",
    )
    .bind(campaign_id)
    .fetch_optional(pool)
    .await
    .map_err(issuance_db_error)?
    .ok_or_else(|| AppError::NotFound("ไม่พบกิจกรรมเกียรติบัตร".to_string()))?;
    require_owner_action(
        pool,
        actor,
        owner_organization_unit_id,
        CertificateAction::Read,
    )
    .await?;
    let status = query.status.map(CertificateStatus::as_str);
    let search = query
        .search
        .as_deref()
        .map(normalize_display_text)
        .filter(|value| !value.is_empty());
    if search
        .as_ref()
        .is_some_and(|value| value.chars().count() > 100)
    {
        return Err(AppError::ValidationError(
            "คำค้นหายาวเกิน 100 ตัวอักษร".to_string(),
        ));
    }
    let rows = sqlx::query_as::<_, CertificateOutcomeRow>(
        "SELECT certificate.id, certificate.campaign_id,
                campaign.name AS campaign_name, campaign.owner_organization_unit_id,
                owner.name AS owner_organization_unit_name, certificate.template_id,
                certificate.template_name_snapshot AS template_name,
                certificate.academic_year_id, certificate.academic_year_value,
                certificate.activity_sequence, certificate.certificate_sequence,
                certificate.certificate_number, certificate.recipient_type,
                certificate.title_snapshot, certificate.first_name_snapshot,
                certificate.last_name_snapshot, certificate.activity_item_snapshot,
                certificate.award_or_role_snapshot, certificate.issue_date,
                certificate.status, certificate.replacement_for_certificate_id,
                certificate.replaced_by_certificate_id,
                replacement_candidate.id AS replacement_candidate_id,
                certificate.created_at
         FROM certificates certificate
         JOIN certificate_campaigns campaign ON campaign.id = certificate.campaign_id
         LEFT JOIN organization_units owner ON owner.id = campaign.owner_organization_unit_id
         LEFT JOIN certificate_candidates replacement_candidate
           ON replacement_candidate.replacement_for_certificate_id = certificate.id
          AND replacement_candidate.deleted_at IS NULL
         WHERE certificate.campaign_id = $1
           AND campaign.status <> 'purging'
           AND ($2::text IS NULL OR certificate.status = $2)
           AND ($3::uuid IS NULL OR certificate.template_id = $3)
           AND (
                $4::text IS NULL
                OR certificate.certificate_number ILIKE '%' || $4 || '%'
                OR certificate.first_name_snapshot ILIKE '%' || $4 || '%'
                OR certificate.last_name_snapshot ILIKE '%' || $4 || '%'
           )
         ORDER BY certificate.certificate_sequence DESC, certificate.id DESC",
    )
    .bind(campaign_id)
    .bind(status)
    .bind(query.template_id)
    .bind(search)
    .fetch_all(pool)
    .await
    .map_err(issuance_db_error)?;
    let can_download = can_owner_action(
        pool,
        actor,
        owner_organization_unit_id,
        CertificateAction::Download,
    )
    .await?;
    let can_revoke = actor.has_permission(codes::CERTIFICATE_REVOKE_SCHOOL);
    rows.into_iter()
        .map(|row| {
            let mut summary = outcome_summary(actor, row)?;
            summary.capabilities.can_read = true;
            summary.capabilities.can_download =
                can_download && summary.status == CertificateStatus::Issued;
            summary.capabilities.can_revoke =
                can_revoke && summary.status == CertificateStatus::Issued;
            Ok(summary)
        })
        .collect()
}

pub async fn list_own_certificates(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Vec<IssuedCertificateSummary>, AppError> {
    let rows = sqlx::query_as::<_, CertificateOutcomeRow>(
        "SELECT certificate.id, certificate.campaign_id,
                campaign.name AS campaign_name, campaign.owner_organization_unit_id,
                owner.name AS owner_organization_unit_name, certificate.template_id,
                certificate.template_name_snapshot AS template_name,
                certificate.academic_year_id, certificate.academic_year_value,
                certificate.activity_sequence, certificate.certificate_sequence,
                certificate.certificate_number, certificate.recipient_type,
                certificate.title_snapshot, certificate.first_name_snapshot,
                certificate.last_name_snapshot, certificate.activity_item_snapshot,
                certificate.award_or_role_snapshot, certificate.issue_date,
                certificate.status, certificate.replacement_for_certificate_id,
                certificate.replaced_by_certificate_id,
                replacement_candidate.id AS replacement_candidate_id,
                certificate.created_at
         FROM certificates certificate
         JOIN certificate_campaigns campaign ON campaign.id = certificate.campaign_id
         LEFT JOIN organization_units owner ON owner.id = campaign.owner_organization_unit_id
         LEFT JOIN certificate_candidates replacement_candidate
           ON replacement_candidate.replacement_for_certificate_id = certificate.id
          AND replacement_candidate.deleted_at IS NULL
         WHERE certificate.user_id = $1
           AND campaign.status <> 'purging'
         ORDER BY certificate.issue_date DESC, certificate.created_at DESC, certificate.id DESC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(issuance_db_error)?;

    rows.into_iter()
        .map(|row| own_certificate_summary(user_id, row))
        .collect()
}

pub async fn get_own_certificate(
    pool: &PgPool,
    user_id: Uuid,
    certificate_id: Uuid,
) -> Result<IssuedCertificateDetail, AppError> {
    let row = load_own_certificate_detail_pool(pool, user_id, certificate_id).await?;
    let actor = ActorContext {
        user_id,
        permissions: Vec::new(),
    };
    let mut detail = certificate_detail(&actor, row)?;
    detail.summary.capabilities.can_read = true;
    detail.summary.capabilities.can_download = detail.summary.status == CertificateStatus::Issued;
    detail.summary.capabilities.can_revoke = false;
    Ok(detail)
}

pub async fn get_certificate(
    pool: &PgPool,
    actor: &ActorContext,
    certificate_id: Uuid,
) -> Result<IssuedCertificateDetail, AppError> {
    let access = fetch_certificate_access(pool, certificate_id).await?;
    require_owner_action(
        pool,
        actor,
        access.owner_organization_unit_id,
        CertificateAction::Read,
    )
    .await?;
    let row = load_certificate_detail_pool(pool, certificate_id).await?;
    let mut detail = certificate_detail(actor, row)?;
    detail.summary.capabilities.can_read = true;
    detail.summary.capabilities.can_download = can_owner_action(
        pool,
        actor,
        access.owner_organization_unit_id,
        CertificateAction::Download,
    )
    .await?
        && detail.summary.status == CertificateStatus::Issued;
    detail.summary.capabilities.can_revoke = actor.has_permission(codes::CERTIFICATE_REVOKE_SCHOOL)
        && detail.summary.status == CertificateStatus::Issued;
    Ok(detail)
}

fn own_certificate_summary(
    user_id: Uuid,
    row: CertificateOutcomeRow,
) -> Result<IssuedCertificateSummary, AppError> {
    let actor = ActorContext {
        user_id,
        permissions: Vec::new(),
    };
    let mut summary = outcome_summary(&actor, row)?;
    summary.capabilities.can_read = true;
    summary.capabilities.can_download = summary.status == CertificateStatus::Issued;
    summary.capabilities.can_revoke = false;
    Ok(summary)
}

fn normalize_revocation_reason(value: &str) -> Result<String, AppError> {
    let reason = normalize_display_text(value);
    if reason.is_empty() || reason.chars().count() > 500 {
        return Err(AppError::ValidationError(
            "เหตุผลเพิกถอนต้องมีความยาว 1 ถึง 500 ตัวอักษร".to_string(),
        ));
    }
    if contains_thirteen_digit_run(&reason) || is_forbidden_header(&reason) {
        return Err(AppError::ValidationError(
            "เหตุผลเพิกถอนห้ามมีเลขประจำตัวประชาชนหรือข้อมูลอ่อนไหว".to_string(),
        ));
    }
    Ok(reason)
}

async fn create_replacement_candidate(
    tx: &mut Transaction<'_, Postgres>,
    actor_user_id: Uuid,
    certificate: &CertificateRevokeRow,
) -> Result<CertificateReplacementCandidate, AppError> {
    let recipient_type = RecipientType::parse(&certificate.recipient_type).ok_or_else(|| {
        AppError::InternalServerError("certificate_recipient_type_invalid".to_string())
    })?;
    let template = sqlx::query_as::<_, ReplacementTemplateRow>(
        "SELECT template.allowed_recipient_types, template.is_active,
                template.background_file_id IS NOT NULL
                    AND background.lifecycle_status = 'ready' AS background_is_ready
         FROM certificate_templates template
         LEFT JOIN files background ON background.id = template.background_file_id
         WHERE template.id = $1 AND template.campaign_id = $2
         FOR SHARE OF template",
    )
    .bind(certificate.template_id)
    .bind(certificate.campaign_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(issuance_db_error)?
    .ok_or_else(|| {
        AppError::InternalServerError("certificate_replacement_template_missing".to_string())
    })?;

    sqlx::query("LOCK TABLE users, student_info IN SHARE MODE")
        .execute(&mut **tx)
        .await
        .map_err(issuance_db_error)?;
    let account = if let Some(user_id) = certificate.user_id {
        sqlx::query_as::<_, AccountRow>(
            "SELECT users.id, users.username, student.student_id, users.user_type,
                    users.status, users.title, users.first_name, users.last_name
             FROM users
             LEFT JOIN student_info student ON student.user_id = users.id
             WHERE users.id = $1",
        )
        .bind(user_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(issuance_db_error)?
    } else {
        None
    };

    let mut validation_codes = BTreeSet::from([CandidateValidationCode::NameSourceRequired]);
    if !template.is_active || !template.background_is_ready {
        validation_codes.insert(CandidateValidationCode::TemplateNotReady);
    }
    if !template
        .allowed_recipient_types
        .iter()
        .any(|value| value == recipient_type.as_str())
    {
        validation_codes.insert(CandidateValidationCode::TemplateIncompatible);
    }
    let (
        lookup_student_id,
        lookup_staff_username,
        account_title,
        account_first_name,
        account_last_name,
        match_status,
    ) = match recipient_type {
        RecipientType::External => (None, None, None, None, None, "not_applicable"),
        RecipientType::Student | RecipientType::Staff => match account.as_ref() {
            Some(account)
                if account.status == "active"
                    && account.user_type == recipient_type.as_str()
                    && (recipient_type != RecipientType::Student
                        || account.student_id.is_some())
                    && (recipient_type != RecipientType::Staff || account.username.is_some()) =>
            {
                let same_name = normalize_name_for_match(&account.first_name)
                    == normalize_name_for_match(&certificate.first_name_snapshot)
                    && normalize_name_for_match(&account.last_name)
                        == normalize_name_for_match(&certificate.last_name_snapshot);
                (
                    (recipient_type == RecipientType::Student)
                        .then(|| account.student_id.clone())
                        .flatten(),
                    (recipient_type == RecipientType::Staff)
                        .then(|| account.username.clone())
                        .flatten(),
                    account.title.clone(),
                    Some(account.first_name.clone()),
                    Some(account.last_name.clone()),
                    if same_name {
                        "matched"
                    } else {
                        "name_mismatch"
                    },
                )
            }
            Some(account) if account.status != "active" => {
                validation_codes.insert(CandidateValidationCode::AccountInactive);
                (
                    account.student_id.clone(),
                    account.username.clone(),
                    account.title.clone(),
                    Some(account.first_name.clone()),
                    Some(account.last_name.clone()),
                    "inactive",
                )
            }
            _ => {
                validation_codes.insert(CandidateValidationCode::AccountNotFound);
                (None, None, None, None, None, "not_found")
            }
        },
    };
    let validation_codes = validation_codes
        .into_iter()
        .map(|code| code.as_str().to_string())
        .collect::<Vec<_>>();
    let batch_id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO certificate_import_batches (
             campaign_id, source, row_count, ready_count, review_count,
             invalid_count, created_by
         )
         VALUES ($1, 'replacement', 1, 0, 1, 0, $2)
         RETURNING id",
    )
    .bind(certificate.campaign_id)
    .bind(actor_user_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(issuance_db_error)?;
    let candidate_id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO certificate_candidates (
             campaign_id, batch_id, template_id, recipient_type, matched_user_id,
             lookup_student_id, lookup_staff_username, imported_title,
             imported_first_name, imported_last_name, account_title,
             account_first_name, account_last_name, selected_name_source,
             activity_item, award_or_role, custom_values, match_status,
             validation_status, validation_codes, replacement_for_certificate_id,
             created_by, updated_by
         )
         VALUES (
             $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, NULL,
             $14, $15, $16, $17, 'needs_review', $18, $19, $20, $20
         )
         RETURNING id",
    )
    .bind(certificate.campaign_id)
    .bind(batch_id)
    .bind(certificate.template_id)
    .bind(recipient_type.as_str())
    .bind(certificate.user_id)
    .bind(lookup_student_id)
    .bind(lookup_staff_username)
    .bind(&certificate.title_snapshot)
    .bind(&certificate.first_name_snapshot)
    .bind(&certificate.last_name_snapshot)
    .bind(account_title)
    .bind(account_first_name)
    .bind(account_last_name)
    .bind(&certificate.activity_item_snapshot)
    .bind(&certificate.award_or_role_snapshot)
    .bind(sqlx::types::Json(
        certificate.custom_values_snapshot.0.clone(),
    ))
    .bind(match_status)
    .bind(&validation_codes)
    .bind(certificate.id)
    .bind(actor_user_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(issuance_db_error)?;
    Ok(CertificateReplacementCandidate {
        id: candidate_id,
        campaign_id: certificate.campaign_id,
        template_id: certificate.template_id,
        validation_status: CandidateValidationStatus::NeedsReview,
    })
}

fn normalize_school_name(value: &str) -> Result<String, AppError> {
    let value = normalize_display_text(value);
    if value.is_empty() || value.chars().count() > 200 {
        Err(AppError::ServiceUnavailable(
            "school_name_lookup_failed".to_string(),
        ))
    } else {
        Ok(value)
    }
}

async fn fetch_request_campaign_id(pool: &PgPool, request_id: Uuid) -> Result<Uuid, AppError> {
    sqlx::query_scalar::<_, Uuid>(
        "SELECT request.campaign_id
         FROM certificate_issue_requests request
         JOIN certificate_campaigns campaign ON campaign.id = request.campaign_id
         WHERE request.id = $1 AND campaign.status <> 'purging'",
    )
    .bind(request_id)
    .fetch_optional(pool)
    .await
    .map_err(issuance_db_error)?
    .ok_or_else(|| AppError::NotFound("ไม่พบคำขอออกเกียรติบัตร".to_string()))
}

async fn fetch_certificate_campaign_id(
    pool: &PgPool,
    certificate_id: Uuid,
) -> Result<Uuid, AppError> {
    sqlx::query_scalar::<_, Uuid>(
        "SELECT certificate.campaign_id
         FROM certificates certificate
         JOIN certificate_campaigns campaign ON campaign.id = certificate.campaign_id
         WHERE certificate.id = $1 AND campaign.status <> 'purging'",
    )
    .bind(certificate_id)
    .fetch_optional(pool)
    .await
    .map_err(issuance_db_error)?
    .ok_or_else(|| AppError::NotFound("ไม่พบเกียรติบัตร".to_string()))
}

fn require_campaign_not_purging(status: &str) -> Result<(), AppError> {
    if status == "purging" {
        Err(AppError::Conflict(
            "certificate_campaign_purging".to_string(),
        ))
    } else {
        Ok(())
    }
}

async fn lock_issue_command(
    tx: &mut Transaction<'_, Postgres>,
    request_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1::uuid::text, 847321))")
        .bind(request_id)
        .execute(&mut **tx)
        .await
        .map_err(issuance_db_error)?;
    Ok(())
}

async fn fetch_issue_run(
    tx: &mut Transaction<'_, Postgres>,
    request_id: Uuid,
) -> Result<Option<IssueRunRow>, AppError> {
    sqlx::query_as::<_, IssueRunRow>(
        "SELECT run.id, run.request_id, request.campaign_id, run.idempotency_key,
                run.outcome, run.first_certificate_sequence, run.last_certificate_sequence,
                run.issue_codes
         FROM certificate_issue_runs run
         JOIN certificate_issue_requests request ON request.id = run.request_id
         WHERE run.request_id = $1",
    )
    .bind(request_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(issuance_db_error)
}

async fn load_matching_run_outcome(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    request_id: Uuid,
    idempotency_key: Uuid,
) -> Result<Option<IssueCertificateOutcome>, AppError> {
    let Some(run) = fetch_issue_run(tx, request_id).await? else {
        return Ok(None);
    };
    if run.idempotency_key != idempotency_key {
        return Err(AppError::Conflict(
            "คำขอนี้ถูกดำเนินการด้วยคำสั่งออกเลขอื่นแล้ว".to_string(),
        ));
    }
    load_run_outcome(tx, actor, run).await.map(Some)
}

async fn lock_request(
    tx: &mut Transaction<'_, Postgres>,
    request_id: Uuid,
) -> Result<IssueRequestRow, AppError> {
    sqlx::query_as::<_, IssueRequestRow>(
        "SELECT id, campaign_id, status
         FROM certificate_issue_requests
         WHERE id = $1
         FOR UPDATE",
    )
    .bind(request_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(issuance_db_error)?
    .ok_or_else(|| AppError::NotFound("ไม่พบคำขอออกเกียรติบัตร".to_string()))
}

async fn lock_campaign(
    tx: &mut Transaction<'_, Postgres>,
    campaign_id: Uuid,
) -> Result<CampaignRow, AppError> {
    sqlx::query_as::<_, CampaignRow>(
        "SELECT campaign.id, campaign.academic_year_id,
                academic_year.year AS academic_year_value,
                campaign.owner_organization_unit_id, campaign.name, campaign.event_date,
                campaign.status, campaign.activity_sequence,
                campaign.next_certificate_sequence
         FROM certificate_campaigns campaign
         JOIN academic_years academic_year ON academic_year.id = campaign.academic_year_id
         WHERE campaign.id = $1
         FOR UPDATE OF campaign",
    )
    .bind(campaign_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(issuance_db_error)?
    .ok_or_else(|| AppError::NotFound("ไม่พบกิจกรรมเกียรติบัตร".to_string()))
}

async fn lock_request_items(
    tx: &mut Transaction<'_, Postgres>,
    request_id: Uuid,
) -> Result<Vec<Uuid>, AppError> {
    sqlx::query_scalar::<_, Uuid>(
        "SELECT candidate_id
         FROM certificate_issue_request_items
         WHERE request_id = $1
         ORDER BY candidate_id
         FOR KEY SHARE",
    )
    .bind(request_id)
    .fetch_all(&mut **tx)
    .await
    .map_err(issuance_db_error)
}

async fn lock_candidates(
    tx: &mut Transaction<'_, Postgres>,
    candidate_ids: &[Uuid],
) -> Result<Vec<CandidateRow>, AppError> {
    sqlx::query_as::<_, CandidateRow>(
        "SELECT id, campaign_id, template_id, recipient_type, matched_user_id,
                lookup_student_id, lookup_staff_username, imported_title,
                imported_first_name, imported_last_name, account_title,
                account_first_name, account_last_name, selected_name_source,
                activity_item, award_or_role, custom_values, match_status,
                validation_status, validation_codes, replacement_for_certificate_id,
                issued_certificate_id, deleted_at
         FROM certificate_candidates
         WHERE id = ANY($1::uuid[])
         ORDER BY id
         FOR UPDATE",
    )
    .bind(candidate_ids)
    .fetch_all(&mut **tx)
    .await
    .map_err(issuance_db_error)
}

async fn lock_templates(
    tx: &mut Transaction<'_, Postgres>,
    template_ids: &[Uuid],
) -> Result<Vec<TemplateRow>, AppError> {
    if template_ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut rows = sqlx::query_as::<_, TemplateRow>(
        "SELECT id, campaign_id, name, background_file_id,
                NULL::text AS background_lifecycle_status, crop_box_width,
                crop_box_height, page_rotation, allowed_recipient_types, layout, is_active
         FROM certificate_templates
         WHERE id = ANY($1::uuid[])
         ORDER BY id
         FOR SHARE",
    )
    .bind(template_ids)
    .fetch_all(&mut **tx)
    .await
    .map_err(issuance_db_error)?;
    let background_ids = rows
        .iter()
        .filter_map(|template| template.background_file_id)
        .collect::<Vec<_>>();
    if !background_ids.is_empty() {
        let statuses = sqlx::query_as::<_, (Uuid, String)>(
            "SELECT id, lifecycle_status
             FROM files
             WHERE id = ANY($1::uuid[])
             ORDER BY id
             FOR SHARE",
        )
        .bind(&background_ids)
        .fetch_all(&mut **tx)
        .await
        .map_err(issuance_db_error)?;
        let by_id = statuses.into_iter().collect::<BTreeMap<_, _>>();
        for template in &mut rows {
            template.background_lifecycle_status = template
                .background_file_id
                .and_then(|file_id| by_id.get(&file_id).cloned());
        }
    }
    Ok(rows)
}

async fn lock_owner(
    tx: &mut Transaction<'_, Postgres>,
    owner_id: Option<Uuid>,
) -> Result<Option<(bool, String)>, AppError> {
    let Some(owner_id) = owner_id else {
        return Ok(None);
    };
    sqlx::query_as::<_, (bool, String)>(
        "SELECT is_active, name FROM organization_units WHERE id = $1 FOR SHARE",
    )
    .bind(owner_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(issuance_db_error)
}

async fn load_accounts(
    tx: &mut Transaction<'_, Postgres>,
    candidates: &[CandidateRow],
) -> Result<BTreeMap<Uuid, AccountRow>, AppError> {
    let account_ids = candidates
        .iter()
        .filter_map(|candidate| candidate.matched_user_id)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let student_ids = candidates
        .iter()
        .filter_map(|candidate| candidate.lookup_student_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let staff_usernames = candidates
        .iter()
        .filter_map(|candidate| candidate.lookup_staff_username.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if account_ids.is_empty() && student_ids.is_empty() && staff_usernames.is_empty() {
        return Ok(BTreeMap::new());
    }
    Ok(sqlx::query_as::<_, AccountRow>(
        "SELECT users.id, users.username, student.student_id, users.user_type,
                users.status, users.title, users.first_name, users.last_name
         FROM users
         LEFT JOIN student_info student ON student.user_id = users.id
         WHERE users.id = ANY($1::uuid[])
            OR (users.user_type = 'student' AND student.student_id = ANY($2::text[]))
            OR (users.user_type = 'staff' AND users.username = ANY($3::text[]))
         ORDER BY users.id",
    )
    .bind(&account_ids)
    .bind(&student_ids)
    .bind(&staff_usernames)
    .fetch_all(&mut **tx)
    .await
    .map_err(issuance_db_error)?
    .into_iter()
    .map(|account| (account.id, account))
    .collect())
}

async fn lock_assets(
    tx: &mut Transaction<'_, Postgres>,
    templates: &[TemplateRow],
) -> Result<BTreeMap<Uuid, Vec<AssetRow>>, AppError> {
    let template_ids = templates
        .iter()
        .map(|template| template.id)
        .collect::<Vec<_>>();
    if template_ids.is_empty() {
        return Ok(BTreeMap::new());
    }
    let rows = sqlx::query_as::<_, AssetRow>(
        "SELECT asset.id, asset.template_id, asset.kind, file.lifecycle_status
         FROM certificate_template_assets asset
         JOIN files file ON file.id = asset.file_id
         WHERE asset.template_id = ANY($1::uuid[])
         ORDER BY asset.template_id, asset.id
         FOR SHARE OF asset, file",
    )
    .bind(&template_ids)
    .fetch_all(&mut **tx)
    .await
    .map_err(issuance_db_error)?;
    let mut grouped = BTreeMap::<Uuid, Vec<AssetRow>>::new();
    for row in rows {
        grouped.entry(row.template_id).or_default().push(row);
    }
    Ok(grouped)
}

async fn lock_school_fonts(
    tx: &mut Transaction<'_, Postgres>,
    templates: &[TemplateRow],
) -> Result<BTreeMap<Uuid, SchoolFontRow>, AppError> {
    let font_ids = templates
        .iter()
        .flat_map(|template| template.layout.elements.iter())
        .filter_map(|element| match element {
            CertificateElement::Text(text) => match text.font_source {
                CertificateFontSource::SchoolFont { font_id } => Some(font_id),
                CertificateFontSource::BuiltIn => None,
            },
            CertificateElement::Image(_) | CertificateElement::Qr(_) => None,
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if font_ids.is_empty() {
        return Ok(BTreeMap::new());
    }
    sqlx::query_as::<_, SchoolFontRow>(
        "SELECT font.id, font.font_family, font.font_weight, font.font_style,
                file.purpose_code, file.visibility, file.lifecycle_status,
                file.retention_class, version.storage_status, version.scan_status
         FROM school_fonts AS font
         JOIN files AS file ON file.id = font.file_id
         JOIN file_versions AS version
           ON version.id = file.current_version_id AND version.file_id = file.id
         WHERE font.id = ANY($1::uuid[])
         ORDER BY font.id
         FOR SHARE OF font, file, version",
    )
    .bind(&font_ids)
    .fetch_all(&mut **tx)
    .await
    .map_err(issuance_db_error)
    .map(|rows| rows.into_iter().map(|font| (font.id, font)).collect())
}

async fn load_custom_headers(
    tx: &mut Transaction<'_, Postgres>,
    campaign_id: Uuid,
) -> Result<Vec<String>, AppError> {
    sqlx::query_scalar::<_, String>(
        "SELECT DISTINCT custom_key
         FROM (
             SELECT jsonb_object_keys(custom_values) AS custom_key
             FROM certificate_candidates
             WHERE campaign_id = $1 AND deleted_at IS NULL
             UNION
             SELECT jsonb_object_keys(custom_values_snapshot) AS custom_key
             FROM certificates
             WHERE campaign_id = $1
         ) custom_keys
         ORDER BY custom_key",
    )
    .bind(campaign_id)
    .fetch_all(&mut **tx)
    .await
    .map_err(issuance_db_error)
}

fn revalidate(
    campaign: &CampaignRow,
    owner: Option<&(bool, String)>,
    candidates: &[CandidateRow],
    templates: &[TemplateRow],
    accounts: &BTreeMap<Uuid, AccountRow>,
    assets: &BTreeMap<Uuid, Vec<AssetRow>>,
    school_fonts: &BTreeMap<Uuid, SchoolFontRow>,
    catalog: &[String],
) -> BTreeMap<Uuid, BTreeSet<CertificateIssueCode>> {
    let mut problems = BTreeMap::<Uuid, BTreeSet<CertificateIssueCode>>::new();
    if !matches!(campaign.status.as_str(), "draft" | "active")
        || owner.is_some_and(|(is_active, _)| !is_active)
        || !(0..=9_999).contains(&campaign.academic_year_value)
    {
        for candidate in candidates {
            problems
                .entry(candidate.id)
                .or_default()
                .insert(CertificateIssueCode::CampaignUnavailable);
        }
    }

    let template_map = templates
        .iter()
        .map(|template| (template.id, template))
        .collect::<BTreeMap<_, _>>();
    for candidate in candidates {
        let recipient_type = RecipientType::parse(&candidate.recipient_type);
        if candidate.deleted_at.is_some()
            || candidate.issued_certificate_id.is_some()
            || candidate.validation_status != CandidateValidationStatus::Ready.as_str()
            || !candidate.validation_codes.is_empty()
            || candidate.selected_name_source.is_none()
            || recipient_type.is_none()
        {
            problems
                .entry(candidate.id)
                .or_default()
                .insert(CertificateIssueCode::CandidateNotReady);
        }
        let Some(recipient_type) = recipient_type else {
            continue;
        };

        if !account_is_current(candidate, recipient_type, accounts) {
            problems
                .entry(candidate.id)
                .or_default()
                .insert(CertificateIssueCode::AccountStateChanged);
        }
        let Some(template_id) = candidate.template_id else {
            problems
                .entry(candidate.id)
                .or_default()
                .insert(CertificateIssueCode::TemplateNotReady);
            continue;
        };
        let Some(template) = template_map.get(&template_id) else {
            problems
                .entry(candidate.id)
                .or_default()
                .insert(CertificateIssueCode::TemplateNotReady);
            continue;
        };
        if template.campaign_id != campaign.id || !template.is_active {
            problems
                .entry(candidate.id)
                .or_default()
                .insert(CertificateIssueCode::TemplateNotReady);
        }
        if !template
            .allowed_recipient_types
            .iter()
            .any(|value| value == recipient_type.as_str())
        {
            problems
                .entry(candidate.id)
                .or_default()
                .insert(CertificateIssueCode::TemplateIncompatible);
        }
        if !template_layout_is_valid(template, catalog) {
            problems
                .entry(candidate.id)
                .or_default()
                .insert(CertificateIssueCode::TemplateNotReady);
        }
        if !template_resources_are_ready(template, assets.get(&template.id), school_fonts) {
            problems
                .entry(candidate.id)
                .or_default()
                .insert(CertificateIssueCode::AssetUnavailable);
        }
    }
    problems
}

fn account_is_current(
    candidate: &CandidateRow,
    recipient_type: RecipientType,
    accounts: &BTreeMap<Uuid, AccountRow>,
) -> bool {
    match recipient_type {
        RecipientType::External => {
            candidate.matched_user_id.is_none()
                && candidate.selected_name_source.as_deref() == Some("file")
                && matches!(
                    candidate.match_status.as_str(),
                    "not_applicable" | "external_confirmed"
                )
                && !converted_external_account_exists(candidate, accounts)
        }
        RecipientType::Student | RecipientType::Staff => {
            let Some(user_id) = candidate.matched_user_id else {
                return false;
            };
            let Some(account) = accounts.get(&user_id) else {
                return false;
            };
            let identity_matches = match recipient_type {
                RecipientType::Student => {
                    account.user_type == "student"
                        && account.student_id.as_ref() == candidate.lookup_student_id.as_ref()
                }
                RecipientType::Staff => {
                    account.user_type == "staff"
                        && account.username.as_deref() == candidate.lookup_staff_username.as_deref()
                }
                RecipientType::External => false,
            };
            identity_matches
                && account.status == "active"
                && account.title == candidate.account_title
                && Some(&account.first_name) == candidate.account_first_name.as_ref()
                && Some(&account.last_name) == candidate.account_last_name.as_ref()
        }
    }
}

fn converted_external_account_exists(
    candidate: &CandidateRow,
    accounts: &BTreeMap<Uuid, AccountRow>,
) -> bool {
    accounts.values().any(|account| {
        candidate.lookup_student_id.is_some()
            && account.user_type == "student"
            && account.student_id.as_ref() == candidate.lookup_student_id.as_ref()
            || candidate.lookup_staff_username.is_some()
                && account.user_type == "staff"
                && account.username.as_deref() == candidate.lookup_staff_username.as_deref()
    })
}

fn template_layout_is_valid(template: &TemplateRow, catalog: &[String]) -> bool {
    let (Some(width), Some(height), Some(rotation), Some(_)) = (
        template.crop_box_width,
        template.crop_box_height,
        template.page_rotation,
        template.background_file_id,
    ) else {
        return false;
    };
    PageGeometry::new(width, height, rotation)
        .ok()
        .is_some_and(|page| validate_layout(&template.layout.0, page, catalog).is_ok())
}

fn template_resources_are_ready(
    template: &TemplateRow,
    assets: Option<&Vec<AssetRow>>,
    school_fonts: &BTreeMap<Uuid, SchoolFontRow>,
) -> bool {
    if template.background_lifecycle_status.as_deref() != Some("ready") {
        return false;
    }
    let mut expected_assets = BTreeMap::<Uuid, ExpectedAsset>::new();
    let mut expected_fonts = BTreeMap::new();
    for element in &template.layout.elements {
        match element {
            CertificateElement::Image(image) => {
                expected_assets.insert(image.asset_id, ExpectedAsset::Image);
            }
            CertificateElement::Text(text) => {
                let CertificateFontSource::SchoolFont { font_id } = text.font_source else {
                    continue;
                };
                let next = (text.font_family.clone(), text.font_weight, text.font_style);
                if expected_fonts
                    .insert(font_id, next.clone())
                    .is_some_and(|previous| previous != next)
                {
                    return false;
                }
            }
            CertificateElement::Qr(_) => {}
        }
    }
    let images_ready = if expected_assets.is_empty() {
        true
    } else {
        let Some(assets) = assets else {
            return false;
        };
        let by_id = assets
            .iter()
            .map(|asset| (asset.id, asset))
            .collect::<BTreeMap<_, _>>();
        expected_assets.keys().all(|asset_id| {
            by_id
                .get(asset_id)
                .is_some_and(|asset| asset.kind == "image" && asset.lifecycle_status == "ready")
        })
    };
    images_ready
        && expected_fonts.into_iter().all(|(font_id, expected)| {
            school_fonts.get(&font_id).is_some_and(|font| {
                school_font_is_ready(font)
                    && font.font_family == expected.0
                    && u16::try_from(font.font_weight).ok() == Some(expected.1)
                    && SchoolFontStyle::parse(&font.font_style) == Some(expected.2)
            })
        })
}

fn school_font_is_ready(font: &SchoolFontRow) -> bool {
    font.purpose_code == "school_font"
        && font.visibility == "private"
        && font.lifecycle_status == "ready"
        && font.retention_class == "standard"
        && font.storage_status == "stored"
        && font.scan_status == "clean"
}

async fn persist_returned_outcome(
    tx: &mut Transaction<'_, Postgres>,
    actor_user_id: Uuid,
    request: &IssueRequestRow,
    idempotency_key: Uuid,
    candidates: &[CandidateRow],
    accounts: &BTreeMap<Uuid, AccountRow>,
    problems: &BTreeMap<Uuid, BTreeSet<CertificateIssueCode>>,
    issue_codes: &BTreeSet<CertificateIssueCode>,
) -> Result<Uuid, AppError> {
    for candidate in candidates {
        let Some(candidate_problems) = problems.get(&candidate.id) else {
            continue;
        };
        let validation_codes = candidate_failure_codes(candidate, candidate_problems, accounts);
        if validation_codes.is_empty() {
            continue;
        }
        sqlx::query(
            "UPDATE certificate_candidates
             SET validation_status = 'needs_review', validation_codes = $2,
                 updated_by = $3, updated_at = clock_timestamp()
             WHERE id = $1",
        )
        .bind(candidate.id)
        .bind(&validation_codes)
        .bind(actor_user_id)
        .execute(&mut **tx)
        .await
        .map_err(issuance_db_error)?;
    }
    let issue_code_values = issue_codes
        .iter()
        .map(|code| code.as_str())
        .collect::<Vec<_>>();
    let run_id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO certificate_issue_runs (
             request_id, idempotency_key, issued_by, outcome, issue_codes
         )
         VALUES ($1, $2, $3, 'returned', $4)
         RETURNING id",
    )
    .bind(request.id)
    .bind(idempotency_key)
    .bind(actor_user_id)
    .bind(&issue_code_values)
    .fetch_one(&mut **tx)
    .await
    .map_err(issuance_db_error)?;
    let mut problem_insert = QueryBuilder::<Postgres>::new(
        "INSERT INTO certificate_issue_run_problems (issue_run_id, candidate_id, issue_codes) ",
    );
    problem_insert.push_values(problems, |mut row, (candidate_id, codes)| {
        let codes = codes
            .iter()
            .map(|code| code.as_str().to_string())
            .collect::<Vec<_>>();
        row.push_bind(run_id)
            .push_bind(candidate_id)
            .push_bind(codes);
    });
    problem_insert
        .build()
        .execute(&mut **tx)
        .await
        .map_err(issuance_db_error)?;
    sqlx::query(
        "UPDATE certificate_issue_requests
         SET status = 'returned', returned_at = clock_timestamp(),
             return_note = 'ข้อมูลเปลี่ยนแปลงระหว่างการตรวจ กรุณาตรวจสอบรายการที่ระบุ',
             issue_codes = $2, updated_at = clock_timestamp()
         WHERE id = $1",
    )
    .bind(request.id)
    .bind(&issue_code_values)
    .execute(&mut **tx)
    .await
    .map_err(issuance_db_error)?;
    record_issue_request_audit(
        tx,
        actor_user_id,
        "issue_returned",
        CertificateIssueRequestAuditMetadata {
            campaign_id: request.campaign_id,
            request_id: request.id,
            from_status: Some("reviewing".to_string()),
            to_status: "returned".to_string(),
            item_count: candidates.len().try_into().unwrap_or(u32::MAX),
            issue_codes: issue_code_values
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
        },
    )
    .await?;
    Ok(run_id)
}

fn candidate_failure_codes(
    candidate: &CandidateRow,
    problems: &BTreeSet<CertificateIssueCode>,
    accounts: &BTreeMap<Uuid, AccountRow>,
) -> Vec<String> {
    let mut codes = candidate
        .validation_codes
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    for problem in problems {
        match problem {
            CertificateIssueCode::AccountStateChanged => {
                codes.insert(
                    account_state_validation_code(candidate, accounts)
                        .as_str()
                        .to_string(),
                );
            }
            CertificateIssueCode::TemplateNotReady | CertificateIssueCode::AssetUnavailable => {
                codes.insert(
                    CandidateValidationCode::TemplateNotReady
                        .as_str()
                        .to_string(),
                );
            }
            CertificateIssueCode::TemplateIncompatible => {
                codes.insert(
                    CandidateValidationCode::TemplateIncompatible
                        .as_str()
                        .to_string(),
                );
            }
            CertificateIssueCode::CandidateNotReady
            | CertificateIssueCode::CampaignUnavailable
            | CertificateIssueCode::ReviewerRequestedChanges => {}
        }
    }
    codes.into_iter().collect()
}

fn account_state_validation_code(
    candidate: &CandidateRow,
    accounts: &BTreeMap<Uuid, AccountRow>,
) -> CandidateValidationCode {
    if candidate.recipient_type == RecipientType::External.as_str() {
        return CandidateValidationCode::UnexpectedInternalLookup;
    }
    let Some(account) = candidate
        .matched_user_id
        .and_then(|user_id| accounts.get(&user_id))
    else {
        return CandidateValidationCode::AccountNotFound;
    };
    if account.status != "active" {
        CandidateValidationCode::AccountInactive
    } else {
        CandidateValidationCode::UnexpectedInternalLookup
    }
}

async fn allocate_activity_sequence(
    tx: &mut Transaction<'_, Postgres>,
    campaign: &mut CampaignRow,
) -> Result<i32, AppError> {
    if let Some(sequence) = campaign.activity_sequence {
        if (1..=MAX_ACTIVITY_SEQUENCE).contains(&sequence) {
            return Ok(sequence);
        }
        return Err(AppError::Conflict("ลำดับกิจกรรมเกินขอบเขตที่รองรับ".to_string()));
    }
    sqlx::query(
        "INSERT INTO certificate_academic_year_counters (academic_year_id)
         VALUES ($1)
         ON CONFLICT (academic_year_id) DO NOTHING",
    )
    .bind(campaign.academic_year_id)
    .execute(&mut **tx)
    .await
    .map_err(issuance_db_error)?;
    let sequence = sqlx::query_scalar::<_, i32>(
        "UPDATE certificate_academic_year_counters
         SET next_activity_sequence = next_activity_sequence + 1,
             updated_at = clock_timestamp()
         WHERE academic_year_id = $1 AND next_activity_sequence <= $2
         RETURNING next_activity_sequence - 1",
    )
    .bind(campaign.academic_year_id)
    .bind(MAX_ACTIVITY_SEQUENCE)
    .fetch_optional(&mut **tx)
    .await
    .map_err(issuance_db_error)?
    .ok_or_else(|| AppError::Conflict("ปีการศึกษานี้มีชุดออกเลขครบขอบเขตแล้ว".to_string()))?;
    sqlx::query(
        "UPDATE certificate_campaigns
         SET activity_sequence = $2, updated_at = clock_timestamp()
         WHERE id = $1",
    )
    .bind(campaign.id)
    .bind(sequence)
    .execute(&mut **tx)
    .await
    .map_err(issuance_db_error)?;
    campaign.activity_sequence = Some(sequence);
    Ok(sequence)
}

async fn allocate_certificate_range(
    tx: &mut Transaction<'_, Postgres>,
    campaign: &mut CampaignRow,
    count: usize,
) -> Result<(i32, i32), AppError> {
    let count = i32::try_from(count)
        .map_err(|_| AppError::Conflict("จำนวนใบเกินขอบเขตที่รองรับ".to_string()))?;
    let first = campaign.next_certificate_sequence;
    let last = first
        .checked_add(count - 1)
        .filter(|last| *last <= MAX_CERTIFICATE_SEQUENCE)
        .ok_or_else(|| AppError::Conflict("กิจกรรมนี้มีเลขเกียรติบัตรครบขอบเขตแล้ว".to_string()))?;
    let next = last + 1;
    let updated = sqlx::query(
        "UPDATE certificate_campaigns
         SET next_certificate_sequence = $2,
             status = CASE WHEN status = 'draft' THEN 'active' ELSE status END,
             updated_at = clock_timestamp()
         WHERE id = $1 AND next_certificate_sequence = $3",
    )
    .bind(campaign.id)
    .bind(next)
    .bind(first)
    .execute(&mut **tx)
    .await
    .map_err(issuance_db_error)?;
    if updated.rows_affected() != 1 {
        return Err(AppError::Conflict(
            "ลำดับเกียรติบัตรเปลี่ยนระหว่างดำเนินการ กรุณาลองใหม่".to_string(),
        ));
    }
    campaign.next_certificate_sequence = next;
    if campaign.status == "draft" {
        campaign.status = "active".to_string();
    }
    Ok((first, last))
}

fn selected_name(candidate: &CandidateRow) -> Result<(Option<String>, String, String), AppError> {
    match candidate.selected_name_source.as_deref() {
        Some("account") => Ok((
            candidate.account_title.clone(),
            candidate.account_first_name.clone().ok_or_else(|| {
                AppError::InternalServerError("certificate_account_first_name_missing".to_string())
            })?,
            candidate.account_last_name.clone().ok_or_else(|| {
                AppError::InternalServerError("certificate_account_last_name_missing".to_string())
            })?,
        )),
        Some("file") => Ok((
            candidate.imported_title.clone(),
            candidate.imported_first_name.clone(),
            candidate.imported_last_name.clone(),
        )),
        _ => Err(AppError::InternalServerError(
            "certificate_name_source_missing".to_string(),
        )),
    }
}

async fn load_run_outcome(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    run: IssueRunRow,
) -> Result<IssueCertificateOutcome, AppError> {
    let issue_codes = parse_issue_codes(&run.issue_codes)?;
    if run.outcome == "returned" {
        let problem_rows = sqlx::query_as::<_, IssueRunProblemRow>(
            "SELECT candidate_id, issue_codes
             FROM certificate_issue_run_problems
             WHERE issue_run_id = $1
             ORDER BY candidate_id",
        )
        .bind(run.id)
        .fetch_all(&mut **tx)
        .await
        .map_err(issuance_db_error)?;
        let candidate_problems = problem_rows
            .into_iter()
            .map(|problem| {
                Ok(CertificateIssueCandidateProblem {
                    candidate_id: problem.candidate_id,
                    issue_codes: parse_issue_codes(&problem.issue_codes)?,
                })
            })
            .collect::<Result<Vec<_>, AppError>>()?;
        return Ok(IssueCertificateOutcome::Returned {
            issue_run_id: run.id,
            request_id: run.request_id,
            campaign_id: run.campaign_id,
            issue_codes,
            candidate_problems,
        });
    }
    if run.outcome != "issued" {
        return Err(AppError::InternalServerError(
            "certificate_issue_run_outcome_invalid".to_string(),
        ));
    }
    let rows = sqlx::query_as::<_, CertificateOutcomeRow>(
        "SELECT certificate.id, certificate.campaign_id,
                campaign.name AS campaign_name, campaign.owner_organization_unit_id,
                owner.name AS owner_organization_unit_name, certificate.template_id,
                certificate.template_name_snapshot AS template_name,
                certificate.academic_year_id, certificate.academic_year_value,
                certificate.activity_sequence, certificate.certificate_sequence,
                certificate.certificate_number, certificate.recipient_type,
                certificate.title_snapshot, certificate.first_name_snapshot,
                certificate.last_name_snapshot, certificate.activity_item_snapshot,
                certificate.award_or_role_snapshot, certificate.issue_date,
                certificate.status, certificate.replacement_for_certificate_id,
                certificate.replaced_by_certificate_id,
                replacement_candidate.id AS replacement_candidate_id,
                certificate.created_at
         FROM certificates certificate
         JOIN certificate_campaigns campaign ON campaign.id = certificate.campaign_id
         LEFT JOIN organization_units owner ON owner.id = campaign.owner_organization_unit_id
         LEFT JOIN certificate_candidates replacement_candidate
           ON replacement_candidate.replacement_for_certificate_id = certificate.id
          AND replacement_candidate.deleted_at IS NULL
         WHERE certificate.issue_run_id = $1
           AND campaign.status <> 'purging'
         ORDER BY certificate.certificate_sequence",
    )
    .bind(run.id)
    .fetch_all(&mut **tx)
    .await
    .map_err(issuance_db_error)?;
    let certificates = rows
        .into_iter()
        .map(|row| outcome_summary(actor, row))
        .collect::<Result<Vec<_>, _>>()?;
    let activity_sequence = certificates
        .first()
        .map(|certificate| certificate.activity_sequence)
        .ok_or_else(|| AppError::InternalServerError("certificate_issue_run_empty".to_string()))?;
    Ok(IssueCertificateOutcome::Issued {
        issue_run_id: run.id,
        request_id: run.request_id,
        campaign_id: run.campaign_id,
        activity_sequence,
        first_certificate_sequence: run.first_certificate_sequence.ok_or_else(|| {
            AppError::InternalServerError(
                "certificate_issue_run_first_sequence_missing".to_string(),
            )
        })?,
        last_certificate_sequence: run.last_certificate_sequence.ok_or_else(|| {
            AppError::InternalServerError("certificate_issue_run_last_sequence_missing".to_string())
        })?,
        certificates,
    })
}

fn outcome_summary(
    actor: &ActorContext,
    row: CertificateOutcomeRow,
) -> Result<IssuedCertificateSummary, AppError> {
    Ok(IssuedCertificateSummary {
        id: row.id,
        campaign_id: row.campaign_id,
        campaign_name: row.campaign_name,
        owner_organization_unit_id: row.owner_organization_unit_id,
        owner_organization_unit_name: row.owner_organization_unit_name,
        template_id: row.template_id,
        template_name: row.template_name,
        academic_year_id: row.academic_year_id,
        academic_year_value: row.academic_year_value,
        activity_sequence: row.activity_sequence,
        certificate_sequence: row.certificate_sequence,
        certificate_number: row.certificate_number,
        recipient_type: RecipientType::parse(&row.recipient_type).ok_or_else(|| {
            AppError::InternalServerError("certificate_recipient_type_invalid".to_string())
        })?,
        title: row.title_snapshot,
        first_name: row.first_name_snapshot,
        last_name: row.last_name_snapshot,
        activity_item: row.activity_item_snapshot,
        award_or_role: row.award_or_role_snapshot,
        issue_date: row.issue_date,
        status: match row.status.as_str() {
            "issued" => CertificateStatus::Issued,
            "revoked" => CertificateStatus::Revoked,
            _ => {
                return Err(AppError::InternalServerError(
                    "certificate_status_invalid".to_string(),
                ));
            }
        },
        replacement_for_certificate_id: row.replacement_for_certificate_id,
        replaced_by_certificate_id: row.replaced_by_certificate_id,
        replacement_candidate_id: row.replacement_candidate_id,
        created_at: row.created_at,
        capabilities: CertificateCapabilities {
            can_read: actor.has_permission(codes::CERTIFICATE_READ_SCHOOL),
            can_download: actor.has_permission(codes::CERTIFICATE_DOWNLOAD_SCHOOL)
                && row.status == "issued",
            can_revoke: actor.has_permission(codes::CERTIFICATE_REVOKE_SCHOOL)
                && row.status == "issued",
        },
    })
}

async fn load_certificate_detail_tx(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    certificate_id: Uuid,
) -> Result<IssuedCertificateDetail, AppError> {
    let row = sqlx::query_as::<_, CertificateDetailRow>(
        "SELECT certificate.id, certificate.campaign_id,
                campaign.name AS campaign_name, campaign.owner_organization_unit_id,
                owner.name AS owner_organization_unit_name, certificate.template_id,
                certificate.template_name_snapshot AS template_name,
                certificate.academic_year_id, certificate.academic_year_value,
                certificate.activity_sequence, certificate.certificate_sequence,
                certificate.certificate_number, certificate.recipient_type,
                certificate.title_snapshot, certificate.first_name_snapshot,
                certificate.last_name_snapshot, certificate.activity_item_snapshot,
                certificate.award_or_role_snapshot, certificate.issue_date,
                certificate.status, certificate.replacement_for_certificate_id,
                certificate.replaced_by_certificate_id,
                replacement_candidate.id AS replacement_candidate_id,
                certificate.created_at, certificate.issue_run_id,
                certificate.custom_values_snapshot, certificate.school_name_snapshot,
                certificate.owner_organization_unit_name_snapshot,
                certificate.revoked_by, certificate.revoked_at,
                certificate.revocation_reason, certificate.updated_at
         FROM certificates certificate
         JOIN certificate_campaigns campaign ON campaign.id = certificate.campaign_id
         LEFT JOIN organization_units owner ON owner.id = campaign.owner_organization_unit_id
         LEFT JOIN certificate_candidates replacement_candidate
           ON replacement_candidate.replacement_for_certificate_id = certificate.id
          AND replacement_candidate.deleted_at IS NULL
         WHERE certificate.id = $1
           AND campaign.status <> 'purging'",
    )
    .bind(certificate_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(issuance_db_error)?
    .ok_or_else(|| AppError::NotFound("ไม่พบเกียรติบัตร".to_string()))?;
    certificate_detail(actor, row)
}

async fn fetch_certificate_access(
    pool: &PgPool,
    certificate_id: Uuid,
) -> Result<CertificateAccessRow, AppError> {
    sqlx::query_as::<_, CertificateAccessRow>(
        "SELECT campaign.owner_organization_unit_id
         FROM certificates certificate
         JOIN certificate_campaigns campaign ON campaign.id = certificate.campaign_id
         WHERE certificate.id = $1
           AND campaign.status <> 'purging'",
    )
    .bind(certificate_id)
    .fetch_optional(pool)
    .await
    .map_err(issuance_db_error)?
    .ok_or_else(|| AppError::NotFound("ไม่พบเกียรติบัตร".to_string()))
}

async fn can_owner_action(
    pool: &PgPool,
    actor: &ActorContext,
    owner_organization_unit_id: Option<Uuid>,
    action: CertificateAction,
) -> Result<bool, AppError> {
    match require_owner_action(pool, actor, owner_organization_unit_id, action).await {
        Ok(_) => Ok(true),
        Err(AppError::Forbidden(_)) => Ok(false),
        Err(error) => Err(error),
    }
}

async fn load_certificate_detail_pool(
    pool: &PgPool,
    certificate_id: Uuid,
) -> Result<CertificateDetailRow, AppError> {
    sqlx::query_as::<_, CertificateDetailRow>(
        "SELECT certificate.id, certificate.campaign_id,
                campaign.name AS campaign_name, campaign.owner_organization_unit_id,
                owner.name AS owner_organization_unit_name, certificate.template_id,
                certificate.template_name_snapshot AS template_name,
                certificate.academic_year_id, certificate.academic_year_value,
                certificate.activity_sequence, certificate.certificate_sequence,
                certificate.certificate_number, certificate.recipient_type,
                certificate.title_snapshot, certificate.first_name_snapshot,
                certificate.last_name_snapshot, certificate.activity_item_snapshot,
                certificate.award_or_role_snapshot, certificate.issue_date,
                certificate.status, certificate.replacement_for_certificate_id,
                certificate.replaced_by_certificate_id,
                replacement_candidate.id AS replacement_candidate_id,
                certificate.created_at, certificate.issue_run_id,
                certificate.custom_values_snapshot, certificate.school_name_snapshot,
                certificate.owner_organization_unit_name_snapshot,
                certificate.revoked_by, certificate.revoked_at,
                certificate.revocation_reason, certificate.updated_at
         FROM certificates certificate
         JOIN certificate_campaigns campaign ON campaign.id = certificate.campaign_id
         LEFT JOIN organization_units owner ON owner.id = campaign.owner_organization_unit_id
         LEFT JOIN certificate_candidates replacement_candidate
           ON replacement_candidate.replacement_for_certificate_id = certificate.id
          AND replacement_candidate.deleted_at IS NULL
         WHERE certificate.id = $1
           AND campaign.status <> 'purging'",
    )
    .bind(certificate_id)
    .fetch_optional(pool)
    .await
    .map_err(issuance_db_error)?
    .ok_or_else(|| AppError::NotFound("ไม่พบเกียรติบัตร".to_string()))
}

async fn load_own_certificate_detail_pool(
    pool: &PgPool,
    user_id: Uuid,
    certificate_id: Uuid,
) -> Result<CertificateDetailRow, AppError> {
    sqlx::query_as::<_, CertificateDetailRow>(
        "SELECT certificate.id, certificate.campaign_id,
                campaign.name AS campaign_name, campaign.owner_organization_unit_id,
                owner.name AS owner_organization_unit_name, certificate.template_id,
                certificate.template_name_snapshot AS template_name,
                certificate.academic_year_id, certificate.academic_year_value,
                certificate.activity_sequence, certificate.certificate_sequence,
                certificate.certificate_number, certificate.recipient_type,
                certificate.title_snapshot, certificate.first_name_snapshot,
                certificate.last_name_snapshot, certificate.activity_item_snapshot,
                certificate.award_or_role_snapshot, certificate.issue_date,
                certificate.status, certificate.replacement_for_certificate_id,
                certificate.replaced_by_certificate_id,
                replacement_candidate.id AS replacement_candidate_id,
                certificate.created_at, certificate.issue_run_id,
                certificate.custom_values_snapshot, certificate.school_name_snapshot,
                certificate.owner_organization_unit_name_snapshot,
                certificate.revoked_by, certificate.revoked_at,
                certificate.revocation_reason, certificate.updated_at
         FROM certificates certificate
         JOIN certificate_campaigns campaign ON campaign.id = certificate.campaign_id
         LEFT JOIN organization_units owner ON owner.id = campaign.owner_organization_unit_id
         LEFT JOIN certificate_candidates replacement_candidate
           ON replacement_candidate.replacement_for_certificate_id = certificate.id
          AND replacement_candidate.deleted_at IS NULL
         WHERE certificate.id = $1 AND certificate.user_id = $2
           AND campaign.status <> 'purging'",
    )
    .bind(certificate_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(issuance_db_error)?
    .ok_or_else(|| AppError::NotFound("ไม่พบเกียรติบัตร".to_string()))
}

fn certificate_detail(
    actor: &ActorContext,
    row: CertificateDetailRow,
) -> Result<IssuedCertificateDetail, AppError> {
    let summary = outcome_summary(
        actor,
        CertificateOutcomeRow {
            id: row.id,
            campaign_id: row.campaign_id,
            campaign_name: row.campaign_name,
            owner_organization_unit_id: row.owner_organization_unit_id,
            owner_organization_unit_name: row.owner_organization_unit_name,
            template_id: row.template_id,
            template_name: row.template_name,
            academic_year_id: row.academic_year_id,
            academic_year_value: row.academic_year_value,
            activity_sequence: row.activity_sequence,
            certificate_sequence: row.certificate_sequence,
            certificate_number: row.certificate_number,
            recipient_type: row.recipient_type,
            title_snapshot: row.title_snapshot,
            first_name_snapshot: row.first_name_snapshot,
            last_name_snapshot: row.last_name_snapshot,
            activity_item_snapshot: row.activity_item_snapshot,
            award_or_role_snapshot: row.award_or_role_snapshot,
            issue_date: row.issue_date,
            status: row.status,
            replacement_for_certificate_id: row.replacement_for_certificate_id,
            replaced_by_certificate_id: row.replaced_by_certificate_id,
            replacement_candidate_id: row.replacement_candidate_id,
            created_at: row.created_at,
        },
    )?;
    Ok(IssuedCertificateDetail {
        summary,
        issue_run_id: row.issue_run_id,
        custom_values: row.custom_values_snapshot.0,
        school_name: row.school_name_snapshot,
        owner_organization_unit_name_snapshot: row.owner_organization_unit_name_snapshot,
        revoked_by: row.revoked_by,
        revoked_at: row.revoked_at,
        revocation_reason: row.revocation_reason,
        updated_at: row.updated_at,
    })
}

fn parse_issue_codes(values: &[String]) -> Result<Vec<CertificateIssueCode>, AppError> {
    values
        .iter()
        .map(|value| {
            CertificateIssueCode::parse(value).ok_or_else(|| {
                AppError::InternalServerError("certificate_issue_code_invalid".to_string())
            })
        })
        .collect()
}

fn issuance_db_error(error: sqlx::Error) -> AppError {
    AppError::DbError(error)
}
