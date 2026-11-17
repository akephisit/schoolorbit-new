//! Preserve every saved period while detaching only removed operational references.
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
    Ok(DbSession::new(connection).query_as("supervision.current_source.deletion_impact",r#"
        SELECT count(DISTINCT o.id) observations,
            encode(sha256(convert_to(COALESCE(jsonb_agg(jsonb_build_array(to_jsonb(o),to_jsonb(p))
                ORDER BY o.id,p.position,p.id),'[]')::text,'UTF8')),'hex') source_digest
        FROM supervision_observations o JOIN supervision_observation_periods p ON p.observation_id=o.id
        WHERE o.academic_year_id=$1 AND o.academic_term_id=$2
          AND (p.learning_group_id=ANY($3) OR ($5 AND p.learning_offering_id=$4))
    "#).bind(scope.academic_year_id).bind(scope.academic_term_id).bind(&scope.learning_group_ids)
        .bind(scope.learning_offering_id).bind(scope.delete_offering).fetch_one().await?)
}

/// Returns distinct affected observations, even when several of their periods detach.
pub async fn detach_deleted_source_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    scope: &DeliveryDeletionScope,
) -> Result<u64, AppError> {
    DbSession::new(&mut **tx).query("supervision.current_source.lock_observations",r#"
        SELECT o.id FROM supervision_observations o WHERE o.academic_year_id=$1 AND o.academic_term_id=$2
          AND EXISTS(SELECT 1 FROM supervision_observation_periods p WHERE p.observation_id=o.id
            AND (p.learning_group_id=ANY($3) OR ($5 AND p.learning_offering_id=$4)))
        ORDER BY o.id FOR UPDATE
    "#).bind(scope.academic_year_id).bind(scope.academic_term_id).bind(&scope.learning_group_ids)
        .bind(scope.learning_offering_id).bind(scope.delete_offering).execute().await?;
    let missing: bool = DbSession::new(&mut **tx)
        .scalar(
            "supervision.current_source.require_saved_evidence",
            r#"
        SELECT EXISTS(SELECT 1 FROM supervision_observation_periods p
            JOIN supervision_observations o ON o.id=p.observation_id
            WHERE o.academic_year_id=$1 AND o.academic_term_id=$2
              AND (p.learning_group_id=ANY($3) OR ($5 AND p.learning_offering_id=$4))
              AND p.lesson_snapshot='{}'::jsonb AND p.timetable_block_group_id IS NULL
              AND NOT(p.manual_subject_name IS NOT NULL AND p.manual_classroom_label IS NOT NULL
                  AND p.manual_period_label IS NOT NULL AND p.manual_reason IS NOT NULL))
    "#,
        )
        .bind(scope.academic_year_id)
        .bind(scope.academic_term_id)
        .bind(&scope.learning_group_ids)
        .bind(scope.learning_offering_id)
        .bind(scope.delete_offering)
        .fetch_one()
        .await?;
    if missing {
        return Err(AppError::Conflict(
            "ข้อมูลนิเทศไม่มีหลักฐานคาบที่บันทึกไว้ จึงยังลบแหล่งอ้างอิงไม่ได้".into(),
        ));
    }
    let count: i64 = DbSession::new(&mut **tx)
        .scalar(
            "supervision.current_source.detach",
            r#"
        WITH affected AS (
            UPDATE supervision_observation_periods p SET
                lesson_snapshot=CASE WHEN p.lesson_snapshot='{}'::jsonb AND p.timetable_block_group_id IS NOT NULL
                    THEN jsonb_build_object('source','removed_timetable','timetableBlockGroupId',p.timetable_block_group_id,'observedAt',p.observed_at)
                    ELSE p.lesson_snapshot END,
                timetable_block_group_id=NULL,
                learning_group_id=NULL,learning_offering_id=NULL,updated_at=clock_timestamp()
            FROM supervision_observations o
            WHERE o.id=p.observation_id AND o.academic_year_id=$1 AND o.academic_term_id=$2
              AND (p.learning_group_id=ANY($3) OR ($5 AND p.learning_offering_id=$4))
            RETURNING p.observation_id
        ) SELECT count(DISTINCT observation_id) FROM affected
    "#,
        )
        .bind(scope.academic_year_id)
        .bind(scope.academic_term_id)
        .bind(&scope.learning_group_ids)
        .bind(scope.learning_offering_id)
        .bind(scope.delete_offering)
        .fetch_one()
        .await?;
    u64::try_from(count).map_err(|_| AppError::InternalServerError("จำนวนรายการนิเทศไม่ถูกต้อง".into()))
}

/// Remove placement pointers independently; saved evidence and other periods remain.
/// The return value retains its established distinct-observation meaning.
pub async fn detach_timetable_sources_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    ids: &[uuid::Uuid],
) -> Result<u64, AppError> {
    if ids.is_empty() {
        return Ok(0);
    }
    DbSession::new(&mut **tx).query("supervision.current_source.lock_exact_observations",
        "SELECT o.id FROM supervision_observations o WHERE EXISTS(SELECT 1 FROM supervision_observation_periods p
         WHERE p.observation_id=o.id AND p.timetable_block_group_id=ANY($1)) ORDER BY o.id FOR UPDATE")
        .bind(ids).execute().await?;
    let count:i64=DbSession::new(&mut **tx).scalar("supervision.current_source.detach_exact_placements",r#"
        WITH affected AS (
            UPDATE supervision_observation_periods SET
                lesson_snapshot=CASE WHEN lesson_snapshot='{}'::jsonb AND timetable_block_group_id IS NOT NULL
                    THEN jsonb_build_object('source','removed_timetable','timetableBlockGroupId',timetable_block_group_id,'observedAt',observed_at)
                    ELSE lesson_snapshot END,
                timetable_block_group_id=NULL,updated_at=clock_timestamp()
            WHERE timetable_block_group_id=ANY($1) RETURNING observation_id
        ) SELECT count(DISTINCT observation_id) FROM affected
    "#).bind(ids).fetch_one().await?;
    u64::try_from(count).map_err(|_| AppError::InternalServerError("จำนวนรายการนิเทศไม่ถูกต้อง".into()))
}
