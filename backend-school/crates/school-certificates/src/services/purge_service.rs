use chrono::{DateTime, Utc};
use school_authorization::ActorContext;
use school_database::DbSession;
use school_errors::AppError;
use school_file_platform::{
    platform_service::FilePlatform,
    repository::{DeleteWork, SqlFileRepository},
};
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::{
    access_policy::CertificateAction,
    models::{
        CertificateCampaignPurgeCounts, CertificateCampaignPurgeImpact,
        CertificateCampaignPurgePhase, CertificateCampaignPurgeStatus,
        StartCertificateCampaignPurgeRequest,
    },
};

const CAMPAIGN_NOT_FOUND: &str = "ไม่พบกิจกรรมเกียรติบัตร";
const PURGE_NOT_FOUND: &str = "ไม่พบรายการลบกิจกรรมเกียรติบัตร";
const CONFIRMATION_MISMATCH: &str = "certificate_purge_confirmation_mismatch";
const IMPACT_CHANGED: &str = "certificate_purge_impact_changed";
const FILE_SHARED: &str = "certificate_purge_file_shared";
const FILE_HELD: &str = "certificate_purge_file_held";
const RETRY_NOT_FAILED: &str = "certificate_purge_retry_not_failed";

#[derive(Debug, FromRow)]
struct PurgeImpactRow {
    campaign_id: Uuid,
    campaign_name: String,
    owner_organization_unit_id: Option<Uuid>,
    status: String,
    updated_at: DateTime<Utc>,
    template_count: i64,
    candidate_count: i64,
    request_count: i64,
    open_request_count: i64,
    issued_certificate_count: i64,
    revoked_certificate_count: i64,
    file_count: i64,
    total_file_bytes: i64,
}

#[derive(Debug, FromRow)]
struct LockedCampaign {
    name: String,
    owner_organization_unit_id: Option<Uuid>,
    status: String,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct InventoryFile {
    file_id: Uuid,
    purpose_code: String,
    visibility: String,
    lifecycle_status: String,
    retention_class: String,
    object_count: i64,
    byte_size: i64,
}

#[derive(Debug, FromRow)]
struct PurgeJobRow {
    status: String,
    file_count: i64,
    deleted_file_count: i64,
    last_error_code: Option<String>,
}

pub async fn impact(
    pool: &PgPool,
    actor: &ActorContext,
    campaign_id: Uuid,
) -> Result<CertificateCampaignPurgeImpact, AppError> {
    let row = load_impact(pool, campaign_id).await?;
    require_purge_owner_action(
        pool,
        actor,
        row.owner_organization_unit_id,
        campaign_not_found,
    )
    .await?;
    if row.status == "purging" {
        return Err(AppError::Conflict("กิจกรรมนี้อยู่ระหว่างลบถาวร".to_string()));
    }
    Ok(impact_from_row(row))
}

pub async fn start(
    pool: &PgPool,
    actor: &ActorContext,
    platform: &FilePlatform,
    campaign_id: Uuid,
    request: StartCertificateCampaignPurgeRequest,
) -> Result<CertificateCampaignPurgeStatus, AppError> {
    let authorization = load_impact(pool, campaign_id).await?;
    require_purge_owner_action(
        pool,
        actor,
        authorization.owner_organization_unit_id,
        campaign_not_found,
    )
    .await?;

    let repository = SqlFileRepository::new(pool.clone());
    let mut transaction =
        school_tenancy::TenantPool::begin_observed(&pool, "certificates.purge_service.start")
            .await?;
    let campaign = lock_campaign(&mut transaction, campaign_id).await?;
    require_purge_owner_action_on_connection(
        &mut transaction,
        actor,
        campaign.owner_organization_unit_id,
        campaign_not_found,
    )
    .await?;

    if campaign.status == "purging" {
        let existing = load_job_in_transaction(&mut transaction, campaign_id).await?;
        transaction.commit().await?;
        return existing
            .map(|job| status_from_job(campaign_id, job))
            .ok_or_else(|| AppError::Conflict("certificate_purge_state_invalid".to_string()));
    }

    if request.confirmation_name != campaign.name {
        return Err(AppError::ValidationError(CONFIRMATION_MISMATCH.to_string()));
    }
    if request.expected_updated_at != campaign.updated_at {
        return Err(AppError::Conflict(IMPACT_CHANGED.to_string()));
    }

    let current_counts = load_counts_in_transaction(&mut transaction, campaign_id).await?;
    if request.expected_impact != current_counts {
        return Err(AppError::Conflict(IMPACT_CHANGED.to_string()));
    }

    let inventory = lock_inventory_files(&mut transaction, campaign_id).await?;
    validate_inventory(&mut transaction, campaign_id, &inventory).await?;
    if i64::try_from(inventory.len()).ok() != Some(current_counts.file_count) {
        return Err(AppError::Conflict(IMPACT_CHANGED.to_string()));
    }

    DbSession::new(&mut *transaction)
        .query(
            "certificates.services.purge_service.start",
            "INSERT INTO certificate_campaign_purge_jobs (
            campaign_id, status, requested_by, template_count,
            candidate_count, request_count, open_request_count,
            issued_certificate_count, revoked_certificate_count,
            file_count, total_file_bytes
         ) VALUES (
            $1, 'deleting_files', $2, $3, $4, $5, $6, $7, $8, $9, $10
         )",
        )
        .bind(campaign_id)
        .bind(actor.user_id)
        .bind(current_counts.template_count)
        .bind(current_counts.candidate_count)
        .bind(current_counts.request_count)
        .bind(current_counts.open_request_count)
        .bind(current_counts.issued_certificate_count)
        .bind(current_counts.revoked_certificate_count)
        .bind(current_counts.file_count)
        .bind(current_counts.total_file_bytes)
        .execute()
        .await?;

    for file in &inventory {
        let object_count = i32::try_from(file.object_count).map_err(|_| {
            AppError::Conflict("certificate_purge_file_inventory_too_large".to_string())
        })?;
        DbSession::new(&mut *transaction)
            .query(
                "certificates.services.purge_service.start",
                "INSERT INTO certificate_campaign_purge_files (
                campaign_id, file_id, object_count, byte_size
             ) VALUES ($1, $2, $3, $4)",
            )
            .bind(campaign_id)
            .bind(file.file_id)
            .bind(object_count)
            .bind(file.byte_size)
            .execute()
            .await?;
    }

    DbSession::new(&mut *transaction)
        .query(
            "certificates.services.purge_service.start",
            "UPDATE certificate_campaigns
         SET status = 'purging', updated_by = $2, updated_at = clock_timestamp()
         WHERE id = $1",
        )
        .bind(campaign_id)
        .bind(actor.user_id)
        .execute()
        .await?;

    let mut prepared = Vec::<DeleteWork>::new();
    for file in &inventory {
        let mut work = repository
            .request_delete_in_transaction(&mut transaction, file.file_id)
            .await
            .map_err(|_| file_preparation_error())?;
        prepared.append(&mut work);
    }
    transaction.commit().await?;

    if let Err(error) = platform
        .complete_prepared_delete(&repository, prepared)
        .await
    {
        let error_code = error.log_safe_code();
        tracing::warn!(
            %campaign_id,
            error_code,
            "certificate campaign file deletion could not complete immediately"
        );
        mark_failed(pool, campaign_id, error_code).await?;
    }

    advance_one(pool, campaign_id).await
}

pub async fn status(
    pool: &PgPool,
    actor: &ActorContext,
    campaign_id: Uuid,
) -> Result<CertificateCampaignPurgeStatus, AppError> {
    let campaign = load_campaign_owner(pool, campaign_id).await?;
    require_purge_owner_action(pool, actor, campaign.0, purge_not_found).await?;
    if campaign.1 != "purging" {
        return Err(purge_not_found());
    }
    load_job(pool, campaign_id)
        .await?
        .map(|job| status_from_job(campaign_id, job))
        .ok_or_else(purge_not_found)
}

pub async fn retry(
    pool: &PgPool,
    actor: &ActorContext,
    platform: &FilePlatform,
    campaign_id: Uuid,
) -> Result<CertificateCampaignPurgeStatus, AppError> {
    let campaign_owner = load_campaign_owner(pool, campaign_id).await?;
    require_purge_owner_action(pool, actor, campaign_owner.0, purge_not_found).await?;

    let repository = SqlFileRepository::new(pool.clone());
    let mut transaction =
        school_tenancy::TenantPool::begin_observed(&pool, "certificates.purge_service.retry")
            .await?;
    let campaign = lock_campaign(&mut transaction, campaign_id).await?;
    require_purge_owner_action_on_connection(
        &mut transaction,
        actor,
        campaign.owner_organization_unit_id,
        purge_not_found,
    )
    .await?;
    if campaign.status != "purging" {
        return Err(purge_not_found());
    }
    let Some(job) = load_job_in_transaction(&mut transaction, campaign_id).await? else {
        return Err(purge_not_found());
    };
    if job.status != "failed" {
        return Err(AppError::Conflict(RETRY_NOT_FAILED.to_string()));
    }

    let transitioned = DbSession::new(&mut *transaction)
        .scalar::<Uuid>(
            "certificates.services.purge_service.retry",
            "UPDATE certificate_campaign_purge_jobs
         SET status = 'deleting_files', last_error_code = NULL,
             updated_at = clock_timestamp()
         WHERE campaign_id = $1 AND status = 'failed'
         RETURNING campaign_id",
        )
        .bind(campaign_id)
        .fetch_optional()
        .await?;
    if transitioned.is_none() {
        return Err(AppError::Conflict(RETRY_NOT_FAILED.to_string()));
    }

    let file_ids = DbSession::new(&mut *transaction)
        .scalar::<Uuid>(
            "certificates.services.purge_service.retry",
            "SELECT file.id
         FROM certificate_campaign_purge_files AS inventory
         JOIN files AS file ON file.id = inventory.file_id
         WHERE inventory.campaign_id = $1
         ORDER BY file.id
         FOR UPDATE OF file",
        )
        .bind(campaign_id)
        .fetch_all()
        .await?;
    let mut prepared = Vec::<DeleteWork>::new();
    for file_id in file_ids {
        let mut work = repository
            .request_delete_in_transaction(&mut transaction, file_id)
            .await
            .map_err(|_| file_preparation_error())?;
        prepared.append(&mut work);
    }
    transaction.commit().await?;

    if let Err(error) = platform
        .complete_prepared_delete(&repository, prepared)
        .await
    {
        let error_code = error.log_safe_code();
        tracing::warn!(
            %campaign_id,
            error_code,
            "certificate campaign file deletion retry could not complete"
        );
        mark_failed(pool, campaign_id, error_code).await?;
    }
    advance_one(pool, campaign_id).await
}

pub async fn reconcile_pending_purges(pool: &PgPool) -> Result<usize, AppError> {
    let mut database_stage_0 = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "certificates.purge_service.reconcile_pending_purges",
    )
    .await?;
    let campaign_ids = DbSession::new(&mut *database_stage_0)
        .scalar::<Uuid>(
            "certificates.services.purge_service.reconcile_pending_purges",
            "SELECT campaign_id
         FROM certificate_campaign_purge_jobs
         WHERE status IN ('deleting_files', 'finalizing')
         ORDER BY updated_at, campaign_id
         LIMIT 50",
        )
        .fetch_all()
        .await?;
    let mut advanced = 0_usize;
    drop(database_stage_0);
    for campaign_id in campaign_ids {
        advance_one(pool, campaign_id).await?;
        advanced += 1;
    }
    Ok(advanced)
}

async fn advance_one(
    pool: &PgPool,
    campaign_id: Uuid,
) -> Result<CertificateCampaignPurgeStatus, AppError> {
    let mut transaction =
        school_tenancy::TenantPool::begin_observed(&pool, "certificates.purge_service.advance_one")
            .await?;
    let campaign = DbSession::new(&mut *transaction)
        .scalar::<String>(
            "certificates.services.purge_service.advance_one",
            "SELECT status FROM certificate_campaigns WHERE id = $1 FOR UPDATE",
        )
        .bind(campaign_id)
        .fetch_optional()
        .await?;
    let Some(campaign_status) = campaign else {
        transaction.rollback().await?;
        return Ok(completed_status(campaign_id, 0));
    };
    if campaign_status != "purging" {
        return Err(AppError::Conflict(
            "certificate_purge_state_invalid".to_string(),
        ));
    }
    let Some(mut job) = load_job_in_transaction(&mut transaction, campaign_id).await? else {
        return Err(AppError::Conflict(
            "certificate_purge_state_invalid".to_string(),
        ));
    };

    if job.status == "failed" {
        transaction.commit().await?;
        return Ok(status_from_job(campaign_id, job));
    }

    let all_files_deleted = job.deleted_file_count == job.file_count;
    if all_files_deleted || job.status == "finalizing" {
        DbSession::new(&mut *transaction)
            .query(
                "certificates.services.purge_service.advance_one",
                "UPDATE certificate_campaign_purge_jobs
             SET status = 'finalizing', last_error_code = NULL,
                 updated_at = clock_timestamp()
             WHERE campaign_id = $1",
            )
            .bind(campaign_id)
            .execute()
            .await?;
        transaction.commit().await?;

        let result = async {
            let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
                pool,
                "certificates.purge.finalize",
            )
            .await?;
            DbSession::new(&mut connection)
                .scalar::<bool>(
                    "certificates.purge.finalize",
                    "SELECT finalize_certificate_campaign_purge($1)",
                )
                .bind(campaign_id)
                .fetch_one()
                .await
        }
        .await;
        match result {
            Ok(true) => return Ok(completed_status(campaign_id, job.file_count)),
            Ok(false) => {
                mark_failed(pool, campaign_id, "certificate_purge_finalize_missing").await?;
            }
            Err(_) => {
                mark_failed(pool, campaign_id, "certificate_purge_finalize_failed").await?;
            }
        }
        return load_job(pool, campaign_id)
            .await?
            .map(|current| status_from_job(campaign_id, current))
            .ok_or_else(purge_not_found);
    }

    let terminal_error = DbSession::new(&mut *transaction)
        .scalar::<Option<String>>(
            "certificates.services.purge_service.advance_one",
            "SELECT operation.last_error_code
         FROM file_operations AS operation
         JOIN certificate_campaign_purge_files AS inventory
           ON inventory.file_id = operation.file_id
         WHERE inventory.campaign_id = $1
           AND operation.operation_type = 'delete_object'
           AND operation.status = 'failed'
           AND NOT EXISTS (
               SELECT 1
               FROM file_operations AS active_operation
               JOIN certificate_campaign_purge_files AS active_inventory
                 ON active_inventory.file_id = active_operation.file_id
               WHERE active_inventory.campaign_id = $1
                 AND active_operation.operation_type = 'delete_object'
                 AND active_operation.status IN (
                     'pending', 'leased', 'retryable_failure'
                 )
           )
         ORDER BY operation.completed_at DESC NULLS LAST,
                  operation.created_at DESC,
                  operation.id DESC
         LIMIT 1",
        )
        .bind(campaign_id)
        .fetch_optional()
        .await?
        .flatten();
    if let Some(error_code) = terminal_error {
        let safe_error_code = bounded_error_code(&error_code);
        DbSession::new(&mut *transaction)
            .query(
                "certificates.services.purge_service.advance_one",
                "UPDATE certificate_campaign_purge_jobs
             SET status = 'failed', last_error_code = $2,
                 updated_at = clock_timestamp()
             WHERE campaign_id = $1",
            )
            .bind(campaign_id)
            .bind(safe_error_code)
            .execute()
            .await?;
        job.status = "failed".to_string();
        job.last_error_code = Some(safe_error_code.to_string());
    }
    transaction.commit().await?;
    Ok(status_from_job(campaign_id, job))
}

async fn load_impact(pool: &PgPool, campaign_id: Uuid) -> Result<PurgeImpactRow, AppError> {
    let mut connection =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "certificates.load_impact")
            .await?;
    load_impact_on_connection(&mut connection, campaign_id).await
}

async fn load_impact_on_connection(
    connection: &mut sqlx::PgConnection,
    campaign_id: Uuid,
) -> Result<PurgeImpactRow, AppError> {
    DbSession::new(&mut *connection)
        .query_as::<PurgeImpactRow>(
            "certificates.services.purge_service.load_impact_on_connection",
            impact_sql(),
        )
        .bind(campaign_id)
        .fetch_optional()
        .await?
        .ok_or_else(campaign_not_found)
}

fn impact_sql() -> &'static str {
    r#"
WITH campaign_files AS (
    SELECT template.background_file_id AS file_id
    FROM certificate_templates AS template
    WHERE template.campaign_id = $1
      AND template.background_file_id IS NOT NULL
    UNION
    SELECT asset.file_id
    FROM certificate_template_assets AS asset
    JOIN certificate_templates AS template ON template.id = asset.template_id
    WHERE template.campaign_id = $1
    UNION
    SELECT upload.file_id
    FROM certificate_template_file_uploads AS upload
    JOIN certificate_templates AS template ON template.id = upload.template_id
    WHERE template.campaign_id = $1
), object_bytes AS (
    SELECT version.byte_size
    FROM file_versions AS version
    JOIN campaign_files AS campaign_file ON campaign_file.file_id = version.file_id
    WHERE version.storage_status <> 'deleted'
    UNION ALL
    SELECT derivative.byte_size
    FROM file_derivatives AS derivative
    JOIN campaign_files AS campaign_file ON campaign_file.file_id = derivative.file_id
    WHERE derivative.storage_status <> 'deleted'
)
SELECT
    campaign.id AS campaign_id,
    campaign.name AS campaign_name,
    campaign.owner_organization_unit_id,
    campaign.status,
    campaign.updated_at,
    (SELECT COUNT(*) FROM certificate_templates AS template
     WHERE template.campaign_id = campaign.id)::BIGINT AS template_count,
    (SELECT COUNT(*) FROM certificate_candidates AS candidate
     WHERE candidate.campaign_id = campaign.id)::BIGINT AS candidate_count,
    (SELECT COUNT(*) FROM certificate_issue_requests AS request
     WHERE request.campaign_id = campaign.id)::BIGINT AS request_count,
    (SELECT COUNT(*) FROM certificate_issue_requests AS request
     WHERE request.campaign_id = campaign.id
       AND request.status IN ('pending', 'reviewing'))::BIGINT AS open_request_count,
    (SELECT COUNT(*) FROM certificates AS certificate
     WHERE certificate.campaign_id = campaign.id
       AND certificate.status = 'issued')::BIGINT AS issued_certificate_count,
    (SELECT COUNT(*) FROM certificates AS certificate
     WHERE certificate.campaign_id = campaign.id
       AND certificate.status = 'revoked')::BIGINT AS revoked_certificate_count,
    (SELECT COUNT(*) FROM campaign_files)::BIGINT AS file_count,
    COALESCE((SELECT SUM(byte_size) FROM object_bytes), 0)::BIGINT AS total_file_bytes
FROM certificate_campaigns AS campaign
WHERE campaign.id = $1
"#
}

async fn load_counts_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    campaign_id: Uuid,
) -> Result<CertificateCampaignPurgeCounts, AppError> {
    #[derive(FromRow)]
    struct CountRow {
        template_count: i64,
        candidate_count: i64,
        request_count: i64,
        open_request_count: i64,
        issued_certificate_count: i64,
        revoked_certificate_count: i64,
        file_count: i64,
        total_file_bytes: i64,
    }

    let sql = format!(
        "SELECT template_count, candidate_count, request_count,
                open_request_count, issued_certificate_count,
                revoked_certificate_count, file_count, total_file_bytes
         FROM ({}) AS impact",
        impact_sql()
    );
    let row = DbSession::new(&mut **transaction)
        .query_as::<CountRow>(
            "certificates.services.purge_service.load_counts_in_transaction",
            sqlx::AssertSqlSafe(sql),
        )
        .bind(campaign_id)
        .fetch_optional()
        .await?
        .ok_or_else(campaign_not_found)?;
    Ok(CertificateCampaignPurgeCounts {
        template_count: row.template_count,
        candidate_count: row.candidate_count,
        request_count: row.request_count,
        open_request_count: row.open_request_count,
        issued_certificate_count: row.issued_certificate_count,
        revoked_certificate_count: row.revoked_certificate_count,
        file_count: row.file_count,
        total_file_bytes: row.total_file_bytes,
    })
}

async fn lock_campaign(
    transaction: &mut Transaction<'_, Postgres>,
    campaign_id: Uuid,
) -> Result<LockedCampaign, AppError> {
    DbSession::new(&mut **transaction)
        .query_as::<LockedCampaign>(
            "certificates.services.purge_service.lock_campaign",
            "SELECT name, owner_organization_unit_id, status, updated_at
         FROM certificate_campaigns
         WHERE id = $1
         FOR UPDATE",
        )
        .bind(campaign_id)
        .fetch_optional()
        .await?
        .ok_or_else(campaign_not_found)
}

async fn lock_inventory_files(
    transaction: &mut Transaction<'_, Postgres>,
    campaign_id: Uuid,
) -> Result<Vec<InventoryFile>, AppError> {
    DbSession::new(&mut **transaction)
        .query_as::<InventoryFile>(
            "certificates.services.purge_service.lock_inventory_files",
            r#"
WITH campaign_files AS (
    SELECT template.background_file_id AS file_id
    FROM certificate_templates AS template
    WHERE template.campaign_id = $1
      AND template.background_file_id IS NOT NULL
    UNION
    SELECT asset.file_id
    FROM certificate_template_assets AS asset
    JOIN certificate_templates AS template ON template.id = asset.template_id
    WHERE template.campaign_id = $1
    UNION
    SELECT upload.file_id
    FROM certificate_template_file_uploads AS upload
    JOIN certificate_templates AS template ON template.id = upload.template_id
    WHERE template.campaign_id = $1
)
SELECT file.id AS file_id,
       file.purpose_code,
       file.visibility,
       file.lifecycle_status,
       file.retention_class,
       (
           (SELECT COUNT(*) FROM file_versions AS version
            WHERE version.file_id = file.id)
           +
           (SELECT COUNT(*) FROM file_derivatives AS derivative
            WHERE derivative.file_id = file.id)
       )::BIGINT AS object_count,
       (
           COALESCE((SELECT SUM(version.byte_size) FROM file_versions AS version
                     WHERE version.file_id = file.id
                       AND version.storage_status <> 'deleted'), 0)
           +
           COALESCE((SELECT SUM(derivative.byte_size) FROM file_derivatives AS derivative
                     WHERE derivative.file_id = file.id
                       AND derivative.storage_status <> 'deleted'), 0)
       )::BIGINT AS byte_size
FROM files AS file
JOIN campaign_files AS campaign_file ON campaign_file.file_id = file.id
ORDER BY file.id
FOR UPDATE OF file
"#,
        )
        .bind(campaign_id)
        .fetch_all()
        .await
        .map_err(Into::into)
}

async fn validate_inventory(
    transaction: &mut Transaction<'_, Postgres>,
    campaign_id: Uuid,
    inventory: &[InventoryFile],
) -> Result<(), AppError> {
    for file in inventory {
        if !matches!(
            file.purpose_code.as_str(),
            "certificate_template_background" | "certificate_template_image"
        ) || file.visibility != "private"
        {
            return Err(AppError::Conflict(
                "certificate_purge_file_relationship_invalid".to_string(),
            ));
        }
        if file.retention_class == "legal_hold" {
            return Err(AppError::Conflict(FILE_HELD.to_string()));
        }
        if file.object_count < 0 || file.byte_size < 0 {
            return Err(AppError::Conflict(
                "certificate_purge_file_inventory_invalid".to_string(),
            ));
        }
        if file.lifecycle_status == "deleted" && file.byte_size != 0 {
            return Err(AppError::Conflict(
                "certificate_purge_deleted_file_inconsistent".to_string(),
            ));
        }
    }

    let has_mismatched_relation: bool = DbSession::new(&mut **transaction)
        .scalar(
            "certificates.services.purge_service.validate_inventory",
            r#"
SELECT EXISTS (
    SELECT 1
    FROM certificate_templates AS template
    JOIN files AS file ON file.id = template.background_file_id
    WHERE template.campaign_id = $1
      AND file.purpose_code <> 'certificate_template_background'
    UNION ALL
    SELECT 1
    FROM certificate_template_assets AS asset
    JOIN certificate_templates AS template ON template.id = asset.template_id
    JOIN files AS file ON file.id = asset.file_id
    WHERE template.campaign_id = $1
      AND file.purpose_code <> CASE asset.kind
          WHEN 'image' THEN 'certificate_template_image'
          ELSE ''
      END
)
"#,
        )
        .bind(campaign_id)
        .fetch_one()
        .await?;
    if has_mismatched_relation {
        return Err(AppError::Conflict(
            "certificate_purge_file_relationship_invalid".to_string(),
        ));
    }

    let file_ids = inventory
        .iter()
        .map(|file| file.file_id)
        .collect::<Vec<_>>();
    let has_shared_file: bool = DbSession::new(&mut **transaction)
        .scalar(
            "certificates.services.purge_service.validate_inventory",
            "SELECT certificate_campaign_purge_has_external_file_consumer($1, $2::UUID[])",
        )
        .bind(campaign_id)
        .bind(file_ids)
        .fetch_one()
        .await?;
    if has_shared_file {
        return Err(AppError::Conflict(FILE_SHARED.to_string()));
    }
    Ok(())
}

async fn load_campaign_owner(
    pool: &PgPool,
    campaign_id: Uuid,
) -> Result<(Option<Uuid>, String), AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "certificates.load_campaign_owner",
    )
    .await?;
    load_campaign_owner_on_connection(&mut connection, campaign_id).await
}

async fn load_campaign_owner_on_connection(
    connection: &mut sqlx::PgConnection,
    campaign_id: Uuid,
) -> Result<(Option<Uuid>, String), AppError> {
    DbSession::new(&mut *connection)
        .query_as(
            "certificates.services.purge_service.load_campaign_owner_on_connection",
            "SELECT owner_organization_unit_id, status
         FROM certificate_campaigns
         WHERE id = $1",
        )
        .bind(campaign_id)
        .fetch_optional()
        .await?
        .ok_or_else(campaign_not_found)
}

async fn load_job(pool: &PgPool, campaign_id: Uuid) -> Result<Option<PurgeJobRow>, AppError> {
    let mut connection =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "certificates.load_job").await?;
    load_job_on_connection(&mut connection, campaign_id).await
}

async fn load_job_on_connection(
    connection: &mut sqlx::PgConnection,
    campaign_id: Uuid,
) -> Result<Option<PurgeJobRow>, AppError> {
    DbSession::new(&mut *connection)
        .query_as::<PurgeJobRow>(
            "certificates.services.purge_service.load_job_on_connection",
            job_sql(false),
        )
        .bind(campaign_id)
        .fetch_optional()
        .await
        .map_err(Into::into)
}

async fn load_job_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    campaign_id: Uuid,
) -> Result<Option<PurgeJobRow>, AppError> {
    DbSession::new(&mut **transaction)
        .query_as::<PurgeJobRow>(
            "certificates.services.purge_service.load_job_in_transaction",
            job_sql(true),
        )
        .bind(campaign_id)
        .fetch_optional()
        .await
        .map_err(Into::into)
}

fn job_sql(lock: bool) -> &'static str {
    if lock {
        r#"
SELECT job.status,
       job.file_count,
       (SELECT COUNT(*)
        FROM certificate_campaign_purge_files AS inventory
        JOIN files AS file ON file.id = inventory.file_id
        WHERE inventory.campaign_id = job.campaign_id
          AND file.lifecycle_status = 'deleted')::BIGINT AS deleted_file_count,
       job.last_error_code
FROM certificate_campaign_purge_jobs AS job
WHERE job.campaign_id = $1
FOR UPDATE OF job
"#
    } else {
        r#"
SELECT job.status,
       job.file_count,
       (SELECT COUNT(*)
        FROM certificate_campaign_purge_files AS inventory
        JOIN files AS file ON file.id = inventory.file_id
        WHERE inventory.campaign_id = job.campaign_id
          AND file.lifecycle_status = 'deleted')::BIGINT AS deleted_file_count,
       job.last_error_code
FROM certificate_campaign_purge_jobs AS job
WHERE job.campaign_id = $1
"#
    }
}

async fn mark_failed(pool: &PgPool, campaign_id: Uuid, error_code: &str) -> Result<(), AppError> {
    let mut connection =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "certificates.mark_failed")
            .await?;
    mark_failed_on_connection(&mut connection, campaign_id, error_code).await
}

async fn mark_failed_on_connection(
    connection: &mut sqlx::PgConnection,
    campaign_id: Uuid,
    error_code: &str,
) -> Result<(), AppError> {
    let error_code = bounded_error_code(error_code);
    DbSession::new(&mut *connection)
        .query(
            "certificates.services.purge_service.mark_failed_on_connection",
            "UPDATE certificate_campaign_purge_jobs
         SET status = 'failed', last_error_code = $2,
             updated_at = clock_timestamp()
         WHERE campaign_id = $1",
        )
        .bind(campaign_id)
        .bind(error_code)
        .execute()
        .await?;
    Ok(())
}

fn bounded_error_code(value: &str) -> &str {
    if !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        value
    } else {
        "certificate_purge_failed"
    }
}

fn impact_from_row(row: PurgeImpactRow) -> CertificateCampaignPurgeImpact {
    CertificateCampaignPurgeImpact {
        campaign_id: row.campaign_id,
        campaign_name: row.campaign_name,
        updated_at: row.updated_at,
        counts: CertificateCampaignPurgeCounts {
            template_count: row.template_count,
            candidate_count: row.candidate_count,
            request_count: row.request_count,
            open_request_count: row.open_request_count,
            issued_certificate_count: row.issued_certificate_count,
            revoked_certificate_count: row.revoked_certificate_count,
            file_count: row.file_count,
            total_file_bytes: row.total_file_bytes,
        },
    }
}

fn status_from_job(campaign_id: Uuid, job: PurgeJobRow) -> CertificateCampaignPurgeStatus {
    let phase = match job.status.as_str() {
        "deleting_files" => CertificateCampaignPurgePhase::DeletingFiles,
        "failed" => CertificateCampaignPurgePhase::Failed,
        "finalizing" => CertificateCampaignPurgePhase::Finalizing,
        _ => CertificateCampaignPurgePhase::Failed,
    };
    CertificateCampaignPurgeStatus {
        campaign_id,
        phase,
        file_count: job.file_count,
        deleted_file_count: job.deleted_file_count,
        last_error_code: job.last_error_code,
    }
}

fn completed_status(campaign_id: Uuid, file_count: i64) -> CertificateCampaignPurgeStatus {
    CertificateCampaignPurgeStatus {
        campaign_id,
        phase: CertificateCampaignPurgePhase::Completed,
        file_count,
        deleted_file_count: file_count,
        last_error_code: None,
    }
}

async fn require_purge_owner_action(
    pool: &PgPool,
    actor: &ActorContext,
    owner_organization_unit_id: Option<Uuid>,
    masked_not_found: fn() -> AppError,
) -> Result<(), AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "certificates.purge_authorization",
    )
    .await?;
    require_purge_owner_action_on_connection(
        &mut connection,
        actor,
        owner_organization_unit_id,
        masked_not_found,
    )
    .await
}

async fn require_purge_owner_action_on_connection(
    connection: &mut sqlx::PgConnection,
    actor: &ActorContext,
    owner_organization_unit_id: Option<Uuid>,
    masked_not_found: fn() -> AppError,
) -> Result<(), AppError> {
    match crate::access_policy::require_owner_action_on_connection(
        connection,
        actor,
        owner_organization_unit_id,
        CertificateAction::Delete,
    )
    .await
    {
        Ok(_) => Ok(()),
        Err(AppError::Forbidden(_)) => Err(masked_not_found()),
        Err(error) => Err(error),
    }
}

fn campaign_not_found() -> AppError {
    AppError::NotFound(CAMPAIGN_NOT_FOUND.to_string())
}

fn purge_not_found() -> AppError {
    AppError::NotFound(PURGE_NOT_FOUND.to_string())
}

fn file_preparation_error() -> AppError {
    AppError::ServiceUnavailable("certificate_file_delete_prepare_failed".to_string())
}
