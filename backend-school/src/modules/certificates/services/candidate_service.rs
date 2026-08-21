use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{FromRow, PgPool, Postgres, QueryBuilder, Transaction};
use uuid::Uuid;

use crate::{
    error::AppError,
    middleware::permission::ActorContext,
    modules::certificates::models::{
        CandidateMatchStatus, CandidateNameSource, CandidateValidationCode,
        CandidateValidationStatus, CertificateAccountSearchQuery, CertificateCandidateAccount,
        CertificateCandidateBulkRequest, CertificateCandidateBulkResult,
        CertificateCandidateCapabilities, CertificateCandidateDetail,
        CertificateCandidateImportResult, CertificateCandidateListQuery,
        CertificateCandidateListResponse, CertificateCandidateSummary,
        CertificateImportBatchSummary, CertificateImportRequest, CertificateImportRowInput,
        CertificateImportSource, CreateAccountCertificateCandidateRequest,
        CreateManualExternalCandidateRequest, RecipientType, UpdateCertificateCandidateRequest,
    },
    policies::certificate_access_policy::{require_owner_action, CertificateAction},
};

use super::import_validation::{
    contains_thirteen_digit_run, normalize_display_text, normalize_name_for_match,
    normalize_template_name, validate_import_headers, validate_import_request, validate_import_row,
    ImportHeaderError, ImportRequestError, ImportRowIssue, StandardColumn,
};

const MAX_BULK_CANDIDATES: usize = 5_000;
const INSERT_CHUNK_SIZE: usize = 400;

#[derive(Debug, FromRow)]
struct CampaignAccessRow {
    owner_organization_unit_id: Option<Uuid>,
    status: String,
}

#[derive(Clone, Debug, FromRow)]
struct TemplateRow {
    id: Uuid,
    name: String,
    normalized_name: String,
    allowed_recipient_types: Vec<String>,
    is_active: bool,
    is_ready: bool,
}

impl TemplateRow {
    fn allows(&self, recipient_type: RecipientType) -> bool {
        self.allowed_recipient_types
            .iter()
            .any(|value| value == recipient_type.as_str())
    }
}

#[derive(Clone, Debug, FromRow)]
struct AccountRow {
    id: Uuid,
    username: Option<String>,
    student_id: Option<String>,
    title: Option<String>,
    first_name: String,
    last_name: String,
    user_type: String,
    status: String,
}

#[derive(Clone, Debug, FromRow)]
struct CandidateRow {
    id: Uuid,
    campaign_id: Uuid,
    batch_id: Option<Uuid>,
    template_id: Option<Uuid>,
    template_name: Option<String>,
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
    duplicate_confirmed: bool,
    issued_certificate_id: Option<Uuid>,
    locked_request_id: Option<Uuid>,
    deleted_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
struct PreparedCandidate {
    id: Uuid,
    campaign_id: Uuid,
    batch_id: Option<Uuid>,
    template_id: Option<Uuid>,
    recipient_type: RecipientType,
    matched_user_id: Option<Uuid>,
    lookup_student_id: Option<String>,
    lookup_staff_username: Option<String>,
    imported_title: Option<String>,
    imported_first_name: String,
    imported_last_name: String,
    account_title: Option<String>,
    account_first_name: Option<String>,
    account_last_name: Option<String>,
    selected_name_source: Option<CandidateNameSource>,
    activity_item: Option<String>,
    award_or_role: Option<String>,
    custom_values: BTreeMap<String, String>,
    match_status: CandidateMatchStatus,
    validation_status: CandidateValidationStatus,
    validation_codes: BTreeSet<CandidateValidationCode>,
    duplicate_confirmed: bool,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct DuplicateFingerprint {
    identity: String,
    template: String,
    activity: String,
    award: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CandidateAuditMetadata {
    campaign_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    candidate_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    batch_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    operation: Option<&'static str>,
    affected_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    ready_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    review_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    invalid_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    custom_header_count: Option<u32>,
}

const CANDIDATE_SELECT: &str = r#"
    SELECT
        candidate.id,
        candidate.campaign_id,
        candidate.batch_id,
        candidate.template_id,
        template.name AS template_name,
        candidate.recipient_type,
        candidate.matched_user_id,
        candidate.lookup_student_id,
        candidate.lookup_staff_username,
        candidate.imported_title,
        candidate.imported_first_name,
        candidate.imported_last_name,
        candidate.account_title,
        candidate.account_first_name,
        candidate.account_last_name,
        candidate.selected_name_source,
        candidate.activity_item,
        candidate.award_or_role,
        candidate.custom_values,
        candidate.match_status,
        candidate.validation_status,
        candidate.validation_codes,
        candidate.duplicate_confirmed,
        candidate.issued_certificate_id,
        (SELECT candidate_lock.request_id
         FROM certificate_candidate_issue_locks candidate_lock
         JOIN certificate_issue_requests request ON request.id = candidate_lock.request_id
         WHERE candidate_lock.candidate_id = candidate.id
           AND request.status IN ('pending', 'reviewing')
         ORDER BY request.submitted_at, request.id
         LIMIT 1) AS locked_request_id,
        candidate.deleted_at,
        candidate.created_at,
        candidate.updated_at
    FROM certificate_candidates candidate
    LEFT JOIN certificate_templates template ON template.id = candidate.template_id
"#;

pub async fn import_candidates(
    pool: &PgPool,
    actor: &ActorContext,
    campaign_id: Uuid,
    request: CertificateImportRequest,
) -> Result<CertificateCandidateImportResult, AppError> {
    // Header/request validation is deliberately completed before an insert transaction exists.
    let validated_headers = validate_import_request(&request).map_err(import_request_error)?;
    if request.rows.iter().any(|row| {
        validate_import_row(row)
            .issues
            .contains(&ImportRowIssue::ForbiddenSensitiveValue)
    }) {
        return Err(AppError::ValidationError(
            "certificate_import_forbidden_sensitive_value".to_string(),
        ));
    }
    let access = campaign_access(pool, campaign_id).await?;
    require_owner_action(
        pool,
        actor,
        access.owner_organization_unit_id,
        CertificateAction::Update,
    )
    .await?;
    let mut tx = pool.begin().await.map_err(candidate_db_error)?;
    let locked_access = lock_campaign(&mut tx, campaign_id).await?;
    require_same_owner(&access, &locked_access)?;
    require_mutable_campaign(&locked_access.status)?;

    let templates = load_templates(&mut tx, campaign_id).await?;
    let (students, staff) = load_import_accounts(&mut tx, &request.rows).await?;
    let canonical_custom_headers = validated_headers
        .custom_headers
        .iter()
        .map(|header| (normalize_name_for_match(header), header.clone()))
        .collect::<BTreeMap<_, _>>();

    let mut prepared = request
        .rows
        .iter()
        .map(|row| {
            prepare_import_candidate(
                campaign_id,
                row,
                &templates,
                &students,
                &staff,
                &canonical_custom_headers,
            )
        })
        .collect::<Vec<_>>();
    apply_duplicate_warnings(&mut tx, campaign_id, &mut prepared).await?;

    let ready_count = count_status(&prepared, CandidateValidationStatus::Ready);
    let review_count = count_status(&prepared, CandidateValidationStatus::NeedsReview);
    let invalid_count = count_status(&prepared, CandidateValidationStatus::Invalid);
    let (batch_id, batch_created_at): (Uuid, DateTime<Utc>) = sqlx::query_as(
        "INSERT INTO certificate_import_batches
            (campaign_id, source, row_count, custom_headers, ready_count, review_count,
             invalid_count, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         RETURNING id, created_at",
    )
    .bind(campaign_id)
    .bind(request.source.as_str())
    .bind(i32::try_from(request.rows.len()).map_err(|_| invalid_import())?)
    .bind(&validated_headers.custom_headers)
    .bind(ready_count)
    .bind(review_count)
    .bind(invalid_count)
    .bind(actor.user_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(candidate_db_error)?;
    for candidate in &mut prepared {
        candidate.batch_id = Some(batch_id);
    }
    insert_candidates(&mut tx, actor.user_id, &prepared).await?;
    recompute_campaign_duplicate_warnings(&mut tx, actor.user_id, campaign_id).await?;
    record_candidate_audit(
        &mut tx,
        actor.user_id,
        "import",
        CandidateAuditMetadata {
            campaign_id,
            candidate_id: None,
            batch_id: Some(batch_id),
            operation: Some(request.source.as_str()),
            affected_count: prepared.len() as u32,
            ready_count: Some(ready_count),
            review_count: Some(review_count),
            invalid_count: Some(invalid_count),
            custom_header_count: Some(validated_headers.custom_headers.len() as u32),
        },
    )
    .await?;
    tx.commit().await.map_err(candidate_db_error)?;

    let candidate_ids = prepared
        .iter()
        .map(|candidate| candidate.id)
        .collect::<Vec<_>>();
    let candidates = fetch_candidate_details(pool, actor, &candidate_ids, true).await?;
    Ok(CertificateCandidateImportResult {
        batch: CertificateImportBatchSummary {
            id: batch_id,
            campaign_id,
            source: request.source,
            row_count: request.rows.len() as i32,
            custom_headers: validated_headers.custom_headers,
            ready_count,
            review_count,
            invalid_count,
            created_at: batch_created_at,
        },
        candidates,
    })
}

pub async fn list_candidates(
    pool: &PgPool,
    actor: &ActorContext,
    campaign_id: Uuid,
    query: CertificateCandidateListQuery,
) -> Result<CertificateCandidateListResponse, AppError> {
    let access = campaign_access(pool, campaign_id).await?;
    require_owner_action(
        pool,
        actor,
        access.owner_organization_unit_id,
        CertificateAction::Read,
    )
    .await?;
    let can_update = require_owner_action(
        pool,
        actor,
        access.owner_organization_unit_id,
        CertificateAction::Update,
    )
    .await
    .is_ok()
        && campaign_status_is_mutable(&access.status);

    let mut builder = QueryBuilder::<Postgres>::new(CANDIDATE_SELECT);
    builder.push(" WHERE candidate.campaign_id = ");
    builder.push_bind(campaign_id);
    builder.push(" AND candidate.deleted_at IS NULL");
    if let Some(status) = query.status {
        builder.push(" AND candidate.validation_status = ");
        builder.push_bind(status.as_str());
    }
    if let Some(template_id) = query.template_id {
        builder.push(" AND candidate.template_id = ");
        builder.push_bind(template_id);
    }
    if let Some(search) = normalized_search(query.search.as_deref())? {
        let pattern = format!("%{search}%");
        builder.push(" AND (candidate.imported_first_name ILIKE ");
        builder.push_bind(pattern.clone());
        builder.push(" OR candidate.imported_last_name ILIKE ");
        builder.push_bind(pattern.clone());
        builder.push(" OR COALESCE(candidate.activity_item, '') ILIKE ");
        builder.push_bind(pattern.clone());
        builder.push(" OR COALESCE(candidate.award_or_role, '') ILIKE ");
        builder.push_bind(pattern);
        builder.push(")");
    }
    builder.push(" ORDER BY candidate.created_at, candidate.id");
    let rows = builder
        .build_query_as::<CandidateRow>()
        .fetch_all(pool)
        .await
        .map_err(candidate_db_error)?;
    let items = rows
        .into_iter()
        .map(|row| candidate_detail(row, can_update))
        .collect::<Result<Vec<_>, _>>()?;
    let summary = sqlx::query_as::<_, (i64, i64, i64, i64)>(
        "SELECT
            COUNT(*)::bigint,
            COUNT(*) FILTER (WHERE validation_status = 'ready')::bigint,
            COUNT(*) FILTER (WHERE validation_status = 'needs_review')::bigint,
            COUNT(*) FILTER (WHERE validation_status = 'invalid')::bigint
         FROM certificate_candidates
         WHERE campaign_id = $1 AND deleted_at IS NULL",
    )
    .bind(campaign_id)
    .fetch_one(pool)
    .await
    .map_err(candidate_db_error)?;
    Ok(CertificateCandidateListResponse {
        items,
        summary: CertificateCandidateSummary {
            total_count: summary.0,
            ready_count: summary.1,
            review_count: summary.2,
            invalid_count: summary.3,
        },
    })
}

pub async fn get_candidate(
    pool: &PgPool,
    actor: &ActorContext,
    candidate_id: Uuid,
) -> Result<CertificateCandidateDetail, AppError> {
    let row = fetch_candidate_row(pool, candidate_id, false).await?;
    let access = campaign_access(pool, row.campaign_id).await?;
    require_owner_action(
        pool,
        actor,
        access.owner_organization_unit_id,
        CertificateAction::Read,
    )
    .await?;
    let can_update = require_owner_action(
        pool,
        actor,
        access.owner_organization_unit_id,
        CertificateAction::Update,
    )
    .await
    .is_ok()
        && campaign_status_is_mutable(&access.status);
    candidate_detail(row, can_update)
}

pub async fn create_manual_external(
    pool: &PgPool,
    actor: &ActorContext,
    campaign_id: Uuid,
    request: CreateManualExternalCandidateRequest,
) -> Result<CertificateCandidateImportResult, AppError> {
    let row = CertificateImportRowInput {
        recipient_type: "external".to_string(),
        student_id: None,
        staff_username: None,
        title: request.title,
        first_name: request.first_name,
        last_name: request.last_name,
        activity_item: request.activity_item,
        award_or_role: request.award_or_role,
        template_name: None,
        custom_values: request.custom_values,
    };
    validate_manual_row(&row)?;
    create_single_candidate(
        pool,
        actor,
        campaign_id,
        CertificateImportSource::Manual,
        row,
        request.template_id,
        None,
    )
    .await
}

pub async fn search_accounts(
    pool: &PgPool,
    actor: &ActorContext,
    campaign_id: Uuid,
    query: CertificateAccountSearchQuery,
) -> Result<Vec<CertificateCandidateAccount>, AppError> {
    if query.recipient_type == RecipientType::External {
        return Err(AppError::ValidationError(
            "ค้นหาบัญชีได้เฉพาะนักเรียนหรือบุคลากร".to_string(),
        ));
    }
    let search = normalized_search(Some(&query.search))?
        .ok_or_else(|| AppError::ValidationError("กรุณาระบุคำค้นอย่างน้อย 2 ตัวอักษร".to_string()))?;
    if search.chars().count() < 2 {
        return Err(AppError::ValidationError(
            "กรุณาระบุคำค้นอย่างน้อย 2 ตัวอักษร".to_string(),
        ));
    }
    let access = campaign_access(pool, campaign_id).await?;
    require_owner_action(
        pool,
        actor,
        access.owner_organization_unit_id,
        CertificateAction::Update,
    )
    .await?;
    let pattern = format!("%{search}%");
    let rows = match query.recipient_type {
        RecipientType::Student => sqlx::query_as::<_, AccountRow>(
            "SELECT u.id, u.username, student.student_id, u.title, u.first_name,
                        u.last_name, u.user_type, u.status
                 FROM users u
                 JOIN student_info student ON student.user_id = u.id
                 WHERE u.user_type = 'student' AND u.status = 'active'
                   AND (student.student_id ILIKE $1 OR u.first_name ILIKE $1
                        OR u.last_name ILIKE $1 OR u.username ILIKE $1)
                 ORDER BY u.first_name, u.last_name, student.student_id
                 LIMIT 20",
        )
        .bind(pattern)
        .fetch_all(pool)
        .await
        .map_err(candidate_db_error)?,
        RecipientType::Staff => sqlx::query_as::<_, AccountRow>(
            "SELECT u.id, u.username, NULL::text AS student_id, u.title, u.first_name,
                        u.last_name, u.user_type, u.status
                 FROM users u
                 WHERE u.user_type = 'staff' AND u.status = 'active'
                   AND (u.username ILIKE $1 OR u.first_name ILIKE $1 OR u.last_name ILIKE $1)
                 ORDER BY u.first_name, u.last_name, u.username
                 LIMIT 20",
        )
        .bind(pattern)
        .fetch_all(pool)
        .await
        .map_err(candidate_db_error)?,
        RecipientType::External => unreachable!(),
    };
    rows.into_iter().map(account_response).collect()
}

pub async fn create_account_candidate(
    pool: &PgPool,
    actor: &ActorContext,
    campaign_id: Uuid,
    request: CreateAccountCertificateCandidateRequest,
) -> Result<CertificateCandidateImportResult, AppError> {
    let access = campaign_access(pool, campaign_id).await?;
    require_owner_action(
        pool,
        actor,
        access.owner_organization_unit_id,
        CertificateAction::Update,
    )
    .await?;
    let account = load_account_by_id(pool, request.user_id).await?;
    if account.status != "active" {
        return Err(AppError::Conflict("บัญชีนี้ไม่ได้อยู่ในสถานะพร้อมใช้งาน".to_string()));
    }
    let recipient_type = account_recipient_type(&account)?;
    let row = CertificateImportRowInput {
        recipient_type: recipient_type.as_str().to_string(),
        student_id: account.student_id.clone(),
        staff_username: if recipient_type == RecipientType::Staff {
            account.username.clone()
        } else {
            None
        },
        title: account.title.clone(),
        first_name: account.first_name.clone(),
        last_name: account.last_name.clone(),
        activity_item: request.activity_item,
        award_or_role: request.award_or_role,
        template_name: None,
        custom_values: request.custom_values,
    };
    validate_manual_row(&row)?;
    create_single_candidate(
        pool,
        actor,
        campaign_id,
        CertificateImportSource::AccountSearch,
        row,
        request.template_id,
        Some(account),
    )
    .await
}

pub async fn update_candidate(
    pool: &PgPool,
    actor: &ActorContext,
    candidate_id: Uuid,
    request: UpdateCertificateCandidateRequest,
) -> Result<CertificateCandidateDetail, AppError> {
    let existing = fetch_candidate_row(pool, candidate_id, false).await?;
    let access = campaign_access(pool, existing.campaign_id).await?;
    require_owner_action(
        pool,
        actor,
        access.owner_organization_unit_id,
        CertificateAction::Update,
    )
    .await?;
    let can_read_locked_request =
        super::request_service::can_read_request(pool, actor, access.owner_organization_unit_id)
            .await?;

    let mut tx = pool.begin().await.map_err(candidate_db_error)?;
    let locked_access = lock_campaign(&mut tx, existing.campaign_id).await?;
    require_same_owner(&access, &locked_access)?;
    require_mutable_campaign(&locked_access.status)?;
    let locked = lock_candidate(&mut tx, candidate_id).await?;
    require_candidate_mutable(&locked, can_read_locked_request)?;
    if locked.updated_at != request.expected_updated_at {
        return Err(AppError::Conflict(
            "รายชื่อผู้รับถูกแก้ไขจากหน้าจออื่น กรุณาโหลดใหม่".to_string(),
        ));
    }
    let previous_type = parse_recipient_type_db(&locked.recipient_type)?;
    let had_invalid_type = locked
        .validation_codes
        .iter()
        .any(|code| code == CandidateValidationCode::InvalidRecipientType.as_str());
    if previous_type != request.recipient_type && !had_invalid_type {
        return Err(AppError::Conflict(
            "การเปลี่ยนเป็นบุคคลภายนอกต้องใช้คำสั่งยืนยันเฉพาะ".to_string(),
        ));
    }
    let mut row = CertificateImportRowInput {
        recipient_type: request.recipient_type.as_str().to_string(),
        student_id: request.student_id,
        staff_username: request.staff_username,
        title: request.imported_title,
        first_name: request.imported_first_name,
        last_name: request.imported_last_name,
        activity_item: request.activity_item,
        award_or_role: request.award_or_role,
        template_name: None,
        custom_values: request.custom_values,
    };
    let converted_external = prepare_converted_external_account_recheck(&locked, &mut row);
    validate_manual_row(&row)?;
    if converted_external {
        lock_account_identity_writes(&mut tx).await?;
    }
    let templates = load_templates(&mut tx, existing.campaign_id).await?;
    let (students, staff) = load_import_accounts(&mut tx, std::slice::from_ref(&row)).await?;
    let canonical = canonical_custom_headers(&row.custom_values)?;
    require_known_campaign_custom_headers(&mut tx, existing.campaign_id, &canonical).await?;
    let mut prepared = prepare_import_candidate(
        existing.campaign_id,
        &row,
        &templates,
        &students,
        &staff,
        &canonical,
    );
    prepared.id = candidate_id;
    prepared.batch_id = locked.batch_id;
    prepared.template_id = request.template_id;
    prepared.selected_name_source = request.selected_name_source;
    restore_converted_external_state(&mut prepared, &locked);
    resolve_explicit_template(&mut prepared, &templates);
    apply_selected_name_resolution(&mut prepared);
    prepared.duplicate_confirmed = locked.duplicate_confirmed;
    if duplicate_fingerprint(&prepared)
        != duplicate_fingerprint(&prepared_from_row(locked.clone())?)
    {
        prepared.duplicate_confirmed = false;
    }
    apply_single_duplicate_warning(&mut tx, &mut prepared).await?;
    update_prepared_candidate(&mut tx, actor.user_id, &prepared).await?;
    recompute_campaign_duplicate_warnings(&mut tx, actor.user_id, existing.campaign_id).await?;
    record_candidate_audit(
        &mut tx,
        actor.user_id,
        "update",
        CandidateAuditMetadata {
            campaign_id: existing.campaign_id,
            candidate_id: Some(candidate_id),
            batch_id: locked.batch_id,
            operation: Some("edit"),
            affected_count: 1,
            ready_count: None,
            review_count: None,
            invalid_count: None,
            custom_header_count: Some(prepared.custom_values.len() as u32),
        },
    )
    .await?;
    tx.commit().await.map_err(candidate_db_error)?;
    get_candidate(pool, actor, candidate_id).await
}

pub async fn confirm_external(
    pool: &PgPool,
    actor: &ActorContext,
    candidate_id: Uuid,
) -> Result<CertificateCandidateDetail, AppError> {
    let outcome = bulk_update(
        pool,
        actor,
        CertificateCandidateBulkRequest::ConfirmExternal {
            candidate_ids: vec![candidate_id],
        },
    )
    .await?;
    outcome
        .candidates
        .into_iter()
        .next()
        .ok_or_else(candidate_not_found)
}

pub async fn delete_candidate(
    pool: &PgPool,
    actor: &ActorContext,
    candidate_id: Uuid,
) -> Result<CertificateCandidateDetail, AppError> {
    let outcome = bulk_update(
        pool,
        actor,
        CertificateCandidateBulkRequest::SoftDelete {
            candidate_ids: vec![candidate_id],
        },
    )
    .await?;
    outcome
        .candidates
        .into_iter()
        .next()
        .ok_or_else(candidate_not_found)
}

pub async fn bulk_update(
    pool: &PgPool,
    actor: &ActorContext,
    request: CertificateCandidateBulkRequest,
) -> Result<CertificateCandidateBulkResult, AppError> {
    bulk_update_inner(pool, actor, None, request).await
}

pub async fn bulk_update_for_campaign(
    pool: &PgPool,
    actor: &ActorContext,
    campaign_id: Uuid,
    request: CertificateCandidateBulkRequest,
) -> Result<CertificateCandidateBulkResult, AppError> {
    bulk_update_inner(pool, actor, Some(campaign_id), request).await
}

pub async fn preview_values(
    pool: &PgPool,
    actor: &ActorContext,
    candidate_id: Uuid,
    template_id: Uuid,
) -> Result<BTreeMap<String, String>, AppError> {
    let candidate = get_candidate(pool, actor, candidate_id).await?;
    if candidate.template_id != Some(template_id) {
        return Err(AppError::ValidationError(
            "รายชื่อผู้รับไม่ได้ใช้แม่แบบนี้".to_string(),
        ));
    }
    let (title, first_name, last_name) = selected_name(&candidate);
    let mut values = candidate.custom_values.clone();
    values.insert("คำนำหน้า".to_string(), title.unwrap_or_default());
    values.insert("ชื่อ".to_string(), first_name);
    values.insert("นามสกุล".to_string(), last_name);
    values.insert(
        "รายการกิจกรรม".to_string(),
        candidate.activity_item.unwrap_or_default(),
    );
    values.insert(
        "รางวัลหรือบทบาท".to_string(),
        candidate.award_or_role.unwrap_or_default(),
    );
    Ok(values)
}

async fn create_single_candidate(
    pool: &PgPool,
    actor: &ActorContext,
    campaign_id: Uuid,
    source: CertificateImportSource,
    mut row: CertificateImportRowInput,
    template_id: Option<Uuid>,
    exact_account: Option<AccountRow>,
) -> Result<CertificateCandidateImportResult, AppError> {
    let access = campaign_access(pool, campaign_id).await?;
    require_owner_action(
        pool,
        actor,
        access.owner_organization_unit_id,
        CertificateAction::Update,
    )
    .await?;
    let canonical = canonical_custom_headers(&row.custom_values)?;
    let custom_headers = canonical.values().cloned().collect::<Vec<_>>();
    let mut tx = pool.begin().await.map_err(candidate_db_error)?;
    let locked_access = lock_campaign(&mut tx, campaign_id).await?;
    require_same_owner(&access, &locked_access)?;
    require_mutable_campaign(&locked_access.status)?;
    let templates = load_templates(&mut tx, campaign_id).await?;
    let (students, staff) = if let Some(account) = exact_account {
        let account = load_account_by_id_tx(&mut tx, account.id).await?;
        if account.status != "active" {
            return Err(AppError::Conflict("บัญชีนี้ไม่ได้อยู่ในสถานะพร้อมใช้งาน".to_string()));
        }
        let recipient_type = account_recipient_type(&account)?;
        row.recipient_type = recipient_type.as_str().to_string();
        row.student_id = account.student_id.clone();
        row.staff_username = if recipient_type == RecipientType::Staff {
            account.username.clone()
        } else {
            None
        };
        row.title = account.title.clone();
        row.first_name = account.first_name.clone();
        row.last_name = account.last_name.clone();
        let mut students = BTreeMap::new();
        let mut staff = BTreeMap::new();
        if let Some(student_id) = account.student_id.clone() {
            students.insert(student_id, account);
        } else if let Some(username) = account.username.clone() {
            staff.insert(username, account);
        }
        (students, staff)
    } else {
        load_import_accounts(&mut tx, std::slice::from_ref(&row)).await?
    };
    let mut prepared =
        prepare_import_candidate(campaign_id, &row, &templates, &students, &staff, &canonical);
    prepared.template_id = template_id;
    resolve_explicit_template(&mut prepared, &templates);
    apply_single_duplicate_warning(&mut tx, &mut prepared).await?;
    let ready_count = i32::from(prepared.validation_status == CandidateValidationStatus::Ready);
    let review_count =
        i32::from(prepared.validation_status == CandidateValidationStatus::NeedsReview);
    let invalid_count = i32::from(prepared.validation_status == CandidateValidationStatus::Invalid);
    let (batch_id, created_at): (Uuid, DateTime<Utc>) = sqlx::query_as(
        "INSERT INTO certificate_import_batches
            (campaign_id, source, row_count, custom_headers, ready_count, review_count,
             invalid_count, created_by)
         VALUES ($1, $2, 1, $3, $4, $5, $6, $7)
         RETURNING id, created_at",
    )
    .bind(campaign_id)
    .bind(source.as_str())
    .bind(&custom_headers)
    .bind(ready_count)
    .bind(review_count)
    .bind(invalid_count)
    .bind(actor.user_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(candidate_db_error)?;
    prepared.batch_id = Some(batch_id);
    insert_candidates(&mut tx, actor.user_id, std::slice::from_ref(&prepared)).await?;
    recompute_campaign_duplicate_warnings(&mut tx, actor.user_id, campaign_id).await?;
    record_candidate_audit(
        &mut tx,
        actor.user_id,
        "create",
        CandidateAuditMetadata {
            campaign_id,
            candidate_id: Some(prepared.id),
            batch_id: Some(batch_id),
            operation: Some(source.as_str()),
            affected_count: 1,
            ready_count: Some(ready_count),
            review_count: Some(review_count),
            invalid_count: Some(invalid_count),
            custom_header_count: Some(custom_headers.len() as u32),
        },
    )
    .await?;
    tx.commit().await.map_err(candidate_db_error)?;
    Ok(CertificateCandidateImportResult {
        batch: CertificateImportBatchSummary {
            id: batch_id,
            campaign_id,
            source,
            row_count: 1,
            custom_headers,
            ready_count,
            review_count,
            invalid_count,
            created_at,
        },
        candidates: fetch_candidate_details(pool, actor, &[prepared.id], true).await?,
    })
}

async fn bulk_update_inner(
    pool: &PgPool,
    actor: &ActorContext,
    expected_campaign_id: Option<Uuid>,
    request: CertificateCandidateBulkRequest,
) -> Result<CertificateCandidateBulkResult, AppError> {
    let mut candidate_ids = request.candidate_ids().to_vec();
    candidate_ids.sort_unstable();
    candidate_ids.dedup();
    if candidate_ids.is_empty() || candidate_ids.len() > MAX_BULK_CANDIDATES {
        return Err(AppError::ValidationError(
            "ต้องเลือกรายชื่อผู้รับ 1–5,000 รายการ".to_string(),
        ));
    }
    if candidate_ids.len() != request.candidate_ids().len() {
        return Err(AppError::ValidationError("รายการที่เลือกมีรหัสซ้ำ".to_string()));
    }
    let first = fetch_candidate_row(pool, candidate_ids[0], false).await?;
    if expected_campaign_id.is_some_and(|campaign_id| campaign_id != first.campaign_id) {
        return Err(candidate_not_found());
    }
    let access = campaign_access(pool, first.campaign_id).await?;
    require_owner_action(
        pool,
        actor,
        access.owner_organization_unit_id,
        CertificateAction::Update,
    )
    .await?;
    let can_read_locked_request =
        super::request_service::can_read_request(pool, actor, access.owner_organization_unit_id)
            .await?;
    let mut tx = pool.begin().await.map_err(candidate_db_error)?;
    let locked_access = lock_campaign(&mut tx, first.campaign_id).await?;
    require_same_owner(&access, &locked_access)?;
    require_mutable_campaign(&locked_access.status)?;
    let rows = lock_candidates(&mut tx, &candidate_ids).await?;
    if rows.len() != candidate_ids.len()
        || rows.iter().any(|row| row.campaign_id != first.campaign_id)
    {
        return Err(candidate_not_found());
    }
    for row in &rows {
        require_candidate_mutable(row, can_read_locked_request)?;
    }
    let reconciles_converted_external =
        !matches!(&request, CertificateCandidateBulkRequest::SoftDelete { .. })
            && rows.iter().any(is_converted_external_candidate);
    if matches!(
        &request,
        CertificateCandidateBulkRequest::ConfirmExternal { .. }
    ) || reconciles_converted_external
    {
        lock_account_identity_writes(&mut tx).await?;
    }

    let templates = load_templates(&mut tx, first.campaign_id).await?;
    let import_rows = rows.iter().map(candidate_as_import_row).collect::<Vec<_>>();
    let (students, staff) = load_import_accounts(&mut tx, &import_rows).await?;
    let operation = bulk_operation_name(&request);
    let mut updated = Vec::with_capacity(rows.len());
    for (row, input) in rows.iter().zip(import_rows.iter()) {
        let canonical = canonical_custom_headers(&input.custom_values)?;
        let mut prepared = prepare_import_candidate(
            first.campaign_id,
            input,
            &templates,
            &students,
            &staff,
            &canonical,
        );
        prepared.id = row.id;
        prepared.batch_id = row.batch_id;
        prepared.template_id = row.template_id;
        prepared.duplicate_confirmed = row.duplicate_confirmed;
        prepared.selected_name_source =
            parse_optional_name_source(row.selected_name_source.as_deref())?;
        restore_converted_external_state(&mut prepared, row);
        let previous_fingerprint = duplicate_fingerprint(&prepared_from_row(row.clone())?);

        match &request {
            CertificateCandidateBulkRequest::AssignTemplate { template_id, .. } => {
                prepared.template_id = Some(*template_id);
            }
            CertificateCandidateBulkRequest::ChooseName { name_source, .. } => {
                if prepared.matched_user_id.is_none()
                    || prepared.match_status != CandidateMatchStatus::NameMismatch
                {
                    return Err(AppError::Conflict(
                        "บางรายการไม่มีชื่อบัญชีและชื่อไฟล์ให้เลือก".to_string(),
                    ));
                }
                prepared.selected_name_source = Some(*name_source);
            }
            CertificateCandidateBulkRequest::ConfirmExternal { .. } => {
                if !matches!(
                    prepared.recipient_type,
                    RecipientType::Student | RecipientType::Staff
                ) || prepared.matched_user_id.is_some()
                    || prepared.match_status != CandidateMatchStatus::NotFound
                    || (prepared.recipient_type == RecipientType::Student
                        && prepared.lookup_student_id.is_none())
                    || (prepared.recipient_type == RecipientType::Staff
                        && prepared.lookup_staff_username.is_none())
                {
                    return Err(AppError::Conflict(
                        "รายการที่มีบัญชีอยู่แล้วไม่สามารถเปลี่ยนเป็นบุคคลภายนอกได้".to_string(),
                    ));
                }
                // This lookup includes every account status and is repeated after the candidate
                // lock, so every account committed before this confirmation point is observed.
                if authoritative_account_exists(&mut tx, &prepared).await? {
                    return Err(AppError::Conflict(
                        "พบบัญชีที่ตรงกับรหัสแล้ว จึงไม่สามารถเปลี่ยนเป็นบุคคลภายนอกได้".to_string(),
                    ));
                }
                prepared.recipient_type = RecipientType::External;
                prepared.matched_user_id = None;
                prepared.account_title = None;
                prepared.account_first_name = None;
                prepared.account_last_name = None;
                prepared.selected_name_source = Some(CandidateNameSource::File);
                prepared.match_status = CandidateMatchStatus::ExternalConfirmed;
                prepared
                    .validation_codes
                    .remove(&CandidateValidationCode::AccountNotFound);
            }
            CertificateCandidateBulkRequest::ConfirmDuplicate { .. } => {
                if !row
                    .validation_codes
                    .iter()
                    .any(|code| code == CandidateValidationCode::DuplicateCandidate.as_str())
                {
                    return Err(AppError::Conflict(
                        "บางรายการไม่มีคำเตือนรายการซ้ำให้ยืนยัน".to_string(),
                    ));
                }
                prepared.duplicate_confirmed = true;
            }
            CertificateCandidateBulkRequest::SoftDelete { .. } => {}
        }

        if !matches!(request, CertificateCandidateBulkRequest::SoftDelete { .. }) {
            resolve_explicit_template(&mut prepared, &templates);
            apply_selected_name_resolution(&mut prepared);
            if !matches!(
                request,
                CertificateCandidateBulkRequest::ConfirmDuplicate { .. }
            ) && duplicate_fingerprint(&prepared) != previous_fingerprint
            {
                prepared.duplicate_confirmed = false;
            }
        }
        updated.push(prepared);
    }

    if matches!(request, CertificateCandidateBulkRequest::SoftDelete { .. }) {
        sqlx::query(
            "UPDATE certificate_candidates
             SET deleted_at = clock_timestamp(), updated_by = $1,
                 updated_at = clock_timestamp()
             WHERE id = ANY($2::uuid[])",
        )
        .bind(actor.user_id)
        .bind(&candidate_ids)
        .execute(&mut *tx)
        .await
        .map_err(candidate_db_error)?;
    } else {
        apply_bulk_duplicate_warnings(&mut tx, first.campaign_id, &candidate_ids, &mut updated)
            .await?;
        for candidate in &updated {
            update_prepared_candidate(&mut tx, actor.user_id, candidate).await?;
        }
    }
    recompute_campaign_duplicate_warnings(&mut tx, actor.user_id, first.campaign_id).await?;
    record_candidate_audit(
        &mut tx,
        actor.user_id,
        "bulk_update",
        CandidateAuditMetadata {
            campaign_id: first.campaign_id,
            candidate_id: None,
            batch_id: None,
            operation: Some(operation),
            affected_count: candidate_ids.len() as u32,
            ready_count: None,
            review_count: None,
            invalid_count: None,
            custom_header_count: None,
        },
    )
    .await?;
    tx.commit().await.map_err(candidate_db_error)?;
    let candidates = fetch_candidate_details(pool, actor, &candidate_ids, true).await?;
    Ok(CertificateCandidateBulkResult {
        updated_count: candidate_ids.len() as u32,
        candidates,
    })
}

fn prepare_import_candidate(
    campaign_id: Uuid,
    row: &CertificateImportRowInput,
    templates: &[TemplateRow],
    students: &BTreeMap<String, AccountRow>,
    staff: &BTreeMap<String, AccountRow>,
    canonical_custom_headers: &BTreeMap<String, String>,
) -> PreparedCandidate {
    let validation = validate_import_row(row);
    let recipient_type = validation.recipient_type.unwrap_or(RecipientType::External);
    let mut codes = validation
        .issues
        .iter()
        .map(import_issue_code)
        .collect::<BTreeSet<_>>();
    let hard_invalid = !validation.issues.is_empty();
    let student_id = normalize_optional(row.student_id.as_deref());
    let staff_username = normalize_optional(row.staff_username.as_deref());
    let mut candidate = PreparedCandidate {
        id: Uuid::new_v4(),
        campaign_id,
        batch_id: None,
        template_id: None,
        recipient_type,
        matched_user_id: None,
        lookup_student_id: if recipient_type == RecipientType::Student {
            student_id.clone()
        } else {
            None
        },
        lookup_staff_username: if recipient_type == RecipientType::Staff {
            staff_username.clone()
        } else {
            None
        },
        imported_title: normalize_optional(row.title.as_deref()),
        imported_first_name: normalize_display_text(&row.first_name),
        imported_last_name: normalize_display_text(&row.last_name),
        account_title: None,
        account_first_name: None,
        account_last_name: None,
        selected_name_source: None,
        activity_item: normalize_optional(row.activity_item.as_deref()),
        award_or_role: normalize_optional(row.award_or_role.as_deref()),
        custom_values: canonicalize_custom_values(&row.custom_values, canonical_custom_headers),
        match_status: CandidateMatchStatus::NotApplicable,
        validation_status: CandidateValidationStatus::NeedsReview,
        validation_codes: BTreeSet::new(),
        duplicate_confirmed: false,
    };
    match recipient_type {
        RecipientType::Student => {
            apply_account_match(
                &mut candidate,
                student_id.as_ref().and_then(|value| students.get(value)),
                &mut codes,
            );
        }
        RecipientType::Staff => {
            apply_account_match(
                &mut candidate,
                staff_username.as_ref().and_then(|value| staff.get(value)),
                &mut codes,
            );
        }
        RecipientType::External => {
            candidate.match_status = CandidateMatchStatus::NotApplicable;
            candidate.selected_name_source = Some(CandidateNameSource::File);
        }
    }
    resolve_template_name(
        &mut candidate,
        row.template_name.as_deref(),
        templates,
        &mut codes,
    );
    candidate.validation_codes = codes;
    refresh_validation_status(&mut candidate, hard_invalid);
    candidate
}

fn apply_account_match(
    candidate: &mut PreparedCandidate,
    account: Option<&AccountRow>,
    codes: &mut BTreeSet<CandidateValidationCode>,
) {
    let Some(account) = account else {
        candidate.match_status = CandidateMatchStatus::NotFound;
        codes.insert(CandidateValidationCode::AccountNotFound);
        return;
    };
    candidate.matched_user_id = Some(account.id);
    candidate.account_title = account.title.clone();
    candidate.account_first_name = Some(account.first_name.clone());
    candidate.account_last_name = Some(account.last_name.clone());
    if account.status != "active" {
        candidate.match_status = CandidateMatchStatus::Inactive;
        codes.insert(CandidateValidationCode::AccountInactive);
        return;
    }
    let imported_name = normalize_name_for_match(&format!(
        "{} {}",
        candidate.imported_first_name, candidate.imported_last_name
    ));
    let account_name =
        normalize_name_for_match(&format!("{} {}", account.first_name, account.last_name));
    if imported_name == account_name {
        candidate.match_status = CandidateMatchStatus::Matched;
        candidate.selected_name_source = Some(CandidateNameSource::Account);
    } else {
        candidate.match_status = CandidateMatchStatus::NameMismatch;
        codes.insert(CandidateValidationCode::NameSourceRequired);
    }
}

fn resolve_template_name(
    candidate: &mut PreparedCandidate,
    requested_name: Option<&str>,
    templates: &[TemplateRow],
    codes: &mut BTreeSet<CandidateValidationCode>,
) {
    let selected = if let Some(name) = normalize_optional(requested_name) {
        let normalized = normalize_template_name(&name);
        templates
            .iter()
            .find(|template| template.normalized_name == normalized)
    } else {
        let compatible = templates
            .iter()
            .filter(|template| template.is_active && template.allows(candidate.recipient_type))
            .collect::<Vec<_>>();
        (compatible.len() == 1).then_some(compatible[0])
    };
    let Some(template) = selected else {
        codes.insert(
            if requested_name.is_some_and(|value| !value.trim().is_empty()) {
                CandidateValidationCode::TemplateNotFound
            } else {
                CandidateValidationCode::TemplateRequired
            },
        );
        return;
    };
    candidate.template_id = Some(template.id);
    apply_template_validation(candidate, template, codes);
}

fn resolve_explicit_template(candidate: &mut PreparedCandidate, templates: &[TemplateRow]) {
    for code in [
        CandidateValidationCode::TemplateRequired,
        CandidateValidationCode::TemplateNotFound,
        CandidateValidationCode::TemplateIncompatible,
        CandidateValidationCode::TemplateNotReady,
    ] {
        candidate.validation_codes.remove(&code);
    }
    let Some(template_id) = candidate.template_id else {
        candidate
            .validation_codes
            .insert(CandidateValidationCode::TemplateRequired);
        refresh_validation_status(candidate, candidate_has_hard_invalid(candidate));
        return;
    };
    let Some(template) = templates.iter().find(|template| template.id == template_id) else {
        candidate.template_id = None;
        candidate
            .validation_codes
            .insert(CandidateValidationCode::TemplateNotFound);
        refresh_validation_status(candidate, candidate_has_hard_invalid(candidate));
        return;
    };
    let mut codes = std::mem::take(&mut candidate.validation_codes);
    apply_template_validation(candidate, template, &mut codes);
    candidate.validation_codes = codes;
    refresh_validation_status(candidate, candidate_has_hard_invalid(candidate));
}

fn apply_template_validation(
    candidate: &PreparedCandidate,
    template: &TemplateRow,
    codes: &mut BTreeSet<CandidateValidationCode>,
) {
    if !template.allows(candidate.recipient_type) {
        codes.insert(CandidateValidationCode::TemplateIncompatible);
    }
    if !template.is_active || !template.is_ready {
        codes.insert(CandidateValidationCode::TemplateNotReady);
    }
}

fn apply_selected_name_resolution(candidate: &mut PreparedCandidate) {
    candidate
        .validation_codes
        .remove(&CandidateValidationCode::NameSourceRequired);
    if candidate.match_status == CandidateMatchStatus::NameMismatch
        && candidate.selected_name_source.is_none()
    {
        candidate
            .validation_codes
            .insert(CandidateValidationCode::NameSourceRequired);
    }
    if candidate.selected_name_source == Some(CandidateNameSource::Account)
        && candidate.matched_user_id.is_none()
    {
        candidate.selected_name_source = None;
        candidate
            .validation_codes
            .insert(CandidateValidationCode::NameSourceRequired);
    }
    refresh_validation_status(candidate, candidate_has_hard_invalid(candidate));
}

fn restore_converted_external_state(candidate: &mut PreparedCandidate, row: &CandidateRow) {
    if !is_converted_external_candidate(row) || candidate.matched_user_id.is_some() {
        return;
    }
    candidate.recipient_type = RecipientType::External;
    candidate.lookup_student_id = row.lookup_student_id.clone();
    candidate.lookup_staff_username = row.lookup_staff_username.clone();
    candidate.matched_user_id = None;
    candidate.account_title = None;
    candidate.account_first_name = None;
    candidate.account_last_name = None;
    candidate.selected_name_source = Some(CandidateNameSource::File);
    candidate.match_status = CandidateMatchStatus::ExternalConfirmed;
    candidate
        .validation_codes
        .remove(&CandidateValidationCode::UnexpectedInternalLookup);
    candidate
        .validation_codes
        .remove(&CandidateValidationCode::AccountNotFound);
    candidate
        .validation_codes
        .remove(&CandidateValidationCode::NameSourceRequired);
    refresh_validation_status(candidate, candidate_has_hard_invalid(candidate));
}

fn refresh_validation_status(candidate: &mut PreparedCandidate, hard_invalid: bool) {
    candidate.validation_status = if hard_invalid {
        CandidateValidationStatus::Invalid
    } else if candidate.validation_codes.is_empty() {
        CandidateValidationStatus::Ready
    } else {
        CandidateValidationStatus::NeedsReview
    };
}

fn candidate_has_hard_invalid(candidate: &PreparedCandidate) -> bool {
    candidate.validation_codes.iter().any(|code| {
        matches!(
            code,
            CandidateValidationCode::InvalidRecipientType
                | CandidateValidationCode::MissingStudentId
                | CandidateValidationCode::MissingStaffUsername
                | CandidateValidationCode::UnexpectedInternalLookup
                | CandidateValidationCode::MissingFirstName
                | CandidateValidationCode::MissingLastName
                | CandidateValidationCode::NameTooLong
                | CandidateValidationCode::ValueTooLong
                | CandidateValidationCode::ForbiddenSensitiveValue
        )
    })
}

async fn load_import_accounts(
    tx: &mut Transaction<'_, Postgres>,
    rows: &[CertificateImportRowInput],
) -> Result<(BTreeMap<String, AccountRow>, BTreeMap<String, AccountRow>), AppError> {
    let student_ids = rows
        .iter()
        .filter_map(|row| {
            (super::import_validation::parse_recipient_type(&row.recipient_type)
                == Some(RecipientType::Student))
            .then(|| normalize_optional(row.student_id.as_deref()))
            .flatten()
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let staff_usernames = rows
        .iter()
        .filter_map(|row| {
            (super::import_validation::parse_recipient_type(&row.recipient_type)
                == Some(RecipientType::Staff))
            .then(|| normalize_optional(row.staff_username.as_deref()))
            .flatten()
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let student_rows = if student_ids.is_empty() {
        Vec::new()
    } else {
        sqlx::query_as::<_, AccountRow>(
            "SELECT u.id, u.username, student.student_id, u.title, u.first_name,
                    u.last_name, u.user_type, u.status
             FROM student_info student
             JOIN users u ON u.id = student.user_id
             WHERE student.student_id = ANY($1::text[]) AND u.user_type = 'student'
             ORDER BY u.id
             FOR SHARE OF student, u",
        )
        .bind(&student_ids)
        .fetch_all(&mut **tx)
        .await
        .map_err(candidate_db_error)?
    };
    let staff_rows = if staff_usernames.is_empty() {
        Vec::new()
    } else {
        sqlx::query_as::<_, AccountRow>(
            "SELECT u.id, u.username, NULL::text AS student_id, u.title, u.first_name,
                    u.last_name, u.user_type, u.status
             FROM users u
             WHERE u.username = ANY($1::text[]) AND u.user_type = 'staff'
             ORDER BY u.id
             FOR SHARE OF u",
        )
        .bind(&staff_usernames)
        .fetch_all(&mut **tx)
        .await
        .map_err(candidate_db_error)?
    };
    Ok((
        student_rows
            .into_iter()
            .filter_map(|row| row.student_id.clone().map(|key| (key, row)))
            .collect(),
        staff_rows
            .into_iter()
            .filter_map(|row| row.username.clone().map(|key| (key, row)))
            .collect(),
    ))
}

async fn load_templates(
    tx: &mut Transaction<'_, Postgres>,
    campaign_id: Uuid,
) -> Result<Vec<TemplateRow>, AppError> {
    sqlx::query_as::<_, TemplateRow>(
        "SELECT template.id, template.name, template.normalized_name,
                template.allowed_recipient_types, template.is_active,
                (template.background_file_id IS NOT NULL
                 AND background.lifecycle_status = 'ready'
                 AND template.crop_box_width IS NOT NULL
                 AND template.crop_box_height IS NOT NULL) AS is_ready
         FROM certificate_templates template
         LEFT JOIN files background ON background.id = template.background_file_id
         WHERE template.campaign_id = $1
         ORDER BY template.created_at, template.id",
    )
    .bind(campaign_id)
    .fetch_all(&mut **tx)
    .await
    .map_err(candidate_db_error)
}

async fn apply_duplicate_warnings(
    tx: &mut Transaction<'_, Postgres>,
    campaign_id: Uuid,
    prepared: &mut [PreparedCandidate],
) -> Result<(), AppError> {
    let existing = fetch_duplicate_fingerprints(tx, campaign_id, None).await?;
    let mut counts = BTreeMap::<DuplicateFingerprint, usize>::new();
    for candidate in prepared.iter() {
        *counts.entry(duplicate_fingerprint(candidate)).or_default() += 1;
    }
    for candidate in prepared.iter_mut() {
        let fingerprint = duplicate_fingerprint(candidate);
        if existing.contains(&fingerprint) || counts.get(&fingerprint).copied().unwrap_or(0) > 1 {
            candidate
                .validation_codes
                .insert(CandidateValidationCode::DuplicateCandidate);
            refresh_validation_status(candidate, candidate_has_hard_invalid(candidate));
        }
    }
    Ok(())
}

async fn apply_single_duplicate_warning(
    tx: &mut Transaction<'_, Postgres>,
    candidate: &mut PreparedCandidate,
) -> Result<(), AppError> {
    candidate
        .validation_codes
        .remove(&CandidateValidationCode::DuplicateCandidate);
    let existing =
        fetch_duplicate_fingerprints(tx, candidate.campaign_id, Some(candidate.id)).await?;
    if existing.contains(&duplicate_fingerprint(candidate)) && !candidate.duplicate_confirmed {
        candidate
            .validation_codes
            .insert(CandidateValidationCode::DuplicateCandidate);
    }
    refresh_validation_status(candidate, candidate_has_hard_invalid(candidate));
    Ok(())
}

async fn fetch_duplicate_fingerprints(
    tx: &mut Transaction<'_, Postgres>,
    campaign_id: Uuid,
    exclude_id: Option<Uuid>,
) -> Result<BTreeSet<DuplicateFingerprint>, AppError> {
    let excluded = exclude_id.into_iter().collect::<Vec<_>>();
    fetch_duplicate_fingerprints_excluding(tx, campaign_id, &excluded).await
}

async fn fetch_duplicate_fingerprints_excluding(
    tx: &mut Transaction<'_, Postgres>,
    campaign_id: Uuid,
    excluded_ids: &[Uuid],
) -> Result<BTreeSet<DuplicateFingerprint>, AppError> {
    let rows = sqlx::query_as::<_, CandidateRow>(&format!(
        "{CANDIDATE_SELECT}
         WHERE candidate.campaign_id = $1 AND candidate.deleted_at IS NULL
           AND NOT (candidate.id = ANY($2::uuid[]))"
    ))
    .bind(campaign_id)
    .bind(excluded_ids)
    .fetch_all(&mut **tx)
    .await
    .map_err(candidate_db_error)?;
    rows.into_iter()
        .map(prepared_from_row)
        .map(|candidate| candidate.map(|candidate| duplicate_fingerprint(&candidate)))
        .collect()
}

async fn apply_bulk_duplicate_warnings(
    tx: &mut Transaction<'_, Postgres>,
    campaign_id: Uuid,
    candidate_ids: &[Uuid],
    candidates: &mut [PreparedCandidate],
) -> Result<(), AppError> {
    let existing = fetch_duplicate_fingerprints_excluding(tx, campaign_id, candidate_ids).await?;
    let mut counts = BTreeMap::<DuplicateFingerprint, usize>::new();
    for candidate in candidates.iter() {
        *counts.entry(duplicate_fingerprint(candidate)).or_default() += 1;
    }
    for candidate in candidates.iter_mut() {
        candidate
            .validation_codes
            .remove(&CandidateValidationCode::DuplicateCandidate);
        let fingerprint = duplicate_fingerprint(candidate);
        let duplicate = existing.contains(&fingerprint)
            || counts.get(&fingerprint).copied().unwrap_or_default() > 1;
        if duplicate && !candidate.duplicate_confirmed {
            candidate
                .validation_codes
                .insert(CandidateValidationCode::DuplicateCandidate);
        }
        refresh_validation_status(candidate, candidate_has_hard_invalid(candidate));
    }
    Ok(())
}

async fn recompute_campaign_duplicate_warnings(
    tx: &mut Transaction<'_, Postgres>,
    actor_user_id: Uuid,
    campaign_id: Uuid,
) -> Result<(), AppError> {
    let rows = sqlx::query_as::<_, CandidateRow>(&format!(
        "{CANDIDATE_SELECT}
         WHERE candidate.campaign_id = $1 AND candidate.deleted_at IS NULL
         ORDER BY candidate.id
         FOR UPDATE OF candidate"
    ))
    .bind(campaign_id)
    .fetch_all(&mut **tx)
    .await
    .map_err(candidate_db_error)?;
    let mut candidates = rows
        .into_iter()
        .map(prepared_from_row)
        .collect::<Result<Vec<_>, _>>()?;
    let mut counts = BTreeMap::<DuplicateFingerprint, usize>::new();
    for candidate in &candidates {
        *counts.entry(duplicate_fingerprint(candidate)).or_default() += 1;
    }
    for candidate in &mut candidates {
        let previous_codes = candidate.validation_codes.clone();
        let previous_status = candidate.validation_status;
        candidate
            .validation_codes
            .remove(&CandidateValidationCode::DuplicateCandidate);
        if counts
            .get(&duplicate_fingerprint(candidate))
            .copied()
            .unwrap_or_default()
            > 1
            && !candidate.duplicate_confirmed
        {
            candidate
                .validation_codes
                .insert(CandidateValidationCode::DuplicateCandidate);
        }
        refresh_validation_status(candidate, candidate_has_hard_invalid(candidate));
        if candidate.validation_codes == previous_codes
            && candidate.validation_status == previous_status
        {
            continue;
        }
        sqlx::query(
            "UPDATE certificate_candidates
             SET validation_codes = $2, validation_status = $3,
                 updated_by = $4, updated_at = clock_timestamp()
             WHERE id = $1",
        )
        .bind(candidate.id)
        .bind(
            candidate
                .validation_codes
                .iter()
                .map(|code| code.as_str())
                .collect::<Vec<_>>(),
        )
        .bind(candidate.validation_status.as_str())
        .bind(actor_user_id)
        .execute(&mut **tx)
        .await
        .map_err(candidate_db_error)?;
    }
    Ok(())
}

fn duplicate_fingerprint(candidate: &PreparedCandidate) -> DuplicateFingerprint {
    let identity = if let Some(user_id) = candidate.matched_user_id {
        format!("user:{user_id}")
    } else if candidate.recipient_type == RecipientType::Student {
        format!(
            "student:{}",
            candidate.lookup_student_id.clone().unwrap_or_default()
        )
    } else if candidate.recipient_type == RecipientType::Staff {
        format!(
            "staff:{}",
            candidate.lookup_staff_username.clone().unwrap_or_default()
        )
    } else {
        format!(
            "external:{}:{}",
            normalize_name_for_match(&candidate.imported_first_name),
            normalize_name_for_match(&candidate.imported_last_name)
        )
    };
    DuplicateFingerprint {
        identity,
        template: candidate
            .template_id
            .map(|id| id.to_string())
            .unwrap_or_default(),
        activity: normalize_name_for_match(candidate.activity_item.as_deref().unwrap_or_default()),
        award: normalize_name_for_match(candidate.award_or_role.as_deref().unwrap_or_default()),
    }
}

async fn insert_candidates(
    tx: &mut Transaction<'_, Postgres>,
    actor_user_id: Uuid,
    candidates: &[PreparedCandidate],
) -> Result<(), AppError> {
    for chunk in candidates.chunks(INSERT_CHUNK_SIZE) {
        let mut builder = QueryBuilder::<Postgres>::new(
            "INSERT INTO certificate_candidates
                (id, campaign_id, batch_id, template_id, recipient_type, matched_user_id,
                 lookup_student_id, lookup_staff_username, imported_title, imported_first_name,
                 imported_last_name, account_title, account_first_name, account_last_name,
                 selected_name_source, activity_item, award_or_role, custom_values, match_status,
                 validation_status, validation_codes, duplicate_confirmed, created_by, updated_by) ",
        );
        builder.push_values(chunk, |mut values, candidate| {
            values
                .push_bind(candidate.id)
                .push_bind(candidate.campaign_id)
                .push_bind(candidate.batch_id)
                .push_bind(candidate.template_id)
                .push_bind(candidate.recipient_type.as_str())
                .push_bind(candidate.matched_user_id)
                .push_bind(candidate.lookup_student_id.clone())
                .push_bind(candidate.lookup_staff_username.clone())
                .push_bind(candidate.imported_title.clone())
                .push_bind(candidate.imported_first_name.clone())
                .push_bind(candidate.imported_last_name.clone())
                .push_bind(candidate.account_title.clone())
                .push_bind(candidate.account_first_name.clone())
                .push_bind(candidate.account_last_name.clone())
                .push_bind(
                    candidate
                        .selected_name_source
                        .map(CandidateNameSource::as_str),
                )
                .push_bind(candidate.activity_item.clone())
                .push_bind(candidate.award_or_role.clone())
                .push_bind(sqlx::types::Json(candidate.custom_values.clone()))
                .push_bind(candidate.match_status.as_str())
                .push_bind(candidate.validation_status.as_str())
                .push_bind(
                    candidate
                        .validation_codes
                        .iter()
                        .map(|code| code.as_str())
                        .collect::<Vec<_>>(),
                )
                .push_bind(candidate.duplicate_confirmed)
                .push_bind(actor_user_id)
                .push_bind(actor_user_id);
        });
        builder
            .build()
            .execute(&mut **tx)
            .await
            .map_err(candidate_db_error)?;
    }
    Ok(())
}

async fn update_prepared_candidate(
    tx: &mut Transaction<'_, Postgres>,
    actor_user_id: Uuid,
    candidate: &PreparedCandidate,
) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE certificate_candidates
         SET template_id = $2, recipient_type = $3, matched_user_id = $4,
             lookup_student_id = $5, lookup_staff_username = $6, imported_title = $7,
             imported_first_name = $8, imported_last_name = $9, account_title = $10,
             account_first_name = $11, account_last_name = $12, selected_name_source = $13,
             activity_item = $14, award_or_role = $15, custom_values = $16,
             match_status = $17, validation_status = $18, validation_codes = $19,
             duplicate_confirmed = $20, updated_by = $21, updated_at = clock_timestamp()
         WHERE id = $1",
    )
    .bind(candidate.id)
    .bind(candidate.template_id)
    .bind(candidate.recipient_type.as_str())
    .bind(candidate.matched_user_id)
    .bind(&candidate.lookup_student_id)
    .bind(&candidate.lookup_staff_username)
    .bind(&candidate.imported_title)
    .bind(&candidate.imported_first_name)
    .bind(&candidate.imported_last_name)
    .bind(&candidate.account_title)
    .bind(&candidate.account_first_name)
    .bind(&candidate.account_last_name)
    .bind(
        candidate
            .selected_name_source
            .map(CandidateNameSource::as_str),
    )
    .bind(&candidate.activity_item)
    .bind(&candidate.award_or_role)
    .bind(sqlx::types::Json(candidate.custom_values.clone()))
    .bind(candidate.match_status.as_str())
    .bind(candidate.validation_status.as_str())
    .bind(
        candidate
            .validation_codes
            .iter()
            .map(|code| code.as_str())
            .collect::<Vec<_>>(),
    )
    .bind(candidate.duplicate_confirmed)
    .bind(actor_user_id)
    .execute(&mut **tx)
    .await
    .map_err(candidate_db_error)?;
    Ok(())
}

async fn authoritative_account_exists(
    tx: &mut Transaction<'_, Postgres>,
    candidate: &PreparedCandidate,
) -> Result<bool, AppError> {
    match candidate.recipient_type {
        RecipientType::Student => {
            let Some(student_id) = candidate.lookup_student_id.as_deref() else {
                return Ok(false);
            };
            sqlx::query_scalar(
                "SELECT EXISTS (
                    SELECT 1 FROM student_info student
                    JOIN users u ON u.id = student.user_id
                    WHERE student.student_id = $1 AND u.user_type = 'student'
                 )",
            )
            .bind(student_id)
            .fetch_one(&mut **tx)
            .await
            .map_err(candidate_db_error)
        }
        RecipientType::Staff => {
            let Some(username) = candidate.lookup_staff_username.as_deref() else {
                return Ok(false);
            };
            sqlx::query_scalar(
                "SELECT EXISTS (
                    SELECT 1 FROM users WHERE username = $1 AND user_type = 'staff'
                 )",
            )
            .bind(username)
            .fetch_one(&mut **tx)
            .await
            .map_err(candidate_db_error)
        }
        RecipientType::External => Ok(false),
    }
}

async fn lock_account_identity_writes(tx: &mut Transaction<'_, Postgres>) -> Result<(), AppError> {
    // A missing identity has no row to lock. SHARE table locks serialize this short,
    // explicit confirmation path with every INSERT/UPDATE that could create the account.
    sqlx::query("LOCK TABLE users, student_info IN SHARE MODE")
        .execute(&mut **tx)
        .await
        .map_err(candidate_db_error)?;
    Ok(())
}

async fn campaign_access(pool: &PgPool, campaign_id: Uuid) -> Result<CampaignAccessRow, AppError> {
    sqlx::query_as(
        "SELECT owner_organization_unit_id, status
         FROM certificate_campaigns
         WHERE id = $1 AND status <> 'purging'",
    )
    .bind(campaign_id)
    .fetch_optional(pool)
    .await
    .map_err(candidate_db_error)?
    .ok_or_else(|| AppError::NotFound("ไม่พบกิจกรรมเกียรติบัตร".to_string()))
}

async fn lock_campaign(
    tx: &mut Transaction<'_, Postgres>,
    campaign_id: Uuid,
) -> Result<CampaignAccessRow, AppError> {
    sqlx::query_as(
        "SELECT owner_organization_unit_id, status
         FROM certificate_campaigns WHERE id = $1 FOR UPDATE",
    )
    .bind(campaign_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(candidate_db_error)?
    .ok_or_else(|| AppError::NotFound("ไม่พบกิจกรรมเกียรติบัตร".to_string()))
}

fn require_same_owner(
    before: &CampaignAccessRow,
    locked: &CampaignAccessRow,
) -> Result<(), AppError> {
    if before.owner_organization_unit_id == locked.owner_organization_unit_id {
        Ok(())
    } else {
        Err(AppError::Conflict(
            "หน่วยงานเจ้าของกิจกรรมเปลี่ยนระหว่างดำเนินการ กรุณาลองใหม่".to_string(),
        ))
    }
}

fn campaign_status_is_mutable(status: &str) -> bool {
    matches!(status, "draft" | "active")
}

fn require_mutable_campaign(status: &str) -> Result<(), AppError> {
    if status == "purging" {
        Err(AppError::Conflict(
            "certificate_campaign_purging".to_string(),
        ))
    } else if campaign_status_is_mutable(status) {
        Ok(())
    } else {
        Err(AppError::Conflict(
            "กิจกรรมสถานะนี้ไม่อนุญาตให้แก้ไขรายชื่อผู้รับ".to_string(),
        ))
    }
}

async fn lock_candidate(
    tx: &mut Transaction<'_, Postgres>,
    candidate_id: Uuid,
) -> Result<CandidateRow, AppError> {
    sqlx::query_as::<_, CandidateRow>(&format!(
        "{CANDIDATE_SELECT} WHERE candidate.id = $1 FOR UPDATE OF candidate"
    ))
    .bind(candidate_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(candidate_db_error)?
    .ok_or_else(candidate_not_found)
}

async fn lock_candidates(
    tx: &mut Transaction<'_, Postgres>,
    candidate_ids: &[Uuid],
) -> Result<Vec<CandidateRow>, AppError> {
    sqlx::query_as::<_, CandidateRow>(&format!(
        "{CANDIDATE_SELECT}
         WHERE candidate.id = ANY($1::uuid[])
         ORDER BY candidate.id
         FOR UPDATE OF candidate"
    ))
    .bind(candidate_ids)
    .fetch_all(&mut **tx)
    .await
    .map_err(candidate_db_error)
}

fn require_candidate_mutable(
    candidate: &CandidateRow,
    can_read_locked_request: bool,
) -> Result<(), AppError> {
    if candidate.deleted_at.is_some() {
        return Err(candidate_not_found());
    }
    if candidate.issued_certificate_id.is_some() {
        return Err(AppError::Conflict(
            "รายชื่อที่ออกเกียรติบัตรแล้วแก้ไขไม่ได้".to_string(),
        ));
    }
    if let Some(request_id) = candidate.locked_request_id {
        Err(super::request_service::resource_locked_error(
            request_id,
            can_read_locked_request,
        ))
    } else {
        Ok(())
    }
}

async fn fetch_candidate_row(
    pool: &PgPool,
    candidate_id: Uuid,
    include_deleted: bool,
) -> Result<CandidateRow, AppError> {
    let deleted_filter = if include_deleted {
        ""
    } else {
        " AND candidate.deleted_at IS NULL"
    };
    sqlx::query_as::<_, CandidateRow>(&format!(
        "{CANDIDATE_SELECT} WHERE candidate.id = $1{deleted_filter}"
    ))
    .bind(candidate_id)
    .fetch_optional(pool)
    .await
    .map_err(candidate_db_error)?
    .ok_or_else(candidate_not_found)
}

async fn fetch_candidate_details(
    pool: &PgPool,
    actor: &ActorContext,
    candidate_ids: &[Uuid],
    include_deleted: bool,
) -> Result<Vec<CertificateCandidateDetail>, AppError> {
    if candidate_ids.is_empty() {
        return Ok(Vec::new());
    }
    let deleted_filter = if include_deleted {
        ""
    } else {
        " AND candidate.deleted_at IS NULL"
    };
    let rows = sqlx::query_as::<_, CandidateRow>(&format!(
        "{CANDIDATE_SELECT}
         WHERE candidate.id = ANY($1::uuid[]){deleted_filter}
         ORDER BY candidate.created_at, candidate.id"
    ))
    .bind(candidate_ids)
    .fetch_all(pool)
    .await
    .map_err(candidate_db_error)?;
    if rows.len() != candidate_ids.len() {
        return Err(candidate_not_found());
    }
    let access = campaign_access(pool, rows[0].campaign_id).await?;
    if rows
        .iter()
        .any(|candidate| candidate.campaign_id != rows[0].campaign_id)
    {
        return Err(candidate_not_found());
    }
    require_owner_action(
        pool,
        actor,
        access.owner_organization_unit_id,
        CertificateAction::Read,
    )
    .await?;
    let can_update = require_owner_action(
        pool,
        actor,
        access.owner_organization_unit_id,
        CertificateAction::Update,
    )
    .await
    .is_ok()
        && campaign_status_is_mutable(&access.status);
    let mut details = rows
        .into_iter()
        .map(|row| candidate_detail(row, can_update))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(|candidate| (candidate.id, candidate))
        .collect::<BTreeMap<_, _>>();
    candidate_ids
        .iter()
        .map(|candidate_id| details.remove(candidate_id).ok_or_else(candidate_not_found))
        .collect()
}

fn candidate_detail(
    row: CandidateRow,
    can_update_campaign: bool,
) -> Result<CertificateCandidateDetail, AppError> {
    let recipient_type = parse_recipient_type_db(&row.recipient_type)?;
    let match_status = CandidateMatchStatus::parse(&row.match_status)
        .ok_or_else(|| invalid_db_value("candidate_match_status"))?;
    let validation_status = CandidateValidationStatus::parse(&row.validation_status)
        .ok_or_else(|| invalid_db_value("candidate_validation_status"))?;
    let validation_codes = row
        .validation_codes
        .iter()
        .map(|value| {
            CandidateValidationCode::parse(value)
                .ok_or_else(|| invalid_db_value("candidate_validation_code"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let selected_name_source = parse_optional_name_source(row.selected_name_source.as_deref())?;
    let mutable = can_update_campaign
        && row.deleted_at.is_none()
        && row.issued_certificate_id.is_none()
        && row.locked_request_id.is_none();
    let has_required_lookup = (recipient_type == RecipientType::Student
        && row.lookup_student_id.is_some())
        || (recipient_type == RecipientType::Staff && row.lookup_staff_username.is_some());
    Ok(CertificateCandidateDetail {
        id: row.id,
        campaign_id: row.campaign_id,
        batch_id: row.batch_id,
        template_id: row.template_id,
        template_name: row.template_name,
        recipient_type,
        matched_user_id: row.matched_user_id,
        student_id: row.lookup_student_id,
        staff_username: row.lookup_staff_username,
        imported_title: row.imported_title,
        imported_first_name: row.imported_first_name,
        imported_last_name: row.imported_last_name,
        account_title: row.account_title,
        account_first_name: row.account_first_name,
        account_last_name: row.account_last_name,
        selected_name_source,
        activity_item: row.activity_item,
        award_or_role: row.award_or_role,
        custom_values: row.custom_values.0,
        match_status,
        validation_status,
        validation_codes: validation_codes.clone(),
        duplicate_confirmed: row.duplicate_confirmed,
        deleted_at: row.deleted_at,
        created_at: row.created_at,
        updated_at: row.updated_at,
        capabilities: CertificateCandidateCapabilities {
            can_update: mutable,
            can_delete: mutable,
            can_choose_name: mutable
                && match_status == CandidateMatchStatus::NameMismatch
                && row.matched_user_id.is_some(),
            can_confirm_external: mutable
                && matches!(
                    recipient_type,
                    RecipientType::Student | RecipientType::Staff
                )
                && match_status == CandidateMatchStatus::NotFound
                && row.matched_user_id.is_none()
                && has_required_lookup,
            can_confirm_duplicate: mutable
                && validation_codes.contains(&CandidateValidationCode::DuplicateCandidate),
        },
    })
}

fn prepared_from_row(row: CandidateRow) -> Result<PreparedCandidate, AppError> {
    Ok(PreparedCandidate {
        id: row.id,
        campaign_id: row.campaign_id,
        batch_id: row.batch_id,
        template_id: row.template_id,
        recipient_type: parse_recipient_type_db(&row.recipient_type)?,
        matched_user_id: row.matched_user_id,
        lookup_student_id: row.lookup_student_id,
        lookup_staff_username: row.lookup_staff_username,
        imported_title: row.imported_title,
        imported_first_name: row.imported_first_name,
        imported_last_name: row.imported_last_name,
        account_title: row.account_title,
        account_first_name: row.account_first_name,
        account_last_name: row.account_last_name,
        selected_name_source: parse_optional_name_source(row.selected_name_source.as_deref())?,
        activity_item: row.activity_item,
        award_or_role: row.award_or_role,
        custom_values: row.custom_values.0,
        match_status: CandidateMatchStatus::parse(&row.match_status)
            .ok_or_else(|| invalid_db_value("candidate_match_status"))?,
        validation_status: CandidateValidationStatus::parse(&row.validation_status)
            .ok_or_else(|| invalid_db_value("candidate_validation_status"))?,
        validation_codes: row
            .validation_codes
            .iter()
            .map(|value| {
                CandidateValidationCode::parse(value)
                    .ok_or_else(|| invalid_db_value("candidate_validation_code"))
            })
            .collect::<Result<_, _>>()?,
        duplicate_confirmed: row.duplicate_confirmed,
    })
}

fn candidate_as_import_row(row: &CandidateRow) -> CertificateImportRowInput {
    let mut input = CertificateImportRowInput {
        recipient_type: row.recipient_type.clone(),
        student_id: row.lookup_student_id.clone(),
        staff_username: row.lookup_staff_username.clone(),
        title: row.imported_title.clone(),
        first_name: row.imported_first_name.clone(),
        last_name: row.imported_last_name.clone(),
        activity_item: row.activity_item.clone(),
        award_or_role: row.award_or_role.clone(),
        template_name: row.template_name.clone(),
        custom_values: row.custom_values.0.clone(),
    };
    prepare_converted_external_account_recheck(row, &mut input);
    input
}

fn is_converted_external_candidate(row: &CandidateRow) -> bool {
    row.recipient_type == RecipientType::External.as_str()
        && row.match_status == CandidateMatchStatus::ExternalConfirmed.as_str()
}

fn prepare_converted_external_account_recheck(
    row: &CandidateRow,
    input: &mut CertificateImportRowInput,
) -> bool {
    if !is_converted_external_candidate(row) {
        return false;
    }
    if let Some(student_id) = row.lookup_student_id.clone() {
        input.recipient_type = RecipientType::Student.as_str().to_string();
        input.student_id = Some(student_id);
        input.staff_username = None;
    } else if let Some(staff_username) = row.lookup_staff_username.clone() {
        input.recipient_type = RecipientType::Staff.as_str().to_string();
        input.student_id = None;
        input.staff_username = Some(staff_username);
    } else {
        input.student_id = None;
        input.staff_username = None;
    }
    true
}

fn validate_manual_row(row: &CertificateImportRowInput) -> Result<(), AppError> {
    let headers = StandardColumn::REQUIRED
        .into_iter()
        .map(|column| column.header().to_string())
        .chain(row.custom_values.keys().cloned())
        .collect::<Vec<_>>();
    validate_import_headers(&headers, 1)
        .map_err(|error| import_request_error(ImportRequestError::Headers(error)))?;
    let outcome = validate_import_row(row);
    if outcome.is_valid() {
        Ok(())
    } else {
        Err(AppError::ValidationError(
            "ข้อมูลผู้รับไม่ครบหรือไม่เป็นไปตามข้อกำหนด".to_string(),
        ))
    }
}

fn canonical_custom_headers(
    custom_values: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>, AppError> {
    let headers = StandardColumn::REQUIRED
        .into_iter()
        .map(|column| column.header().to_string())
        .chain(custom_values.keys().cloned())
        .collect::<Vec<_>>();
    let validated = validate_import_headers(&headers, 1)
        .map_err(|error| import_request_error(ImportRequestError::Headers(error)))?;
    Ok(validated
        .custom_headers
        .into_iter()
        .map(|header| (normalize_name_for_match(&header), header))
        .collect())
}

async fn require_known_campaign_custom_headers(
    tx: &mut Transaction<'_, Postgres>,
    campaign_id: Uuid,
    requested: &BTreeMap<String, String>,
) -> Result<(), AppError> {
    if requested.is_empty() {
        return Ok(());
    }
    let known = sqlx::query_scalar::<_, String>(
        "SELECT DISTINCT custom.header
         FROM certificate_import_batches batch
         CROSS JOIN LATERAL unnest(batch.custom_headers) AS custom(header)
         WHERE batch.campaign_id = $1",
    )
    .bind(campaign_id)
    .fetch_all(&mut **tx)
    .await
    .map_err(candidate_db_error)?
    .into_iter()
    .map(|header| normalize_name_for_match(&header))
    .collect::<BTreeSet<_>>();
    if requested.keys().all(|header| known.contains(header)) {
        Ok(())
    } else {
        Err(AppError::ValidationError(
            "มีตัวแปรเพิ่มเติมที่ไม่ได้ประกาศในกิจกรรม".to_string(),
        ))
    }
}

fn canonicalize_custom_values(
    values: &BTreeMap<String, String>,
    headers: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    values
        .iter()
        .filter_map(|(key, value)| {
            headers
                .get(&normalize_name_for_match(key))
                .map(|header| (header.clone(), normalize_display_text(value)))
        })
        .collect()
}

fn normalize_optional(value: Option<&str>) -> Option<String> {
    value
        .map(normalize_display_text)
        .filter(|value| !value.is_empty())
}

fn normalized_search(search: Option<&str>) -> Result<Option<String>, AppError> {
    let search = normalize_optional(search);
    if search
        .as_ref()
        .is_some_and(|value| contains_thirteen_digit_run(value))
    {
        return Err(AppError::ValidationError(
            "certificate_search_forbidden_sensitive_value".to_string(),
        ));
    }
    if search
        .as_ref()
        .is_some_and(|value| value.chars().count() > 100)
    {
        Err(AppError::ValidationError("คำค้นยาวเกินกำหนด".to_string()))
    } else {
        Ok(search)
    }
}

fn import_issue_code(issue: &ImportRowIssue) -> CandidateValidationCode {
    match issue {
        ImportRowIssue::InvalidRecipientType => CandidateValidationCode::InvalidRecipientType,
        ImportRowIssue::MissingStudentId => CandidateValidationCode::MissingStudentId,
        ImportRowIssue::MissingStaffUsername => CandidateValidationCode::MissingStaffUsername,
        ImportRowIssue::UnexpectedInternalLookup => {
            CandidateValidationCode::UnexpectedInternalLookup
        }
        ImportRowIssue::MissingFirstName => CandidateValidationCode::MissingFirstName,
        ImportRowIssue::MissingLastName => CandidateValidationCode::MissingLastName,
        ImportRowIssue::NameTooLong => CandidateValidationCode::NameTooLong,
        ImportRowIssue::ValueTooLong => CandidateValidationCode::ValueTooLong,
        ImportRowIssue::ForbiddenSensitiveValue => CandidateValidationCode::ForbiddenSensitiveValue,
    }
}

fn count_status(candidates: &[PreparedCandidate], status: CandidateValidationStatus) -> i32 {
    candidates
        .iter()
        .filter(|candidate| candidate.validation_status == status)
        .count() as i32
}

fn selected_name(candidate: &CertificateCandidateDetail) -> (Option<String>, String, String) {
    if candidate.selected_name_source == Some(CandidateNameSource::Account) {
        if let (Some(first_name), Some(last_name)) = (
            candidate.account_first_name.clone(),
            candidate.account_last_name.clone(),
        ) {
            return (candidate.account_title.clone(), first_name, last_name);
        }
    }
    (
        candidate.imported_title.clone(),
        candidate.imported_first_name.clone(),
        candidate.imported_last_name.clone(),
    )
}

async fn load_account_by_id(pool: &PgPool, user_id: Uuid) -> Result<AccountRow, AppError> {
    sqlx::query_as(
        "SELECT u.id, u.username, student.student_id, u.title, u.first_name,
                u.last_name, u.user_type, u.status
         FROM users u
         LEFT JOIN student_info student ON student.user_id = u.id
         WHERE u.id = $1 AND u.user_type IN ('student', 'staff')",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(candidate_db_error)?
    .ok_or_else(|| AppError::NotFound("ไม่พบบัญชีนักเรียนหรือบุคลากร".to_string()))
}

async fn load_account_by_id_tx(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
) -> Result<AccountRow, AppError> {
    sqlx::query_as(
        "SELECT u.id, u.username, student.student_id, u.title, u.first_name,
                u.last_name, u.user_type, u.status
         FROM users u
         LEFT JOIN student_info student ON student.user_id = u.id
         WHERE u.id = $1 AND u.user_type IN ('student', 'staff')
         FOR SHARE OF u",
    )
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(candidate_db_error)?
    .ok_or_else(|| AppError::NotFound("ไม่พบบัญชีนักเรียนหรือบุคลากร".to_string()))
}

fn account_response(account: AccountRow) -> Result<CertificateCandidateAccount, AppError> {
    let recipient_type = account_recipient_type(&account)?;
    Ok(CertificateCandidateAccount {
        user_id: account.id,
        recipient_type,
        student_id: account.student_id,
        staff_username: if recipient_type == RecipientType::Staff {
            account.username
        } else {
            None
        },
        title: account.title,
        first_name: account.first_name,
        last_name: account.last_name,
    })
}

fn account_recipient_type(account: &AccountRow) -> Result<RecipientType, AppError> {
    match account.user_type.as_str() {
        "student" if account.student_id.is_some() => Ok(RecipientType::Student),
        "staff"
            if account
                .username
                .as_ref()
                .is_some_and(|value| !value.trim().is_empty()) =>
        {
            Ok(RecipientType::Staff)
        }
        _ => Err(invalid_db_value("certificate_account_recipient_type")),
    }
}

fn parse_recipient_type_db(value: &str) -> Result<RecipientType, AppError> {
    RecipientType::parse(value).ok_or_else(|| invalid_db_value("candidate_recipient_type"))
}

fn parse_optional_name_source(
    value: Option<&str>,
) -> Result<Option<CandidateNameSource>, AppError> {
    value
        .map(|value| {
            CandidateNameSource::parse(value)
                .ok_or_else(|| invalid_db_value("candidate_name_source"))
        })
        .transpose()
}

fn bulk_operation_name(request: &CertificateCandidateBulkRequest) -> &'static str {
    match request {
        CertificateCandidateBulkRequest::AssignTemplate { .. } => "assign_template",
        CertificateCandidateBulkRequest::ChooseName { .. } => "choose_name",
        CertificateCandidateBulkRequest::ConfirmExternal { .. } => "confirm_external",
        CertificateCandidateBulkRequest::ConfirmDuplicate { .. } => "confirm_duplicate",
        CertificateCandidateBulkRequest::SoftDelete { .. } => "soft_delete",
    }
}

async fn record_candidate_audit(
    tx: &mut Transaction<'_, Postgres>,
    actor_user_id: Uuid,
    action: &'static str,
    metadata: CandidateAuditMetadata,
) -> Result<(), AppError> {
    let entity_id = metadata.candidate_id.unwrap_or(metadata.campaign_id);
    let metadata = serde_json::to_value(metadata)
        .map_err(|_| AppError::InternalServerError("ไม่สามารถบันทึกประวัติรายการได้".to_string()))?;
    sqlx::query(
        "INSERT INTO audit_logs (user_id, action, entity_type, entity_id, metadata)
         VALUES ($1, $2, 'certificate_candidate', $3, $4)",
    )
    .bind(actor_user_id)
    .bind(action)
    .bind(entity_id)
    .bind(metadata)
    .execute(&mut **tx)
    .await
    .map_err(candidate_db_error)?;
    Ok(())
}

fn import_request_error(error: ImportRequestError) -> AppError {
    let code = match error {
        ImportRequestError::InvalidSource => "invalid_source",
        ImportRequestError::UnknownCustomColumn => "unknown_custom_column",
        ImportRequestError::DuplicateCustomColumn => "duplicate_custom_column",
        ImportRequestError::Headers(header) => match header {
            ImportHeaderError::NoRows => "no_rows",
            ImportHeaderError::TooManyRows => "too_many_rows",
            ImportHeaderError::EmptyHeader => "empty_header",
            ImportHeaderError::HeaderTooLong => "header_too_long",
            ImportHeaderError::DuplicateHeader => "duplicate_header",
            ImportHeaderError::ForbiddenHeader => "forbidden_header",
            ImportHeaderError::ReservedHeader => "reserved_header",
            ImportHeaderError::MissingRequired(_) => "missing_required_header",
            ImportHeaderError::TooManyCustomColumns => "too_many_custom_columns",
        },
    };
    AppError::ValidationError(format!("certificate_import_{code}"))
}

fn invalid_import() -> AppError {
    AppError::ValidationError("จำนวนแถวไม่ถูกต้อง".to_string())
}

fn candidate_not_found() -> AppError {
    AppError::NotFound("ไม่พบรายชื่อผู้รับเกียรติบัตร".to_string())
}

fn invalid_db_value(field: &'static str) -> AppError {
    AppError::InternalServerError(format!("certificate_invalid_{field}"))
}

fn candidate_db_error(_error: sqlx::Error) -> AppError {
    // Never log database details here: constraint details may contain recipient values.
    AppError::InternalServerError("ไม่สามารถดำเนินการข้อมูลผู้รับเกียรติบัตรได้".to_string())
}
