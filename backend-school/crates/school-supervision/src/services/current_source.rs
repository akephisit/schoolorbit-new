//! Preserve saved lesson evidence while detaching removed operational references.
use school_academic_delivery::models::current::DeliveryDeletionScope;
use school_database::DbSession;
use school_errors::AppError;
use sqlx::{PgConnection, Postgres, Transaction};

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SupervisionDeletionImpact {
    pub observations: i64,
    pub source_digest: String,
}

pub async fn deletion_impact_on_connection(
    connection: &mut PgConnection,
    scope: &DeliveryDeletionScope,
) -> Result<SupervisionDeletionImpact, AppError> {
    Ok(DbSession::new(connection).query_as("supervision.current_source.deletion_impact", r#"
        SELECT count(*) observations,
            encode(sha256(convert_to(COALESCE(jsonb_agg(to_jsonb(observation) ORDER BY observation.id),'[]')::text,'UTF8')),'hex') source_digest
        FROM supervision_observations observation
        WHERE observation.academic_year_id=$1 AND observation.academic_term_id=$2
          AND (observation.learning_group_id=ANY($3) OR EXISTS(
            SELECT 1 FROM academic_timetable_block_groups placed
            WHERE placed.id=observation.timetable_block_group_id AND placed.learning_offering_id=$4
              AND ($5 OR placed.learning_group_id=ANY($3))))
    "#).bind(scope.academic_year_id).bind(scope.academic_term_id).bind(&scope.learning_group_ids).bind(scope.learning_offering_id).bind(scope.delete_offering).fetch_one().await?)
}

/// Called before Timetable and Delivery remove their rows, on the same transaction.
/// Lesson snapshots, evaluations, status, actions and manual reasons are preserved.
pub async fn detach_deleted_source_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    scope: &DeliveryDeletionScope,
) -> Result<u64, AppError> {
    let placements:Vec<uuid::Uuid>=DbSession::new(&mut **tx).scalar("supervision.current_source.exact_deleted_placements", "SELECT DISTINCT observation.timetable_block_group_id FROM supervision_observations observation WHERE observation.academic_year_id=$1 AND observation.academic_term_id=$2 AND observation.timetable_block_group_id IS NOT NULL AND (observation.learning_group_id=ANY($3) OR EXISTS(SELECT 1 FROM academic_timetable_block_groups placed WHERE placed.id=observation.timetable_block_group_id AND placed.learning_offering_id=$4 AND ($5 OR placed.learning_group_id=ANY($3)))) ORDER BY observation.timetable_block_group_id").bind(scope.academic_year_id).bind(scope.academic_term_id).bind(&scope.learning_group_ids).bind(scope.learning_offering_id).bind(scope.delete_offering).fetch_all().await?;
    detach_timetable_sources_in_transaction(tx, &placements).await?;
    let missing: bool = DbSession::new(&mut **tx).scalar("supervision.current_source.require_saved_evidence", r#"
        SELECT EXISTS(SELECT 1 FROM supervision_observations observation
        WHERE observation.academic_year_id=$1 AND observation.academic_term_id=$2
          AND (observation.learning_group_id=ANY($3) OR EXISTS(SELECT 1 FROM academic_timetable_block_groups placed WHERE placed.id=observation.timetable_block_group_id AND placed.learning_offering_id=$4 AND ($5 OR placed.learning_group_id=ANY($3))))
          AND (jsonb_typeof(observation.lesson_snapshot) IS DISTINCT FROM 'object' OR observation.lesson_snapshot='{}'::jsonb)
          AND NOT(observation.manual_subject_name IS NOT NULL AND observation.manual_classroom_label IS NOT NULL AND observation.manual_period_label IS NOT NULL AND observation.manual_reason IS NOT NULL))
    "#).bind(scope.academic_year_id).bind(scope.academic_term_id).bind(&scope.learning_group_ids).bind(scope.learning_offering_id).bind(scope.delete_offering).fetch_one().await?;
    if missing {
        return Err(AppError::Conflict(
            "ข้อมูลนิเทศไม่มีหลักฐานคาบที่บันทึกไว้ จึงยังลบแหล่งอ้างอิงไม่ได้".into(),
        ));
    }
    Ok(DbSession::new(&mut **tx).query("supervision.current_source.detach", r#"
        UPDATE supervision_observations observation SET timetable_block_group_id=NULL,learning_group_id=NULL,updated_at=now()
        WHERE observation.academic_year_id=$1 AND observation.academic_term_id=$2
          AND (observation.learning_group_id=ANY($3) OR EXISTS(SELECT 1 FROM academic_timetable_block_groups placed WHERE placed.id=observation.timetable_block_group_id AND placed.learning_offering_id=$4 AND ($5 OR placed.learning_group_id=ANY($3))))
    "#).bind(scope.academic_year_id).bind(scope.academic_term_id).bind(&scope.learning_group_ids).bind(scope.learning_offering_id).bind(scope.delete_offering).execute().await?.rows_affected())
}

/// Remove only the operational placement pointer. Actual lesson evidence and
/// the still-current learning group remain independently owned facts.
pub async fn detach_timetable_sources_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    block_group_ids: &[uuid::Uuid],
) -> Result<u64, AppError> {
    if block_group_ids.is_empty() {
        return Ok(0);
    }
    let missing:bool=DbSession::new(&mut **tx).scalar("supervision.current_source.require_exact_lesson_evidence", r#"
        SELECT EXISTS(SELECT 1 FROM supervision_observations observation
        WHERE timetable_block_group_id=ANY($1)
          AND (jsonb_typeof(lesson_snapshot) IS DISTINCT FROM 'object' OR (lesson_snapshot='{}'::jsonb
          AND observed_at IS NULL
          AND NOT(manual_subject_name IS NOT NULL AND manual_classroom_label IS NOT NULL AND manual_period_label IS NOT NULL AND manual_reason IS NOT NULL))))
    "#).bind(block_group_ids).fetch_one().await?;
    if missing {
        return Err(AppError::Conflict(
            "SUPERVISION_LESSON_EVIDENCE_UNMAPPABLE".into(),
        ));
    }
    Ok(DbSession::new(&mut **tx).query("supervision.current_source.detach_exact_placements", r#"
        UPDATE supervision_observations SET
          lesson_snapshot=CASE WHEN lesson_snapshot='{}'::jsonb AND observed_at IS NOT NULL
            THEN jsonb_build_object('source','removed_timetable','timetableBlockGroupId',timetable_block_group_id,'observedAt',observed_at)
            ELSE lesson_snapshot END,
          timetable_block_group_id=NULL,updated_at=now()
        WHERE timetable_block_group_id=ANY($1)
    "#).bind(block_group_ids).execute().await?.rows_affected())
}
