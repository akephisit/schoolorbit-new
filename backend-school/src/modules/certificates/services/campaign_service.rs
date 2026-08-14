use std::collections::HashSet;

use chrono::{DateTime, NaiveDate, Utc};
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::{
    error::AppError,
    middleware::permission::ActorContext,
    modules::{
        certificates::models::{
            CertificateCampaignCapabilities, CertificateCampaignDetail,
            CertificateCampaignListQuery, CertificateCampaignStatus, CertificateCampaignSummary,
            ChangeCertificateCampaignStatusRequest, CreateCertificateCampaignRequest,
            UpdateCertificateCampaignRequest,
        },
        lookup::models::OrganizationUnitLookupItem,
    },
    policies::certificate_access_policy::{
        owner_list_scope, require_owner_action, CertificateAction, CertificateOwnerListScope,
    },
};

use super::{
    audit_service::{record_campaign_audit, CertificateCampaignAuditMetadata},
    import_validation::normalize_display_text,
};

const CAMPAIGN_SELECT: &str = r#"
    SELECT
        c.id,
        c.academic_year_id,
        ay.year AS academic_year_value,
        ay.name AS academic_year_name,
        c.owner_organization_unit_id,
        ou.code AS owner_organization_unit_code,
        ou.name AS owner_organization_unit_name,
        c.name,
        c.event_date,
        c.status,
        c.activity_sequence,
        c.next_certificate_sequence,
        c.created_by,
        c.updated_by,
        c.created_at,
        c.updated_at,
        (SELECT COUNT(*) FROM certificate_templates t WHERE t.campaign_id = c.id)
            AS template_count,
        (SELECT COUNT(*) FROM certificate_candidates candidate
         WHERE candidate.campaign_id = c.id AND candidate.deleted_at IS NULL)
            AS candidate_count,
        (SELECT COUNT(*) FROM certificates certificate WHERE certificate.campaign_id = c.id)
            AS issued_certificate_count,
        (SELECT COUNT(*) FROM certificate_issue_requests request WHERE request.campaign_id = c.id)
            AS issue_request_count,
        EXISTS (
            SELECT 1 FROM certificate_issue_requests request
            WHERE request.campaign_id = c.id AND request.status IN ('pending', 'reviewing')
        ) AS has_open_issue_request
    FROM certificate_campaigns c
    JOIN academic_years ay ON ay.id = c.academic_year_id
    LEFT JOIN organization_units ou ON ou.id = c.owner_organization_unit_id
"#;

#[derive(Debug, FromRow)]
struct CampaignRow {
    id: Uuid,
    academic_year_id: Uuid,
    academic_year_value: i32,
    academic_year_name: String,
    owner_organization_unit_id: Option<Uuid>,
    owner_organization_unit_code: Option<String>,
    owner_organization_unit_name: Option<String>,
    name: String,
    event_date: NaiveDate,
    status: String,
    activity_sequence: Option<i32>,
    next_certificate_sequence: i32,
    created_by: Option<Uuid>,
    updated_by: Option<Uuid>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    template_count: i64,
    candidate_count: i64,
    issued_certificate_count: i64,
    issue_request_count: i64,
    has_open_issue_request: bool,
}

#[derive(Debug, FromRow)]
struct LockedCampaign {
    id: Uuid,
    academic_year_id: Uuid,
    owner_organization_unit_id: Option<Uuid>,
    name: String,
    event_date: NaiveDate,
    status: String,
    activity_sequence: Option<i32>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct OwnerOptionRow {
    id: Uuid,
    code: String,
    name: String,
    name_en: Option<String>,
    description: Option<String>,
    category: Option<String>,
    display_order: i32,
    is_active: bool,
    parent_unit_id: Option<Uuid>,
    unit_type: Option<String>,
    subject_group_id: Option<Uuid>,
}

#[derive(Clone, Debug)]
enum OwnerCapabilityScope {
    School,
    Units(HashSet<Uuid>),
}

impl OwnerCapabilityScope {
    fn allows(&self, owner_id: Option<Uuid>) -> bool {
        match self {
            Self::School => true,
            Self::Units(units) => owner_id.is_some_and(|id| units.contains(&id)),
        }
    }
}

#[derive(Debug)]
struct CapabilityScopes {
    read: OwnerCapabilityScope,
    create: OwnerCapabilityScope,
    update: OwnerCapabilityScope,
    delete: OwnerCapabilityScope,
    submit: OwnerCapabilityScope,
    download: OwnerCapabilityScope,
}

pub async fn list_campaigns(
    pool: &PgPool,
    actor: &ActorContext,
    query: CertificateCampaignListQuery,
) -> Result<Vec<CertificateCampaignSummary>, AppError> {
    let read_scope = owner_list_scope(pool, actor, CertificateAction::Read).await?;
    let exact_units = match &read_scope {
        CertificateOwnerListScope::School => Vec::new(),
        CertificateOwnerListScope::ExactUnits(units) => units.clone(),
    };
    let school_scope = matches!(read_scope, CertificateOwnerListScope::School);
    let status = query.status.map(CertificateCampaignStatus::as_str);
    let search = normalize_optional_search(query.search)?;
    let sql = format!(
        "{CAMPAIGN_SELECT}
         WHERE ($1::boolean OR c.owner_organization_unit_id = ANY($2::uuid[]))
           AND ($3::uuid IS NULL OR c.academic_year_id = $3)
           AND ($4::text IS NULL OR c.status = $4)
           AND ($5::text IS NULL OR c.name ILIKE '%' || $5 || '%')
         ORDER BY c.event_date DESC, c.created_at DESC, c.id"
    );
    let rows = sqlx::query_as::<_, CampaignRow>(&sql)
        .bind(school_scope)
        .bind(exact_units)
        .bind(query.academic_year_id)
        .bind(status)
        .bind(search)
        .fetch_all(pool)
        .await
        .map_err(campaign_db_error)?;
    let scopes = load_capability_scopes(pool, actor).await?;

    rows.into_iter()
        .map(|row| row_to_summary(row, &scopes))
        .collect()
}

pub async fn get_campaign(
    pool: &PgPool,
    actor: &ActorContext,
    campaign_id: Uuid,
) -> Result<CertificateCampaignDetail, AppError> {
    let row = fetch_campaign_row(pool, campaign_id).await?;
    require_owner_action(
        pool,
        actor,
        row.owner_organization_unit_id,
        CertificateAction::Read,
    )
    .await?;
    let scopes = load_capability_scopes(pool, actor).await?;
    row_to_detail(row, &scopes)
}

pub async fn create_campaign(
    pool: &PgPool,
    actor: &ActorContext,
    payload: CreateCertificateCampaignRequest,
) -> Result<CertificateCampaignDetail, AppError> {
    let name = validate_campaign_name(&payload.name)?;
    require_owner_action(
        pool,
        actor,
        payload.owner_organization_unit_id,
        CertificateAction::Create,
    )
    .await?;
    require_valid_owner(pool, payload.owner_organization_unit_id).await?;
    require_academic_year(pool, payload.academic_year_id).await?;

    let mut tx = pool.begin().await.map_err(campaign_db_error)?;
    let campaign_id: Uuid = sqlx::query_scalar(
        "INSERT INTO certificate_campaigns
            (academic_year_id, owner_organization_unit_id, name, event_date, created_by, updated_by)
         VALUES ($1, $2, $3, $4, $5, $5)
         RETURNING id",
    )
    .bind(payload.academic_year_id)
    .bind(payload.owner_organization_unit_id)
    .bind(name)
    .bind(payload.event_date)
    .bind(actor.user_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(campaign_db_error)?;
    record_campaign_audit(
        &mut tx,
        actor.user_id,
        "create",
        campaign_audit_metadata(
            campaign_id,
            payload.owner_organization_unit_id,
            None,
            Some(CertificateCampaignStatus::Draft),
            [
                "academicYearId",
                "ownerOrganizationUnitId",
                "name",
                "eventDate",
            ],
        ),
    )
    .await?;
    tx.commit().await.map_err(campaign_db_error)?;

    fetch_detail_with_capabilities(pool, actor, campaign_id).await
}

pub async fn update_campaign(
    pool: &PgPool,
    actor: &ActorContext,
    campaign_id: Uuid,
    payload: UpdateCertificateCampaignRequest,
) -> Result<CertificateCampaignDetail, AppError> {
    let requested_name = payload
        .name
        .as_deref()
        .map(validate_campaign_name)
        .transpose()?;
    let requested_owner = payload
        .owner_organization_unit_id
        .as_ref()
        .map(|update| update.value);

    let authorization_row = fetch_campaign_row(pool, campaign_id).await?;
    require_owner_action(
        pool,
        actor,
        authorization_row.owner_organization_unit_id,
        CertificateAction::Update,
    )
    .await?;
    require_valid_owner(pool, authorization_row.owner_organization_unit_id).await?;
    if let Some(next_owner) = requested_owner {
        if next_owner != authorization_row.owner_organization_unit_id {
            require_owner_action(pool, actor, next_owner, CertificateAction::Create).await?;
            require_valid_owner(pool, next_owner).await?;
        }
    }
    if let Some(academic_year_id) = payload.academic_year_id {
        if academic_year_id != authorization_row.academic_year_id {
            require_academic_year(pool, academic_year_id).await?;
        }
    }

    let mut tx = pool.begin().await.map_err(campaign_db_error)?;
    let current = lock_campaign(&mut tx, campaign_id).await?;
    require_authorized_owner_unchanged(
        authorization_row.owner_organization_unit_id,
        current.owner_organization_unit_id,
    )?;
    require_valid_owner_in_transaction(&mut tx, current.owner_organization_unit_id).await?;
    require_expected_update(&current, payload.expected_updated_at)?;
    require_campaign_not_open_locked(&mut tx, campaign_id).await?;

    let next_academic_year_id = payload.academic_year_id.unwrap_or(current.academic_year_id);
    let next_owner = requested_owner.unwrap_or(current.owner_organization_unit_id);
    let next_name = requested_name.unwrap_or_else(|| current.name.clone());
    let next_event_date = payload.event_date.unwrap_or(current.event_date);
    let has_issued = current.activity_sequence.is_some()
        || campaign_has_issued_certificates(&mut tx, campaign_id).await?;

    let academic_year_changed = next_academic_year_id != current.academic_year_id;
    let owner_changed = next_owner != current.owner_organization_unit_id;
    let name_changed = next_name != current.name;
    let event_date_changed = next_event_date != current.event_date;
    if has_issued && (academic_year_changed || owner_changed) {
        return Err(AppError::Conflict(
            "ไม่สามารถเปลี่ยนปีการศึกษาหรือหน่วยงานเจ้าของหลังออกเกียรติบัตรแล้ว".to_string(),
        ));
    }
    if has_issued
        && (name_changed || event_date_changed)
        && !payload.confirm_affects_issued_certificates
    {
        return Err(AppError::Conflict(
            "การแก้ชื่อหรือวันที่จะมีผลต่อเกียรติบัตรที่ออกแล้ว กรุณายืนยันก่อนบันทึก".to_string(),
        ));
    }

    if owner_changed {
        require_valid_owner_in_transaction(&mut tx, next_owner).await?;
    }
    if academic_year_changed {
        require_academic_year_in_transaction(&mut tx, next_academic_year_id).await?;
    }

    let mut changed_fields = Vec::new();
    if academic_year_changed {
        changed_fields.push("academicYearId");
    }
    if owner_changed {
        changed_fields.push("ownerOrganizationUnitId");
    }
    if name_changed {
        changed_fields.push("name");
    }
    if event_date_changed {
        changed_fields.push("eventDate");
    }
    if changed_fields.is_empty() {
        tx.commit().await.map_err(campaign_db_error)?;
        return fetch_detail_with_capabilities(pool, actor, campaign_id).await;
    }

    sqlx::query(
        "UPDATE certificate_campaigns
         SET academic_year_id = $2,
             owner_organization_unit_id = $3,
             name = $4,
             event_date = $5,
             updated_by = $6,
             updated_at = clock_timestamp()
         WHERE id = $1",
    )
    .bind(campaign_id)
    .bind(next_academic_year_id)
    .bind(next_owner)
    .bind(next_name)
    .bind(next_event_date)
    .bind(actor.user_id)
    .execute(&mut *tx)
    .await
    .map_err(campaign_db_error)?;
    record_campaign_audit(
        &mut tx,
        actor.user_id,
        "update",
        campaign_audit_metadata(campaign_id, next_owner, None, None, changed_fields),
    )
    .await?;
    tx.commit().await.map_err(campaign_db_error)?;

    fetch_detail_with_capabilities(pool, actor, campaign_id).await
}

pub async fn change_campaign_status(
    pool: &PgPool,
    actor: &ActorContext,
    campaign_id: Uuid,
    payload: ChangeCertificateCampaignStatusRequest,
) -> Result<CertificateCampaignDetail, AppError> {
    let authorization_row = fetch_campaign_row(pool, campaign_id).await?;
    require_owner_action(
        pool,
        actor,
        authorization_row.owner_organization_unit_id,
        CertificateAction::Update,
    )
    .await?;
    require_valid_owner(pool, authorization_row.owner_organization_unit_id).await?;

    let mut tx = pool.begin().await.map_err(campaign_db_error)?;
    let current = lock_campaign(&mut tx, campaign_id).await?;
    require_authorized_owner_unchanged(
        authorization_row.owner_organization_unit_id,
        current.owner_organization_unit_id,
    )?;
    require_valid_owner_in_transaction(&mut tx, current.owner_organization_unit_id).await?;
    require_expected_update(&current, payload.expected_updated_at)?;
    let current_status = parse_status(&current.status)?;
    if current_status == payload.status {
        tx.commit().await.map_err(campaign_db_error)?;
        return fetch_detail_with_capabilities(pool, actor, campaign_id).await;
    }
    require_campaign_not_open_locked(&mut tx, campaign_id).await?;
    validate_manual_status_transition(current_status, payload.status)?;

    sqlx::query(
        "UPDATE certificate_campaigns
         SET status = $2, updated_by = $3, updated_at = clock_timestamp()
         WHERE id = $1",
    )
    .bind(campaign_id)
    .bind(payload.status.as_str())
    .bind(actor.user_id)
    .execute(&mut *tx)
    .await
    .map_err(campaign_db_error)?;
    record_campaign_audit(
        &mut tx,
        actor.user_id,
        "status_change",
        campaign_audit_metadata(
            campaign_id,
            current.owner_organization_unit_id,
            Some(current_status),
            Some(payload.status),
            ["status"],
        ),
    )
    .await?;
    tx.commit().await.map_err(campaign_db_error)?;

    fetch_detail_with_capabilities(pool, actor, campaign_id).await
}

pub async fn delete_campaign(
    pool: &PgPool,
    actor: &ActorContext,
    campaign_id: Uuid,
) -> Result<Vec<Uuid>, AppError> {
    let authorization_row = fetch_campaign_row(pool, campaign_id).await?;
    require_owner_action(
        pool,
        actor,
        authorization_row.owner_organization_unit_id,
        CertificateAction::Delete,
    )
    .await?;
    require_valid_owner(pool, authorization_row.owner_organization_unit_id).await?;

    let mut tx = pool.begin().await.map_err(campaign_db_error)?;
    let current = lock_campaign(&mut tx, campaign_id).await?;
    require_authorized_owner_unchanged(
        authorization_row.owner_organization_unit_id,
        current.owner_organization_unit_id,
    )?;
    require_valid_owner_in_transaction(&mut tx, current.owner_organization_unit_id).await?;
    let status = parse_status(&current.status)?;
    if status != CertificateCampaignStatus::Draft || current.activity_sequence.is_some() {
        return Err(AppError::Conflict(
            "ลบได้เฉพาะกิจกรรมฉบับร่างที่ยังไม่เคยออกเกียรติบัตร".to_string(),
        ));
    }
    let (issued_count, request_count): (i64, i64) = sqlx::query_as(
        "SELECT
            (SELECT COUNT(*) FROM certificates WHERE campaign_id = $1),
            (SELECT COUNT(*) FROM certificate_issue_requests WHERE campaign_id = $1)",
    )
    .bind(campaign_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(campaign_db_error)?;
    if issued_count > 0 || request_count > 0 {
        return Err(AppError::Conflict(
            "กิจกรรมที่มีประวัติคำขอหรือออกเกียรติบัตรแล้วไม่สามารถลบได้".to_string(),
        ));
    }

    let mut detached_file_ids = sqlx::query_scalar::<_, Uuid>(
        "SELECT background_file_id
         FROM certificate_templates
         WHERE campaign_id = $1 AND background_file_id IS NOT NULL
         UNION
         SELECT asset.file_id
         FROM certificate_template_assets asset
         JOIN certificate_templates template ON template.id = asset.template_id
         WHERE template.campaign_id = $1",
    )
    .bind(campaign_id)
    .fetch_all(&mut *tx)
    .await
    .map_err(campaign_db_error)?;

    record_campaign_audit(
        &mut tx,
        actor.user_id,
        "delete",
        campaign_audit_metadata(
            campaign_id,
            current.owner_organization_unit_id,
            Some(status),
            None,
            std::iter::empty::<&str>(),
        ),
    )
    .await?;
    sqlx::query("DELETE FROM certificate_campaigns WHERE id = $1")
        .bind(campaign_id)
        .execute(&mut *tx)
        .await
        .map_err(campaign_db_error)?;
    tx.commit().await.map_err(campaign_db_error)?;
    detached_file_ids.sort_unstable();
    detached_file_ids.dedup();
    Ok(detached_file_ids)
}

pub async fn list_owner_options(
    pool: &PgPool,
    actor: &ActorContext,
) -> Result<Vec<OrganizationUnitLookupItem>, AppError> {
    let scope = owner_list_scope(pool, actor, CertificateAction::Create).await?;
    let (school_scope, units) = match scope {
        CertificateOwnerListScope::School => (true, Vec::new()),
        CertificateOwnerListScope::ExactUnits(units) => (false, units),
    };
    let rows = sqlx::query_as::<_, OwnerOptionRow>(
        "SELECT id, code, name, name_en, description, category, display_order,
                is_active, parent_unit_id, unit_type, subject_group_id
         FROM organization_units
         WHERE is_active IS TRUE
           AND upper(code) <> 'SCHOOL'
           AND ($1::boolean OR id = ANY($2::uuid[]))
         ORDER BY display_order, name, id",
    )
    .bind(school_scope)
    .bind(units)
    .fetch_all(pool)
    .await
    .map_err(campaign_db_error)?;
    Ok(rows
        .into_iter()
        .map(|row| OrganizationUnitLookupItem {
            id: row.id,
            code: row.code,
            name: row.name,
            name_en: row.name_en,
            description: row.description,
            category: row.category,
            display_order: row.display_order,
            is_active: row.is_active,
            parent_unit_id: row.parent_unit_id,
            unit_type: row.unit_type,
            subject_group_id: row.subject_group_id,
        })
        .collect())
}

async fn fetch_detail_with_capabilities(
    pool: &PgPool,
    actor: &ActorContext,
    campaign_id: Uuid,
) -> Result<CertificateCampaignDetail, AppError> {
    let row = fetch_campaign_row(pool, campaign_id).await?;
    let scopes = load_capability_scopes(pool, actor).await?;
    row_to_detail(row, &scopes)
}

async fn fetch_campaign_row(pool: &PgPool, campaign_id: Uuid) -> Result<CampaignRow, AppError> {
    let sql = format!("{CAMPAIGN_SELECT} WHERE c.id = $1");
    sqlx::query_as::<_, CampaignRow>(&sql)
        .bind(campaign_id)
        .fetch_optional(pool)
        .await
        .map_err(campaign_db_error)?
        .ok_or_else(campaign_not_found)
}

async fn lock_campaign(
    tx: &mut Transaction<'_, Postgres>,
    campaign_id: Uuid,
) -> Result<LockedCampaign, AppError> {
    sqlx::query_as(
        "SELECT id, academic_year_id, owner_organization_unit_id, name, event_date,
                status, activity_sequence, updated_at
         FROM certificate_campaigns
         WHERE id = $1
         FOR UPDATE",
    )
    .bind(campaign_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(campaign_db_error)?
    .ok_or_else(campaign_not_found)
}

async fn load_capability_scopes(
    pool: &PgPool,
    actor: &ActorContext,
) -> Result<CapabilityScopes, AppError> {
    Ok(CapabilityScopes {
        read: optional_owner_scope(pool, actor, CertificateAction::Read).await?,
        create: optional_owner_scope(pool, actor, CertificateAction::Create).await?,
        update: optional_owner_scope(pool, actor, CertificateAction::Update).await?,
        delete: optional_owner_scope(pool, actor, CertificateAction::Delete).await?,
        submit: optional_owner_scope(pool, actor, CertificateAction::Submit).await?,
        download: optional_owner_scope(pool, actor, CertificateAction::Download).await?,
    })
}

async fn optional_owner_scope(
    pool: &PgPool,
    actor: &ActorContext,
    action: CertificateAction,
) -> Result<OwnerCapabilityScope, AppError> {
    match owner_list_scope(pool, actor, action).await {
        Ok(CertificateOwnerListScope::School) => Ok(OwnerCapabilityScope::School),
        Ok(CertificateOwnerListScope::ExactUnits(units)) => {
            Ok(OwnerCapabilityScope::Units(units.into_iter().collect()))
        }
        Err(AppError::Forbidden(_)) => Ok(OwnerCapabilityScope::Units(HashSet::new())),
        Err(error) => Err(error),
    }
}

fn row_to_summary(
    row: CampaignRow,
    scopes: &CapabilityScopes,
) -> Result<CertificateCampaignSummary, AppError> {
    let status = parse_status(&row.status)?;
    let capabilities = capabilities_for(&row, status, scopes);
    Ok(CertificateCampaignSummary {
        id: row.id,
        academic_year_id: row.academic_year_id,
        academic_year_value: row.academic_year_value,
        academic_year_name: row.academic_year_name,
        owner_organization_unit_id: row.owner_organization_unit_id,
        owner_organization_unit_code: row.owner_organization_unit_code,
        owner_organization_unit_name: row.owner_organization_unit_name,
        name: row.name,
        event_date: row.event_date,
        status,
        activity_sequence: row.activity_sequence,
        template_count: row.template_count,
        candidate_count: row.candidate_count,
        issued_certificate_count: row.issued_certificate_count,
        has_open_issue_request: row.has_open_issue_request,
        created_at: row.created_at,
        updated_at: row.updated_at,
        capabilities,
    })
}

fn row_to_detail(
    row: CampaignRow,
    scopes: &CapabilityScopes,
) -> Result<CertificateCampaignDetail, AppError> {
    let status = parse_status(&row.status)?;
    let capabilities = capabilities_for(&row, status, scopes);
    Ok(CertificateCampaignDetail {
        id: row.id,
        academic_year_id: row.academic_year_id,
        academic_year_value: row.academic_year_value,
        academic_year_name: row.academic_year_name,
        owner_organization_unit_id: row.owner_organization_unit_id,
        owner_organization_unit_code: row.owner_organization_unit_code,
        owner_organization_unit_name: row.owner_organization_unit_name,
        name: row.name,
        event_date: row.event_date,
        status,
        activity_sequence: row.activity_sequence,
        next_certificate_sequence: row.next_certificate_sequence,
        template_count: row.template_count,
        candidate_count: row.candidate_count,
        issued_certificate_count: row.issued_certificate_count,
        has_open_issue_request: row.has_open_issue_request,
        created_by: row.created_by,
        updated_by: row.updated_by,
        created_at: row.created_at,
        updated_at: row.updated_at,
        capabilities,
    })
}

fn capabilities_for(
    row: &CampaignRow,
    status: CertificateCampaignStatus,
    scopes: &CapabilityScopes,
) -> CertificateCampaignCapabilities {
    let owner = row.owner_organization_unit_id;
    let unlocked = !row.has_open_issue_request;
    let can_update = scopes.update.allows(owner) && unlocked;
    CertificateCampaignCapabilities {
        can_read: scopes.read.allows(owner),
        can_update,
        can_delete: scopes.delete.allows(owner)
            && unlocked
            && status == CertificateCampaignStatus::Draft
            && row.activity_sequence.is_none()
            && row.issued_certificate_count == 0
            && row.issue_request_count == 0,
        can_submit: scopes.submit.allows(owner)
            && unlocked
            && matches!(
                status,
                CertificateCampaignStatus::Draft | CertificateCampaignStatus::Active
            ),
        can_download: scopes.download.allows(owner) && row.issued_certificate_count > 0,
        can_change_status: can_update && status != CertificateCampaignStatus::Draft,
        can_manage_templates: can_manage_templates(scopes, owner),
    }
}

fn can_manage_templates(scopes: &CapabilityScopes, owner: Option<Uuid>) -> bool {
    scopes.create.allows(owner) && scopes.update.allows(owner)
}

fn parse_status(value: &str) -> Result<CertificateCampaignStatus, AppError> {
    CertificateCampaignStatus::parse(value).ok_or_else(|| {
        tracing::error!(
            status = value,
            "invalid certificate campaign status in database"
        );
        AppError::InternalServerError("สถานะกิจกรรมเกียรติบัตรไม่ถูกต้อง".to_string())
    })
}

fn validate_campaign_name(value: &str) -> Result<String, AppError> {
    let normalized = normalize_display_text(value);
    if normalized.is_empty() {
        return Err(AppError::ValidationError(
            "กรุณาระบุชื่อกิจกรรมเกียรติบัตร".to_string(),
        ));
    }
    if normalized.chars().count() > 200 {
        return Err(AppError::ValidationError(
            "ชื่อกิจกรรมเกียรติบัตรต้องไม่เกิน 200 ตัวอักษร".to_string(),
        ));
    }
    Ok(normalized)
}

fn normalize_optional_search(value: Option<String>) -> Result<Option<String>, AppError> {
    value
        .map(|value| {
            let normalized = normalize_display_text(&value);
            if normalized.chars().count() > 200 {
                Err(AppError::ValidationError(
                    "คำค้นหาต้องไม่เกิน 200 ตัวอักษร".to_string(),
                ))
            } else if normalized.is_empty() {
                Ok(None)
            } else {
                Ok(Some(normalized))
            }
        })
        .unwrap_or(Ok(None))
}

async fn require_valid_owner(pool: &PgPool, owner_id: Option<Uuid>) -> Result<(), AppError> {
    let Some(owner_id) = owner_id else {
        return Ok(());
    };
    let valid: bool = sqlx::query_scalar(
        "SELECT EXISTS (
            SELECT 1 FROM organization_units
            WHERE id = $1 AND is_active IS TRUE AND upper(code) <> 'SCHOOL'
        )",
    )
    .bind(owner_id)
    .fetch_one(pool)
    .await
    .map_err(campaign_db_error)?;
    if valid {
        Ok(())
    } else {
        Err(AppError::ValidationError(
            "หน่วยงานเจ้าของกิจกรรมไม่ถูกต้องหรือไม่ได้เปิดใช้งาน".to_string(),
        ))
    }
}

async fn require_academic_year(pool: &PgPool, academic_year_id: Uuid) -> Result<(), AppError> {
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM academic_years WHERE id = $1)")
            .bind(academic_year_id)
            .fetch_one(pool)
            .await
            .map_err(campaign_db_error)?;
    if exists {
        Ok(())
    } else {
        Err(AppError::ValidationError("ไม่พบปีการศึกษาที่เลือก".to_string()))
    }
}

async fn require_valid_owner_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    owner_id: Option<Uuid>,
) -> Result<(), AppError> {
    let Some(owner_id) = owner_id else {
        return Ok(());
    };
    let valid: bool = sqlx::query_scalar(
        "SELECT EXISTS (
            SELECT 1 FROM organization_units
            WHERE id = $1 AND is_active IS TRUE AND upper(code) <> 'SCHOOL'
        )",
    )
    .bind(owner_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(campaign_db_error)?;
    if valid {
        Ok(())
    } else {
        Err(AppError::ValidationError(
            "หน่วยงานเจ้าของกิจกรรมไม่ถูกต้องหรือไม่ได้เปิดใช้งาน".to_string(),
        ))
    }
}

async fn require_academic_year_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    academic_year_id: Uuid,
) -> Result<(), AppError> {
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM academic_years WHERE id = $1)")
            .bind(academic_year_id)
            .fetch_one(&mut **tx)
            .await
            .map_err(campaign_db_error)?;
    if exists {
        Ok(())
    } else {
        Err(AppError::ValidationError("ไม่พบปีการศึกษาที่เลือก".to_string()))
    }
}

fn require_authorized_owner_unchanged(
    authorized_owner_id: Option<Uuid>,
    locked_owner_id: Option<Uuid>,
) -> Result<(), AppError> {
    if authorized_owner_id == locked_owner_id {
        Ok(())
    } else {
        Err(AppError::Conflict(
            "หน่วยงานเจ้าของกิจกรรมถูกเปลี่ยน กรุณาโหลดข้อมูลใหม่".to_string(),
        ))
    }
}

fn require_expected_update(
    campaign: &LockedCampaign,
    expected_updated_at: DateTime<Utc>,
) -> Result<(), AppError> {
    if campaign.updated_at == expected_updated_at {
        Ok(())
    } else {
        Err(AppError::Conflict(
            "กิจกรรมถูกแก้ไขโดยผู้ใช้อื่น กรุณาโหลดข้อมูลใหม่".to_string(),
        ))
    }
}

async fn require_campaign_not_open_locked(
    tx: &mut Transaction<'_, Postgres>,
    campaign_id: Uuid,
) -> Result<(), AppError> {
    let locked: bool = sqlx::query_scalar(
        "SELECT EXISTS (
            SELECT 1 FROM certificate_issue_requests
            WHERE campaign_id = $1 AND status IN ('pending', 'reviewing')
        )",
    )
    .bind(campaign_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(campaign_db_error)?;
    if locked {
        Err(AppError::Conflict(
            "กิจกรรมนี้มีคำขอออกเกียรติบัตรที่กำลังตรวจสอบ จึงยังแก้ไขไม่ได้".to_string(),
        ))
    } else {
        Ok(())
    }
}

async fn campaign_has_issued_certificates(
    tx: &mut Transaction<'_, Postgres>,
    campaign_id: Uuid,
) -> Result<bool, AppError> {
    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM certificates WHERE campaign_id = $1)")
        .bind(campaign_id)
        .fetch_one(&mut **tx)
        .await
        .map_err(campaign_db_error)
}

fn validate_manual_status_transition(
    from: CertificateCampaignStatus,
    to: CertificateCampaignStatus,
) -> Result<(), AppError> {
    let allowed = matches!(
        (from, to),
        (
            CertificateCampaignStatus::Active,
            CertificateCampaignStatus::Closed | CertificateCampaignStatus::Archived
        ) | (
            CertificateCampaignStatus::Closed,
            CertificateCampaignStatus::Active | CertificateCampaignStatus::Archived
        ) | (
            CertificateCampaignStatus::Archived,
            CertificateCampaignStatus::Active
        )
    );
    if allowed {
        Ok(())
    } else if from == CertificateCampaignStatus::Draft {
        Err(AppError::Conflict(
            "ฉบับร่างจะเปิดใช้งานอัตโนมัติเมื่อออกเกียรติบัตรครั้งแรก และลบได้หากยังไม่เคยออก".to_string(),
        ))
    } else {
        Err(AppError::ValidationError(
            "ไม่สามารถเปลี่ยนเป็นสถานะที่เลือกได้".to_string(),
        ))
    }
}

fn campaign_audit_metadata<I, S>(
    campaign_id: Uuid,
    owner_organization_unit_id: Option<Uuid>,
    from_status: Option<CertificateCampaignStatus>,
    to_status: Option<CertificateCampaignStatus>,
    changed_fields: I,
) -> CertificateCampaignAuditMetadata
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    CertificateCampaignAuditMetadata {
        campaign_id,
        owner_organization_unit_id,
        from_status: from_status.map(|status| status.as_str().to_string()),
        to_status: to_status.map(|status| status.as_str().to_string()),
        changed_fields: changed_fields
            .into_iter()
            .map(|field| field.as_ref().to_string())
            .collect(),
    }
}

fn campaign_not_found() -> AppError {
    AppError::NotFound("ไม่พบกิจกรรมเกียรติบัตร".to_string())
}

fn campaign_db_error(error: sqlx::Error) -> AppError {
    tracing::error!(%error, "certificate campaign database operation failed");
    AppError::DbError(error)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit_scope(ids: impl IntoIterator<Item = Uuid>) -> OwnerCapabilityScope {
        OwnerCapabilityScope::Units(ids.into_iter().collect())
    }

    fn template_scopes(
        create: OwnerCapabilityScope,
        update: OwnerCapabilityScope,
    ) -> CapabilityScopes {
        CapabilityScopes {
            read: unit_scope([]),
            create,
            update,
            delete: unit_scope([]),
            submit: unit_scope([]),
            download: unit_scope([]),
        }
    }

    #[test]
    fn template_workflow_requires_create_and_update_for_the_exact_owner() {
        let unit_a = Uuid::from_u128(1);
        let unit_b = Uuid::from_u128(2);
        let both = template_scopes(unit_scope([unit_a]), unit_scope([unit_a]));
        let create_only = template_scopes(unit_scope([unit_a]), unit_scope([]));

        assert!(can_manage_templates(&both, Some(unit_a)));
        assert!(!can_manage_templates(&both, Some(unit_b)));
        assert!(!can_manage_templates(&create_only, Some(unit_a)));
    }
}
