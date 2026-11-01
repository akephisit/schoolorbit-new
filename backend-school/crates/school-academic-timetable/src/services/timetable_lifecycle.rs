use crate::collaboration::{CommitPermit, LifecycleOutcome, TimetableMutationContext};
use school_database::DbSession;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use chrono::NaiveDate;
#[cfg(test)]
use chrono::Utc;
use school_academic_core::models::TimetableVersionStatus;
use school_academic_delivery::models::versions::DeliverySnapshot;
use school_academic_delivery::models::{ActivitySchedulingMode, LearningOfferingSnapshot};
use school_academic_delivery::services::versions as delivery;
use school_errors::AppError;
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, PgPool, Postgres, Transaction};
use uuid::Uuid;

use super::{timetable_block_queries, timetable_source, timetable_version_service};
use crate::models::timetable_block::{TimetableBlock, TimetableBlockSyncStatus};
use crate::models::timetable_publication::*;
use crate::models::timetable_version::{
    DeleteTimetableDraftRequest, DeletedTimetableDraft, TimetableVersion,
    UpdateTimetableDeliverySourceRequest,
};

#[derive(sqlx::FromRow)]
struct DraftContext {
    academic_year_id: Uuid,
    academic_term_id: Uuid,
    delivery_version_id: Uuid,
    source_version_id: Option<Uuid>,
    row_version: i64,
    status: TimetableVersionStatus,
}

async fn lock_draft(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
    revision: i64,
) -> Result<DraftContext, AppError> {
    if revision <= 0 {
        return Err(AppError::ValidationError("rowVersion ต้องมากกว่าศูนย์".into()));
    }
    timetable_version_service::require_version_term_write(tx, id).await?;
    let context: DraftContext = DbSession::new(&mut **tx).query_as("academic_timetable.services.timetable_lifecycle.lock_draft", "SELECT academic_year_id,academic_term_id,delivery_version_id,source_version_id,row_version,status FROM academic_timetable_versions WHERE id=$1 FOR UPDATE")
        .bind(id).fetch_optional().await?.ok_or_else(|| AppError::NotFound("แบบร่างถูกลบแล้ว กรุณาเลือกรุ่นตารางสอนใหม่".into()))?;
    if context.status != TimetableVersionStatus::Draft {
        return Err(AppError::Conflict("ดำเนินการได้เฉพาะตารางสอนแบบร่าง".into()));
    }
    if context.row_version != revision {
        return Err(AppError::Conflict(
            "แบบร่างเปลี่ยนไป กรุณาโหลดข้อมูลล่าสุดก่อนดำเนินการ".into(),
        ));
    }
    Ok(context)
}

pub async fn update_source(
    pool: &PgPool,
    context: &TimetableMutationContext,
    id: Uuid,
    request: UpdateTimetableDeliverySourceRequest,
) -> Result<TimetableVersion, AppError> {
    context
        .execute_lifecycle_outcome(pool, id, move |pool, permit| async move {
            update_source_in_permit(&pool, permit, id, request).await
        })
        .await
}

async fn update_source_in_permit(
    pool: &PgPool,
    permit: Arc<CommitPermit>,
    id: Uuid,
    request: UpdateTimetableDeliverySourceRequest,
) -> Result<LifecycleOutcome<TimetableVersion>, AppError> {
    let actor = permit.actor_id;
    permit.require_version(id)?;
    let mut tx = school_tenancy::TenantPool::begin_observed(
        pool,
        "academic_timetable.services.timetable_lifecycle.update_source",
    )
    .await?;
    let context = lock_draft(&mut tx, id, request.row_version).await?;
    let changed = align_latest_source_in_transaction(&mut tx, actor, id, &context).await?;
    let result = timetable_version_service::get_version_on_connection(
        &mut tx,
        id,
        timetable_version_service::bangkok_today(),
    )
    .await?;
    permit.commit(tx).await?;
    Ok(if changed {
        LifecycleOutcome::Changed(result)
    } else {
        LifecycleOutcome::Unchanged(result)
    })
}

/// The caller holds the exclusive term lock, draft row lock and an existing-draft permit.
async fn align_latest_source_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    actor: Uuid,
    id: Uuid,
    context: &DraftContext,
) -> Result<bool, AppError> {
    let latest = delivery::latest_published_id(tx, context.academic_term_id).await?;
    let changed = latest != context.delivery_version_id;
    if changed {
        DbSession::new(&mut **tx).query("academic_timetable.services.timetable_lifecycle.update_source", "UPDATE academic_timetable_versions SET delivery_version_id=$2,row_version=row_version+1,updated_at=now() WHERE id=$1")
            .bind(id).bind(latest).execute().await?;
        audit(tx,actor,id,context,"academic_timetable_version.source_updated",serde_json::json!({"previousDeliveryVersionId":context.delivery_version_id,"deliveryVersionId":latest})).await?;
    }
    Ok(changed)
}

pub(crate) async fn align_resumed_source(
    tx: &mut Transaction<'_, Postgres>,
    permit: &crate::collaboration::VersionMutationPermit,
    id: Uuid,
    revision: i64,
) -> Result<bool, AppError> {
    permit.require_existing(id)?;
    let context = lock_draft(tx, id, revision).await?;
    align_latest_source_in_transaction(tx, permit.actor_id(), id, &context).await
}

pub async fn delete_draft(
    pool: &PgPool,
    context: &TimetableMutationContext,
    id: Uuid,
    request: DeleteTimetableDraftRequest,
) -> Result<DeletedTimetableDraft, AppError> {
    context
        .execute_lifecycle(pool, id, move |pool, permit| async move {
            delete_draft_in_permit(&pool, permit, id, request).await
        })
        .await
}

async fn delete_draft_in_permit(
    pool: &PgPool,
    permit: Arc<CommitPermit>,
    id: Uuid,
    request: DeleteTimetableDraftRequest,
) -> Result<DeletedTimetableDraft, AppError> {
    permit.require_version(id)?;
    let mut tx = school_tenancy::TenantPool::begin_observed(
        pool,
        "academic_timetable.services.timetable_lifecycle.delete_draft",
    )
    .await?;
    let deletion = TimetableDeletionPermit::from_commit(permit.clone(), id)?;
    let result = delete_in_transaction(&mut tx, &deletion, id, &request, true).await?;
    permit.commit(tx).await?;
    Ok(result)
}

/// Only an admitted draft commit or a database-proven immutable cancelled draft may be deleted.
pub struct TimetableDeletionPermit {
    version_id: Uuid,
    actor_id: Uuid,
    commit: Option<Arc<CommitPermit>>,
    cancelled_only: bool,
}

impl TimetableDeletionPermit {
    fn from_commit(commit: Arc<CommitPermit>, version_id: Uuid) -> Result<Self, AppError> {
        commit.require_version(version_id)?;
        Ok(Self {
            version_id,
            actor_id: commit.actor_id,
            commit: Some(commit),
            cancelled_only: false,
        })
    }

    pub async fn cancelled_cleanup(
        tx: &mut Transaction<'_, Postgres>,
        actor_id: Uuid,
        version_id: Uuid,
    ) -> Result<Self, AppError> {
        timetable_version_service::require_version_term_write(tx, version_id).await?;
        let cancelled: bool = DbSession::new(&mut **tx)
            .scalar(
                "academic_timetable.services.timetable_lifecycle.cancelled_cleanup_permit",
                "SELECT status='cancelled' FROM academic_timetable_versions WHERE id=$1 FOR UPDATE",
            )
            .bind(version_id)
            .fetch_optional()
            .await?
            .ok_or_else(|| AppError::NotFound("Cancelled timetable not found".into()))?;
        if !cancelled {
            return Err(AppError::Conflict(
                "Operator cleanup only accepts a proven cancelled timetable".into(),
            ));
        }
        Ok(Self {
            version_id,
            actor_id,
            commit: None,
            cancelled_only: true,
        })
    }
}

pub async fn delete_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    permit: &TimetableDeletionPermit,
    id: Uuid,
    request: &DeleteTimetableDraftRequest,
    apply: bool,
) -> Result<DeletedTimetableDraft, AppError> {
    if permit.version_id != id {
        return Err(AppError::Forbidden(
            "Deletion permit does not cover this timetable".into(),
        ));
    }
    if let Some(commit) = &permit.commit {
        commit.require_version(id)?;
    }
    let actor = permit.actor_id;
    if request.row_version <= 0 {
        return Err(AppError::ValidationError("rowVersion ต้องมากกว่าศูนย์".into()));
    }
    timetable_version_service::require_version_term_write(tx, id).await?;
    let context:DraftContext=DbSession::new(&mut **tx).query_as("academic_timetable.services.timetable_lifecycle.delete_in_transaction", "SELECT academic_year_id,academic_term_id,delivery_version_id,source_version_id,row_version,status FROM academic_timetable_versions WHERE id=$1 FOR UPDATE").bind(id).fetch_optional().await?.ok_or_else(||AppError::NotFound("รุ่นตารางถูกลบแล้ว".into()))?;
    if permit.cancelled_only && context.status != TimetableVersionStatus::Cancelled {
        return Err(AppError::Conflict(
            "Cancelled cleanup eligibility changed".into(),
        ));
    }
    if !matches!(
        context.status,
        TimetableVersionStatus::Draft | TimetableVersionStatus::Cancelled
    ) {
        return Err(AppError::Conflict("ลบได้เฉพาะแบบร่างหรือรุ่นที่ยกเลิก".into()));
    }
    if context.row_version != request.row_version {
        return Err(AppError::Conflict(
            "แบบร่างเปลี่ยนไป กรุณาโหลดข้อมูลล่าสุดก่อนลบ".into(),
        ));
    }
    let (supervision,handoff,preparation,dependent_version): (bool,bool,bool,bool)=DbSession::new(&mut **tx).query_as("academic_timetable.services.timetable_lifecycle.delete_in_transaction", "SELECT
        EXISTS(SELECT 1 FROM supervision_observations observation JOIN academic_timetable_block_groups target ON target.id=observation.timetable_block_group_id JOIN academic_timetable_blocks block ON block.id=target.block_id WHERE block.timetable_version_id=$1),
        EXISTS(SELECT 1 FROM academic_teacher_handoff_runs WHERE timetable_version_id=$1),
        EXISTS(SELECT 1 FROM academic_term_preparation_runs run,jsonb_array_elements(run.outcome) outcome WHERE outcome->'targetIds' @> jsonb_build_array($1::text)),
        EXISTS(SELECT 1 FROM academic_timetable_versions WHERE source_version_id=$1)")
        .bind(id).fetch_one().await?;
    let reason = if supervision {
        Some("มีประวัตินิเทศอ้างอิงคาบในร่างนี้")
    } else if handoff {
        Some("มีประวัติรับช่วงคาบอ้างอิงร่างนี้")
    } else if preparation {
        Some("มีประวัติเตรียมภาคเรียนอ้างอิงร่างนี้")
    } else if dependent_version {
        Some("มีรุ่นตารางอื่นอ้างอิงร่างนี้")
    } else {
        None
    };
    if let Some(reason) = reason {
        return Err(AppError::Conflict(format!("ลบแบบร่างไม่ได้: {reason}")));
    }
    let count: i64 = DbSession::new(&mut **tx)
        .scalar(
            "academic_timetable.services.timetable_lifecycle.delete_in_transaction",
            "SELECT count(*) FROM academic_timetable_blocks WHERE timetable_version_id=$1",
        )
        .bind(id)
        .fetch_one()
        .await?;
    if request.expected_block_count < 0 || count != request.expected_block_count {
        return Err(AppError::Conflict(
            "จำนวนคาบในร่างเปลี่ยนไป กรุณาโหลดข้อมูลและยืนยันการลบอีกครั้ง".into(),
        ));
    }
    if apply {
        DbSession::new(&mut **tx).query("academic_timetable.services.timetable_lifecycle.delete_in_transaction", "DELETE FROM academic_timetable_block_group_sync sync USING academic_timetable_blocks block WHERE sync.block_id=block.id AND block.timetable_version_id=$1")
        .bind(id).execute().await?;
        DbSession::new(&mut **tx)
            .query(
                "academic_timetable.services.timetable_lifecycle.delete_in_transaction",
                "DELETE FROM academic_timetable_blocks WHERE timetable_version_id=$1",
            )
            .bind(id)
            .execute()
            .await
            .map_err(delete_error)?;
        let deleted =
        DbSession::new(&mut **tx).query("academic_timetable.services.timetable_lifecycle.delete_in_transaction", "DELETE FROM academic_timetable_versions WHERE id=$1 AND status IN('draft','cancelled')")
            .bind(id)
            .execute()
            .await
            .map_err(delete_error)?;
        if deleted.rows_affected() != 1 {
            return Err(AppError::Conflict("แบบร่างเปลี่ยนไป กรุณาโหลดข้อมูลใหม่".into()));
        }
        audit(tx,actor,id,&context,"academic_timetable_version.draft_deleted",serde_json::json!({"sourceVersionId":context.source_version_id,"deliveryVersionId":context.delivery_version_id,"deletedBlockCount":count})).await?;
    }
    Ok(DeletedTimetableDraft {
        id,
        source_version_id: context.source_version_id,
        deleted_block_count: count,
    })
}

fn delete_error(error: sqlx::Error) -> AppError {
    if let sqlx::Error::Database(database) = &error {
        if database.code().as_deref() == Some("23503") {
            return AppError::Conflict("ลบแบบร่างไม่ได้: มีข้อมูลอื่นอ้างอิง กรุณาโหลดข้อมูลล่าสุด".into());
        }
    }
    AppError::DbError(error)
}

pub async fn preview(
    history: &dyn school_academic_delivery::ports::PublicationHistoryPort,
    pool: &PgPool,
    id: Uuid,
    request: PreviewTimetablePublicationRequest,
) -> Result<TimetablePublicationPreview, AppError> {
    let mut tx = school_tenancy::TenantPool::begin_observed(
        pool,
        "academic_timetable.services.timetable_lifecycle.preview",
    )
    .await?;
    let context = lock_draft(&mut tx, id, request.row_version).await?;
    let result = build_preview(history, &mut tx, id, &context, request.effective_from).await?;
    tx.commit().await?;
    Ok(result)
}

pub async fn publish(
    history: &'static dyn school_academic_delivery::ports::PublicationHistoryPort,
    pool: &PgPool,
    context: &TimetableMutationContext,
    id: Uuid,
    request: PublishTimetableVersionRequest,
) -> Result<TimetableVersion, AppError> {
    let request_hash = publication_request_fingerprint(id, &request)?;
    let key = request.idempotency_key;
    context
        .execute_publication(
            pool,
            id,
            move |pool| async move {
                let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
                    &pool,
                    "academic_timetable.services.timetable_lifecycle.publication_receipt",
                )
                .await?;
                read_publication_receipt(&mut connection, id, key, &request_hash).await
            },
            move |pool, permit| async move {
                publish_in_permit(history, &pool, permit, id, request).await
            },
        )
        .await
}

fn publication_request_fingerprint(
    id: Uuid,
    request: &PublishTimetableVersionRequest,
) -> Result<String, AppError> {
    hash(&(
        id,
        &request.row_version,
        &request.effective_from,
        &request.preview_hash,
    ))
}

async fn read_publication_receipt(
    connection: &mut PgConnection,
    id: Uuid,
    key: Uuid,
    fingerprint: &str,
) -> Result<Option<TimetableVersion>, AppError> {
    let receipt:Option<(Uuid,String)>=DbSession::new(&mut *connection).query_as("academic_timetable.services.timetable_lifecycle.publication_receipt.lookup",
        "SELECT id,publication_request_hash FROM academic_timetable_versions WHERE publication_idempotency_key=$1 AND status='published'")
        .bind(key).fetch_optional().await?;
    match receipt {
        Some((accepted, hash)) => {
            if accepted != id || hash != fingerprint {
                return Err(AppError::Conflict("idempotencyKey ถูกใช้กับคำขออื่นแล้ว".into()));
            }
            Ok(Some(
                timetable_version_service::get_version_on_connection(
                    connection,
                    id,
                    timetable_version_service::bangkok_today(),
                )
                .await?,
            ))
        }
        None => Ok(None),
    }
}

async fn publish_in_permit(
    history: &dyn school_academic_delivery::ports::PublicationHistoryPort,
    pool: &PgPool,
    permit: Arc<CommitPermit>,
    id: Uuid,
    request: PublishTimetableVersionRequest,
) -> Result<TimetableVersion, AppError> {
    let actor = permit.actor_id;
    permit.require_version(id)?;
    let request_hash = publication_request_fingerprint(id, &request)?;
    let mut tx = school_tenancy::TenantPool::begin_observed(
        pool,
        "academic_timetable.services.timetable_lifecycle.publish",
    )
    .await?;
    // Serializes lifecycle and idempotency with autosaves before consulting a receipt.
    let write_state = timetable_version_service::lock_version_term_context(&mut tx, id).await?;
    let receipt: Option<(Uuid,String)>=DbSession::new(&mut *tx).query_as("academic_timetable.services.timetable_lifecycle.publish", "SELECT id,publication_request_hash FROM academic_timetable_versions WHERE publication_idempotency_key=$1 FOR SHARE")
        .bind(request.idempotency_key).fetch_optional().await?;
    if let Some((published_id, stored_hash)) = receipt {
        if published_id != id || stored_hash != request_hash {
            return Err(AppError::Conflict("idempotencyKey ถูกใช้กับคำขออื่นแล้ว".into()));
        }
        let result = timetable_version_service::get_version_on_connection(
            &mut tx,
            id,
            timetable_version_service::bangkok_today(),
        )
        .await?;
        permit.commit(tx).await?;
        return Ok(result);
    }
    write_state.require_writable()?;
    let context = lock_draft(&mut tx, id, request.row_version).await?;
    let preview = build_preview(history, &mut tx, id, &context, request.effective_from).await?;
    if preview.preview_hash != request.preview_hash {
        return Err(AppError::Conflict(
            "ตารางหรือข้อมูลเปิดสอนเปลี่ยนไป กรุณาตรวจความพร้อมใหม่".into(),
        ));
    }
    if !preview.can_publish {
        return Err(AppError::ValidationError(
            "ตารางยังไม่พร้อมเผยแพร่ กรุณาแก้รายการที่แจ้งก่อน".into(),
        ));
    }
    let written = DbSession::new(&mut *tx).query("academic_timetable.services.timetable_lifecycle.publish", "UPDATE academic_timetable_versions SET publication_no=(SELECT COALESCE(max(publication_no),0)+1 FROM academic_timetable_versions WHERE academic_term_id=(SELECT academic_term_id FROM academic_timetable_versions WHERE id=$1) AND status='published'),effective_from=$2,status='published',published_by=$3,published_at=now(),row_version=row_version+1,updated_at=now(),publication_idempotency_key=$4,publication_request_hash=$5 WHERE id=$1 AND status='draft' AND row_version=$6")
        .bind(id).bind(request.effective_from).bind(actor).bind(request.idempotency_key).bind(&request_hash) .bind(request.row_version).execute().await?;
    if written.rows_affected() != 1 {
        return Err(AppError::Conflict(
            "แบบร่างเปลี่ยนไป กรุณาตรวจความพร้อมใหม่".into(),
        ));
    }
    audit(&mut tx,actor,id,&context,"academic_timetable_version.published",serde_json::json!({"deliveryVersionId":context.delivery_version_id,"effectiveFrom":request.effective_from,"previewHash":preview.preview_hash,"idempotencyKey":request.idempotency_key})).await?;
    let result = timetable_version_service::get_version_on_connection(
        &mut tx,
        id,
        timetable_version_service::bangkok_today(),
    )
    .await?;
    permit.commit(tx).await?;
    Ok(result)
}

async fn blocks(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
) -> Result<Vec<TimetableBlock>, AppError> {
    let ids: Vec<Uuid>=DbSession::new(&mut **tx).scalar("academic_timetable.services.timetable_lifecycle.blocks", "SELECT id FROM academic_timetable_blocks WHERE timetable_version_id=$1 AND is_active ORDER BY id FOR SHARE")
        .bind(id).fetch_all().await?;
    timetable_block_queries::get_blocks_on_connection(tx, &ids).await
}

#[derive(Serialize)]
struct PlacementResourceEvidence {
    rooms: Vec<(Uuid, bool)>,
    homerooms: Vec<(Uuid, bool)>,
    periods: Vec<(Uuid, Uuid, bool, Option<String>)>,
    school_days: String,
    schedule_ready: bool,
}

async fn resource_evidence(
    tx: &mut Transaction<'_, Postgres>,
    term_id: Uuid,
    blocks: &[TimetableBlock],
) -> Result<PlacementResourceEvidence, AppError> {
    let room_ids = blocks
        .iter()
        .flat_map(|block| {
            block
                .groups
                .iter()
                .filter(|group| group.is_active)
                .filter_map(|group| group.room_id)
                .chain(
                    block
                        .homerooms
                        .iter()
                        .filter(|target| target.is_active)
                        .filter_map(|target| target.room_id),
                )
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let homeroom_ids = blocks
        .iter()
        .flat_map(|block| {
            block
                .groups
                .iter()
                .filter(|group| group.is_active)
                .flat_map(|group| group.homeroom_ids.iter().copied())
                .chain(
                    block
                        .homerooms
                        .iter()
                        .filter(|target| target.is_active)
                        .map(|target| target.homeroom_id),
                )
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let period_ids = blocks
        .iter()
        .map(|block| block.bell_schedule_period_id)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let rooms = DbSession::new(&mut **tx)
        .query_as(
            "academic_timetable.services.timetable_lifecycle.resource_evidence",
            "SELECT id,status='ACTIVE' FROM rooms WHERE id=ANY($1) ORDER BY id FOR SHARE",
        )
        .bind(&room_ids)
        .fetch_all()
        .await?;
    let homerooms = DbSession::new(&mut **tx)
        .query_as(
            "academic_timetable.services.timetable_lifecycle.resource_evidence",
            "SELECT id,is_active FROM homerooms WHERE id=ANY($1) ORDER BY id FOR SHARE",
        )
        .bind(&homeroom_ids)
        .fetch_all()
        .await?;
    let periods = DbSession::new(&mut **tx).query_as("academic_timetable.services.timetable_lifecycle.resource_evidence", "SELECT id,bell_schedule_id,is_active,applicable_days FROM bell_schedule_periods WHERE id=ANY($1) ORDER BY id FOR SHARE").bind(&period_ids).fetch_all().await?;
    let school_days = DbSession::new(&mut **tx).scalar("academic_timetable.services.timetable_lifecycle.resource_evidence", "SELECT year.school_days FROM academic_years year JOIN academic_terms term ON term.academic_year_id=year.id WHERE term.id=$1 FOR SHARE OF year").bind(term_id).fetch_one().await?;
    let schedule_ready:bool=DbSession::new(&mut **tx).scalar("academic_timetable.services.timetable_lifecycle.resource_evidence", "SELECT EXISTS(SELECT 1 FROM academic_terms term JOIN bell_schedules schedule ON schedule.id=term.bell_schedule_id AND schedule.academic_year_id=term.academic_year_id WHERE term.id=$1 AND schedule.status='published' FOR SHARE OF schedule)").bind(term_id).fetch_one().await?;
    Ok(PlacementResourceEvidence {
        rooms,
        homerooms,
        periods,
        school_days,
        schedule_ready,
    })
}

fn resources_ready(evidence: &PlacementResourceEvidence, blocks: &[TimetableBlock]) -> bool {
    if !evidence.schedule_ready {
        return false;
    }
    let rooms = evidence.rooms.iter().copied().collect::<BTreeMap<_, _>>();
    let homerooms = evidence
        .homerooms
        .iter()
        .copied()
        .collect::<BTreeMap<_, _>>();
    blocks.iter().all(|block| {
        evidence
            .school_days
            .split(',')
            .any(|day| day == block.day_of_week)
            && evidence.periods.iter().any(|(id, schedule, active, days)| {
                *id == block.bell_schedule_period_id
                    && *schedule == block.bell_schedule_id
                    && *active
                    && days
                        .as_ref()
                        .is_none_or(|days| days.split(',').any(|day| day == block.day_of_week))
            })
            && block
                .groups
                .iter()
                .filter(|group| group.is_active)
                .all(|group| {
                    group
                        .room_id
                        .is_none_or(|room| rooms.get(&room) == Some(&true))
                        && group
                            .homeroom_ids
                            .iter()
                            .all(|room| homerooms.get(room) == Some(&true))
                })
            && block
                .homerooms
                .iter()
                .filter(|target| target.is_active)
                .all(|target| {
                    homerooms.get(&target.homeroom_id) == Some(&true)
                        && target
                            .room_id
                            .is_none_or(|room| rooms.get(&room) == Some(&true))
                })
    })
}

async fn build_preview(
    history: &dyn school_academic_delivery::ports::PublicationHistoryPort,
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
    context: &DraftContext,
    date: NaiveDate,
) -> Result<TimetablePublicationPreview, AppError> {
    let source =
        delivery::published_source(tx, context.delivery_version_id, context.academic_term_id)
            .await?;
    let latest_id = delivery::latest_published_id(tx, context.academic_term_id).await?;
    let blocks = blocks(tx, id).await?;
    let content = timetable_source::content_hash(source.id, &blocks)?;
    let (changed, earlier_activation) = if let Some(original_id) = context.source_version_id {
        let (original_source, original_date): (Uuid, NaiveDate) = DbSession::new(&mut **tx)
            .query_as(
                "academic_timetable.services.timetable_lifecycle.build_preview",
                "SELECT delivery_version_id,effective_from FROM academic_timetable_versions WHERE id=$1 FOR SHARE",
            )
            .bind(original_id)
            .fetch_one()
            .await?;
        let original_blocks = self::blocks(tx, original_id).await?;
        (
            content != timetable_source::content_hash(original_source, &original_blocks)?,
            date < original_date,
        )
    } else {
        (true, false)
    };
    let mut findings = readiness(&source.snapshot, &blocks);
    if source.id != latest_id {
        findings.push(finding(
            TimetablePublicationFindingCode::DeliverySourceOutdated,
            "แบบร่างยังไม่ได้ปรับเป็นรุ่นเปิดสอนล่าสุด กรุณาปรับข้อมูลก่อนตรวจและเผยแพร่",
            None,
            None,
            None,
        ));
    }
    if !delivery::contains_date(&source, date) {
        findings.push(finding(
            TimetablePublicationFindingCode::DeliveryDateMismatch,
            "วันที่เริ่มใช้ต้องอยู่ในช่วงที่รุ่นเปิดสอนล่าสุดมีผล กรุณาเลือกวันที่ใหม่",
            None,
            None,
            None,
        ));
    }
    if !changed && !earlier_activation {
        findings.push(finding(
            TimetablePublicationFindingCode::NoContentChange,
            "คาบและรุ่นเปิดสอนยังตรงกับตารางต้นทาง",
            None,
            None,
            None,
        ));
    }
    let teacher_ids: Vec<Uuid> =
        blocks
            .iter()
            .flat_map(|block| {
                block
                    .teachers
                    .iter()
                    .map(|teacher| teacher.teacher_id)
                    .chain(block.groups.iter().flat_map(|group| {
                        group.instructors.iter().map(|teacher| teacher.teacher_id)
                    }))
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
    let teacher_states: Vec<(Uuid,String)>=DbSession::new(&mut **tx).query_as("academic_timetable.services.timetable_lifecycle.build_preview", "SELECT id,status::text FROM users WHERE id=ANY($1) AND user_type='staff' ORDER BY id FOR SHARE").bind(&teacher_ids).fetch_all().await?;
    if teacher_states.len() != teacher_ids.len()
        || teacher_states.iter().any(|(_, status)| status != "active")
    {
        findings.push(finding(
            TimetablePublicationFindingCode::InactiveResource,
            "มีครูที่ไม่ได้อยู่ในสถานะใช้งาน กรุณาตรวจแก้ก่อนเผยแพร่",
            None,
            None,
            None,
        ));
    }
    let resource_evidence = resource_evidence(tx, context.academic_term_id, &blocks).await?;
    if !resources_ready(&resource_evidence, &blocks) {
        findings.push(finding(
            TimetablePublicationFindingCode::InactiveResource,
            "มีห้องหรือช่วงคาบที่ไม่พร้อมใช้งาน หรือวันเรียนไม่ตรงกับที่กำหนด กรุณาตรวจแก้ก่อนเผยแพร่",
            None,
            None,
            None,
        ));
    }
    let source_issues = timetable_source::source_issues(&source.snapshot, &blocks);
    let date_rules = school_academic_delivery::services::publication::date_rules_on_connection(
        &mut **tx,
        context.academic_term_id,
    )
    .await?;
    if date_rules
        .minimum_effective_from
        .is_some_and(|minimum| date < minimum)
    {
        findings.push(finding(
            TimetablePublicationFindingCode::InvalidDate,
            "ภาคเรียนที่เปิดใช้งานแล้วไม่สามารถกำหนดวันที่เริ่มใช้ย้อนหลังได้",
            None,
            None,
            None,
        ));
    }
    if history
        .has_consumed_history(&mut **tx, context.academic_term_id, date)
        .await?
    {
        findings.push(finding(
            TimetablePublicationFindingCode::ConsumedHistoryConflict,
            "วันที่เลือกมีประวัติการเช็คชื่อหรือการปิดวันแล้ว กรุณาเลือกวันที่หลังประวัติเดิม",
            None,
            None,
            None,
        ));
    }
    let preview_hash = hash(&(
        id,
        context.row_version,
        date,
        source.id,
        source.row_version,
        &source.effective_until,
        latest_id,
        &content,
        (&teacher_states, &resource_evidence),
        &findings,
        &source_issues,
    ))?;
    let latest_delivery_publication =
        school_academic_delivery::services::publication::context_on_connection(
            &mut **tx, latest_id,
        )
        .await?;
    let delivery_publication = school_academic_delivery::services::publication::context(&source)?;
    Ok(TimetablePublicationPreview {
        delivery_publication,
        latest_delivery_publication,
        timetable_version_id: id,
        row_version: context.row_version,
        delivery_version_id: source.id,
        latest_delivery_version_id: latest_id,
        effective_from: date,
        block_count: blocks.len(),
        content_changed: changed,
        can_publish: findings.is_empty(),
        source_issues,
        findings,
        preview_hash,
    })
}

fn finding(
    code: TimetablePublicationFindingCode,
    message: &str,
    block_id: Option<Uuid>,
    learning_offering_id: Option<Uuid>,
    learning_group_id: Option<Uuid>,
) -> TimetablePublicationFinding {
    TimetablePublicationFinding {
        code,
        message: message.into(),
        block_id,
        learning_offering_id,
        learning_group_id,
    }
}

/// Readiness is based on complete content and pinned delivery identities.
pub fn readiness(
    source: &DeliverySnapshot,
    blocks: &[TimetableBlock],
) -> Vec<TimetablePublicationFinding> {
    use TimetablePublicationFindingCode as Code;
    let mut findings = Vec::new();
    for issue in timetable_source::source_issues(source, blocks) {
        findings.push(finding(
            Code::SourceNeedsReview,
            "คาบไม่ตรงกับข้อมูลเปิดสอน กรุณาปรับหรือถอดคาบนี้",
            Some(issue.block_id),
            None,
            issue.learning_group_id,
        ));
    }
    for offering in &source.offerings {
        let synchronized = matches!(&offering.catalog,LearningOfferingSnapshot::Activity(activity) if activity.scheduling_mode==ActivitySchedulingMode::Synchronized);
        if synchronized {
            let count = blocks
                .iter()
                .filter(|block| block.is_active && block.learning_offering_id == Some(offering.id))
                .count() as i32;
            if count != offering.weekly_period_target {
                findings.push(finding(
                    Code::PeriodCountMismatch,
                    "จำนวนคาบกิจกรรมไม่ตรงกับจำนวนที่เปิดสอน",
                    None,
                    Some(offering.id),
                    None,
                ));
            }
            for block in blocks
                .iter()
                .filter(|block| block.is_active && block.learning_offering_id == Some(offering.id))
            {
                let coverage = block
                    .homerooms
                    .iter()
                    .filter(|target| target.is_active)
                    .map(|target| target.homeroom_id)
                    .collect::<BTreeSet<_>>();
                if coverage != offering.homeroom_ids.iter().copied().collect() {
                    findings.push(finding(
                        Code::MissingTargets,
                        "ขอบเขตห้องของกิจกรรมไม่ตรงกับรุ่นเปิดสอน",
                        Some(block.id),
                        Some(offering.id),
                        None,
                    ));
                }
                for group in &offering.groups {
                    let linked = block
                        .groups
                        .iter()
                        .any(|placed| placed.is_active && placed.learning_group_id == group.id);
                    let state = block
                        .sync_states
                        .iter()
                        .find(|state| state.learning_group_id == group.id);
                    let intentionally_excluded = state
                        .is_some_and(|state| state.status == TimetableBlockSyncStatus::Excluded);
                    if !linked && !intentionally_excluded {
                        findings.push(finding(
                            Code::PendingSynchronization,
                            "กิจกรรมยังไม่ได้เชื่อมกลุ่มตามรุ่นเปิดสอน กรุณาตรวจและซิงค์กลุ่ม",
                            Some(block.id),
                            Some(offering.id),
                            Some(group.id),
                        ));
                    }
                    if state.is_some_and(|state| {
                        state.status == TimetableBlockSyncStatus::Linked
                            && !block.groups.iter().any(|placed| {
                                placed.is_active
                                    && Some(placed.id) == state.linked_block_group_id
                                    && placed.learning_group_id == group.id
                            })
                    }) {
                        findings.push(finding(
                            Code::PendingSynchronization,
                            "สถานะเชื่อมกลุ่มไม่ตรงกับคาบ กรุณาซิงค์ใหม่",
                            Some(block.id),
                            Some(offering.id),
                            Some(group.id),
                        ));
                    }
                }
            }
        } else {
            for group in &offering.groups {
                let count = blocks
                    .iter()
                    .filter(|block| block.is_active)
                    .flat_map(|block| block.groups.iter())
                    .filter(|placed| placed.is_active && placed.learning_group_id == group.id)
                    .count() as i32;
                if count != offering.weekly_period_target {
                    findings.push(finding(
                        Code::PeriodCountMismatch,
                        if offering.weekly_period_target == 0 {
                            "รายการเปิดสอนนี้ตั้งเป็น 0 คาบต่อสัปดาห์ กรุณาถอดคาบเดิมออกจากตารางก่อนเผยแพร่"
                        } else {
                            "จำนวนคาบกลุ่มเรียนไม่ตรงกับจำนวนที่เปิดสอน"
                        },
                        None,
                        Some(offering.id),
                        Some(group.id),
                    ));
                }
            }
        }
    }
    let mut occupied: BTreeMap<(&str, Uuid, &str, Uuid), Uuid> = BTreeMap::new();
    for block in blocks.iter().filter(|block| block.is_active) {
        if block.groups.iter().all(|group| !group.is_active)
            && block.homerooms.iter().all(|room| !room.is_active)
            && block.teachers.iter().all(|teacher| !teacher.is_active)
        {
            findings.push(finding(
                Code::MissingTargets,
                "คาบไม่มีเป้าหมาย กรุณาระบุหรือถอดคาบนี้",
                Some(block.id),
                block.learning_offering_id,
                None,
            ));
        }
        if block.sync_states.iter().any(|state| {
            !matches!(
                state.status,
                TimetableBlockSyncStatus::Linked | TimetableBlockSyncStatus::Excluded
            )
        }) {
            findings.push(finding(
                Code::PendingSynchronization,
                "กิจกรรมยังมีกลุ่มที่รอเชื่อมหรือมีปัญหา",
                Some(block.id),
                block.learning_offering_id,
                None,
            ));
        }
        let mut resources = BTreeSet::new();
        for group in block.groups.iter().filter(|group| group.is_active) {
            resources.insert(("group", group.learning_group_id));
            resources.extend(group.homeroom_ids.iter().map(|id| ("homeroom", *id)));
            resources.extend(
                group
                    .instructors
                    .iter()
                    .map(|teacher| ("teacher", teacher.teacher_id)),
            );
            resources.extend(group.room_id.map(|id| ("room", id)));
        }
        for target in block.homerooms.iter().filter(|target| target.is_active) {
            resources.insert(("homeroom", target.homeroom_id));
            resources.extend(target.room_id.map(|id| ("room", id)));
        }
        resources.extend(
            block
                .teachers
                .iter()
                .filter(|target| target.is_active)
                .map(|target| ("teacher", target.teacher_id)),
        );
        for (kind, resource_id) in resources {
            if occupied
                .insert(
                    (
                        &block.day_of_week,
                        block.bell_schedule_period_id,
                        kind,
                        resource_id,
                    ),
                    block.id,
                )
                .is_some_and(|other| other != block.id)
            {
                findings.push(finding(
                    Code::Collision,
                    "มีห้อง กลุ่ม ครู หรือห้องเรียนชนในคาบเดียวกัน",
                    Some(block.id),
                    block.learning_offering_id,
                    None,
                ));
            }
        }
    }
    findings
}

pub(crate) fn hash(value: &impl Serialize) -> Result<String, AppError> {
    let bytes = serde_json::to_vec(value)
        .map_err(|_| AppError::InternalServerError("ไม่สามารถตรวจรุ่นข้อมูลตารางสอนได้".into()))?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

async fn audit(
    tx: &mut Transaction<'_, Postgres>,
    actor: Uuid,
    id: Uuid,
    context: &DraftContext,
    code: &str,
    payload: serde_json::Value,
) -> Result<(), AppError> {
    DbSession::new(&mut **tx).query("academic_timetable.services.timetable_lifecycle.audit", "INSERT INTO academic_audit_events(id,event_code,entity_type,entity_id,academic_year_id,academic_term_id,actor_user_id,payload) VALUES(gen_random_uuid(),$1,'academic_timetable_version',$2,$3,$4,$5,$6)")
        .bind(code).bind(id).bind(context.academic_year_id).bind(context.academic_term_id).bind(actor).bind(payload).execute().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::timetable_block::{
        TimetableBlockHomeroom, TimetableBlockKind, TimetableStructuralKind,
    };
    use chrono::NaiveTime;

    fn lesson(id: u128, day: &str, room: u128) -> TimetableBlock {
        TimetableBlock {
            id: Uuid::from_u128(id),
            timetable_version_id: Uuid::from_u128(10),
            academic_term_id: Uuid::from_u128(11),
            academic_year_id: Uuid::from_u128(12),
            bell_schedule_id: Uuid::from_u128(13),
            bell_schedule_period_id: Uuid::from_u128(14),
            period_name: "คาบ 1".into(),
            start_time: NaiveTime::from_hms_opt(8, 0, 0).unwrap(),
            end_time: NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
            day_of_week: day.into(),
            block_kind: TimetableBlockKind::Structural,
            scheduling_mode: None,
            learning_offering_id: None,
            offering_code: None,
            offering_name: None,
            structural_kind: Some(TimetableStructuralKind::Homeroom),
            title: Some("โฮมรูม".into()),
            note: None,
            series_id: None,
            groups: vec![],
            teachers: vec![],
            sync_states: vec![],
            homerooms: vec![TimetableBlockHomeroom {
                id: Uuid::from_u128(id + 100),
                homeroom_id: Uuid::from_u128(room),
                code: "1".into(),
                name: "ม.1/1".into(),
                room_id: None,
                room_code: None,
                room_name: None,
                row_version: 1,
                is_active: true,
            }],
            row_version: 1,
            is_active: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn readiness_detects_shared_homeroom_at_same_slot() {
        let source = DeliverySnapshot { offerings: vec![] };
        let first = lesson(1, "MON", 20);
        let second = lesson(2, "MON", 20);
        let findings = readiness(&source, &[first, second]);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].code, TimetablePublicationFindingCode::Collision);
        assert_eq!(findings[0].block_id, Some(Uuid::from_u128(2)));
    }

    #[test]
    fn readiness_allows_same_room_on_different_days_and_ignores_retired_lessons() {
        let source = DeliverySnapshot { offerings: vec![] };
        let first = lesson(1, "MON", 20);
        let second = lesson(2, "TUE", 20);
        let mut retired = lesson(3, "MON", 20);
        retired.is_active = false;
        assert!(readiness(&source, &[first, second, retired]).is_empty());
    }

    #[test]
    fn readiness_rejects_an_empty_active_structural_lesson() {
        let mut empty = lesson(1, "MON", 20);
        empty.homerooms.clear();
        let findings = readiness(&DeliverySnapshot { offerings: vec![] }, &[empty]);
        assert_eq!(
            findings[0].code,
            TimetablePublicationFindingCode::MissingTargets
        );
    }

    #[test]
    fn placement_resources_reject_retired_rooms_homerooms_and_unavailable_days() {
        let mut block = lesson(1, "MON", 20);
        block.homerooms[0].room_id = Some(Uuid::from_u128(20));
        let mut evidence = PlacementResourceEvidence {
            rooms: vec![(Uuid::from_u128(20), true)],
            homerooms: block
                .homerooms
                .iter()
                .map(|target| (target.homeroom_id, true))
                .collect(),
            periods: vec![(
                block.bell_schedule_period_id,
                block.bell_schedule_id,
                true,
                Some("MON,TUE".into()),
            )],
            school_days: "MON,TUE".into(),
            schedule_ready: true,
        };
        assert!(resources_ready(&evidence, std::slice::from_ref(&block)));
        evidence.schedule_ready = false;
        assert!(!resources_ready(&evidence, std::slice::from_ref(&block)));
        evidence.schedule_ready = true;
        evidence.rooms[0].1 = false;
        assert!(!resources_ready(&evidence, std::slice::from_ref(&block)));
        evidence.rooms[0].1 = true;
        evidence.homerooms[0].1 = false;
        assert!(!resources_ready(&evidence, std::slice::from_ref(&block)));
        evidence.homerooms[0].1 = true;
        evidence.periods[0].3 = Some("TUE".into());
        assert!(!resources_ready(&evidence, std::slice::from_ref(&block)));
        evidence.periods[0].3 = None;
        evidence.school_days = "TUE".into();
        assert!(!resources_ready(&evidence, &[block]));
    }

    #[test]
    fn idempotency_fingerprint_is_bound_to_date_content_preview_and_target() {
        let id = Uuid::from_u128(1);
        let date = NaiveDate::from_ymd_opt(2027, 5, 10).unwrap();
        let original = hash(&(id, 5, date, "preview-one")).unwrap();
        assert_eq!(original, hash(&(id, 5, date, "preview-one")).unwrap());
        assert_ne!(original, hash(&(id, 5, date, "preview-two")).unwrap());
        assert_ne!(
            original,
            hash(&(Uuid::from_u128(2), 5, date, "preview-one")).unwrap()
        );
        assert_ne!(
            original,
            hash(&(id, 5, date.succ_opt().unwrap(), "preview-one")).unwrap()
        );
    }
}
