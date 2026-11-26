use super::{models::*, policy::*, repository::claim};
use crate::permission_codes_generated as codes;
use school_authorization::resource_access_policy::{
    eligible_active_staff_for_resource_on_connection, ResourceAccessPermissions,
    ResourceAccessTarget,
};
use school_authorization::ActorContext;
use school_database::DbSession;
use school_errors::AppError;
use sqlx::{types::Json, PgConnection, PgPool};
use std::collections::BTreeSet;
use uuid::Uuid;

const COMPANION_PERMISSIONS: &[&str] = &[
    codes::STAFF_LEAVE_READ_ASSIGNED,
    codes::STAFF_LEAVE_UPDATE_ASSIGNED,
];

async fn eligible_companions(
    connection: &mut PgConnection,
    ids: &[Uuid],
) -> Result<BTreeSet<Uuid>, AppError> {
    let access = ResourceAccessPermissions {
        assigned: Some(codes::STAFF_LEAVE_READ_ASSIGNED),
        ..Default::default()
    };
    let intended_assignments = ResourceAccessTarget {
        assigned_user_ids: ids.to_vec(),
        ..Default::default()
    };
    let eligible = eligible_active_staff_for_resource_on_connection(
        connection,
        ids,
        COMPANION_PERMISSIONS,
        access,
        &intended_assignments,
    )
    .await?;
    Ok(eligible.into_iter().collect())
}

async fn require_companion_permissions(
    connection: &mut PgConnection,
    actor: &ActorContext,
) -> Result<(), AppError> {
    if !actor.has_all_permissions(COMPANION_PERMISSIONS)
        || !eligible_companions(connection, &[actor.user_id])
            .await?
            .contains(&actor.user_id)
    {
        return Err(denied());
    }
    Ok(())
}

pub(super) async fn resolve_on_connection(
    connection: &mut PgConnection,
    owner: Uuid,
    kind: LeaveKind,
    ids: &[Uuid],
) -> Result<Vec<LeaveRoutingUser>, AppError> {
    if ids.is_empty() {
        return Ok(vec![]);
    }
    if ids.len() > MAX_COMPANIONS {
        return Err(invalid("เลือกผู้ร่วมเดินทางได้ไม่เกิน 100 คนต่อคำขอ"));
    }
    let unique: BTreeSet<_> = ids.iter().copied().collect();
    if kind == LeaveKind::Leave || unique.len() != ids.len() || unique.contains(&owner) {
        return Err(invalid(
            "ผู้ร่วมเดินทางต้องไม่ซ้ำกับผู้ยื่นหรือผู้ร่วมเดินทางคนอื่น และใช้ได้เฉพาะคำขอเดินทาง",
        ));
    }
    if eligible_companions(connection, ids).await? != unique {
        return Err(invalid(
            "กรุณาเลือกบุคลากรที่ปฏิบัติงานอยู่และมีสิทธิ์ทั้งดูและยืนยันคำขอที่ได้รับมอบหมาย",
        ));
    }
    let users: Vec<(Uuid, String)> = DbSession::new(connection)
        .query_as(
            "staff.leave.companions.resolve",
            "SELECT id, display_name FROM user_display_names WHERE id=ANY($1) ORDER BY id",
        )
        .bind(ids)
        .fetch_all()
        .await?;
    Ok(users
        .into_iter()
        .map(|(id, display_name)| LeaveRoutingUser { id, display_name })
        .collect())
}

pub async fn companion_options(
    pool: &PgPool,
    actor: &ActorContext,
    query: LeaveCompanionOptionsQuery,
) -> Result<Vec<LeaveRoutingUser>, AppError> {
    actor.require_permission(codes::STAFF_LEAVE_CREATE_OWN)?;
    let offset = query.offset.unwrap_or(0);
    if !(0..=10000).contains(&offset) {
        return Err(invalid("จุดเริ่มต้นของรายการไม่ถูกต้อง"));
    }
    let mut connection =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "staff.leave.companions.options")
            .await?;
    let candidates = school_staff::services::coverage_candidates_on_connection(
        &mut connection,
        query.search.as_deref(),
    )
    .await?;
    let mut skipped = 0;
    let mut page = Vec::new();
    for batch in candidates.chunks(250) {
        let ids: Vec<_> = batch
            .iter()
            .map(|(id, _)| *id)
            .filter(|id| *id != actor.user_id)
            .collect();
        let eligible = eligible_companions(&mut connection, &ids).await?;
        for (id, display_name) in batch {
            if eligible.contains(id) {
                if skipped < offset {
                    skipped += 1;
                } else {
                    page.push(LeaveRoutingUser {
                        id: *id,
                        display_name: display_name.clone(),
                    });
                    if page.len() == 50 {
                        return Ok(page);
                    }
                }
            }
        }
    }
    Ok(page)
}

pub(super) async fn statuses(
    connection: &mut PgConnection,
    request: &LeaveRequestSummary,
) -> Result<Vec<LeaveCompanionStatus>, AppError> {
    let confirmed: Vec<Uuid> = if let Some(revision) = request.companion_revision {
        DbSession::new(connection).scalar("staff.leave.companions.statuses",
            "SELECT user_id FROM staff_leave_companion_confirmations WHERE request_id=$1 AND submission_revision=$2")
            .bind(request.id).bind(revision).fetch_all().await?
    } else {
        vec![]
    };
    Ok(request
        .companions
        .iter()
        .map(|user| LeaveCompanionStatus {
            user_id: user.id,
            confirmed: confirmed.contains(&user.id),
        })
        .collect())
}

pub(super) async fn require_ready(
    connection: &mut PgConnection,
    request: &LeaveRequestSummary,
) -> Result<(), AppError> {
    let ids: Vec<_> = request.companions.iter().map(|u| u.id).collect();
    resolve_on_connection(connection, request.owner_id, request.kind, &ids).await?;
    if statuses(connection, request)
        .await?
        .iter()
        .any(|s| !s.confirmed)
    {
        return Err(invalid("รอผู้ร่วมเดินทางยืนยันครบทุกคนก่อนพิจารณาอนุมัติ"));
    }
    Ok(())
}

pub async fn companion_invitations(
    pool: &PgPool,
    actor: &ActorContext,
    query: LeaveCompanionInvitationsQuery,
) -> Result<Vec<LeaveCompanionInvitation>, AppError> {
    actor.require_permission(codes::STAFF_LEAVE_READ_ASSIGNED)?;
    actor.require_permission(codes::STAFF_LEAVE_UPDATE_ASSIGNED)?;
    let offset = query.offset.unwrap_or(0);
    if !(0..=10000).contains(&offset) {
        return Err(invalid("จุดเริ่มต้นของรายการไม่ถูกต้อง"));
    }
    let mut tx =
        school_tenancy::TenantPool::begin_observed(pool, "staff.leave.companions.invitations")
            .await?;
    require_companion_permissions(&mut tx, actor).await?;
    let rows: Vec<(Json<LeaveRequestSummary>, bool)> = DbSession::new(&mut *tx)
        .query_as(
            "staff.leave.companions.invitations",
            r#"
        SELECT r.document, c.confirmed_at IS NOT NULL FROM staff_leave_requests r
        LEFT JOIN staff_leave_companion_confirmations c ON c.request_id=r.id
            AND c.submission_revision=(r.document->>'companionRevision')::bigint AND c.user_id=$1
        WHERE r.status='submitted' AND r.document ? 'companions'
            AND (r.document->'companions') @> jsonb_build_array(jsonb_build_object('id',$1::uuid))
        ORDER BY (c.confirmed_at IS NOT NULL), r.created_at DESC, r.id LIMIT 50 OFFSET $2"#,
        )
        .bind(actor.user_id)
        .bind(offset)
        .fetch_all()
        .await?;
    tx.commit().await?;
    Ok(rows
        .into_iter()
        .map(|(Json(r), confirmed)| LeaveCompanionInvitation {
            request_id: r.id,
            revision: r.revision,
            kind: r.kind,
            owner_name: r.owner_name,
            destination: r.destination,
            start_date: r.start_date,
            end_date: r.end_date,
            start_time: r.start_time,
            end_time: r.end_time,
            confirmed,
        })
        .collect())
}

pub async fn confirm_companion(
    pool: &PgPool,
    actor: &ActorContext,
    id: Uuid,
    input: LeaveMutationRequest,
) -> Result<(), AppError> {
    actor.require_permission(codes::STAFF_LEAVE_READ_ASSIGNED)?;
    actor.require_permission(codes::STAFF_LEAVE_UPDATE_ASSIGNED)?;
    let mut tx =
        school_tenancy::TenantPool::begin_observed(pool, "staff.leave.companions.confirm").await?;
    require_companion_permissions(&mut tx, actor).await?;
    if claim(
        &mut tx,
        actor.user_id,
        input.request_id,
        "companion_confirm",
        id,
        &(id, &input),
    )
    .await?
    .is_some()
    {
        tx.commit().await?;
        return Ok(());
    }
    let request: Json<LeaveRequestSummary> = DbSession::new(&mut *tx)
        .scalar("staff.leave.companions.confirm", r#"
            SELECT document FROM staff_leave_requests WHERE id=$1 AND status='submitted'
            AND document ? 'companions' AND document->'companions' @> jsonb_build_array(jsonb_build_object('id',$2::uuid))
            FOR UPDATE"#)
        .bind(id).bind(actor.user_id).fetch_optional().await?.ok_or_else(denied)?;
    if request.revision != input.expected_revision {
        return Err(stale());
    }
    let revision = request.companion_revision.ok_or_else(stale)?;
    DbSession::new(&mut *tx)
        .query(
            "staff.leave.companions.confirm",
            r#"
        INSERT INTO staff_leave_companion_confirmations(request_id,submission_revision,user_id)
        VALUES ($1,$2,$3) ON CONFLICT DO NOTHING"#,
        )
        .bind(id)
        .bind(revision)
        .bind(actor.user_id)
        .execute()
        .await?;
    tx.commit().await?;
    Ok(())
}
