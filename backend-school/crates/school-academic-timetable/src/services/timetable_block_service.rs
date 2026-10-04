use sqlx::{FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::models::timetable_block::{
    CreateOrdinaryTimetableBlockRequest, CreateStructuralTimetableBlocksRequest,
    CreateSynchronizedTimetableBlockRequest, RemoveTimetableBlockTargetRequest,
    RestoreTimetableBlockGroupRequest, RetryTimetableBlockSyncRequest, SwapTimetableBlocksRequest,
    SwapTimetableBlocksResponse, TimetableBlock, TimetableBlockPlacementPreview,
    TimetableBlockPlacementPreviewRequest, TimetableBlockWorkspace, TimetableBlockWorkspaceQuery,
    TimetableStructuralKind, TimetableTargetKind, UpdateTimetableBlockRequest,
};
use crate::policy::TimetableAccessFilter;
use school_errors::AppError;

use super::timetable_block_conflicts::{canonical_ids, map_write_error, normalize_day};
use super::timetable_block_queries;
use super::timetable_block_sync;
use super::timetable_version_service::require_version_term_write;

#[derive(Debug, FromRow)]
struct VersionContext {
    academic_term_id: Uuid,
    academic_year_id: Uuid,
    bell_schedule_id: Uuid,
    delivery_version_id: Uuid,
    status: String,
}

#[derive(Debug, FromRow)]
struct InstructorAssignment {
    teacher_id: Uuid,
    role: String,
}

#[derive(Debug, FromRow)]
struct LockedBlock {
    timetable_version_id: Uuid,
    academic_term_id: Uuid,
    academic_year_id: Uuid,
    bell_schedule_id: Uuid,
    bell_schedule_period_id: Uuid,
    day_of_week: String,
    block_kind: String,
    scheduling_mode: Option<String>,
    row_version: i64,
}

pub async fn get_block(pool: &PgPool, block_id: Uuid) -> Result<TimetableBlock, AppError> {
    timetable_block_queries::get_block(pool, block_id).await
}

pub async fn list_student_blocks(
    pool: &PgPool,
    timetable_version_id: Uuid,
    academic_term_id: Uuid,
    student_id: Uuid,
    on_date: chrono::NaiveDate,
) -> Result<Vec<TimetableBlock>, AppError> {
    let ids: Vec<Uuid> = sqlx::query_scalar(
        r#"SELECT DISTINCT block.id
           FROM academic_timetable_blocks block
           LEFT JOIN academic_timetable_block_groups block_group
             ON block_group.block_id = block.id AND block_group.is_active
           LEFT JOIN academic_timetable_block_homerooms homeroom_target
             ON homeroom_target.block_id = block.id AND homeroom_target.is_active
           WHERE block.timetable_version_id = $1
             AND block.academic_term_id = $2
             AND block.is_active
             AND (
                 EXISTS (
                     SELECT 1
                     FROM learning_group_students membership
                     JOIN student_academic_years student_year
                       ON student_year.id = membership.student_academic_year_id
                     JOIN learning_groups roster_group
                       ON roster_group.id = membership.learning_group_id
                     WHERE membership.learning_group_id = block_group.learning_group_id
                       AND student_year.student_id = $3
                       AND membership.published_at IS NOT NULL
                       AND membership.joined_at <= $4
                       AND (membership.left_at IS NULL OR membership.left_at >= $4)
                       AND roster_group.roster_status IN ('published', 'closed')
                 )
                 OR EXISTS (
                     SELECT 1
                     FROM student_academic_years student_year
                     JOIN homeroom_placements placement
                       ON placement.student_academic_year_id = student_year.id
                     WHERE student_year.student_id = $3
                       AND student_year.academic_year_id = block.academic_year_id
                       AND placement.homeroom_id = homeroom_target.homeroom_id
                       AND placement.status = 'current'
                 )
             )
           ORDER BY block.id"#,
    )
    .bind(timetable_version_id)
    .bind(academic_term_id)
    .bind(student_id)
    .bind(on_date)
    .fetch_all(pool)
    .await?;
    timetable_block_queries::get_blocks(pool, &ids).await
}

pub async fn list_instructor_blocks(
    pool: &PgPool,
    timetable_version_id: Uuid,
    academic_term_id: Uuid,
    instructor_id: Uuid,
) -> Result<Vec<TimetableBlock>, AppError> {
    let ids: Vec<Uuid> = sqlx::query_scalar(
        r#"SELECT DISTINCT block.id
           FROM academic_timetable_blocks block
           LEFT JOIN academic_timetable_block_groups block_group
             ON block_group.block_id = block.id AND block_group.is_active
           LEFT JOIN academic_timetable_block_group_instructors instructor
             ON instructor.block_group_id = block_group.id
           LEFT JOIN academic_timetable_block_teachers teacher_target
             ON teacher_target.block_id = block.id AND teacher_target.is_active
           WHERE block.timetable_version_id = $1
             AND block.academic_term_id = $2
             AND block.is_active
             AND (instructor.instructor_id = $3 OR teacher_target.teacher_id = $3)
           ORDER BY block.id"#,
    )
    .bind(timetable_version_id)
    .bind(academic_term_id)
    .bind(instructor_id)
    .fetch_all(pool)
    .await?;
    timetable_block_queries::get_blocks(pool, &ids).await
}

pub async fn get_workspace(
    pool: &PgPool,
    query: TimetableBlockWorkspaceQuery,
    access: &TimetableAccessFilter,
) -> Result<TimetableBlockWorkspace, AppError> {
    timetable_block_queries::get_workspace(pool, query, access).await
}

pub async fn preview_placement(
    pool: &PgPool,
    request: TimetableBlockPlacementPreviewRequest,
) -> Result<TimetableBlockPlacementPreview, AppError> {
    super::timetable_block_conflicts::preview_placement(pool, request).await
}

pub async fn create_ordinary_block(
    pool: &PgPool,
    actor_id: Uuid,
    request: CreateOrdinaryTimetableBlockRequest,
) -> Result<TimetableBlock, AppError> {
    let day = normalize_day(&request.day_of_week)?;
    let instructor_ids = canonical_ids(&request.instructor_ids);
    if instructor_ids.is_empty() {
        return Err(AppError::ValidationError(
            "ต้องเลือกครูอย่างน้อยหนึ่งคนสำหรับคาบนี้".to_string(),
        ));
    }
    let mut transaction = pool.begin().await?;
    let version = lock_draft_version(
        &mut transaction,
        request.timetable_version_id,
        request.academic_term_id,
        request.bell_schedule_period_id,
    )
    .await?;
    let source = school_academic_delivery::services::versions::published_source(
        &mut transaction,
        version.delivery_version_id,
        version.academic_term_id,
    )
    .await?;
    let (offering, group) = source
        .snapshot
        .offerings
        .iter()
        .find_map(|offering| {
            offering
                .groups
                .iter()
                .find(|group| group.id == request.learning_group_id)
                .map(|group| (offering, group))
        })
        .ok_or_else(|| AppError::ValidationError("กลุ่มเรียนไม่อยู่ในรุ่นเปิดสอนที่ตารางอ้างอิง".into()))?;
    let scheduling_mode = match &offering.catalog {
        school_academic_delivery::models::LearningOfferingSnapshot::Course(_) => {
            Some("independent")
        }
        school_academic_delivery::models::LearningOfferingSnapshot::Activity(activity) => {
            Some(match activity.scheduling_mode {
                school_academic_delivery::models::ActivitySchedulingMode::Independent => {
                    "independent"
                }
                school_academic_delivery::models::ActivitySchedulingMode::Synchronized => {
                    "synchronized"
                }
            })
        }
    };
    if scheduling_mode == Some("synchronized") {
        return Err(AppError::ValidationError(
            "กิจกรรมแบบพร้อมกันต้องวางจากช่วงกิจกรรมหลัก".into(),
        ));
    }
    let assignments = source_assignments(group, &instructor_ids)?;

    let block_id = Uuid::new_v4();
    let block_kind =
        if offering.kind == school_academic_delivery::models::LearningOfferingKind::Course {
            "COURSE"
        } else {
            "ACTIVITY"
        };
    insert_block(
        &mut transaction,
        block_id,
        &version,
        request.timetable_version_id,
        request.bell_schedule_period_id,
        &day,
        block_kind,
        scheduling_mode,
        Some(offering.id),
        None,
        None,
        request.note.as_deref(),
        None,
        actor_id,
    )
    .await?;
    let block_group_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO academic_timetable_block_groups (
               id, block_id, learning_group_id, learning_offering_id,
               academic_term_id, academic_year_id, room_id, created_by, updated_by, homeroom_ids
           ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $8, $9)"#,
    )
    .bind(block_group_id)
    .bind(block_id)
    .bind(request.learning_group_id)
    .bind(offering.id)
    .bind(version.academic_term_id)
    .bind(version.academic_year_id)
    .bind(request.room_id)
    .bind(actor_id)
    .bind(&group.homeroom_ids)
    .execute(&mut *transaction)
    .await
    .map_err(map_write_error)?;
    for (index, teacher_id) in instructor_ids.iter().enumerate() {
        let assignment = assignments
            .iter()
            .find(|assignment| assignment.teacher_id == *teacher_id)
            .ok_or_else(|| AppError::ValidationError("ครูไม่อยู่ในข้อมูลเปิดสอนของกลุ่ม".into()))?;
        sqlx::query(
            r#"INSERT INTO academic_timetable_block_group_instructors (
                   id, block_group_id, instructor_id, role, display_order
               ) VALUES (gen_random_uuid(), $1, $2, $3, $4)"#,
        )
        .bind(block_group_id)
        .bind(teacher_id)
        .bind(&assignment.role)
        .bind((index + 1) as i32)
        .execute(&mut *transaction)
        .await
        .map_err(map_write_error)?;
    }
    transaction.commit().await?;
    get_block(pool, block_id).await
}

pub async fn create_synchronized_block(
    pool: &PgPool,
    actor_id: Uuid,
    request: CreateSynchronizedTimetableBlockRequest,
) -> Result<TimetableBlock, AppError> {
    let day = normalize_day(&request.day_of_week)?;
    let homeroom_ids = canonical_ids(&request.intended_homeroom_ids);
    let teacher_ids = canonical_ids(&request.teacher_ids);
    if homeroom_ids.is_empty() {
        return Err(AppError::ValidationError(
            "ต้องระบุห้องประจำชั้นที่เข้าร่วมช่วงกิจกรรมหลัก".to_string(),
        ));
    }
    let mut transaction = pool.begin().await?;
    let version = lock_draft_version(
        &mut transaction,
        request.timetable_version_id,
        request.academic_term_id,
        request.bell_schedule_period_id,
    )
    .await?;
    let source = school_academic_delivery::services::versions::published_source(
        &mut transaction,
        version.delivery_version_id,
        version.academic_term_id,
    )
    .await?;
    let offering = source
        .snapshot
        .offerings
        .iter()
        .find(|offering| offering.id == request.learning_offering_id)
        .ok_or_else(|| AppError::ValidationError("รายการเปิดสอนไม่อยู่ในรุ่นที่ตารางอ้างอิง".into()))?;
    if !matches!(&offering.catalog,school_academic_delivery::models::LearningOfferingSnapshot::Activity(activity)
        if activity.scheduling_mode==school_academic_delivery::models::ActivitySchedulingMode::Synchronized)
    {
        return Err(AppError::ValidationError(
            "รายการนี้ไม่ใช่กิจกรรมแบบจัดพร้อมกัน".into(),
        ));
    }
    let scoped_homeroom_ids = &offering.homeroom_ids;
    if homeroom_ids
        .iter()
        .any(|homeroom_id| !scoped_homeroom_ids.contains(homeroom_id))
    {
        return Err(AppError::ValidationError(
            "ห้องประจำชั้นอยู่นอกขอบเขตรายการเปิดสอน".to_string(),
        ));
    }
    ensure_homerooms(&mut transaction, version.academic_year_id, &homeroom_ids).await?;
    ensure_teachers(&mut transaction, &teacher_ids).await?;
    let block_id = Uuid::new_v4();
    insert_block(
        &mut transaction,
        block_id,
        &version,
        request.timetable_version_id,
        request.bell_schedule_period_id,
        &day,
        "ACTIVITY",
        Some("synchronized"),
        Some(request.learning_offering_id),
        None,
        None,
        request.note.as_deref(),
        None,
        actor_id,
    )
    .await?;
    for homeroom_id in homeroom_ids {
        sqlx::query(
            r#"INSERT INTO academic_timetable_block_homerooms (
                   id, block_id, homeroom_id, academic_term_id, academic_year_id,
                   target_kind, room_id, created_by, updated_by
               ) VALUES (gen_random_uuid(), $1, $2, $3, $4, 'reservation', $5, $6, $6)"#,
        )
        .bind(block_id)
        .bind(homeroom_id)
        .bind(version.academic_term_id)
        .bind(version.academic_year_id)
        .bind(request.room_id)
        .bind(actor_id)
        .execute(&mut *transaction)
        .await
        .map_err(map_write_error)?;
    }
    for teacher_id in teacher_ids {
        sqlx::query(
            r#"INSERT INTO academic_timetable_block_teachers (
                   id, block_id, teacher_id, academic_term_id, academic_year_id,
                   created_by, updated_by
               ) VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, $5)"#,
        )
        .bind(block_id)
        .bind(teacher_id)
        .bind(version.academic_term_id)
        .bind(version.academic_year_id)
        .bind(actor_id)
        .execute(&mut *transaction)
        .await
        .map_err(map_write_error)?;
    }
    timetable_block_sync::sync_offering_groups_in_tx(&mut transaction, block_id, actor_id).await?;
    transaction.commit().await?;
    get_block(pool, block_id).await
}

pub async fn create_structural_blocks(
    pool: &PgPool,
    actor_id: Uuid,
    request: CreateStructuralTimetableBlocksRequest,
) -> Result<Vec<TimetableBlock>, AppError> {
    let title = request.title.trim();
    if title.is_empty() || request.slots.is_empty() {
        return Err(AppError::ValidationError(
            "ต้องระบุชื่อและอย่างน้อยหนึ่งช่วงเวลาสำหรับคาบพิเศษ".to_string(),
        ));
    }
    let mut homeroom_ids = canonical_ids(&request.homeroom_ids);
    let mut teacher_ids = canonical_ids(&request.teacher_ids);
    let mut transaction = pool.begin().await?;
    let first_slot = request
        .slots
        .first()
        .expect("validated structural slots must not be empty");
    let version = lock_draft_version(
        &mut transaction,
        request.timetable_version_id,
        request.academic_term_id,
        first_slot.bell_schedule_period_id,
    )
    .await?;
    if request.all_homerooms {
        homeroom_ids = sqlx::query_scalar(
            r#"SELECT id FROM homerooms
               WHERE academic_year_id = $1 AND is_active
               ORDER BY id"#,
        )
        .bind(version.academic_year_id)
        .fetch_all(&mut *transaction)
        .await?;
    }
    if request.all_teachers {
        teacher_ids = sqlx::query_scalar(
            r#"SELECT id FROM users
               WHERE user_type = 'staff' AND status = 'active'
               ORDER BY id"#,
        )
        .fetch_all(&mut *transaction)
        .await?;
    }
    if homeroom_ids.is_empty() && teacher_ids.is_empty() {
        return Err(AppError::ValidationError(
            "คาบพิเศษต้องมีห้องประจำชั้นหรือครูเป้าหมายอย่างน้อยหนึ่งรายการ".to_string(),
        ));
    }
    ensure_homerooms(&mut transaction, version.academic_year_id, &homeroom_ids).await?;
    ensure_teachers(&mut transaction, &teacher_ids).await?;
    let series_id = Uuid::new_v4();
    let mut block_ids = Vec::with_capacity(request.slots.len());
    for slot in request.slots {
        let day = normalize_day(&slot.day_of_week)?;
        ensure_period(
            &mut transaction,
            version.bell_schedule_id,
            slot.bell_schedule_period_id,
        )
        .await?;
        let block_id = Uuid::new_v4();
        insert_block(
            &mut transaction,
            block_id,
            &version,
            request.timetable_version_id,
            slot.bell_schedule_period_id,
            &day,
            "STRUCTURAL",
            None,
            None,
            Some(structural_kind_wire(request.structural_kind)),
            Some(title),
            request.note.as_deref(),
            Some(series_id),
            actor_id,
        )
        .await?;
        for homeroom_id in &homeroom_ids {
            sqlx::query(
                r#"INSERT INTO academic_timetable_block_homerooms (
                       id, block_id, homeroom_id, academic_term_id, academic_year_id,
                       target_kind, room_id, created_by, updated_by
                   ) VALUES (gen_random_uuid(), $1, $2, $3, $4, 'structural', $5, $6, $6)"#,
            )
            .bind(block_id)
            .bind(homeroom_id)
            .bind(version.academic_term_id)
            .bind(version.academic_year_id)
            .bind(request.room_id)
            .bind(actor_id)
            .execute(&mut *transaction)
            .await
            .map_err(map_write_error)?;
        }
        for teacher_id in &teacher_ids {
            sqlx::query(
                r#"INSERT INTO academic_timetable_block_teachers (
                       id, block_id, teacher_id, academic_term_id, academic_year_id,
                       created_by, updated_by
                   ) VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, $5)"#,
            )
            .bind(block_id)
            .bind(teacher_id)
            .bind(version.academic_term_id)
            .bind(version.academic_year_id)
            .bind(actor_id)
            .execute(&mut *transaction)
            .await
            .map_err(map_write_error)?;
        }
        block_ids.push(block_id);
    }
    transaction.commit().await?;
    timetable_block_queries::get_blocks(pool, &block_ids).await
}

pub async fn remove_target(
    pool: &PgPool,
    actor_id: Uuid,
    block_id: Uuid,
    request: RemoveTimetableBlockTargetRequest,
) -> Result<TimetableBlock, AppError> {
    let mut transaction = pool.begin().await?;
    ensure_draft_version_id(&mut transaction, request.timetable_version_id).await?;
    let (block_kind, scheduling_mode): (String, Option<String>) = sqlx::query_as(
        r#"SELECT block.block_kind, block.scheduling_mode
           FROM academic_timetable_blocks block
           JOIN academic_timetable_versions version ON version.id = block.timetable_version_id
           WHERE block.id = $1
             AND block.timetable_version_id = $2
             AND block.row_version = $3
             AND version.status = 'draft'
           FOR UPDATE OF block, version"#,
    )
    .bind(block_id)
    .bind(request.timetable_version_id)
    .bind(request.block_row_version)
    .fetch_optional(&mut *transaction)
    .await?
    .ok_or_else(stale_block)?;

    let mut increment_parent_revision = true;
    match request.target_kind {
        TimetableTargetKind::Group if scheduling_mode.as_deref() == Some("synchronized") => {
            let learning_group_id: Uuid = sqlx::query_scalar(
                r#"UPDATE academic_timetable_block_groups
                   SET is_active = false, row_version = row_version + 1,
                       updated_by = $4, updated_at = now()
                   WHERE id = $1 AND block_id = $2 AND row_version = $3 AND is_active
                   RETURNING learning_group_id"#,
            )
            .bind(request.target_id)
            .bind(block_id)
            .bind(request.target_row_version)
            .bind(actor_id)
            .fetch_optional(&mut *transaction)
            .await?
            .ok_or_else(stale_block)?;
            sqlx::query(
                r#"UPDATE academic_timetable_block_group_sync
                   SET status = 'EXCLUDED', linked_block_group_id = NULL,
                       conflict_code = NULL, conflict_message = NULL,
                       row_version = row_version + 1, updated_by = $3, updated_at = now()
                   WHERE block_id = $1 AND learning_group_id = $2"#,
            )
            .bind(block_id)
            .bind(learning_group_id)
            .bind(actor_id)
            .execute(&mut *transaction)
            .await?;
        }
        TimetableTargetKind::Group => {
            let changed = sqlx::query(
                r#"UPDATE academic_timetable_blocks
                   SET is_active = false, row_version = row_version + 1,
                       updated_by = $2, updated_at = now()
                   WHERE id = $1 AND is_active"#,
            )
            .bind(block_id)
            .bind(actor_id)
            .execute(&mut *transaction)
            .await?;
            if changed.rows_affected() != 1 {
                return Err(stale_block());
            }
            increment_parent_revision = false;
        }
        TimetableTargetKind::Homeroom => {
            deactivate_child(
                &mut transaction,
                "academic_timetable_block_homerooms",
                request.target_id,
                block_id,
                request.target_row_version,
                actor_id,
            )
            .await?;
        }
        TimetableTargetKind::Teacher => {
            let confirmed: bool = sqlx::query_scalar(
                r#"SELECT EXISTS (
                       SELECT 1
                       FROM academic_timetable_block_teachers target
                       JOIN academic_timetable_block_groups block_group
                         ON block_group.block_id = target.block_id AND block_group.is_active
                       JOIN academic_timetable_block_group_instructors instructor
                         ON instructor.block_group_id = block_group.id
                        AND instructor.instructor_id = target.teacher_id
                       WHERE target.id = $1
                         AND target.block_id = $2
                         AND target.is_active
                   )"#,
            )
            .bind(request.target_id)
            .bind(block_id)
            .fetch_one(&mut *transaction)
            .await?;
            if confirmed {
                return Err(AppError::ValidationError(
                    "ครูประจำกลุ่มต้องย้ายหรือยกเลิกการมอบหมายก่อนนำออกจากคาบ".to_string(),
                ));
            }
            deactivate_child(
                &mut transaction,
                "academic_timetable_block_teachers",
                request.target_id,
                block_id,
                request.target_row_version,
                actor_id,
            )
            .await?;
        }
    }
    if block_kind == "STRUCTURAL" {
        let target_count: i64 = sqlx::query_scalar(
            r#"SELECT
                   (SELECT count(*) FROM academic_timetable_block_homerooms
                    WHERE block_id = $1 AND is_active)
                 + (SELECT count(*) FROM academic_timetable_block_teachers
                    WHERE block_id = $1 AND is_active)"#,
        )
        .bind(block_id)
        .fetch_one(&mut *transaction)
        .await?;
        if target_count == 0 {
            sqlx::query("UPDATE academic_timetable_blocks SET is_active = false WHERE id = $1")
                .bind(block_id)
                .execute(&mut *transaction)
                .await?;
        }
    }
    if increment_parent_revision {
        increment_block_revision(&mut transaction, block_id, actor_id).await?;
    }
    transaction.commit().await?;
    get_block(pool, block_id).await
}

pub async fn update_block(
    pool: &PgPool,
    actor_id: Uuid,
    block_id: Uuid,
    request: UpdateTimetableBlockRequest,
) -> Result<TimetableBlock, AppError> {
    if request.clear_title && request.title.is_some()
        || request.clear_note && request.note.is_some()
        || request.clear_room && request.room_id.is_some()
    {
        return Err(AppError::ValidationError(
            "คำสั่งแก้ไขตารางสอนมีค่าที่ขัดแย้งกัน".to_string(),
        ));
    }
    let mut transaction = pool.begin().await?;
    let block = lock_block(
        &mut transaction,
        block_id,
        request.timetable_version_id,
        request.row_version,
    )
    .await?;
    let target_period_id = request
        .bell_schedule_period_id
        .unwrap_or(block.bell_schedule_period_id);
    ensure_period(&mut transaction, block.bell_schedule_id, target_period_id).await?;
    let target_day = request
        .day_of_week
        .as_deref()
        .map(normalize_day)
        .transpose()?
        .unwrap_or_else(|| block.day_of_week.clone());

    sqlx::query("UPDATE academic_timetable_blocks SET is_active = false WHERE id = $1")
        .bind(block_id)
        .execute(&mut *transaction)
        .await?;

    if let Some(instructor_ids) = request.instructor_ids.as_ref() {
        if block.block_kind == "STRUCTURAL"
            || block.scheduling_mode.as_deref() == Some("synchronized")
        {
            return Err(AppError::ValidationError(
                "ครูของกิจกรรมพร้อมกันหรือคาบพิเศษต้องแก้จากเป้าหมายของรายการ".to_string(),
            ));
        }
        let instructor_ids = canonical_ids(instructor_ids);
        if instructor_ids.is_empty() {
            return Err(AppError::ValidationError(
                "คาบเรียนต้องมีครูอย่างน้อยหนึ่งคน".to_string(),
            ));
        }
        let block_group_id: Uuid = sqlx::query_scalar(
            r#"SELECT id FROM academic_timetable_block_groups
               WHERE block_id = $1 AND is_active"#,
        )
        .bind(block_id)
        .fetch_one(&mut *transaction)
        .await?;
        replace_group_instructors(
            &mut transaction,
            block_group_id,
            request.timetable_version_id,
            &instructor_ids,
        )
        .await?;
    }

    if let Some(teacher_ids) = request.teacher_ids.as_ref() {
        if block.scheduling_mode.as_deref() != Some("synchronized") {
            return Err(AppError::ValidationError(
                "ครูที่กันเวลาโดยตรงแก้ได้เฉพาะกิจกรรมพร้อมกัน".to_string(),
            ));
        }
        let teacher_ids = canonical_ids(teacher_ids);
        ensure_teachers(&mut transaction, &teacher_ids).await?;
        replace_block_teacher_targets(
            &mut transaction,
            block_id,
            block.academic_term_id,
            block.academic_year_id,
            &teacher_ids,
            actor_id,
        )
        .await?;
    }

    if request.room_id.is_some() || request.clear_room {
        let room_id = if request.clear_room {
            None
        } else {
            request.room_id
        };
        match block.block_kind.as_str() {
            "STRUCTURAL" => {
                sqlx::query(
                    r#"UPDATE academic_timetable_block_homerooms
                       SET room_id = $2, row_version = row_version + 1,
                           updated_by = $3, updated_at = now()
                       WHERE block_id = $1 AND is_active"#,
                )
                .bind(block_id)
                .bind(room_id)
                .bind(actor_id)
                .execute(&mut *transaction)
                .await?;
            }
            _ if block.scheduling_mode.as_deref() == Some("synchronized") => {
                sqlx::query(
                    r#"UPDATE academic_timetable_block_homerooms
                       SET room_id = $2, row_version = row_version + 1,
                           updated_by = $3, updated_at = now()
                       WHERE block_id = $1 AND is_active"#,
                )
                .bind(block_id)
                .bind(room_id)
                .bind(actor_id)
                .execute(&mut *transaction)
                .await?;
            }
            _ => {
                sqlx::query(
                    r#"UPDATE academic_timetable_block_groups
                       SET room_id = $2, row_version = row_version + 1,
                           updated_by = $3, updated_at = now()
                       WHERE block_id = $1 AND is_active"#,
                )
                .bind(block_id)
                .bind(room_id)
                .bind(actor_id)
                .execute(&mut *transaction)
                .await?;
            }
        }
    }

    let changed = sqlx::query(
        r#"UPDATE academic_timetable_blocks
           SET day_of_week = $2, bell_schedule_period_id = $3,
               title = CASE WHEN $4 THEN NULL ELSE COALESCE($5, title) END,
               note = CASE WHEN $6 THEN NULL ELSE COALESCE($7, note) END,
               is_active = true, row_version = row_version + 1,
               updated_by = $8, updated_at = now()
           WHERE id = $1 AND row_version = $9"#,
    )
    .bind(block_id)
    .bind(target_day)
    .bind(target_period_id)
    .bind(request.clear_title)
    .bind(request.title.as_deref().map(str::trim))
    .bind(request.clear_note)
    .bind(request.note.as_deref().map(str::trim))
    .bind(actor_id)
    .bind(request.row_version)
    .execute(&mut *transaction)
    .await
    .map_err(map_write_error)?;
    if changed.rows_affected() != 1 {
        return Err(stale_block());
    }
    if block.scheduling_mode.as_deref() == Some("synchronized") {
        timetable_block_sync::sync_offering_groups_in_tx(&mut transaction, block_id, actor_id)
            .await?;
    }
    transaction.commit().await?;
    get_block(pool, block_id).await
}

pub async fn swap_blocks(
    pool: &PgPool,
    actor_id: Uuid,
    request: SwapTimetableBlocksRequest,
) -> Result<SwapTimetableBlocksResponse, AppError> {
    if request.block_a_id == request.block_b_id {
        return Err(AppError::ValidationError(
            "ต้องเลือกคนละรายการเพื่อสลับคาบ".to_string(),
        ));
    }
    let mut transaction = pool.begin().await?;
    ensure_draft_version_id(&mut transaction, request.timetable_version_id).await?;
    let mut ids = [request.block_a_id, request.block_b_id];
    ids.sort_unstable();
    let locked = sqlx::query(
        r#"SELECT id FROM academic_timetable_blocks
           WHERE id = ANY($1) AND timetable_version_id=$2 AND is_active
           ORDER BY id FOR UPDATE"#,
    )
    .bind(&ids[..])
    .bind(request.timetable_version_id)
    .fetch_all(&mut *transaction)
    .await?;
    if locked.len() != 2 {
        return Err(stale_block());
    }
    let block_a = load_locked_block(
        &mut transaction,
        request.block_a_id,
        request.timetable_version_id,
    )
    .await?;
    let block_b = load_locked_block(
        &mut transaction,
        request.block_b_id,
        request.timetable_version_id,
    )
    .await?;
    if block_a.timetable_version_id != request.timetable_version_id
        || block_b.timetable_version_id != request.timetable_version_id
        || block_a.row_version != request.block_a_row_version
        || block_b.row_version != request.block_b_row_version
    {
        return Err(stale_block());
    }
    sqlx::query("UPDATE academic_timetable_blocks SET is_active = false WHERE id = ANY($1)")
        .bind(&ids[..])
        .execute(&mut *transaction)
        .await?;
    update_block_slot(
        &mut transaction,
        request.block_a_id,
        &block_b.day_of_week,
        block_b.bell_schedule_period_id,
        request.block_a_row_version,
        actor_id,
    )
    .await?;
    update_block_slot(
        &mut transaction,
        request.block_b_id,
        &block_a.day_of_week,
        block_a.bell_schedule_period_id,
        request.block_b_row_version,
        actor_id,
    )
    .await?;
    transaction.commit().await?;
    Ok(SwapTimetableBlocksResponse {
        block_a: get_block(pool, request.block_a_id).await?,
        block_b: get_block(pool, request.block_b_id).await?,
    })
}

pub async fn retry_sync(
    pool: &PgPool,
    actor_id: Uuid,
    block_id: Uuid,
    request: RetryTimetableBlockSyncRequest,
) -> Result<TimetableBlock, AppError> {
    let mut transaction = pool.begin().await?;
    let block = lock_block(
        &mut transaction,
        block_id,
        request.timetable_version_id,
        request.block_row_version,
    )
    .await?;
    if block.scheduling_mode.as_deref() != Some("synchronized") {
        return Err(AppError::ValidationError(
            "รายการนี้ไม่ใช่กิจกรรมแบบจัดพร้อมกัน".to_string(),
        ));
    }
    timetable_block_sync::retry_groups_in_tx(
        &mut transaction,
        block_id,
        actor_id,
        &canonical_ids(&request.learning_group_ids),
    )
    .await?;
    increment_block_revision(&mut transaction, block_id, actor_id).await?;
    transaction.commit().await?;
    get_block(pool, block_id).await
}

pub async fn restore_group(
    pool: &PgPool,
    actor_id: Uuid,
    block_id: Uuid,
    request: RestoreTimetableBlockGroupRequest,
) -> Result<TimetableBlock, AppError> {
    let mut transaction = pool.begin().await?;
    let block = lock_block(
        &mut transaction,
        block_id,
        request.timetable_version_id,
        request.block_row_version,
    )
    .await?;
    if block.scheduling_mode.as_deref() != Some("synchronized") {
        return Err(AppError::ValidationError(
            "รายการนี้ไม่ใช่กิจกรรมแบบจัดพร้อมกัน".to_string(),
        ));
    }
    timetable_block_sync::restore_group_in_tx(
        &mut transaction,
        block_id,
        actor_id,
        request.learning_group_id,
    )
    .await?;
    increment_block_revision(&mut transaction, block_id, actor_id).await?;
    transaction.commit().await?;
    get_block(pool, block_id).await
}

pub async fn deactivate_block(
    pool: &PgPool,
    actor_id: Uuid,
    block_id: Uuid,
    timetable_version_id: Uuid,
    row_version: i64,
) -> Result<TimetableBlock, AppError> {
    let mut transaction = pool.begin().await?;
    let _ = lock_block(
        &mut transaction,
        block_id,
        timetable_version_id,
        row_version,
    )
    .await?;
    let changed = sqlx::query(
        r#"UPDATE academic_timetable_blocks
           SET is_active = false, row_version = row_version + 1,
               updated_by = $4, updated_at = now()
           WHERE id = $1 AND timetable_version_id = $2
             AND row_version = $3 AND is_active"#,
    )
    .bind(block_id)
    .bind(timetable_version_id)
    .bind(row_version)
    .bind(actor_id)
    .execute(&mut *transaction)
    .await?;
    if changed.rows_affected() != 1 {
        return Err(stale_block());
    }
    transaction.commit().await?;
    get_block(pool, block_id).await
}

pub async fn deactivate_series(
    pool: &PgPool,
    actor_id: Uuid,
    series_id: Uuid,
    timetable_version_id: Uuid,
) -> Result<Vec<TimetableBlock>, AppError> {
    let mut transaction = pool.begin().await?;
    ensure_draft_version_id(&mut transaction, timetable_version_id).await?;
    let block_ids: Vec<Uuid> = sqlx::query_scalar(
        r#"SELECT id FROM academic_timetable_blocks
           WHERE timetable_version_id = $1 AND series_id = $2 AND is_active
           ORDER BY id FOR UPDATE"#,
    )
    .bind(timetable_version_id)
    .bind(series_id)
    .fetch_all(&mut *transaction)
    .await?;
    if block_ids.is_empty() {
        return Err(AppError::NotFound("ไม่พบชุดคาบพิเศษ".to_string()));
    }
    sqlx::query(
        r#"UPDATE academic_timetable_blocks
           SET is_active = false, row_version = row_version + 1,
               updated_by = $3, updated_at = now()
           WHERE timetable_version_id = $1 AND series_id = $2 AND is_active"#,
    )
    .bind(timetable_version_id)
    .bind(series_id)
    .bind(actor_id)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    timetable_block_queries::get_blocks(pool, &block_ids).await
}

async fn lock_block(
    transaction: &mut Transaction<'_, Postgres>,
    block_id: Uuid,
    timetable_version_id: Uuid,
    row_version: i64,
) -> Result<LockedBlock, AppError> {
    ensure_draft_version_id(transaction, timetable_version_id).await?;
    let block = load_locked_block(transaction, block_id, timetable_version_id).await?;
    if block.timetable_version_id != timetable_version_id || block.row_version != row_version {
        return Err(stale_block());
    }
    Ok(block)
}

async fn load_locked_block(
    transaction: &mut Transaction<'_, Postgres>,
    block_id: Uuid,
    version_id: Uuid,
) -> Result<LockedBlock, AppError> {
    sqlx::query_as(
        r#"SELECT timetable_version_id, academic_term_id, academic_year_id, bell_schedule_id,
                  bell_schedule_period_id, day_of_week, block_kind,
                  scheduling_mode, row_version
           FROM academic_timetable_blocks
           WHERE id = $1 AND timetable_version_id=$2 AND is_active FOR UPDATE"#,
    )
    .bind(block_id)
    .bind(version_id)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or_else(stale_block)
}

async fn ensure_draft_version_id(
    transaction: &mut Transaction<'_, Postgres>,
    version_id: Uuid,
) -> Result<(), AppError> {
    require_version_term_write(transaction, version_id).await?;
    let editable: bool = sqlx::query_scalar(
        r#"SELECT version.status = 'draft'
           FROM academic_timetable_versions version
           WHERE version.id = $1 FOR UPDATE OF version"#,
    )
    .bind(version_id)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบรุ่นตารางสอน".to_string()))?;
    if editable {
        Ok(())
    } else {
        Err(AppError::Conflict(
            "แก้ไขได้เฉพาะรุ่นตารางสอนแบบร่างในภาคเรียนที่ยังเปิดอยู่".to_string(),
        ))
    }
}

async fn replace_group_instructors(
    transaction: &mut Transaction<'_, Postgres>,
    block_group_id: Uuid,
    timetable_version_id: Uuid,
    instructor_ids: &[Uuid],
) -> Result<(), AppError> {
    let (delivery_id,term_id,group_id): (Uuid,Uuid,Uuid)=sqlx::query_as("SELECT version.delivery_version_id,version.academic_term_id,placed.learning_group_id FROM academic_timetable_block_groups placed JOIN academic_timetable_blocks block ON block.id=placed.block_id JOIN academic_timetable_versions version ON version.id=block.timetable_version_id WHERE placed.id=$1 AND version.id=$2")
        .bind(block_group_id).bind(timetable_version_id).fetch_one(&mut **transaction).await?;
    let source = school_academic_delivery::services::versions::published_source(
        transaction,
        delivery_id,
        term_id,
    )
    .await?;
    let group = source
        .snapshot
        .offerings
        .iter()
        .flat_map(|offering| offering.groups.iter())
        .find(|group| group.id == group_id)
        .ok_or_else(|| AppError::ValidationError("กลุ่มนี้ไม่มีในข้อมูลเปิดสอนใหม่ กรุณาถอดคาบ".into()))?;
    let assignments = source_assignments(group, instructor_ids)?;
    sqlx::query("DELETE FROM academic_timetable_block_group_instructors WHERE block_group_id = $1")
        .bind(block_group_id)
        .execute(&mut **transaction)
        .await?;
    for (index, teacher_id) in instructor_ids.iter().enumerate() {
        let assignment = assignments
            .iter()
            .find(|assignment| assignment.teacher_id == *teacher_id)
            .ok_or_else(|| AppError::ValidationError("ครูไม่อยู่ในข้อมูลเปิดสอนของกลุ่ม".into()))?;
        sqlx::query(
            r#"INSERT INTO academic_timetable_block_group_instructors (
                   id, block_group_id, instructor_id, role, display_order
               ) VALUES (gen_random_uuid(), $1, $2, $3, $4)"#,
        )
        .bind(block_group_id)
        .bind(teacher_id)
        .bind(&assignment.role)
        .bind((index + 1) as i32)
        .execute(&mut **transaction)
        .await?;
    }
    Ok(())
}

async fn update_block_slot(
    transaction: &mut Transaction<'_, Postgres>,
    block_id: Uuid,
    day_of_week: &str,
    period_id: Uuid,
    row_version: i64,
    actor_id: Uuid,
) -> Result<(), AppError> {
    let changed = sqlx::query(
        r#"UPDATE academic_timetable_blocks
           SET day_of_week = $2, bell_schedule_period_id = $3, is_active = true,
               row_version = row_version + 1, updated_by = $5, updated_at = now()
           WHERE id = $1 AND row_version = $4"#,
    )
    .bind(block_id)
    .bind(day_of_week)
    .bind(period_id)
    .bind(row_version)
    .bind(actor_id)
    .execute(&mut **transaction)
    .await
    .map_err(map_write_error)?;
    if changed.rows_affected() == 1 {
        Ok(())
    } else {
        Err(stale_block())
    }
}

async fn increment_block_revision(
    transaction: &mut Transaction<'_, Postgres>,
    block_id: Uuid,
    actor_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query(
        r#"UPDATE academic_timetable_blocks
           SET row_version = row_version + 1, updated_by = $2, updated_at = now()
           WHERE id = $1"#,
    )
    .bind(block_id)
    .bind(actor_id)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn lock_draft_version(
    transaction: &mut Transaction<'_, Postgres>,
    version_id: Uuid,
    academic_term_id: Uuid,
    period_id: Uuid,
) -> Result<VersionContext, AppError> {
    require_version_term_write(transaction, version_id).await?;
    let version: VersionContext = sqlx::query_as(
        r#"SELECT version.academic_term_id, version.academic_year_id,
                  version.bell_schedule_id, version.delivery_version_id, version.status
           FROM academic_timetable_versions version
           WHERE version.id = $1 AND version.academic_term_id = $2
           FOR UPDATE OF version"#,
    )
    .bind(version_id)
    .bind(academic_term_id)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบรุ่นตารางสอนในภาคเรียนที่เลือก".to_string()))?;
    if version.status != "draft" {
        return Err(AppError::Conflict(
            "แก้ไขได้เฉพาะรุ่นตารางสอนแบบร่าง".to_string(),
        ));
    }
    ensure_period(transaction, version.bell_schedule_id, period_id).await?;
    Ok(version)
}

async fn ensure_period(
    transaction: &mut Transaction<'_, Postgres>,
    bell_schedule_id: Uuid,
    period_id: Uuid,
) -> Result<(), AppError> {
    let valid: bool = sqlx::query_scalar(
        r#"SELECT EXISTS (
               SELECT 1 FROM bell_schedule_periods
               WHERE id = $1 AND bell_schedule_id = $2 AND is_active
           )"#,
    )
    .bind(period_id)
    .bind(bell_schedule_id)
    .fetch_one(&mut **transaction)
    .await?;
    if valid {
        Ok(())
    } else {
        Err(AppError::ValidationError(
            "คาบไม่อยู่ในตารางเวลาของภาคเรียน".to_string(),
        ))
    }
}

fn source_assignments(
    group: &school_academic_delivery::models::versions::DeliveryVersionGroup,
    ids: &[Uuid],
) -> Result<Vec<InstructorAssignment>, AppError> {
    let assignments = group
        .teachers
        .iter()
        .filter(|teacher| ids.contains(&teacher.teacher_id))
        .map(|teacher| InstructorAssignment {
            teacher_id: teacher.teacher_id,
            role: super::timetable_source::role_text(teacher.role).into(),
        })
        .collect::<Vec<_>>();
    if assignments.len() != ids.len() || ids.is_empty() {
        return Err(AppError::ValidationError(
            "ครูที่เลือกต้องเป็นครูของกลุ่มตามรุ่นเปิดสอนที่ตารางอ้างอิง".into(),
        ));
    }
    Ok(assignments)
}

async fn ensure_homerooms(
    transaction: &mut Transaction<'_, Postgres>,
    academic_year_id: Uuid,
    homeroom_ids: &[Uuid],
) -> Result<(), AppError> {
    let count: i64 = sqlx::query_scalar(
        r#"SELECT count(*) FROM homerooms
           WHERE id = ANY($1) AND academic_year_id = $2 AND is_active"#,
    )
    .bind(homeroom_ids)
    .bind(academic_year_id)
    .fetch_one(&mut **transaction)
    .await?;
    if count == homeroom_ids.len() as i64 {
        Ok(())
    } else {
        Err(AppError::ValidationError(
            "ห้องประจำชั้นบางรายการไม่อยู่ในปีการศึกษาที่เลือก".to_string(),
        ))
    }
}

async fn ensure_teachers(
    transaction: &mut Transaction<'_, Postgres>,
    teacher_ids: &[Uuid],
) -> Result<(), AppError> {
    let count: i64 = sqlx::query_scalar(
        r#"SELECT count(*) FROM users
           WHERE id = ANY($1) AND user_type = 'staff' AND status = 'active'"#,
    )
    .bind(teacher_ids)
    .fetch_one(&mut **transaction)
    .await?;
    if count == teacher_ids.len() as i64 {
        Ok(())
    } else {
        Err(AppError::ValidationError(
            "ครูบางรายการไม่พร้อมใช้งาน".to_string(),
        ))
    }
}

async fn replace_block_teacher_targets(
    transaction: &mut Transaction<'_, Postgres>,
    block_id: Uuid,
    academic_term_id: Uuid,
    academic_year_id: Uuid,
    teacher_ids: &[Uuid],
    actor_id: Uuid,
) -> Result<(), AppError> {
    let confirmed_removal_exists: bool = sqlx::query_scalar(
        r#"SELECT EXISTS (
               SELECT 1
               FROM academic_timetable_block_teachers target
               JOIN academic_timetable_block_groups block_group
                 ON block_group.block_id = target.block_id AND block_group.is_active
               JOIN academic_timetable_block_group_instructors instructor
                 ON instructor.block_group_id = block_group.id
                AND instructor.instructor_id = target.teacher_id
               WHERE target.block_id = $1
                 AND target.is_active
                 AND NOT (target.teacher_id = ANY($2))
           )"#,
    )
    .bind(block_id)
    .bind(teacher_ids)
    .fetch_one(&mut **transaction)
    .await?;
    if confirmed_removal_exists {
        return Err(AppError::ValidationError(
            "ครูประจำกลุ่มต้องย้ายหรือยกเลิกการมอบหมายก่อนนำออกจากคาบ".to_string(),
        ));
    }

    sqlx::query(
        r#"UPDATE academic_timetable_block_teachers
           SET is_active = false, row_version = row_version + 1,
               updated_by = $3, updated_at = now()
           WHERE block_id = $1
             AND is_active
             AND NOT (teacher_id = ANY($2))"#,
    )
    .bind(block_id)
    .bind(teacher_ids)
    .bind(actor_id)
    .execute(&mut **transaction)
    .await?;

    for teacher_id in teacher_ids {
        sqlx::query(
            r#"INSERT INTO academic_timetable_block_teachers (
                   id, block_id, teacher_id, academic_term_id, academic_year_id,
                   created_by, updated_by
               ) VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, $5)
               ON CONFLICT (block_id, teacher_id) DO UPDATE
               SET is_active = true,
                   row_version = academic_timetable_block_teachers.row_version + 1,
                   updated_by = EXCLUDED.updated_by,
                   updated_at = now()
               WHERE NOT academic_timetable_block_teachers.is_active"#,
        )
        .bind(block_id)
        .bind(teacher_id)
        .bind(academic_term_id)
        .bind(academic_year_id)
        .bind(actor_id)
        .execute(&mut **transaction)
        .await
        .map_err(map_write_error)?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn insert_block(
    transaction: &mut Transaction<'_, Postgres>,
    block_id: Uuid,
    version: &VersionContext,
    timetable_version_id: Uuid,
    bell_schedule_period_id: Uuid,
    day_of_week: &str,
    block_kind: &str,
    scheduling_mode: Option<&str>,
    learning_offering_id: Option<Uuid>,
    structural_kind: Option<&str>,
    title: Option<&str>,
    note: Option<&str>,
    series_id: Option<Uuid>,
    actor_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query(
        r#"INSERT INTO academic_timetable_blocks (
               id, timetable_version_id, academic_term_id, academic_year_id,
               bell_schedule_id, bell_schedule_period_id, day_of_week,
               block_kind, scheduling_mode, learning_offering_id, structural_kind,
               title, note, series_id, created_by, updated_by
           ) VALUES (
               $1, $2, $3, $4, $5, $6, $7,
               $8, $9, $10, $11, $12, $13, $14, $15, $15
           )"#,
    )
    .bind(block_id)
    .bind(timetable_version_id)
    .bind(version.academic_term_id)
    .bind(version.academic_year_id)
    .bind(version.bell_schedule_id)
    .bind(bell_schedule_period_id)
    .bind(day_of_week)
    .bind(block_kind)
    .bind(scheduling_mode)
    .bind(learning_offering_id)
    .bind(structural_kind)
    .bind(title)
    .bind(note)
    .bind(series_id)
    .bind(actor_id)
    .execute(&mut **transaction)
    .await
    .map_err(map_write_error)?;
    Ok(())
}

async fn deactivate_child(
    transaction: &mut Transaction<'_, Postgres>,
    table: &'static str,
    target_id: Uuid,
    block_id: Uuid,
    row_version: i64,
    actor_id: Uuid,
) -> Result<(), AppError> {
    let query = format!(
        "UPDATE {table} SET is_active = false, row_version = row_version + 1, \
         updated_by = $4, updated_at = now() \
         WHERE id = $1 AND block_id = $2 AND row_version = $3 AND is_active"
    );
    let changed = sqlx::query(sqlx::AssertSqlSafe(query))
        .bind(target_id)
        .bind(block_id)
        .bind(row_version)
        .bind(actor_id)
        .execute(&mut **transaction)
        .await?;
    if changed.rows_affected() == 1 {
        Ok(())
    } else {
        Err(stale_block())
    }
}

fn structural_kind_wire(kind: TimetableStructuralKind) -> &'static str {
    match kind {
        TimetableStructuralKind::Break => "BREAK",
        TimetableStructuralKind::Homeroom => "HOMEROOM",
        TimetableStructuralKind::FlagCeremony => "FLAG_CEREMONY",
        TimetableStructuralKind::TeacherMeeting => "TEACHER_MEETING",
        TimetableStructuralKind::Academic => "ACADEMIC",
        TimetableStructuralKind::Other => "OTHER",
    }
}

fn stale_block() -> AppError {
    AppError::Conflict("ตารางสอนถูกแก้ไขจากผู้ใช้อื่น กรุณาโหลดใหม่".to_string())
}
