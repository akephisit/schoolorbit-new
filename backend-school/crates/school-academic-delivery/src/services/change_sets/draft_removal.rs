use super::*;
use crate::models::{
    versions::DeliverySnapshot, ActivitySchedulingMode, LearningOfferingSnapshot,
    RemoveDeliveryDraftResourceRequest,
};

pub async fn remove_draft_offering(
    pool: &PgPool,
    actor: Uuid,
    change_set_id: Uuid,
    offering_id: Uuid,
    request: RemoveDeliveryDraftResourceRequest,
) -> Result<AcademicTermChangeSet, AppError> {
    remove_resource(pool, actor, change_set_id, offering_id, None, None, request).await
}

pub async fn remove_draft_group(
    pool: &PgPool,
    actor: Uuid,
    change_set_id: Uuid,
    group_id: Uuid,
    request: RemoveDeliveryDraftResourceRequest,
) -> Result<AcademicTermChangeSet, AppError> {
    remove_resource(
        pool,
        actor,
        change_set_id,
        group_id,
        Some(group_id),
        None,
        request,
    )
    .await
}

pub async fn remove_draft_offering_homeroom(
    pool: &PgPool,
    actor: Uuid,
    change_set_id: Uuid,
    offering_id: Uuid,
    homeroom_id: Uuid,
    request: RemoveDeliveryDraftResourceRequest,
) -> Result<AcademicTermChangeSet, AppError> {
    remove_resource(
        pool,
        actor,
        change_set_id,
        offering_id,
        None,
        Some(homeroom_id),
        request,
    )
    .await
}

async fn remove_resource(
    pool: &PgPool,
    actor: Uuid,
    change_set_id: Uuid,
    resource_id: Uuid,
    group_id: Option<Uuid>,
    homeroom_id: Option<Uuid>,
    request: RemoveDeliveryDraftResourceRequest,
) -> Result<AcademicTermChangeSet, AppError> {
    validate_row_version(request.change_set_row_version)?;
    validate_row_version(request.target_delivery_version_row_version)?;
    let mut tx = school_tenancy::TenantPool::begin_observed(
        pool,
        "academic_delivery.services.change_sets.remove_draft_resource",
    )
    .await?;
    let term_id: Uuid = DbSession::new(&mut *tx)
        .scalar(
            "academic_delivery.services.change_sets.remove_draft_resource",
            "SELECT academic_term_id FROM academic_term_change_sets WHERE id=$1",
        )
        .bind(change_set_id)
        .fetch_optional()
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบรุ่นเปิดสอนแบบร่าง".into()))?;
    require_writable_term(&mut tx, term_id, true).await?;
    let revision = require_draft_change_set_for_update(&mut tx, change_set_id).await?;
    if revision.row_version != request.change_set_row_version {
        return Err(AppError::Conflict(
            "รุ่นเปิดสอนเปลี่ยนไป กรุณาโหลดใหม่ก่อนลบจากรุ่นนี้".into(),
        ));
    }
    let version_id =
        required_version_id(revision.target_delivery_version_id, "ไม่พบรุ่นเปิดสอนแบบร่าง")?;
    let graph: sqlx::types::Json<DeliverySnapshot> = DbSession::new(&mut *tx)
        .scalar(
            "academic_delivery.services.change_sets.remove_draft_resource",
            "SELECT snapshot FROM academic_delivery_versions WHERE id=$1 AND status='draft'
            AND academic_term_id=$2 AND academic_year_id=$3 AND row_version=$4 FOR UPDATE",
        )
        .bind(version_id)
        .bind(term_id)
        .bind(revision.academic_year_id)
        .bind(request.target_delivery_version_row_version)
        .fetch_optional()
        .await?
        .ok_or_else(|| AppError::Conflict("รุ่นเปิดสอนเปลี่ยนไป กรุณาโหลดใหม่ก่อนลบจากรุ่นนี้".into()))?;
    let offering = graph
        .offerings
        .iter()
        .find(|offering| match group_id {
            Some(group) => offering.groups.iter().any(|entry| entry.id == group),
            None => offering.id == resource_id,
        })
        .ok_or_else(|| AppError::Conflict("รายการนี้ไม่ได้อยู่ในรุ่นเปิดสอนแบบร่างที่เลือก".into()))?;
    let offering_id = offering.id;
    if homeroom_id.is_some_and(|room| !offering.homeroom_ids.contains(&room)) {
        return Err(AppError::Conflict(
            "ห้องนี้ไม่ได้อยู่ในรายการเปิดสอนของรุ่นที่เลือก".into(),
        ));
    }
    let mut allowed_rooms = offering.homeroom_ids.clone();
    if let Some(room) = homeroom_id {
        allowed_rooms.retain(|id| *id != room);
    } else if let Some(group) = group_id {
        let central = matches!(&offering.catalog,LearningOfferingSnapshot::Activity(a) if a.scheduling_mode==ActivitySchedulingMode::Synchronized);
        if !central {
            let remaining: std::collections::HashSet<_> = offering
                .groups
                .iter()
                .filter(|g| g.id != group)
                .flat_map(|g| g.homeroom_ids.iter().copied())
                .collect();
            let removed = offering.groups.iter().find(|g| g.id == group);
            if let Some(removed) = removed {
                allowed_rooms.retain(|room| {
                    !removed.homeroom_ids.contains(room) || remaining.contains(room)
                });
            }
        }
    } else {
        allowed_rooms.clear();
    }

    let offering_exists: bool = DbSession::new(&mut *tx)
        .scalar(
            "academic_delivery.services.change_sets.remove_draft_resource",
            "SELECT true FROM learning_offerings WHERE id=$1 AND academic_term_id=$2
            AND academic_year_id=$3 FOR UPDATE",
        )
        .bind(offering_id)
        .bind(term_id)
        .bind(revision.academic_year_id)
        .fetch_optional()
        .await?
        .unwrap_or(false);
    if !offering_exists {
        return Err(AppError::Conflict(
            "ข้อมูลรายการเปิดสอนเปลี่ยนไป กรุณาโหลดใหม่".into(),
        ));
    }
    // Lock every affected live group in stable order before journal or child cleanup.
    let affected_groups: Vec<Uuid> = DbSession::new(&mut *tx)
        .scalar(
            "academic_delivery.services.change_sets.remove_draft_resource",
            "SELECT id FROM learning_groups WHERE learning_offering_id=$1
            AND ($2::uuid IS NULL OR id=$2) ORDER BY id FOR UPDATE",
        )
        .bind(offering_id)
        .bind(group_id)
        .fetch_all()
        .await?;
    if group_id.is_some_and(|group| !affected_groups.contains(&group)) {
        return Err(AppError::Conflict("ข้อมูลกลุ่มเรียนเปลี่ยนไป กรุณาโหลดใหม่".into()));
    }
    let mut removed_commands = if homeroom_id.is_some() {
        0
    } else {
        DbSession::new(&mut *tx)
            .query(
                "academic_delivery.services.change_sets.remove_draft_resource",
                "DELETE FROM academic_term_change_items WHERE change_set_id=$1
            AND (($2::uuid IS NULL AND learning_offering_id=$3) OR learning_group_id=ANY($4))",
            )
            .bind(change_set_id)
            .bind(group_id)
            .bind(offering_id)
            .bind(&affected_groups)
            .execute()
            .await
            .map_err(deletion_error)?
            .rows_affected()
    };
    if let Some(group) = group_id {
        DbSession::new(&mut *tx).query(
            "academic_delivery.services.change_sets.remove_draft_resource",
            "UPDATE academic_term_change_sets SET excluded_group_ids=array_append(excluded_group_ids,$2)
                WHERE id=$1 AND NOT ($2=ANY(excluded_group_ids))",
        ).bind(change_set_id).bind(group).execute().await?;
    } else if homeroom_id.is_none() {
        DbSession::new(&mut *tx).query(
            "academic_delivery.services.change_sets.remove_draft_resource",
            "UPDATE academic_term_change_sets SET excluded_offering_ids=array_append(excluded_offering_ids,$2)
                WHERE id=$1 AND NOT ($2=ANY(excluded_offering_ids))",
        ).bind(change_set_id).bind(offering_id).execute().await?;
    }
    super::super::audiences::set_revision_audience(
        &mut tx,
        change_set_id,
        offering_id,
        &allowed_rooms,
    )
    .await?;
    removed_commands +=
        super::super::audiences::clear_omitted_group_commands(&mut tx, change_set_id, offering_id)
            .await?;
    let (offering_status, downstream_count) =
        offering_deletion_evidence(&mut tx, offering_id).await?;
    let mut deleted_offering = false;
    let mut deleted_groups = 0;
    if downstream_count == 0 && homeroom_id.is_none() {
        if let Some(group) = group_id {
            let exclusive: bool = DbSession::new(&mut *tx).scalar(
                "academic_delivery.services.change_sets.remove_draft_resource",
                "SELECT g.status='draft' AND g.roster_status='draft'
                    AND NOT EXISTS(SELECT 1 FROM academic_delivery_versions v,
                        jsonb_array_elements(v.snapshot->'offerings') o,
                        jsonb_array_elements(o->'groups') item WHERE v.id<>$2 AND (item->>'id')::uuid=g.id)
                    AND NOT EXISTS(SELECT 1 FROM academic_term_change_items WHERE learning_group_id=g.id)
                    AND NOT EXISTS(SELECT 1 FROM learning_group_teachers WHERE learning_group_id=g.id
                        AND (started_by_change_set_id IS NOT NULL OR ended_by_change_set_id IS NOT NULL))
                    FROM learning_groups g WHERE g.id=$1",
            ).bind(group).bind(version_id).fetch_one().await?;
            if exclusive {
                deleted_groups = DbSession::new(&mut *tx).query(
                    "academic_delivery.services.change_sets.remove_draft_resource",
                    "DELETE FROM learning_groups WHERE id=$1 AND status='draft' AND roster_status='draft'",
                ).bind(group).execute().await.map_err(deletion_error)?.rows_affected();
            }
        } else if offering_status == LearningOfferingStatus::Draft
            && offering_is_exclusive(&mut tx, offering_id, version_id).await?
        {
            deleted_groups = DbSession::new(&mut *tx)
                .query(
                    "academic_delivery.services.change_sets.remove_draft_resource",
                    "DELETE FROM learning_groups WHERE learning_offering_id=$1",
                )
                .bind(offering_id)
                .execute()
                .await
                .map_err(deletion_error)?
                .rows_affected();
            deleted_offering = DbSession::new(&mut *tx)
                .query(
                    "academic_delivery.services.change_sets.remove_draft_resource",
                    "DELETE FROM learning_offerings WHERE id=$1 AND status='draft'",
                )
                .bind(offering_id)
                .execute()
                .await
                .map_err(deletion_error)?
                .rows_affected()
                == 1;
        }
    }
    increment_change_set_revision(&mut tx, change_set_id).await?;
    append_audit(
        &mut tx,
        "academic_delivery_draft_resource.removed",
        "academic_term_change_set",
        change_set_id,
        revision.academic_year_id,
        term_id,
        actor,
        serde_json::json!({
            "targetDeliveryVersionId":version_id, "learningOfferingId":offering_id,
            "learningGroupId":group_id, "homeroomId":homeroom_id, "allowedHomeroomIds":allowed_rooms, "removedCommandCount":removed_commands,
            "deletedOffering":deleted_offering, "deletedGroupCount":deleted_groups,
        }),
    )
    .await?;
    tx.commit().await?;
    get_change_set(pool, change_set_id).await
}

pub(super) async fn offering_is_exclusive(
    tx: &mut Transaction<'_, Postgres>,
    offering_id: Uuid,
    version_id: Uuid,
) -> Result<bool, AppError> {
    Ok(DbSession::new(&mut **tx).scalar(
        "academic_delivery.services.change_sets.offering_is_exclusive",
        "SELECT NOT EXISTS(SELECT 1 FROM academic_delivery_versions v,
            jsonb_array_elements(v.snapshot->'offerings') o WHERE v.id<>$2 AND (o->>'id')::uuid=$1)
            AND NOT EXISTS(SELECT 1 FROM academic_delivery_revision_audiences WHERE learning_offering_id=$1)
            AND NOT EXISTS(SELECT 1 FROM academic_term_change_items item
                LEFT JOIN learning_groups g ON g.id=item.learning_group_id
                WHERE item.learning_offering_id=$1 OR g.learning_offering_id=$1)
            AND NOT EXISTS(SELECT 1 FROM learning_groups WHERE learning_offering_id=$1
                AND (status<>'draft' OR roster_status<>'draft'))
            AND NOT EXISTS(SELECT 1 FROM learning_group_teachers t JOIN learning_groups g ON g.id=t.learning_group_id
                WHERE g.learning_offering_id=$1
                AND (t.started_by_change_set_id IS NOT NULL OR t.ended_by_change_set_id IS NOT NULL))",
    ).bind(offering_id).bind(version_id).fetch_one().await?)
}
