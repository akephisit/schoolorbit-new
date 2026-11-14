//! Exact current offering/group cleanup. Application composition owns transaction order.
use crate::collaboration::CommitPermit;
use school_academic_delivery::models::current::DeliveryDeletionScope;
use school_database::DbSession;
use school_errors::AppError;
use sqlx::{PgConnection, Postgres, Transaction};
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TimetableDeletionImpact {
    pub timetable_id: Option<Uuid>,
    pub blocks: i64,
    pub block_groups: i64,
    pub source_digest: String,
}

pub async fn deletion_impact_on_connection(
    connection: &mut PgConnection,
    scope: &DeliveryDeletionScope,
) -> Result<TimetableDeletionImpact, AppError> {
    Ok(DbSession::new(connection).query_as("academic_timetable.deletion.impact", r#"
        WITH affected AS (
          SELECT block.* FROM academic_timetable_blocks block
          WHERE block.academic_year_id=$1 AND block.academic_term_id=$2 AND block.learning_offering_id=$3
            AND ($5 OR EXISTS(SELECT 1 FROM academic_timetable_block_groups child WHERE child.block_id=block.id AND child.learning_group_id=ANY($4))
              OR EXISTS(SELECT 1 FROM academic_timetable_block_group_sync child WHERE child.block_id=block.id AND child.learning_group_id=ANY($4)))
        ), evidence AS (
          SELECT 'block' kind,b.id,to_jsonb(b) value FROM affected b
          UNION ALL SELECT 'group',g.id,to_jsonb(g) FROM academic_timetable_block_groups g JOIN affected b ON b.id=g.block_id
          UNION ALL SELECT 'instructor',i.id,to_jsonb(i) FROM academic_timetable_block_group_instructors i JOIN academic_timetable_block_groups g ON g.id=i.block_group_id JOIN affected b ON b.id=g.block_id
          UNION ALL SELECT 'sync',s.id,to_jsonb(s) FROM academic_timetable_block_group_sync s JOIN affected b ON b.id=s.block_id
          UNION ALL SELECT 'homeroom',h.id,to_jsonb(h) FROM academic_timetable_block_homerooms h JOIN affected b ON b.id=h.block_id
          UNION ALL SELECT 'teacher',t.id,to_jsonb(t) FROM academic_timetable_block_teachers t JOIN affected b ON b.id=t.block_id
        )
        SELECT (SELECT id FROM academic_timetables WHERE academic_year_id=$1 AND academic_term_id=$2) timetable_id,
          (SELECT count(*) FROM affected) blocks,
          (SELECT count(*) FROM academic_timetable_block_groups g JOIN affected b ON b.id=g.block_id WHERE $5 OR g.learning_group_id=ANY($4)) block_groups,
          encode(sha256(convert_to(COALESCE((SELECT jsonb_agg(value ORDER BY kind,id) FROM evidence),'[]')::text,'UTF8')),'hex') source_digest
    "#).bind(scope.academic_year_id).bind(scope.academic_term_id).bind(scope.learning_offering_id).bind(&scope.learning_group_ids).bind(scope.delete_offering).fetch_one().await?)
}

/// Acquire the room gate and complete whole-board commit footprint before taking
/// the shared transaction's lifecycle locks; retain the permit through actual COMMIT.
/// Supervision must detach its saved references before this operation.
pub async fn delete_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    scope: &DeliveryDeletionScope,
    permit: Option<&CommitPermit>,
) -> Result<(), AppError> {
    let current =
        super::timetable_service::get_for_term_on_connection(&mut **tx, scope.academic_term_id)
            .await?;
    let Some(current) = current else {
        return Ok(());
    };
    if current.academic_year_id != scope.academic_year_id {
        return Err(AppError::Conflict("บริบทตารางเรียนไม่ตรงกับขอบเขตลบ".into()));
    }
    permit
        .ok_or_else(|| AppError::Conflict("การลบต้องถือสิทธิ์เขียนตารางเรียนจนกว่าธุรกรรมจะสิ้นสุด".into()))?
        .require_timetable(current.id)?;
    DbSession::new(&mut **tx)
        .query(
            "academic_timetable.deletion.lock",
            "SELECT id FROM academic_timetables WHERE id=$1 FOR UPDATE",
        )
        .bind(current.id)
        .execute()
        .await?;
    let block_ids: Vec<Uuid> = DbSession::new(&mut **tx).scalar("academic_timetable.deletion.exact_blocks", "SELECT block.id FROM academic_timetable_blocks block WHERE block.timetable_id=$1 AND block.learning_offering_id=$2 AND ($4 OR EXISTS(SELECT 1 FROM academic_timetable_block_groups child WHERE child.block_id=block.id AND child.learning_group_id=ANY($3)) OR EXISTS(SELECT 1 FROM academic_timetable_block_group_sync child WHERE child.block_id=block.id AND child.learning_group_id=ANY($3))) ORDER BY block.id FOR UPDATE").bind(current.id).bind(scope.learning_offering_id).bind(&scope.learning_group_ids).bind(scope.delete_offering).fetch_all().await?;
    // Sync has a restrictive FK to its linked allocation; remove exact scope first.
    DbSession::new(&mut **tx).query("academic_timetable.deletion.sync", "DELETE FROM academic_timetable_block_group_sync WHERE academic_year_id=$1 AND academic_term_id=$2 AND learning_offering_id=$3 AND ($5 OR learning_group_id=ANY($4))").bind(scope.academic_year_id).bind(scope.academic_term_id).bind(scope.learning_offering_id).bind(&scope.learning_group_ids).bind(scope.delete_offering).execute().await?;
    DbSession::new(&mut **tx).query("academic_timetable.deletion.groups", "DELETE FROM academic_timetable_block_groups WHERE academic_year_id=$1 AND academic_term_id=$2 AND learning_offering_id=$3 AND ($5 OR learning_group_id=ANY($4))").bind(scope.academic_year_id).bind(scope.academic_term_id).bind(scope.learning_offering_id).bind(&scope.learning_group_ids).bind(scope.delete_offering).execute().await?;
    DbSession::new(&mut **tx).query("academic_timetable.deletion.blocks", r#"
        DELETE FROM academic_timetable_blocks block
        WHERE block.id=ANY($1) AND block.learning_offering_id=$2
          AND ($3 OR (block.scheduling_mode IS DISTINCT FROM 'synchronized'
            AND NOT EXISTS(SELECT 1 FROM academic_timetable_block_groups child WHERE child.block_id=block.id)))
    "#).bind(&block_ids).bind(scope.learning_offering_id).bind(scope.delete_offering).execute().await?;
    Ok(())
}

/// Physically remove current placements. The returned tombstones exist only in
/// the accepted HTTP result and bounded process-local collaboration journal.
pub(crate) async fn remove_blocks_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    permit: &CommitPermit,
    block_ids: &[Uuid],
) -> Result<Vec<crate::models::timetable_block::TimetableBlock>, AppError> {
    let mut blocks =
        super::timetable_block_queries::get_blocks_on_connection(&mut **tx, block_ids).await?;
    for block in &blocks {
        permit.require_timetable(block.timetable_id)?;
    }
    let groups: Vec<Uuid> = DbSession::new(&mut **tx)
        .scalar(
            "academic_timetable.removal.block_groups",
            "SELECT id FROM academic_timetable_block_groups WHERE block_id=ANY($1) ORDER BY id",
        )
        .bind(block_ids)
        .fetch_all()
        .await?;
    permit.detach_timetable_sources(tx, &groups).await?;
    DbSession::new(&mut **tx)
        .query(
            "academic_timetable.removal.block_sync",
            "DELETE FROM academic_timetable_block_group_sync WHERE block_id=ANY($1)",
        )
        .bind(block_ids)
        .execute()
        .await?;
    DbSession::new(&mut **tx)
        .query(
            "academic_timetable.removal.blocks",
            "DELETE FROM academic_timetable_blocks WHERE id=ANY($1)",
        )
        .bind(block_ids)
        .execute()
        .await?;
    for block in &mut blocks {
        block.is_active = false;
        block.row_version += 1;
        block.updated_at = chrono::Utc::now();
    }
    Ok(blocks)
}
pub(crate) async fn remove_groups_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    permit: &CommitPermit,
    block_group_ids: &[Uuid],
) -> Result<(), AppError> {
    if block_group_ids.is_empty() {
        return Ok(());
    }
    let timetables:Vec<Uuid>=DbSession::new(&mut **tx).scalar("academic_timetable.removal.require_group_context", "SELECT DISTINCT block.timetable_id FROM academic_timetable_block_groups child JOIN academic_timetable_blocks block ON block.id=child.block_id WHERE child.id=ANY($1)").bind(block_group_ids).fetch_all().await?;
    for timetable in timetables {
        permit.require_timetable(timetable)?;
    }
    permit.detach_timetable_sources(tx, block_group_ids).await?;
    DbSession::new(&mut **tx).query("academic_timetable.removal.unlink_group_sync", "UPDATE academic_timetable_block_group_sync SET linked_block_group_id=NULL,status='WAITING_FOR_DATA',conflict_code=NULL,conflict_message=NULL,row_version=row_version+1,updated_at=now() WHERE linked_block_group_id=ANY($1)").bind(block_group_ids).execute().await?;
    DbSession::new(&mut **tx)
        .query(
            "academic_timetable.removal.groups",
            "DELETE FROM academic_timetable_block_groups WHERE id=ANY($1)",
        )
        .bind(block_group_ids)
        .execute()
        .await?;
    Ok(())
}
