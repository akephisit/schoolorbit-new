use crate::{models::current::*, ports::DeliveryDeletionPort};
use school_authorization::ActorContext;
use school_database::DbSession;
use school_errors::AppError;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

pub fn require_school_management(actor: &ActorContext) -> Result<(), AppError> {
    if actor.permissions.iter().any(|permission| {
        permission == school_permissions::registry::codes::LEARNING_OFFERING_MANAGE_SCHOOL
    }) {
        Ok(())
    } else {
        Err(AppError::Forbidden(
            "การลบรายการเปิดสอนต้องมีสิทธิ์จัดการทั้งโรงเรียน".into(),
        ))
    }
}

async fn scope(
    connection: &mut PgConnection,
    id: Uuid,
    year: Uuid,
    term: Uuid,
    group_id: Option<Uuid>,
) -> Result<Option<(DeliveryDeletionScope, i64)>, AppError> {
    let revision: Option<i64> = DbSession::new(connection).scalar("academic_delivery.deletion.scope", "SELECT row_version FROM learning_offerings WHERE id=$1 AND academic_year_id=$2 AND academic_term_id=$3").bind(id).bind(year).bind(term).fetch_optional().await?;
    let Some(revision) = revision else {
        return Ok(None);
    };
    let revision = if let Some(group) = group_id {
        DbSession::new(connection).scalar("academic_delivery.deletion.group_revision", "SELECT row_version FROM learning_groups WHERE id=$1 AND learning_offering_id=$2 AND academic_year_id=$3 AND academic_term_id=$4").bind(group).bind(id).bind(year).bind(term).fetch_optional().await?.ok_or_else(||AppError::NotFound("ไม่พบกลุ่มเรียนในรายการเปิดสอนที่เลือก".into()))?
    } else {
        revision
    };
    let mut groups: Vec<Uuid> = DbSession::new(connection).scalar("academic_delivery.deletion.groups", "SELECT id FROM learning_groups WHERE learning_offering_id=$1 AND academic_year_id=$2 AND academic_term_id=$3 ORDER BY id").bind(id).bind(year).bind(term).fetch_all().await?;
    if let Some(group) = group_id {
        groups.retain(|id| *id == group);
    }
    Ok(Some((
        DeliveryDeletionScope {
            delete_offering: group_id.is_none(),
            learning_offering_id: id,
            academic_year_id: year,
            academic_term_id: term,
            learning_group_ids: groups,
        },
        revision,
    )))
}

async fn preview_on_connection(
    port: &dyn DeliveryDeletionPort,
    connection: &mut PgConnection,
    scope: &DeliveryDeletionScope,
    revision: i64,
) -> Result<DeliveryDeletionPreview, AppError> {
    let mut dependencies = port.preview(connection, scope).await?;
    let (teachers,members): (i64,i64)=DbSession::new(connection).query_as("academic_delivery.deletion.own_counts", "SELECT (SELECT count(*) FROM learning_group_teachers WHERE learning_group_id=ANY($1)),(SELECT count(*) FROM learning_group_students WHERE learning_group_id=ANY($1))").bind(&scope.learning_group_ids).fetch_one().await?;
    dependencies.impact.learning_groups = scope.learning_group_ids.len() as i64;
    dependencies.impact.teacher_assignments = teachers;
    dependencies.impact.memberships = members;
    let live = super::current::workspace_on_connection(
        connection,
        scope.academic_year_id,
        scope.academic_term_id,
    )
    .await?;
    let offering = live
        .offerings
        .iter()
        .find(|offering| offering.id == scope.learning_offering_id)
        .ok_or_else(|| AppError::Conflict("รายการเปิดสอนเปลี่ยนไป กรุณาโหลดใหม่".into()))?;
    let own_digest: String=DbSession::new(connection).scalar("academic_delivery.deletion.owned_digest", "SELECT md5(COALESCE((SELECT jsonb_agg(to_jsonb(member) ORDER BY member.id)::text FROM learning_group_students member WHERE member.learning_group_id=ANY($1)),'[]') || COALESCE((SELECT jsonb_agg(to_jsonb(config) ORDER BY config.learning_group_id)::text FROM learning_group_roster_tracking config WHERE config.learning_group_id=ANY($1)),'[]'))").bind(&scope.learning_group_ids).fetch_one().await?;
    let source_digest = super::stable_hash(&(
        scope,
        revision,
        live.row_version,
        offering,
        own_digest,
        &dependencies,
    ))?;
    Ok(DeliveryDeletionPreview {
        learning_group_id: (!scope.delete_offering).then(|| scope.learning_group_ids[0]),
        academic_year_id: scope.academic_year_id,
        academic_term_id: scope.academic_term_id,
        learning_offering_id: scope.learning_offering_id,
        row_version: revision,
        source_digest,
        impact: dependencies.impact,
    })
}

pub async fn preview(
    port: &dyn DeliveryDeletionPort,
    pool: &PgPool,
    actor: &ActorContext,
    id: Uuid,
    year: Uuid,
    term: Uuid,
    group_id: Option<Uuid>,
) -> Result<DeliveryDeletionPreview, AppError> {
    require_school_management(actor)?;
    let mut tx =
        school_tenancy::TenantPool::begin_observed(pool, "academic_delivery.deletion.preview")
            .await?;
    DbSession::new(&mut *tx)
        .query(
            "academic_delivery.deletion.preview",
            "SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY",
        )
        .execute()
        .await?;
    let (scope, revision) = scope(&mut tx, id, year, term, group_id)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบรายการเปิดสอนในภาคเรียนที่เลือก".into()))?;
    let result = preview_on_connection(port, &mut tx, &scope, revision).await?;
    tx.commit().await?;
    Ok(result)
}

pub async fn delete(
    port: &dyn DeliveryDeletionPort,
    pool: &PgPool,
    actor: &ActorContext,
    id: Uuid,
    group_id: Option<Uuid>,
    request: DeleteDeliveryResourceRequest,
) -> Result<DeletedDeliveryResource, AppError> {
    require_school_management(actor)?;
    super::validate_row_version(request.row_version)?;
    if request.source_digest.len() != 64
        || !request
            .source_digest
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(AppError::ValidationError("sourceDigest ไม่ถูกต้อง".into()));
    }
    let mut tx =
        school_tenancy::TenantPool::begin_observed(pool, "academic_delivery.deletion.delete")
            .await?;
    school_academic_core::services::lifecycle_guard::require_term_write_exclusive(
        &mut tx,
        request.academic_year_id,
        request.academic_term_id,
    )
    .await?;
    let Some((scope, revision)) = scope(
        &mut tx,
        id,
        request.academic_year_id,
        request.academic_term_id,
        group_id,
    )
    .await?
    else {
        let revision = super::current::term_revision_on_connection(
            &mut tx,
            request.academic_year_id,
            request.academic_term_id,
        )
        .await?;
        tx.commit().await?;
        return Ok(DeletedDeliveryResource {
            learning_group_id: group_id,
            learning_offering_id: id,
            academic_year_id: request.academic_year_id,
            academic_term_id: request.academic_term_id,
            deleted: false,
            delivery_row_version: revision,
        });
    };
    DbSession::new(&mut *tx)
        .query(
            "academic_delivery.deletion.lock_offering",
            "SELECT id FROM learning_offerings WHERE id=$1 FOR UPDATE",
        )
        .bind(id)
        .execute()
        .await?;
    DbSession::new(&mut *tx)
        .query(
            "academic_delivery.deletion.lock_groups",
            "SELECT id FROM learning_groups WHERE id=ANY($1) ORDER BY id FOR UPDATE",
        )
        .bind(&scope.learning_group_ids)
        .execute()
        .await?;
    let accepted = preview_on_connection(port, &mut tx, &scope, revision).await?;
    if revision != request.row_version || accepted.source_digest != request.source_digest {
        return Err(AppError::Conflict(
            "ข้อมูลหรือผลกระทบการลบเปลี่ยนไป กรุณาตรวจสอบใหม่".into(),
        ));
    }
    port.delete(&mut tx, &scope, actor.user_id).await?;
    for table in [
        "learning_group_students",
        "learning_group_teachers",
        "learning_group_homerooms",
        "learning_group_preferred_rooms",
        "learning_group_roster_tracking",
    ] {
        let query = format!("DELETE FROM {table} WHERE learning_group_id=ANY($1)");
        DbSession::new(&mut *tx)
            .query(
                "academic_delivery.deletion.remove_group_children",
                sqlx::AssertSqlSafe(query),
            )
            .bind(&scope.learning_group_ids)
            .execute()
            .await?;
    }
    DbSession::new(&mut *tx)
        .query(
            "academic_delivery.deletion.remove_groups",
            "DELETE FROM learning_groups WHERE id=ANY($1) AND learning_offering_id=$2",
        )
        .bind(&scope.learning_group_ids)
        .bind(id)
        .execute()
        .await?;
    if scope.delete_offering {
        for table in [
            "learning_offering_targets",
            "learning_offering_curriculum_sources",
            "course_offering_details",
            "activity_offering_details",
        ] {
            let query = format!("DELETE FROM {table} WHERE learning_offering_id=$1");
            DbSession::new(&mut *tx)
                .query(
                    "academic_delivery.deletion.remove_offering_children",
                    sqlx::AssertSqlSafe(query),
                )
                .bind(id)
                .execute()
                .await?;
        }
        DbSession::new(&mut *tx)
            .query(
                "academic_delivery.deletion.remove_offering",
                "DELETE FROM learning_offerings WHERE id=$1",
            )
            .bind(id)
            .execute()
            .await?;
    }
    let revision = super::current::term_revision_on_connection(
        &mut tx,
        scope.academic_year_id,
        scope.academic_term_id,
    )
    .await?;
    tx.commit().await?;
    Ok(DeletedDeliveryResource {
        learning_group_id: group_id,
        learning_offering_id: id,
        academic_year_id: scope.academic_year_id,
        academic_term_id: scope.academic_term_id,
        deleted: true,
        delivery_row_version: revision,
    })
}
