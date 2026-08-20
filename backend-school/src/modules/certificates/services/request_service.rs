use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::{
    error::AppError,
    middleware::permission::ActorContext,
    modules::certificates::models::{
        CandidateValidationCode, CandidateValidationStatus, CertificateIssueCode,
        CertificateIssueRequestCapabilities, CertificateIssueRequestDetail,
        CertificateIssueRequestItem, CertificateIssueRequestListQuery,
        CertificateIssueRequestStatus, CertificateIssueRequestSummary, CertificateLayoutV1,
        RecipientType,
    },
    permissions::registry::codes,
    policies::certificate_access_policy::{
        owner_list_scope, require_owner_action, CertificateAction, CertificateOwnerListScope,
    },
};

use super::{
    audit_service::{record_issue_request_audit, CertificateIssueRequestAuditMetadata},
    import_validation::{contains_thirteen_digit_run, is_forbidden_header, normalize_display_text},
    template_service::referenced_asset_ids,
};

const MAX_REQUEST_CANDIDATES: usize = 5_000;
const MAX_RETURN_NOTE_CHARS: usize = 500;

const REQUEST_SELECT: &str = r#"
    SELECT
        request.id,
        request.campaign_id,
        campaign.name AS campaign_name,
        campaign.owner_organization_unit_id,
        owner.name AS owner_organization_unit_name,
        request.status,
        request.submitted_by,
        BTRIM(CONCAT(COALESCE(submitter.title, ''), submitter.first_name, ' ',
                     submitter.last_name)) AS submitted_by_name,
        request.reviewed_by,
        CASE WHEN reviewer.id IS NULL THEN NULL
             ELSE BTRIM(CONCAT(COALESCE(reviewer.title, ''), reviewer.first_name, ' ',
                               reviewer.last_name)) END AS reviewed_by_name,
        request.submitted_at,
        request.reviewed_at,
        request.returned_at,
        request.withdrawn_at,
        request.issued_at,
        request.return_note,
        request.issue_codes,
        (SELECT COUNT(*)::bigint
         FROM certificate_issue_request_items item
         WHERE item.request_id = request.id) AS item_count,
        (SELECT COUNT(DISTINCT candidate.template_id)::bigint
         FROM certificate_issue_request_items item
         JOIN certificate_candidates candidate ON candidate.id = item.candidate_id
         WHERE item.request_id = request.id) AS template_count,
        (SELECT COUNT(*) FILTER (WHERE candidate.validation_status = 'ready')::bigint
         FROM certificate_issue_request_items item
         JOIN certificate_candidates candidate ON candidate.id = item.candidate_id
         WHERE item.request_id = request.id) AS ready_count,
        (SELECT COUNT(*) FILTER (WHERE candidate.validation_status = 'needs_review')::bigint
         FROM certificate_issue_request_items item
         JOIN certificate_candidates candidate ON candidate.id = item.candidate_id
         WHERE item.request_id = request.id) AS review_count,
        (SELECT COUNT(*) FILTER (WHERE candidate.validation_status = 'invalid')::bigint
         FROM certificate_issue_request_items item
         JOIN certificate_candidates candidate ON candidate.id = item.candidate_id
         WHERE item.request_id = request.id) AS invalid_count,
        request.created_at,
        request.updated_at
    FROM certificate_issue_requests request
    JOIN certificate_campaigns campaign ON campaign.id = request.campaign_id
    LEFT JOIN organization_units owner ON owner.id = campaign.owner_organization_unit_id
    JOIN users submitter ON submitter.id = request.submitted_by
    LEFT JOIN users reviewer ON reviewer.id = request.reviewed_by
"#;

#[derive(Debug, FromRow)]
struct CampaignRow {
    id: Uuid,
    owner_organization_unit_id: Option<Uuid>,
    owner_is_active: Option<bool>,
    status: String,
}

#[derive(Debug, FromRow)]
struct SubmissionCandidateRow {
    id: Uuid,
    campaign_id: Uuid,
    template_id: Option<Uuid>,
    recipient_type: String,
    matched_user_id: Option<Uuid>,
    lookup_student_id: Option<String>,
    lookup_staff_username: Option<String>,
    selected_name_source: Option<String>,
    match_status: String,
    validation_status: String,
    validation_codes: Vec<String>,
    issued_certificate_id: Option<Uuid>,
    deleted_at: Option<DateTime<Utc>>,
    template_is_active: Option<bool>,
    template_allowed_recipient_types: Option<Vec<String>>,
    template_layout: Option<sqlx::types::Json<CertificateLayoutV1>>,
    crop_box_width: Option<f64>,
    crop_box_height: Option<f64>,
    background_file_id: Option<Uuid>,
    background_lifecycle_status: Option<String>,
}

#[derive(Debug, FromRow)]
struct AccountStateRow {
    id: Uuid,
    username: Option<String>,
    user_type: String,
    status: String,
}

#[derive(Debug, FromRow)]
struct TemplateAssetStateRow {
    template_id: Uuid,
    asset_id: Uuid,
    lifecycle_status: String,
}

#[derive(Debug, FromRow)]
struct IssueRequestRow {
    id: Uuid,
    campaign_id: Uuid,
    campaign_name: String,
    owner_organization_unit_id: Option<Uuid>,
    owner_organization_unit_name: Option<String>,
    status: String,
    submitted_by: Uuid,
    submitted_by_name: String,
    reviewed_by: Option<Uuid>,
    reviewed_by_name: Option<String>,
    submitted_at: DateTime<Utc>,
    reviewed_at: Option<DateTime<Utc>>,
    returned_at: Option<DateTime<Utc>>,
    withdrawn_at: Option<DateTime<Utc>>,
    issued_at: Option<DateTime<Utc>>,
    return_note: Option<String>,
    issue_codes: Vec<String>,
    item_count: i64,
    template_count: i64,
    ready_count: i64,
    review_count: i64,
    invalid_count: i64,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct IssueRequestItemRow {
    candidate_id: Uuid,
    template_id: Option<Uuid>,
    template_name: Option<String>,
    recipient_type: String,
    title: Option<String>,
    first_name: String,
    last_name: String,
    activity_item: Option<String>,
    award_or_role: Option<String>,
    validation_status: String,
    validation_codes: Vec<String>,
}

#[derive(Debug, FromRow)]
struct RequestAccessRow {
    id: Uuid,
    campaign_id: Uuid,
    owner_organization_unit_id: Option<Uuid>,
    status: String,
    submitted_by: Uuid,
    item_count: i64,
}

#[derive(Debug)]
struct CapabilityContext {
    submit_scope: Option<CertificateOwnerListScope>,
    can_issue: bool,
}

impl CapabilityContext {
    fn can_submit_owner(&self, owner_id: Option<Uuid>) -> bool {
        match &self.submit_scope {
            Some(CertificateOwnerListScope::School) => true,
            Some(CertificateOwnerListScope::ExactUnits(units)) => {
                owner_id.is_some_and(|id| units.contains(&id))
            }
            None => false,
        }
    }
}

pub async fn submit_issue_request(
    pool: &PgPool,
    actor: &ActorContext,
    campaign_id: Uuid,
    candidate_ids: Vec<Uuid>,
) -> Result<CertificateIssueRequestDetail, AppError> {
    let candidate_ids = normalize_candidate_ids(candidate_ids)?;
    let authorized_campaign = fetch_campaign(pool, campaign_id).await?;
    require_owner_action(
        pool,
        actor,
        authorized_campaign.owner_organization_unit_id,
        CertificateAction::Submit,
    )
    .await?;
    require_active_owner(&authorized_campaign)?;

    let can_read_request =
        can_read_request(pool, actor, authorized_campaign.owner_organization_unit_id).await?;
    let mut tx = pool.begin().await.map_err(request_db_error)?;
    let campaign = lock_campaign(&mut tx, campaign_id).await?;
    require_same_owner(&authorized_campaign, &campaign)?;
    lock_and_require_active_owner(&mut tx, campaign.owner_organization_unit_id).await?;
    require_submittable_campaign(&campaign.status)?;

    let candidates = lock_submission_candidates(&mut tx, &candidate_ids).await?;
    if candidates.len() != candidate_ids.len()
        || candidates
            .iter()
            .any(|candidate| candidate.campaign_id != campaign_id)
    {
        return Err(AppError::NotFound("ไม่พบรายชื่อที่เลือกในกิจกรรมนี้".to_string()));
    }

    if let Some(request_id) = active_lock_for_candidates(&mut tx, &candidate_ids).await? {
        return Err(resource_locked_error(request_id, can_read_request));
    }
    require_candidates_ready(&mut tx, &candidates).await?;

    let request_id: Uuid = sqlx::query_scalar(
        "INSERT INTO certificate_issue_requests (campaign_id, submitted_by)
         VALUES ($1, $2) RETURNING id",
    )
    .bind(campaign_id)
    .bind(actor.user_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(request_db_error)?;
    sqlx::query(
        "INSERT INTO certificate_issue_request_items (request_id, candidate_id, campaign_id)
         SELECT $1, selected.candidate_id, $2
         FROM UNNEST($3::uuid[]) AS selected(candidate_id)",
    )
    .bind(request_id)
    .bind(campaign_id)
    .bind(&candidate_ids)
    .execute(&mut *tx)
    .await
    .map_err(request_db_error)?;
    sqlx::query(
        "INSERT INTO certificate_candidate_issue_locks (candidate_id, request_id)
         SELECT selected.candidate_id, $1
         FROM UNNEST($2::uuid[]) AS selected(candidate_id)",
    )
    .bind(request_id)
    .bind(&candidate_ids)
    .execute(&mut *tx)
    .await
    .map_err(|error| {
        if database_code(&error).as_deref() == Some("23505") {
            resource_locked_error(request_id, false)
        } else {
            request_db_error(error)
        }
    })?;
    record_request_audit(
        &mut tx,
        actor.user_id,
        "submit",
        request_id,
        campaign_id,
        None,
        CertificateIssueRequestStatus::Pending,
        candidate_ids.len() as u32,
        Vec::new(),
    )
    .await?;
    tx.commit().await.map_err(request_db_error)?;

    fetch_request_detail_unchecked(pool, actor, request_id).await
}

pub async fn withdraw(
    pool: &PgPool,
    actor: &ActorContext,
    request_id: Uuid,
) -> Result<CertificateIssueRequestDetail, AppError> {
    let authorized = fetch_request_access(pool, request_id).await?;
    require_owner_action(
        pool,
        actor,
        authorized.owner_organization_unit_id,
        CertificateAction::Submit,
    )
    .await?;
    if authorized.submitted_by != actor.user_id {
        return Err(AppError::Forbidden("ถอนคำขอได้เฉพาะผู้ส่งคำขอนี้".to_string()));
    }

    let mut tx = pool.begin().await.map_err(request_db_error)?;
    let locked = lock_request(&mut tx, request_id).await?;
    require_same_request(&authorized, &locked)?;
    let campaign = lock_campaign(&mut tx, authorized.campaign_id).await?;
    require_request_owner_unchanged(&authorized, &campaign)?;
    require_request_status(&locked.status, CertificateIssueRequestStatus::Pending)?;
    transition_to_withdrawn(&mut tx, actor.user_id, &locked).await?;
    tx.commit().await.map_err(request_db_error)?;

    fetch_request_detail_unchecked(pool, actor, request_id).await
}

pub async fn start_review(
    pool: &PgPool,
    actor: &ActorContext,
    request_id: Uuid,
) -> Result<CertificateIssueRequestDetail, AppError> {
    actor.require_permission(codes::CERTIFICATE_ISSUE_SCHOOL)?;
    let authorized = fetch_request_access(pool, request_id).await?;
    let mut tx = pool.begin().await.map_err(request_db_error)?;
    let locked = lock_request(&mut tx, request_id).await?;
    require_same_request(&authorized, &locked)?;
    let campaign = lock_campaign(&mut tx, authorized.campaign_id).await?;
    require_request_owner_unchanged(&authorized, &campaign)?;
    require_request_status(&locked.status, CertificateIssueRequestStatus::Pending)?;

    sqlx::query(
        "UPDATE certificate_issue_requests
         SET status = 'reviewing', reviewed_by = $2, reviewed_at = clock_timestamp(),
             updated_at = clock_timestamp()
         WHERE id = $1",
    )
    .bind(request_id)
    .bind(actor.user_id)
    .execute(&mut *tx)
    .await
    .map_err(request_db_error)?;
    record_request_audit(
        &mut tx,
        actor.user_id,
        "start_review",
        request_id,
        authorized.campaign_id,
        Some(CertificateIssueRequestStatus::Pending),
        CertificateIssueRequestStatus::Reviewing,
        authorized.item_count as u32,
        Vec::new(),
    )
    .await?;
    tx.commit().await.map_err(request_db_error)?;

    fetch_request_detail_unchecked(pool, actor, request_id).await
}

pub async fn return_request(
    pool: &PgPool,
    actor: &ActorContext,
    request_id: Uuid,
    issue_codes: Vec<CertificateIssueCode>,
    return_note: String,
) -> Result<CertificateIssueRequestDetail, AppError> {
    actor.require_permission(codes::CERTIFICATE_ISSUE_SCHOOL)?;
    let issue_codes = normalize_issue_codes(issue_codes)?;
    let return_note = validate_return_note(&return_note)?;
    let authorized = fetch_request_access(pool, request_id).await?;
    let mut tx = pool.begin().await.map_err(request_db_error)?;
    let locked = lock_request(&mut tx, request_id).await?;
    require_same_request(&authorized, &locked)?;
    let campaign = lock_campaign(&mut tx, authorized.campaign_id).await?;
    require_request_owner_unchanged(&authorized, &campaign)?;
    require_request_status(&locked.status, CertificateIssueRequestStatus::Reviewing)?;
    let issue_code_values = issue_codes
        .iter()
        .map(|code| code.as_str())
        .collect::<Vec<_>>();

    sqlx::query(
        "UPDATE certificate_issue_requests
         SET status = 'returned', returned_at = clock_timestamp(), return_note = $2,
             issue_codes = $3, updated_at = clock_timestamp()
         WHERE id = $1",
    )
    .bind(request_id)
    .bind(return_note)
    .bind(&issue_code_values)
    .execute(&mut *tx)
    .await
    .map_err(request_db_error)?;
    record_request_audit(
        &mut tx,
        actor.user_id,
        "return",
        request_id,
        authorized.campaign_id,
        Some(CertificateIssueRequestStatus::Reviewing),
        CertificateIssueRequestStatus::Returned,
        authorized.item_count as u32,
        issue_code_values
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
    )
    .await?;
    tx.commit().await.map_err(request_db_error)?;

    fetch_request_detail_unchecked(pool, actor, request_id).await
}

pub async fn list_campaign_requests(
    pool: &PgPool,
    actor: &ActorContext,
    campaign_id: Uuid,
) -> Result<Vec<CertificateIssueRequestSummary>, AppError> {
    let campaign = fetch_campaign(pool, campaign_id).await?;
    require_owner_action(
        pool,
        actor,
        campaign.owner_organization_unit_id,
        CertificateAction::Read,
    )
    .await?;
    let rows = sqlx::query_as::<_, IssueRequestRow>(&format!(
        "{REQUEST_SELECT}
         WHERE request.campaign_id = $1
         ORDER BY request.submitted_at DESC, request.id DESC"
    ))
    .bind(campaign_id)
    .fetch_all(pool)
    .await
    .map_err(request_db_error)?;
    rows_to_summaries(pool, actor, rows).await
}

pub async fn list_issue_queue(
    pool: &PgPool,
    actor: &ActorContext,
    query: CertificateIssueRequestListQuery,
) -> Result<Vec<CertificateIssueRequestSummary>, AppError> {
    actor.require_permission(codes::CERTIFICATE_ISSUE_SCHOOL)?;
    let rows = if let Some(status) = query.status {
        sqlx::query_as::<_, IssueRequestRow>(&format!(
            "{REQUEST_SELECT}
             WHERE request.status = $1
             ORDER BY request.submitted_at, request.id"
        ))
        .bind(status.as_str())
        .fetch_all(pool)
        .await
        .map_err(request_db_error)?
    } else {
        sqlx::query_as::<_, IssueRequestRow>(&format!(
            "{REQUEST_SELECT}
             ORDER BY CASE request.status WHEN 'pending' THEN 0 WHEN 'reviewing' THEN 1 ELSE 2 END,
                      request.submitted_at, request.id"
        ))
        .fetch_all(pool)
        .await
        .map_err(request_db_error)?
    };
    rows_to_summaries(pool, actor, rows).await
}

pub async fn get_issue_request(
    pool: &PgPool,
    actor: &ActorContext,
    request_id: Uuid,
) -> Result<CertificateIssueRequestDetail, AppError> {
    let access = fetch_request_access(pool, request_id).await?;
    if !actor.has_permission(codes::CERTIFICATE_ISSUE_SCHOOL) {
        require_owner_action(
            pool,
            actor,
            access.owner_organization_unit_id,
            CertificateAction::Read,
        )
        .await?;
    }
    fetch_request_detail_unchecked(pool, actor, request_id).await
}

pub fn resource_locked_error(request_id: Uuid, can_read_request: bool) -> AppError {
    AppError::CertificateResourceLocked {
        request_id: can_read_request.then_some(request_id),
    }
}

pub async fn can_read_request(
    pool: &PgPool,
    actor: &ActorContext,
    owner_organization_unit_id: Option<Uuid>,
) -> Result<bool, AppError> {
    if actor.has_permission(codes::CERTIFICATE_ISSUE_SCHOOL) {
        return Ok(true);
    }
    match require_owner_action(
        pool,
        actor,
        owner_organization_unit_id,
        CertificateAction::Read,
    )
    .await
    {
        Ok(_) => Ok(true),
        Err(AppError::Forbidden(_)) => Ok(false),
        Err(error) => Err(error),
    }
}

fn normalize_candidate_ids(candidate_ids: Vec<Uuid>) -> Result<Vec<Uuid>, AppError> {
    let candidate_ids = candidate_ids.into_iter().collect::<BTreeSet<_>>();
    if candidate_ids.is_empty() || candidate_ids.len() > MAX_REQUEST_CANDIDATES {
        return Err(AppError::ValidationError(
            "คำขอต้องมีรายชื่อ 1 ถึง 5,000 รายการ".to_string(),
        ));
    }
    Ok(candidate_ids.into_iter().collect())
}

fn normalize_issue_codes(
    issue_codes: Vec<CertificateIssueCode>,
) -> Result<Vec<CertificateIssueCode>, AppError> {
    let issue_codes = issue_codes.into_iter().collect::<BTreeSet<_>>();
    if issue_codes.is_empty() {
        return Err(AppError::ValidationError(
            "กรุณาเลือกเหตุผลส่งกลับอย่างน้อยหนึ่งข้อ".to_string(),
        ));
    }
    Ok(issue_codes.into_iter().collect())
}

fn validate_return_note(value: &str) -> Result<String, AppError> {
    let note = normalize_display_text(value);
    if note.is_empty() || note.chars().count() > MAX_RETURN_NOTE_CHARS {
        return Err(AppError::ValidationError(
            "หมายเหตุส่งกลับต้องมีความยาว 1 ถึง 500 ตัวอักษร".to_string(),
        ));
    }
    if contains_thirteen_digit_run(&note) || is_forbidden_header(&note) {
        return Err(AppError::ValidationError(
            "หมายเหตุส่งกลับห้ามมีเลขประจำตัวประชาชนหรือข้อมูลอ่อนไหว".to_string(),
        ));
    }
    Ok(note)
}

async fn require_candidates_ready(
    tx: &mut Transaction<'_, Postgres>,
    candidates: &[SubmissionCandidateRow],
) -> Result<(), AppError> {
    let account_ids = candidates
        .iter()
        .filter_map(|candidate| candidate.matched_user_id)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let accounts = if account_ids.is_empty() {
        BTreeMap::new()
    } else {
        sqlx::query_as::<_, AccountStateRow>(
            "SELECT id, username, user_type, status
             FROM users
             WHERE id = ANY($1::uuid[])
             ORDER BY id
             FOR SHARE",
        )
        .bind(&account_ids)
        .fetch_all(&mut **tx)
        .await
        .map_err(request_db_error)?
        .into_iter()
        .map(|row| (row.id, row))
        .collect()
    };
    let student_ids = if account_ids.is_empty() {
        BTreeMap::new()
    } else {
        sqlx::query_as::<_, (Uuid, String)>(
            "SELECT user_id, student_id
             FROM student_info
             WHERE user_id = ANY($1::uuid[])
             ORDER BY user_id
             FOR SHARE",
        )
        .bind(&account_ids)
        .fetch_all(&mut **tx)
        .await
        .map_err(request_db_error)?
        .into_iter()
        .collect()
    };
    let template_ids = candidates
        .iter()
        .filter_map(|candidate| candidate.template_id)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let asset_states = if template_ids.is_empty() {
        Vec::new()
    } else {
        sqlx::query_as::<_, TemplateAssetStateRow>(
            "SELECT asset.template_id, asset.id AS asset_id, file.lifecycle_status
             FROM certificate_template_assets asset
             JOIN files file ON file.id = asset.file_id
             WHERE asset.template_id = ANY($1::uuid[])
             ORDER BY asset.template_id, asset.id",
        )
        .bind(&template_ids)
        .fetch_all(&mut **tx)
        .await
        .map_err(request_db_error)?
    };
    let mut assets_by_template = BTreeMap::<Uuid, BTreeMap<Uuid, String>>::new();
    for asset in asset_states {
        assets_by_template
            .entry(asset.template_id)
            .or_default()
            .insert(asset.asset_id, asset.lifecycle_status);
    }

    if candidates
        .iter()
        .all(|candidate| candidate_ready(candidate, &accounts, &student_ids, &assets_by_template))
    {
        Ok(())
    } else {
        Err(AppError::ValidationError(
            "รายการที่เลือกมีข้อมูลหรือแม่แบบที่ยังไม่พร้อมออกเกียรติบัตร".to_string(),
        ))
    }
}

fn candidate_ready(
    candidate: &SubmissionCandidateRow,
    accounts: &BTreeMap<Uuid, AccountStateRow>,
    student_ids: &BTreeMap<Uuid, String>,
    assets_by_template: &BTreeMap<Uuid, BTreeMap<Uuid, String>>,
) -> bool {
    if candidate.deleted_at.is_some()
        || candidate.issued_certificate_id.is_some()
        || candidate.validation_status != CandidateValidationStatus::Ready.as_str()
        || !candidate.validation_codes.is_empty()
        || candidate.selected_name_source.is_none()
    {
        return false;
    }
    let Some(recipient_type) = RecipientType::parse(&candidate.recipient_type) else {
        return false;
    };
    let account_ready = match recipient_type {
        RecipientType::External => {
            candidate.selected_name_source.as_deref() == Some("file")
                && matches!(
                    candidate.match_status.as_str(),
                    "not_applicable" | "external_confirmed"
                )
                && candidate.matched_user_id.is_none()
        }
        RecipientType::Student => candidate.matched_user_id.is_some_and(|user_id| {
            accounts.get(&user_id).is_some_and(|account| {
                account.status == "active"
                    && account.user_type == "student"
                    && student_ids.get(&user_id) == candidate.lookup_student_id.as_ref()
            })
        }),
        RecipientType::Staff => candidate.matched_user_id.is_some_and(|user_id| {
            accounts.get(&user_id).is_some_and(|account| {
                account.status == "active"
                    && account.user_type == "staff"
                    && account.username.as_deref() == candidate.lookup_staff_username.as_deref()
            })
        }),
    };
    if !account_ready {
        return false;
    }
    let Some(template_id) = candidate.template_id else {
        return false;
    };
    let template_ready = candidate.template_is_active == Some(true)
        && candidate.background_file_id.is_some()
        && candidate.background_lifecycle_status.as_deref() == Some("ready")
        && candidate.crop_box_width.is_some()
        && candidate.crop_box_height.is_some()
        && candidate
            .template_allowed_recipient_types
            .as_ref()
            .is_some_and(|types| types.iter().any(|value| value == recipient_type.as_str()));
    if !template_ready {
        return false;
    }
    let Some(layout) = candidate.template_layout.as_ref() else {
        return false;
    };
    let assets = assets_by_template.get(&template_id);
    referenced_asset_ids(&layout.0).iter().all(|asset_id| {
        assets
            .and_then(|items| items.get(asset_id))
            .is_some_and(|status| status == "ready")
    })
}

async fn fetch_campaign(pool: &PgPool, campaign_id: Uuid) -> Result<CampaignRow, AppError> {
    sqlx::query_as::<_, CampaignRow>(
        "SELECT campaign.id, campaign.owner_organization_unit_id,
                owner.is_active AS owner_is_active, campaign.status
         FROM certificate_campaigns campaign
         LEFT JOIN organization_units owner ON owner.id = campaign.owner_organization_unit_id
         WHERE campaign.id = $1",
    )
    .bind(campaign_id)
    .fetch_optional(pool)
    .await
    .map_err(request_db_error)?
    .ok_or_else(request_not_found)
}

async fn lock_campaign(
    tx: &mut Transaction<'_, Postgres>,
    campaign_id: Uuid,
) -> Result<CampaignRow, AppError> {
    sqlx::query_as::<_, CampaignRow>(
        "SELECT campaign.id, campaign.owner_organization_unit_id,
                owner.is_active AS owner_is_active, campaign.status
         FROM certificate_campaigns campaign
         LEFT JOIN organization_units owner ON owner.id = campaign.owner_organization_unit_id
         WHERE campaign.id = $1
         FOR UPDATE OF campaign",
    )
    .bind(campaign_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(request_db_error)?
    .ok_or_else(request_not_found)
}

async fn lock_and_require_active_owner(
    tx: &mut Transaction<'_, Postgres>,
    owner_organization_unit_id: Option<Uuid>,
) -> Result<(), AppError> {
    let Some(owner_id) = owner_organization_unit_id else {
        return Ok(());
    };
    let is_active = sqlx::query_scalar::<_, bool>(
        "SELECT is_active
         FROM organization_units
         WHERE id = $1
         FOR SHARE",
    )
    .bind(owner_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(request_db_error)?;
    if is_active == Some(true) {
        Ok(())
    } else {
        Err(AppError::ValidationError(
            "หน่วยงานเจ้าของกิจกรรมไม่พร้อมใช้งาน".to_string(),
        ))
    }
}

async fn lock_submission_candidates(
    tx: &mut Transaction<'_, Postgres>,
    candidate_ids: &[Uuid],
) -> Result<Vec<SubmissionCandidateRow>, AppError> {
    sqlx::query_as::<_, SubmissionCandidateRow>(
        "SELECT candidate.id, candidate.campaign_id, candidate.template_id,
                candidate.recipient_type, candidate.matched_user_id,
                candidate.lookup_student_id, candidate.lookup_staff_username,
                candidate.selected_name_source, candidate.match_status,
                candidate.validation_status, candidate.validation_codes,
                candidate.issued_certificate_id, candidate.deleted_at,
                template.is_active AS template_is_active,
                template.allowed_recipient_types AS template_allowed_recipient_types,
                template.layout AS template_layout,
                template.crop_box_width, template.crop_box_height,
                template.background_file_id,
                background.lifecycle_status AS background_lifecycle_status
         FROM certificate_candidates candidate
         LEFT JOIN certificate_templates template ON template.id = candidate.template_id
         LEFT JOIN files background ON background.id = template.background_file_id
         WHERE candidate.id = ANY($1::uuid[])
         ORDER BY candidate.id
         FOR UPDATE OF candidate",
    )
    .bind(candidate_ids)
    .fetch_all(&mut **tx)
    .await
    .map_err(request_db_error)
}

async fn active_lock_for_candidates(
    tx: &mut Transaction<'_, Postgres>,
    candidate_ids: &[Uuid],
) -> Result<Option<Uuid>, AppError> {
    sqlx::query_scalar(
        "SELECT candidate_lock.request_id
         FROM certificate_candidate_issue_locks candidate_lock
         JOIN certificate_issue_requests request ON request.id = candidate_lock.request_id
         WHERE candidate_lock.candidate_id = ANY($1::uuid[])
           AND request.status IN ('pending', 'reviewing')
         ORDER BY candidate_lock.candidate_id, candidate_lock.request_id
         LIMIT 1",
    )
    .bind(candidate_ids)
    .fetch_optional(&mut **tx)
    .await
    .map_err(request_db_error)
}

async fn fetch_request_access(
    pool: &PgPool,
    request_id: Uuid,
) -> Result<RequestAccessRow, AppError> {
    sqlx::query_as::<_, RequestAccessRow>(
        "SELECT request.id, request.campaign_id, campaign.owner_organization_unit_id,
                request.status, request.submitted_by,
                (SELECT COUNT(*)::bigint FROM certificate_issue_request_items item
                 WHERE item.request_id = request.id) AS item_count
         FROM certificate_issue_requests request
         JOIN certificate_campaigns campaign ON campaign.id = request.campaign_id
         WHERE request.id = $1",
    )
    .bind(request_id)
    .fetch_optional(pool)
    .await
    .map_err(request_db_error)?
    .ok_or_else(request_not_found)
}

async fn lock_request(
    tx: &mut Transaction<'_, Postgres>,
    request_id: Uuid,
) -> Result<RequestAccessRow, AppError> {
    sqlx::query_as::<_, RequestAccessRow>(
        "SELECT request.id, request.campaign_id, campaign.owner_organization_unit_id,
                request.status, request.submitted_by,
                (SELECT COUNT(*)::bigint FROM certificate_issue_request_items item
                 WHERE item.request_id = request.id) AS item_count
         FROM certificate_issue_requests request
         JOIN certificate_campaigns campaign ON campaign.id = request.campaign_id
         WHERE request.id = $1
         FOR UPDATE OF request",
    )
    .bind(request_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(request_db_error)?
    .ok_or_else(request_not_found)
}

async fn transition_to_withdrawn(
    tx: &mut Transaction<'_, Postgres>,
    actor_user_id: Uuid,
    request: &RequestAccessRow,
) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE certificate_issue_requests
         SET status = 'withdrawn', withdrawn_at = clock_timestamp(),
             updated_at = clock_timestamp()
         WHERE id = $1",
    )
    .bind(request.id)
    .execute(&mut **tx)
    .await
    .map_err(request_db_error)?;
    record_request_audit(
        tx,
        actor_user_id,
        "withdraw",
        request.id,
        request.campaign_id,
        Some(CertificateIssueRequestStatus::Pending),
        CertificateIssueRequestStatus::Withdrawn,
        request.item_count as u32,
        Vec::new(),
    )
    .await
}

async fn fetch_request_detail_unchecked(
    pool: &PgPool,
    actor: &ActorContext,
    request_id: Uuid,
) -> Result<CertificateIssueRequestDetail, AppError> {
    let row =
        sqlx::query_as::<_, IssueRequestRow>(&format!("{REQUEST_SELECT} WHERE request.id = $1"))
            .bind(request_id)
            .fetch_optional(pool)
            .await
            .map_err(request_db_error)?
            .ok_or_else(request_not_found)?;
    let items = fetch_request_items(pool, request_id).await?;
    let context = capability_context(pool, actor).await?;
    detail_from_row(row, items, actor, &context)
}

async fn fetch_request_items(
    pool: &PgPool,
    request_id: Uuid,
) -> Result<Vec<CertificateIssueRequestItem>, AppError> {
    let rows = sqlx::query_as::<_, IssueRequestItemRow>(
        "SELECT candidate.id AS candidate_id, candidate.template_id,
                template.name AS template_name, candidate.recipient_type,
                CASE WHEN candidate.selected_name_source = 'account'
                     THEN candidate.account_title ELSE candidate.imported_title END AS title,
                CASE WHEN candidate.selected_name_source = 'account'
                     THEN COALESCE(candidate.account_first_name, candidate.imported_first_name)
                     ELSE candidate.imported_first_name END AS first_name,
                CASE WHEN candidate.selected_name_source = 'account'
                     THEN COALESCE(candidate.account_last_name, candidate.imported_last_name)
                     ELSE candidate.imported_last_name END AS last_name,
                candidate.activity_item, candidate.award_or_role,
                candidate.validation_status, candidate.validation_codes
         FROM certificate_issue_request_items item
         JOIN certificate_candidates candidate ON candidate.id = item.candidate_id
         LEFT JOIN certificate_templates template ON template.id = candidate.template_id
         WHERE item.request_id = $1
         ORDER BY item.created_at, candidate.id",
    )
    .bind(request_id)
    .fetch_all(pool)
    .await
    .map_err(request_db_error)?;
    rows.into_iter().map(item_from_row).collect()
}

async fn rows_to_summaries(
    pool: &PgPool,
    actor: &ActorContext,
    rows: Vec<IssueRequestRow>,
) -> Result<Vec<CertificateIssueRequestSummary>, AppError> {
    let context = capability_context(pool, actor).await?;
    rows.into_iter()
        .map(|row| summary_from_row(row, actor, &context))
        .collect()
}

async fn capability_context(
    pool: &PgPool,
    actor: &ActorContext,
) -> Result<CapabilityContext, AppError> {
    let submit_scope = match owner_list_scope(pool, actor, CertificateAction::Submit).await {
        Ok(scope) => Some(scope),
        Err(AppError::Forbidden(_)) => None,
        Err(error) => return Err(error),
    };
    Ok(CapabilityContext {
        submit_scope,
        can_issue: actor.has_permission(codes::CERTIFICATE_ISSUE_SCHOOL),
    })
}

fn summary_from_row(
    row: IssueRequestRow,
    actor: &ActorContext,
    context: &CapabilityContext,
) -> Result<CertificateIssueRequestSummary, AppError> {
    let status = parse_request_status(&row.status)?;
    let issue_codes = parse_issue_codes(&row.issue_codes)?;
    let capabilities = request_capabilities(&row, status, actor, context);
    Ok(CertificateIssueRequestSummary {
        id: row.id,
        campaign_id: row.campaign_id,
        campaign_name: row.campaign_name,
        owner_organization_unit_id: row.owner_organization_unit_id,
        owner_organization_unit_name: row.owner_organization_unit_name,
        status,
        submitted_by: row.submitted_by,
        submitted_by_name: row.submitted_by_name,
        reviewed_by: row.reviewed_by,
        reviewed_by_name: row.reviewed_by_name,
        submitted_at: row.submitted_at,
        reviewed_at: row.reviewed_at,
        returned_at: row.returned_at,
        withdrawn_at: row.withdrawn_at,
        issued_at: row.issued_at,
        return_note: row.return_note,
        issue_codes,
        item_count: row.item_count,
        template_count: row.template_count,
        ready_count: row.ready_count,
        review_count: row.review_count,
        invalid_count: row.invalid_count,
        created_at: row.created_at,
        updated_at: row.updated_at,
        capabilities,
    })
}

fn detail_from_row(
    row: IssueRequestRow,
    items: Vec<CertificateIssueRequestItem>,
    actor: &ActorContext,
    context: &CapabilityContext,
) -> Result<CertificateIssueRequestDetail, AppError> {
    let status = parse_request_status(&row.status)?;
    let issue_codes = parse_issue_codes(&row.issue_codes)?;
    let capabilities = request_capabilities(&row, status, actor, context);
    Ok(CertificateIssueRequestDetail {
        id: row.id,
        campaign_id: row.campaign_id,
        campaign_name: row.campaign_name,
        owner_organization_unit_id: row.owner_organization_unit_id,
        owner_organization_unit_name: row.owner_organization_unit_name,
        status,
        submitted_by: row.submitted_by,
        submitted_by_name: row.submitted_by_name,
        reviewed_by: row.reviewed_by,
        reviewed_by_name: row.reviewed_by_name,
        submitted_at: row.submitted_at,
        reviewed_at: row.reviewed_at,
        returned_at: row.returned_at,
        withdrawn_at: row.withdrawn_at,
        issued_at: row.issued_at,
        return_note: row.return_note,
        issue_codes,
        item_count: row.item_count,
        template_count: row.template_count,
        ready_count: row.ready_count,
        review_count: row.review_count,
        invalid_count: row.invalid_count,
        created_at: row.created_at,
        updated_at: row.updated_at,
        capabilities,
        items,
    })
}

fn request_capabilities(
    row: &IssueRequestRow,
    status: CertificateIssueRequestStatus,
    actor: &ActorContext,
    context: &CapabilityContext,
) -> CertificateIssueRequestCapabilities {
    CertificateIssueRequestCapabilities {
        can_withdraw: status == CertificateIssueRequestStatus::Pending
            && row.submitted_by == actor.user_id
            && context.can_submit_owner(row.owner_organization_unit_id),
        can_start_review: status == CertificateIssueRequestStatus::Pending && context.can_issue,
        can_return: status == CertificateIssueRequestStatus::Reviewing && context.can_issue,
        can_issue: status == CertificateIssueRequestStatus::Reviewing && context.can_issue,
    }
}

fn item_from_row(row: IssueRequestItemRow) -> Result<CertificateIssueRequestItem, AppError> {
    Ok(CertificateIssueRequestItem {
        candidate_id: row.candidate_id,
        template_id: row.template_id,
        template_name: row.template_name,
        recipient_type: RecipientType::parse(&row.recipient_type)
            .ok_or_else(|| invalid_persisted_request("recipient_type"))?,
        title: row.title,
        first_name: row.first_name,
        last_name: row.last_name,
        activity_item: row.activity_item,
        award_or_role: row.award_or_role,
        validation_status: CandidateValidationStatus::parse(&row.validation_status)
            .ok_or_else(|| invalid_persisted_request("validation_status"))?,
        validation_codes: row
            .validation_codes
            .iter()
            .map(|value| {
                CandidateValidationCode::parse(value)
                    .ok_or_else(|| invalid_persisted_request("validation_code"))
            })
            .collect::<Result<_, _>>()?,
    })
}

fn parse_request_status(value: &str) -> Result<CertificateIssueRequestStatus, AppError> {
    CertificateIssueRequestStatus::parse(value)
        .ok_or_else(|| invalid_persisted_request("request_status"))
}

fn parse_issue_codes(values: &[String]) -> Result<Vec<CertificateIssueCode>, AppError> {
    values
        .iter()
        .map(|value| {
            CertificateIssueCode::parse(value)
                .ok_or_else(|| invalid_persisted_request("issue_code"))
        })
        .collect()
}

fn require_active_owner(campaign: &CampaignRow) -> Result<(), AppError> {
    if campaign.owner_organization_unit_id.is_none() || campaign.owner_is_active == Some(true) {
        Ok(())
    } else {
        Err(AppError::ValidationError(
            "หน่วยงานเจ้าของกิจกรรมไม่ได้เปิดใช้งาน กรุณาย้ายเจ้าของก่อนส่งคำขอ".to_string(),
        ))
    }
}

fn require_submittable_campaign(status: &str) -> Result<(), AppError> {
    if matches!(status, "draft" | "active") {
        Ok(())
    } else {
        Err(AppError::Conflict(
            "กิจกรรมสถานะนี้ไม่สามารถส่งคำขอออกเกียรติบัตรได้".to_string(),
        ))
    }
}

fn require_same_owner(before: &CampaignRow, locked: &CampaignRow) -> Result<(), AppError> {
    if before.id == locked.id
        && before.owner_organization_unit_id == locked.owner_organization_unit_id
    {
        Ok(())
    } else {
        Err(AppError::Conflict(
            "หน่วยงานเจ้าของกิจกรรมเปลี่ยนแล้ว กรุณาลองใหม่".to_string(),
        ))
    }
}

fn require_request_owner_unchanged(
    request: &RequestAccessRow,
    campaign: &CampaignRow,
) -> Result<(), AppError> {
    if request.campaign_id == campaign.id
        && request.owner_organization_unit_id == campaign.owner_organization_unit_id
    {
        Ok(())
    } else {
        Err(AppError::Conflict(
            "หน่วยงานเจ้าของกิจกรรมเปลี่ยนแล้ว กรุณาลองใหม่".to_string(),
        ))
    }
}

fn require_same_request(
    before: &RequestAccessRow,
    locked: &RequestAccessRow,
) -> Result<(), AppError> {
    if before.id == locked.id
        && before.campaign_id == locked.campaign_id
        && before.submitted_by == locked.submitted_by
    {
        Ok(())
    } else {
        Err(AppError::Conflict(
            "คำขอถูกเปลี่ยนระหว่างดำเนินการ กรุณาลองใหม่".to_string(),
        ))
    }
}

fn require_request_status(
    value: &str,
    expected: CertificateIssueRequestStatus,
) -> Result<(), AppError> {
    let current = parse_request_status(value)?;
    if current == expected {
        Ok(())
    } else {
        Err(AppError::Conflict(
            "สถานะคำขอไม่อนุญาตให้ทำรายการนี้".to_string(),
        ))
    }
}

#[allow(clippy::too_many_arguments)]
async fn record_request_audit(
    tx: &mut Transaction<'_, Postgres>,
    actor_user_id: Uuid,
    action: &'static str,
    request_id: Uuid,
    campaign_id: Uuid,
    from_status: Option<CertificateIssueRequestStatus>,
    to_status: CertificateIssueRequestStatus,
    item_count: u32,
    issue_codes: Vec<String>,
) -> Result<(), AppError> {
    record_issue_request_audit(
        tx,
        actor_user_id,
        action,
        CertificateIssueRequestAuditMetadata {
            campaign_id,
            request_id,
            from_status: from_status.map(|status| status.as_str().to_string()),
            to_status: to_status.as_str().to_string(),
            item_count,
            issue_codes,
        },
    )
    .await
}

fn request_not_found() -> AppError {
    AppError::NotFound("ไม่พบคำขอออกเกียรติบัตร".to_string())
}

fn invalid_persisted_request(field: &'static str) -> AppError {
    tracing::error!(field, "invalid persisted certificate issue request value");
    AppError::InternalServerError("ข้อมูลคำขอออกเกียรติบัตรไม่ถูกต้อง".to_string())
}

fn database_code(error: &sqlx::Error) -> Option<String> {
    match error {
        sqlx::Error::Database(error) => error.code().map(|code| code.into_owned()),
        _ => None,
    }
}

fn request_db_error(error: sqlx::Error) -> AppError {
    let code = database_code(&error);
    tracing::error!(database_code = ?code, "certificate issue request database operation failed");
    AppError::DbError(error)
}
