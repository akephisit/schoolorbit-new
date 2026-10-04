use std::collections::HashMap;

use chrono::{NaiveDate, Utc};
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;

use super::timetable_block_conflicts::map_write_error;
use crate::models::timetable_version::{
    CloneTimetableVersionRequest, CreateTimetableVersionRequest, TimetableVersion,
    TimetableVersionDisplayState, TimetableVersionStatus, TimetableVersionTarget,
};
use school_academic_core::services::lifecycle_guard;
use school_errors::AppError;

pub mod opening;
mod term_preparation;
pub use term_preparation::apply as apply_term_preparation;

pub async fn pending_term_work(
    tx: &mut Transaction<'_, Postgres>,
    year: Uuid,
    term: Uuid,
) -> Result<Vec<school_academic_core::ports::PendingTermWork>, AppError> {
    Ok(sqlx::query_as(
        "SELECT version.id,
         md5(to_jsonb(version)::text || COALESCE((SELECT jsonb_agg(to_jsonb(block) ORDER BY block.id)::text
             FROM academic_timetable_blocks block WHERE block.timetable_version_id=version.id),'[]')) AS revision,
         false AS blocks_closure FROM academic_timetable_versions version WHERE academic_year_id=$1
         AND academic_term_id=$2 AND status='draft' ORDER BY version.id",
    ).bind(year).bind(term).fetch_all(&mut **tx).await?)
}

#[derive(Debug, Clone, FromRow)]
struct TimetableVersionRow {
    id: Uuid,
    academic_term_id: Uuid,
    academic_year_id: Uuid,
    effective_from: Option<NaiveDate>,
    effective_until: Option<NaiveDate>,
    status: TimetableVersionStatus,
    source_version_id: Option<Uuid>,
    delivery_version_id: Uuid,
    bell_schedule_id: Uuid,
    row_version: i64,
    created_by: Option<Uuid>,
    published_by: Option<Uuid>,
    published_at: Option<chrono::DateTime<Utc>>,
    created_at: chrono::DateTime<Utc>,
    updated_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
struct CloneSourceRow {
    id: Uuid,
    academic_term_id: Uuid,
    academic_year_id: Uuid,
    status: TimetableVersionStatus,
    row_version: i64,
    bell_schedule_id: Uuid,
}

const VERSION_SELECT: &str = r#"
    SELECT version.id,
           version.academic_term_id,
           version.academic_year_id,
           version.effective_from,
           CASE
               WHEN version.status = 'published' THEN COALESCE(
                   (
                       SELECT next_version.effective_from - 1
                       FROM academic_timetable_versions next_version
                       WHERE next_version.academic_term_id = version.academic_term_id
                         AND next_version.status = 'published'
                         AND next_version.effective_from > version.effective_from
                       ORDER BY next_version.effective_from, next_version.id
                       LIMIT 1
                   ),
                   term.closed_on
               )
               ELSE NULL
           END AS effective_until,
           version.status,
           version.source_version_id,
           version.delivery_version_id,
           version.bell_schedule_id,
           version.row_version,
           version.created_by,
           version.published_by,
           version.published_at,
           version.created_at,
           version.updated_at
    FROM academic_timetable_versions version
    JOIN academic_terms term ON term.id = version.academic_term_id
"#;

pub(crate) fn derive_display_state(
    effective_from: NaiveDate,
    effective_until: Option<NaiveDate>,
    today: NaiveDate,
) -> TimetableVersionDisplayState {
    if today < effective_from {
        TimetableVersionDisplayState::Upcoming
    } else if effective_until.is_some_and(|end| today > end) {
        TimetableVersionDisplayState::Historical
    } else {
        TimetableVersionDisplayState::Current
    }
}

pub async fn list_versions(
    pool: &PgPool,
    term_id: Uuid,
) -> Result<Vec<TimetableVersion>, AppError> {
    let sql = format!(
        "{VERSION_SELECT} WHERE version.academic_term_id = $1 \
         ORDER BY version.effective_from DESC, \
                  CASE version.status WHEN 'draft' THEN 0 WHEN 'published' THEN 1 ELSE 2 END, \
                  version.id"
    );
    let rows = sqlx::query_as::<_, TimetableVersionRow>(sqlx::AssertSqlSafe(sql))
        .bind(term_id)
        .fetch_all(pool)
        .await?;
    hydrate_versions(pool, rows, Utc::now().date_naive()).await
}

/// Public read DTOs expose only opening targets within the timetable read scope.
pub async fn restrict_targets(
    pool: &PgPool,
    versions: &mut [TimetableVersion],
    access: &crate::policy::TimetableAccessFilter,
) -> Result<(), AppError> {
    if access.includes_school_owned {
        return Ok(());
    }
    let ids = versions
        .iter()
        .map(|version| version.delivery_version_id)
        .collect::<Vec<_>>();
    let snapshots = school_academic_delivery::services::versions::snapshots(pool, &ids).await?;
    for version in versions {
        let snapshot = snapshots
            .get(&version.delivery_version_id)
            .ok_or_else(|| AppError::Conflict("ไม่พบรุ่นเปิดสอนของตาราง".into()))?;
        version.targets.retain(|target| {
            snapshot.offerings.iter().any(|offering| {
                offering.id == target.learning_offering_id
                    && (access
                        .organization_unit_ids
                        .contains(&offering.owning_organization_unit_id)
                        || access
                            .organization_tree_unit_ids
                            .contains(&offering.owning_organization_unit_id)
                        || access.assigned_actor_id.is_some_and(|actor| {
                            offering.groups.iter().any(|group| {
                                group
                                    .teachers
                                    .iter()
                                    .any(|teacher| teacher.teacher_id == actor)
                            })
                        }))
            })
        });
    }
    Ok(())
}

pub async fn resolve_for_date(
    pool: &PgPool,
    term_id: Uuid,
    on_date: NaiveDate,
) -> Result<TimetableVersion, AppError> {
    let version_id = resolve_version_id_for_date(pool, term_id, on_date).await?;
    let sql = format!("{VERSION_SELECT} WHERE version.id = $1");
    let row = sqlx::query_as::<_, TimetableVersionRow>(sqlx::AssertSqlSafe(sql))
        .bind(version_id)
        .fetch_one(pool)
        .await?;
    let mut versions = hydrate_versions(pool, vec![row], on_date).await?;
    versions.pop().ok_or_else(|| {
        AppError::InternalServerError("ไม่สามารถโหลดตารางเรียนตามวันที่เลือกได้".to_string())
    })
}

pub async fn resolve_version_id_for_date<'e>(
    executor: impl sqlx::Executor<'e, Database = Postgres>,
    term_id: Uuid,
    on_date: NaiveDate,
) -> Result<Uuid, AppError> {
    sqlx::query_scalar(
        r#"SELECT id
           FROM academic_timetable_versions
           WHERE academic_term_id = $1
             AND status = 'published'
             AND effective_from <= $2
           ORDER BY effective_from DESC, id
           LIMIT 1"#,
    )
    .bind(term_id)
    .bind(on_date)
    .fetch_optional(executor)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("ไม่พบตารางเรียนที่เผยแพร่และมีผลในวันที่ {on_date}")))
}

/// Start the first timetable after independently publishing its opening graph.
/// Term serialization also makes a repeated request resume the same blank draft.
pub async fn create_initial_draft(
    pool: &PgPool,
    actor: Uuid,
    request: CreateTimetableVersionRequest,
) -> Result<TimetableVersion, AppError> {
    let mut tx = pool.begin().await?;
    let (year, bell): (Uuid, Option<Uuid>) =
        sqlx::query_as("SELECT academic_year_id,bell_schedule_id FROM academic_terms WHERE id=$1")
            .bind(request.academic_term_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| AppError::NotFound("ไม่พบภาคเรียน".into()))?;
    lifecycle_guard::require_term_write_exclusive(&mut tx, year, request.academic_term_id).await?;
    let bell = bell.ok_or_else(|| {
        AppError::ValidationError("ตั้งค่าตารางเวลาของภาคเรียนก่อนสร้างตารางสอน".into())
    })?;
    let published: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM academic_timetable_versions WHERE academic_term_id=$1 AND status='published')")
        .bind(request.academic_term_id).fetch_one(&mut *tx).await?;
    if published {
        return Err(AppError::Conflict(
            "มีตารางที่เผยแพร่แล้ว กรุณาเปิดรุ่นต้นทางแล้วกดแก้ไข".into(),
        ));
    }
    let drafts: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM academic_timetable_versions WHERE academic_term_id=$1 AND status='draft' ORDER BY created_at,id FOR UPDATE")
        .bind(request.academic_term_id).fetch_all(&mut *tx).await?;
    if drafts.len() > 1 {
        return Err(AppError::Conflict(
            "มีหลายแบบร่าง กรุณาเลือกแบบร่างที่จะจัดต่อ".into(),
        ));
    }
    let id = if let Some(id) = drafts.first() {
        *id
    } else {
        let delivery = school_academic_delivery::services::versions::latest_published_id(
            &mut tx,
            request.academic_term_id,
        )
        .await?;
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO academic_timetable_versions(id,academic_term_id,academic_year_id,effective_from,status,delivery_version_id,bell_schedule_id,created_by) VALUES($1,$2,$3,NULL,'draft',$4,$5,$6)")
            .bind(id).bind(request.academic_term_id).bind(year).bind(delivery).bind(bell).bind(actor).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO academic_audit_events(event_code,entity_type,entity_id,academic_year_id,academic_term_id,actor_user_id,payload) VALUES('academic_timetable_version.draft_created','academic_timetable_version',$1,$2,$3,$4,$5)")
            .bind(id).bind(year).bind(request.academic_term_id).bind(actor).bind(serde_json::json!({"deliveryVersionId":delivery})).execute(&mut *tx).await?;
        id
    };
    tx.commit().await?;
    get_version(pool, id, Utc::now().date_naive()).await
}

pub async fn clone_draft(
    pool: &PgPool,
    actor_id: Uuid,
    source_id: Uuid,
    request: CloneTimetableVersionRequest,
) -> Result<TimetableVersion, AppError> {
    let mut transaction = pool.begin().await?;
    let new_version_id =
        clone_draft_in_transaction(&mut transaction, actor_id, source_id, request).await?;
    transaction.commit().await?;
    get_version(pool, new_version_id, Utc::now().date_naive()).await
}

/// Coordinate before version/block locks. Resolve immutable IDs without row
/// locks and acquire the term write mode initially, including nested Delivery
/// callers which already hold that same term lock.
pub(crate) async fn require_version_term_write(
    transaction: &mut Transaction<'_, Postgres>,
    version_id: Uuid,
) -> Result<(), AppError> {
    let (year_id, term_id): (Uuid, Uuid) = sqlx::query_as(
        "SELECT academic_year_id, academic_term_id FROM academic_timetable_versions WHERE id=$1",
    )
    .bind(version_id)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบรุ่นตารางสอน".into()))?;
    lifecycle_guard::require_term_write_exclusive(transaction, year_id, term_id).await
}

pub async fn clone_draft_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    actor_id: Uuid,
    source_id: Uuid,
    request: CloneTimetableVersionRequest,
) -> Result<Uuid, AppError> {
    let source_row_version = request.source_row_version;
    if source_row_version <= 0 {
        return Err(AppError::ValidationError(
            "sourceRowVersion ต้องมากกว่าศูนย์".to_string(),
        ));
    }

    require_version_term_write(transaction, source_id).await?;
    let source: CloneSourceRow = sqlx::query_as(
        r#"SELECT source.id,
                  source.academic_term_id,
                  source.academic_year_id,
                  source.status,
                  source.row_version,
                  term.bell_schedule_id
           FROM academic_timetable_versions source
           JOIN academic_terms term ON term.id = source.academic_term_id
           JOIN academic_years year ON year.id = source.academic_year_id
           WHERE source.id = $1
           FOR UPDATE OF source"#,
    )
    .bind(source_id)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบรุ่นตารางเรียนต้นทาง".to_string()))?;

    if source.status != TimetableVersionStatus::Published {
        return Err(AppError::Conflict(
            "สร้างแบบร่างใหม่ได้จากรุ่นตารางเรียนที่เผยแพร่แล้วเท่านั้น".to_string(),
        ));
    }
    if source.row_version != source_row_version {
        return Err(AppError::Conflict(format!(
            "รุ่นตารางเรียนต้นทางถูกแก้ไขแล้ว (expected {}, actual {})",
            source_row_version, source.row_version
        )));
    }
    let delivery_version_id = school_academic_delivery::services::versions::latest_published_id(
        transaction,
        source.academic_term_id,
    )
    .await?;
    let drafts: Vec<(Uuid, i64)> = sqlx::query_as("SELECT id,row_version FROM academic_timetable_versions WHERE source_version_id=$1 AND status='draft' ORDER BY created_at,id FOR UPDATE")
        .bind(source.id).fetch_all(&mut **transaction).await?;
    let resume = match request.resume_draft_id {
        Some(id) => Some(
            *drafts
                .iter()
                .find(|(draft_id, _)| *draft_id == id)
                .ok_or_else(|| AppError::Conflict("ไม่พบแบบร่างจากรุ่นต้นทางที่เลือก".into()))?,
        ),
        None if drafts.len() == 1 => drafts.first().copied(),
        None if drafts.len() > 1 => {
            return Err(AppError::Conflict(
                "มีหลายแบบร่างจากรุ่นนี้ กรุณาเลือกแบบร่างที่ต้องการแก้ต่อ".into(),
            ))
        }
        None => None,
    };
    if let Some((id, revision)) = resume {
        if request
            .draft_row_version
            .is_some_and(|expected| expected != revision)
        {
            return Err(AppError::Conflict("แบบร่างถูกแก้ไข กรุณาโหลดข้อมูลใหม่".into()));
        }
        let updated=sqlx::query("UPDATE academic_timetable_versions SET delivery_version_id=$2,row_version=row_version+1,updated_at=now() WHERE id=$1 AND delivery_version_id<>$2")
            .bind(id).bind(delivery_version_id).execute(&mut **transaction).await?;
        if updated.rows_affected() == 1 {
            sqlx::query("INSERT INTO academic_audit_events(event_code,entity_type,entity_id,academic_year_id,academic_term_id,actor_user_id,payload) VALUES('academic_timetable_version.source_updated','academic_timetable_version',$1,$2,$3,$4,$5)")
                .bind(id).bind(source.academic_year_id).bind(source.academic_term_id).bind(actor_id)
                .bind(sqlx::types::Json(serde_json::json!({"deliveryVersionId":delivery_version_id,"sourceVersionId":source.id,"resumed":true}))).execute(&mut **transaction).await?;
        }
        return Ok(id);
    }

    let new_version_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO academic_timetable_versions (
               id, academic_term_id, academic_year_id, effective_from, status,
               source_version_id, delivery_version_id, bell_schedule_id, created_by
           ) VALUES ($1, $2, $3, NULL, 'draft', $4, $5, $6, $7)"#,
    )
    .bind(new_version_id)
    .bind(source.academic_term_id)
    .bind(source.academic_year_id)
    .bind(source.id)
    .bind(delivery_version_id)
    .bind(source.bell_schedule_id)
    .bind(actor_id)
    .execute(&mut **transaction)
    .await
    .map_err(map_clone_write_error)?;

    sqlx::query(
        r#"CREATE TEMP TABLE timetable_clone_block_map ON COMMIT DROP AS
           SELECT block.id AS source_id,
                  uuid_generate_v5($1, 'block:' || block.id::text) AS target_id
           FROM academic_timetable_blocks block
           WHERE block.timetable_version_id = $2 AND block.is_active"#,
    )
    .bind(new_version_id)
    .bind(source.id)
    .execute(&mut **transaction)
    .await?;
    sqlx::query(
        r#"CREATE TEMP TABLE timetable_clone_group_map ON COMMIT DROP AS
           SELECT block_group.id AS source_id,
                  uuid_generate_v5($1, 'block-group:' || block_group.id::text) AS target_id
           FROM academic_timetable_block_groups block_group
           JOIN timetable_clone_block_map block_map ON block_map.source_id = block_group.block_id
           WHERE block_group.is_active"#,
    )
    .bind(new_version_id)
    .execute(&mut **transaction)
    .await?;
    sqlx::query(
        r#"INSERT INTO academic_timetable_blocks (
               id, timetable_version_id, academic_term_id, academic_year_id,
               bell_schedule_id, bell_schedule_period_id, day_of_week,
               block_kind, scheduling_mode, learning_offering_id, structural_kind,
               title, note, series_id, row_version, is_active, migration_provenance,
               created_by, updated_by, created_at, updated_at
           )
           SELECT block_map.target_id, $1, source.academic_term_id, source.academic_year_id,
                  source.bell_schedule_id, source.bell_schedule_period_id, source.day_of_week,
                  source.block_kind, source.scheduling_mode, source.learning_offering_id,
                  source.structural_kind, source.title, source.note,
                  CASE WHEN source.series_id IS NULL THEN NULL
                       ELSE uuid_generate_v5($1, 'series:' || source.series_id::text) END,
                  1, true,
                  source.migration_provenance || jsonb_build_object(
                      'clonedFromBlockId', source.id::text,
                      'sourceVersionId', $2::text
                  ),
                  $3, $3, now(), now()
           FROM timetable_clone_block_map block_map
           JOIN academic_timetable_blocks source ON source.id = block_map.source_id"#,
    )
    .bind(new_version_id)
    .bind(source.id)
    .bind(actor_id)
    .execute(&mut **transaction)
    .await
    .map_err(map_write_error)?;
    sqlx::query(
        r#"INSERT INTO academic_timetable_block_groups (
               id, block_id, learning_group_id, learning_offering_id,
               academic_term_id, academic_year_id, room_id, row_version, homeroom_ids,
               is_active, migration_provenance, created_by, updated_by,
               created_at, updated_at
           )
           SELECT group_map.target_id, block_map.target_id, source.learning_group_id,
                  source.learning_offering_id, source.academic_term_id,
                  source.academic_year_id, source.room_id, 1, source.homeroom_ids, true,
                  source.migration_provenance || jsonb_build_object(
                      'clonedFromBlockGroupId', source.id::text,
                      'sourceVersionId', $2::text
                  ),
                  $3, $3, now(), now()
           FROM timetable_clone_group_map group_map
           JOIN academic_timetable_block_groups source ON source.id = group_map.source_id
           JOIN timetable_clone_block_map block_map ON block_map.source_id = source.block_id"#,
    )
    .bind(new_version_id)
    .bind(source.id)
    .bind(actor_id)
    .execute(&mut **transaction)
    .await
    .map_err(map_write_error)?;
    sqlx::query(
        r#"INSERT INTO academic_timetable_block_group_instructors (
               id, block_group_id, instructor_id, role, display_order,
               row_version, created_at, updated_at
           )
           SELECT gen_random_uuid(), group_map.target_id, source.instructor_id,
                  source.role, source.display_order, 1, now(), now()
           FROM timetable_clone_group_map group_map
           JOIN academic_timetable_block_group_instructors source
             ON source.block_group_id = group_map.source_id"#,
    )
    .execute(&mut **transaction)
    .await
    .map_err(map_write_error)?;
    sqlx::query(
        r#"INSERT INTO academic_timetable_block_homerooms (
               id, block_id, homeroom_id, academic_term_id, academic_year_id,
               target_kind, room_id, row_version, is_active, migration_provenance,
               created_by, updated_by, created_at, updated_at
           )
           SELECT uuid_generate_v5($1, 'homeroom-target:' || source.id::text),
                  block_map.target_id, source.homeroom_id, source.academic_term_id,
                  source.academic_year_id, source.target_kind, source.room_id,
                  1, true,
                  source.migration_provenance || jsonb_build_object(
                      'clonedFromTargetId', source.id::text,
                      'sourceVersionId', $2::text
                  ),
                  $3, $3, now(), now()
           FROM timetable_clone_block_map block_map
           JOIN academic_timetable_block_homerooms source
             ON source.block_id = block_map.source_id
           WHERE source.is_active"#,
    )
    .bind(new_version_id)
    .bind(source.id)
    .bind(actor_id)
    .execute(&mut **transaction)
    .await
    .map_err(map_write_error)?;
    sqlx::query(
        r#"INSERT INTO academic_timetable_block_teachers (
               id, block_id, teacher_id, academic_term_id, academic_year_id,
               row_version, is_active, migration_provenance,
               created_by, updated_by, created_at, updated_at
           )
           SELECT uuid_generate_v5($1, 'teacher-target:' || source.id::text),
                  block_map.target_id, source.teacher_id, source.academic_term_id,
                  source.academic_year_id, 1, true,
                  source.migration_provenance || jsonb_build_object(
                      'clonedFromTargetId', source.id::text,
                      'sourceVersionId', $2::text
                  ),
                  $3, $3, now(), now()
           FROM timetable_clone_block_map block_map
           JOIN academic_timetable_block_teachers source
             ON source.block_id = block_map.source_id
           WHERE source.is_active"#,
    )
    .bind(new_version_id)
    .bind(source.id)
    .bind(actor_id)
    .execute(&mut **transaction)
    .await
    .map_err(map_write_error)?;
    sqlx::query(
        r#"INSERT INTO academic_timetable_block_group_sync (
               id, block_id, learning_group_id, learning_offering_id,
               academic_term_id, academic_year_id, status, linked_block_group_id,
               conflict_code, conflict_message, attempted_group_row_version,
               row_version, created_by, updated_by, created_at, updated_at
           )
           SELECT uuid_generate_v5($1, 'sync:' || source.id::text),
                  block_map.target_id, source.learning_group_id,
                  source.learning_offering_id, source.academic_term_id,
                  source.academic_year_id, source.status, group_map.target_id,
                  source.conflict_code, source.conflict_message,
                  source.attempted_group_row_version, 1, $3, $3, now(), now()
           FROM timetable_clone_block_map block_map
           JOIN academic_timetable_block_group_sync source
             ON source.block_id = block_map.source_id
           LEFT JOIN timetable_clone_group_map group_map
             ON group_map.source_id = source.linked_block_group_id"#,
    )
    .bind(new_version_id)
    .bind(source.id)
    .bind(actor_id)
    .execute(&mut **transaction)
    .await
    .map_err(map_write_error)?;

    let (source_block_count, cloned_block_count, source_child_count, cloned_child_count): (
        i64,
        i64,
        i64,
        i64,
    ) = sqlx::query_as(
        r#"SELECT
               (SELECT count(*) FROM academic_timetable_blocks
                WHERE timetable_version_id = $1 AND is_active),
               (SELECT count(*) FROM academic_timetable_blocks
                WHERE timetable_version_id = $2 AND is_active),
               (SELECT count(*) FROM academic_timetable_block_groups child
                JOIN academic_timetable_blocks block ON block.id = child.block_id
                WHERE block.timetable_version_id = $1 AND block.is_active AND child.is_active)
                 + (SELECT count(*) FROM academic_timetable_block_homerooms child
                    JOIN academic_timetable_blocks block ON block.id = child.block_id
                    WHERE block.timetable_version_id = $1 AND block.is_active AND child.is_active)
                 + (SELECT count(*) FROM academic_timetable_block_teachers child
                    JOIN academic_timetable_blocks block ON block.id = child.block_id
                    WHERE block.timetable_version_id = $1 AND block.is_active AND child.is_active),
               (SELECT count(*) FROM academic_timetable_block_groups child
                JOIN academic_timetable_blocks block ON block.id = child.block_id
                WHERE block.timetable_version_id = $2 AND block.is_active AND child.is_active)
                 + (SELECT count(*) FROM academic_timetable_block_homerooms child
                    JOIN academic_timetable_blocks block ON block.id = child.block_id
                    WHERE block.timetable_version_id = $2 AND block.is_active AND child.is_active)
                 + (SELECT count(*) FROM academic_timetable_block_teachers child
                    JOIN academic_timetable_blocks block ON block.id = child.block_id
                    WHERE block.timetable_version_id = $2 AND block.is_active AND child.is_active)"#,
    )
    .bind(source.id)
    .bind(new_version_id)
    .fetch_one(&mut **transaction)
    .await?;
    if source_block_count != cloned_block_count || source_child_count != cloned_child_count {
        return Err(AppError::InternalServerError(
            "คัดลอกรุ่นตารางสอนไม่ครบถ้วน กรุณาลองใหม่".to_string(),
        ));
    }
    sqlx::query("INSERT INTO academic_audit_events(event_code,entity_type,entity_id,academic_year_id,academic_term_id,actor_user_id,payload) VALUES('academic_timetable_version.draft_created','academic_timetable_version',$1,$2,$3,$4,$5)")
        .bind(new_version_id).bind(source.academic_year_id).bind(source.academic_term_id).bind(actor_id)
        .bind(sqlx::types::Json(serde_json::json!({"sourceVersionId":source.id,"deliveryVersionId":delivery_version_id,"copiedBlockCount":cloned_block_count}))).execute(&mut **transaction).await?;
    Ok(new_version_id)
}

pub async fn get_version(
    pool: &PgPool,
    version_id: Uuid,
    display_date: NaiveDate,
) -> Result<TimetableVersion, AppError> {
    let sql = format!("{VERSION_SELECT} WHERE version.id = $1");
    let row = sqlx::query_as::<_, TimetableVersionRow>(sqlx::AssertSqlSafe(sql))
        .bind(version_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบรุ่นตารางเรียน".to_string()))?;
    let mut versions = hydrate_versions(pool, vec![row], display_date).await?;
    versions
        .pop()
        .ok_or_else(|| AppError::InternalServerError("ไม่สามารถโหลดรุ่นตารางเรียนได้".to_string()))
}

async fn hydrate_versions(
    pool: &PgPool,
    rows: Vec<TimetableVersionRow>,
    display_date: NaiveDate,
) -> Result<Vec<TimetableVersion>, AppError> {
    if rows.is_empty() {
        return Ok(Vec::new());
    }

    let source_ids: Vec<Uuid> = rows.iter().map(|row| row.delivery_version_id).collect();
    let snapshots =
        school_academic_delivery::services::versions::snapshots(pool, &source_ids).await?;
    let mut targets_by_version: HashMap<Uuid, Vec<TimetableVersionTarget>> = HashMap::new();
    for row in &rows {
        let snapshot = snapshots
            .get(&row.delivery_version_id)
            .ok_or_else(|| AppError::Conflict("รุ่นเปิดสอนของตารางไม่พร้อม กรุณาติดต่อผู้ดูแลระบบ".into()))?;
        targets_by_version.insert(
            row.id,
            snapshot
                .offerings
                .iter()
                .map(|offering| TimetableVersionTarget {
                    timetable_version_id: row.id,
                    learning_offering_id: offering.id,
                    weekly_period_target: offering.weekly_period_target,
                    standard_periods_per_week: match &offering.catalog {
                        school_academic_delivery::models::LearningOfferingSnapshot::Course(
                            course,
                        ) => Some(course.standard_periods_per_week),
                        school_academic_delivery::models::LearningOfferingSnapshot::Activity(_) => {
                            None
                        }
                    },
                })
                .collect(),
        );
    }

    Ok(rows
        .into_iter()
        .map(|row| {
            let display_state = row
                .effective_from
                .filter(|_| row.status == TimetableVersionStatus::Published)
                .map(|date| derive_display_state(date, row.effective_until, display_date));
            TimetableVersion {
                id: row.id,
                academic_term_id: row.academic_term_id,
                academic_year_id: row.academic_year_id,
                effective_from: row.effective_from,
                effective_until: row.effective_until,
                status: row.status,
                display_state,
                source_version_id: row.source_version_id,
                delivery_version_id: row.delivery_version_id,
                bell_schedule_id: row.bell_schedule_id,
                row_version: row.row_version,
                created_by: row.created_by,
                published_by: row.published_by,
                published_at: row.published_at,
                created_at: row.created_at,
                updated_at: row.updated_at,
                targets: targets_by_version.remove(&row.id).unwrap_or_default(),
            }
        })
        .collect())
}

fn map_clone_write_error(error: sqlx::Error) -> AppError {
    if let sqlx::Error::Database(database) = &error {
        if database.constraint() == Some("academic_timetable_versions_live_effective_key") {
            return AppError::Conflict("มีรุ่นตารางเรียนแบบร่างหรือเผยแพร่ในวันที่นี้แล้ว".to_string());
        }
    }
    AppError::DbError(error)
}

#[cfg(test)]
mod tests {
    use super::derive_display_state;
    use crate::models::timetable_version::TimetableVersionDisplayState;
    use chrono::NaiveDate;

    #[test]
    fn display_state_uses_the_effective_interval() {
        let today = NaiveDate::from_ymd_opt(2027, 6, 15).unwrap();

        assert_eq!(
            derive_display_state(NaiveDate::from_ymd_opt(2027, 7, 1).unwrap(), None, today,),
            TimetableVersionDisplayState::Upcoming
        );
        assert_eq!(
            derive_display_state(
                NaiveDate::from_ymd_opt(2027, 5, 1).unwrap(),
                Some(NaiveDate::from_ymd_opt(2027, 6, 30).unwrap()),
                today,
            ),
            TimetableVersionDisplayState::Current
        );
        assert_eq!(
            derive_display_state(
                NaiveDate::from_ymd_opt(2027, 5, 1).unwrap(),
                Some(NaiveDate::from_ymd_opt(2027, 6, 14).unwrap()),
                today,
            ),
            TimetableVersionDisplayState::Historical
        );
    }
}
