use sqlx::{FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::academic::models::timetable_block::{
    CreateOrdinaryTimetableBlockRequest, CreateStructuralTimetableBlocksRequest,
    CreateSynchronizedTimetableBlockRequest, RemoveTimetableBlockTargetRequest,
    RestoreTimetableBlockGroupRequest, RetryTimetableBlockSyncRequest, SwapTimetableBlocksRequest,
    SwapTimetableBlocksResponse, TimetableBlock, TimetableBlockPlacementPreview,
    TimetableBlockPlacementPreviewRequest, TimetableBlockWorkspace, TimetableBlockWorkspaceQuery,
    TimetableStructuralKind, TimetableTargetKind, UpdateTimetableBlockRequest,
};
use crate::policies::timetable_access_policy::TimetableAccessFilter;

use super::timetable_block_conflicts::{canonical_ids, map_write_error, normalize_day};
use super::timetable_block_queries;
use super::timetable_block_sync;

#[derive(Debug, FromRow)]
struct VersionContext {
    academic_term_id: Uuid,
    academic_year_id: Uuid,
    bell_schedule_id: Uuid,
    status: String,
    term_status: String,
}

#[derive(Debug, FromRow)]
struct GroupContext {
    learning_offering_id: Uuid,
    academic_term_id: Uuid,
    academic_year_id: Uuid,
    offering_kind: String,
    scheduling_mode: Option<String>,
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
    bell_schedule_id: Uuid,
    bell_schedule_period_id: Uuid,
    day_of_week: String,
    block_kind: String,
    scheduling_mode: Option<String>,
    row_version: i64,
    series_id: Option<Uuid>,
}

pub async fn get_block(pool: &PgPool, block_id: Uuid) -> Result<TimetableBlock, AppError> {
    timetable_block_queries::get_block(pool, block_id).await
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
    let group: GroupContext = sqlx::query_as(
        r#"SELECT learning_group.learning_offering_id,
                  learning_group.academic_term_id,
                  learning_group.academic_year_id,
                  offering.kind AS offering_kind,
                  activity_detail.scheduling_mode
           FROM learning_groups learning_group
           JOIN learning_offerings offering
             ON offering.id = learning_group.learning_offering_id
           LEFT JOIN activity_offering_details activity_detail
             ON activity_detail.learning_offering_id = offering.id
           WHERE learning_group.id = $1"#,
    )
    .bind(request.learning_group_id)
    .fetch_optional(&mut *transaction)
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบกลุ่มเรียน".to_string()))?;
    if group.academic_term_id != version.academic_term_id
        || group.academic_year_id != version.academic_year_id
    {
        return Err(AppError::BadRequest(
            "กลุ่มเรียนไม่อยู่ในปีและภาคเรียนของรุ่นตารางสอน".to_string(),
        ));
    }
    if group.scheduling_mode.as_deref() == Some("synchronized") {
        return Err(AppError::ValidationError(
            "กิจกรรมแบบพร้อมกันต้องวางจากช่วงกิจกรรมหลัก".to_string(),
        ));
    }
    ensure_version_offering_target(
        &mut transaction,
        request.timetable_version_id,
        group.learning_offering_id,
    )
    .await?;
    let assignments: Vec<InstructorAssignment> = sqlx::query_as(
        r#"SELECT teacher.teacher_id, teacher.role
           FROM learning_group_teachers teacher
           JOIN academic_timetable_versions version ON version.id = $2
           WHERE teacher.learning_group_id = $1
             AND teacher.teacher_id = ANY($3)
             AND teacher.starts_on <= version.effective_from
             AND (teacher.ends_on IS NULL OR teacher.ends_on >= version.effective_from)
           ORDER BY teacher.teacher_id"#,
    )
    .bind(request.learning_group_id)
    .bind(request.timetable_version_id)
    .bind(&instructor_ids)
    .fetch_all(&mut *transaction)
    .await?;
    if assignments.len() != instructor_ids.len() {
        return Err(AppError::ValidationError(
            "ครูที่เลือกต้องเป็นครูของกลุ่มเรียนในวันที่รุ่นตารางเริ่มใช้".to_string(),
        ));
    }

    let block_id = Uuid::new_v4();
    let block_kind = if group.offering_kind == "course" {
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
        group.scheduling_mode.as_deref(),
        Some(group.learning_offering_id),
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
               academic_term_id, academic_year_id, room_id, created_by, updated_by
           ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $8)"#,
    )
    .bind(block_group_id)
    .bind(block_id)
    .bind(request.learning_group_id)
    .bind(group.learning_offering_id)
    .bind(version.academic_term_id)
    .bind(version.academic_year_id)
    .bind(request.room_id)
    .bind(actor_id)
    .execute(&mut *transaction)
    .await
    .map_err(map_write_error)?;
    for (index, teacher_id) in instructor_ids.iter().enumerate() {
        let assignment = assignments
            .iter()
            .find(|assignment| assignment.teacher_id == *teacher_id)
            .expect("validated assignment set must contain every selected teacher");
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
    let offering_valid: bool = sqlx::query_scalar(
        r#"SELECT EXISTS (
               SELECT 1
               FROM learning_offerings offering
               JOIN activity_offering_details detail
                 ON detail.learning_offering_id = offering.id
               WHERE offering.id = $1
                 AND offering.academic_term_id = $2
                 AND offering.academic_year_id = $3
                 AND offering.kind = 'activity'
                 AND detail.scheduling_mode = 'synchronized'
           )"#,
    )
    .bind(request.learning_offering_id)
    .bind(version.academic_term_id)
    .bind(version.academic_year_id)
    .fetch_one(&mut *transaction)
    .await?;
    if !offering_valid {
        return Err(AppError::ValidationError(
            "รายการนี้ไม่ใช่กิจกรรมแบบจัดพร้อมกันในภาคเรียนที่เลือก".to_string(),
        ));
    }
    ensure_version_offering_target(
        &mut transaction,
        request.timetable_version_id,
        request.learning_offering_id,
    )
    .await?;
    ensure_homerooms(&mut transaction, version.academic_year_id, &homeroom_ids).await?;
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
    let mut ids = [request.block_a_id, request.block_b_id];
    ids.sort_unstable();
    sqlx::query(
        r#"SELECT id FROM academic_timetable_blocks
           WHERE id = ANY($1) ORDER BY id FOR UPDATE"#,
    )
    .bind(&ids[..])
    .fetch_all(&mut *transaction)
    .await?;
    let block_a = load_locked_block(&mut transaction, request.block_a_id).await?;
    let block_b = load_locked_block(&mut transaction, request.block_b_id).await?;
    if block_a.timetable_version_id != request.timetable_version_id
        || block_b.timetable_version_id != request.timetable_version_id
        || block_a.row_version != request.block_a_row_version
        || block_b.row_version != request.block_b_row_version
    {
        return Err(stale_block());
    }
    ensure_draft_version_id(&mut transaction, request.timetable_version_id).await?;
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
    let block = load_locked_block(transaction, block_id).await?;
    if block.timetable_version_id != timetable_version_id || block.row_version != row_version {
        return Err(stale_block());
    }
    ensure_draft_version_id(transaction, timetable_version_id).await?;
    Ok(block)
}

async fn load_locked_block(
    transaction: &mut Transaction<'_, Postgres>,
    block_id: Uuid,
) -> Result<LockedBlock, AppError> {
    sqlx::query_as(
        r#"SELECT timetable_version_id, academic_term_id, bell_schedule_id,
                  bell_schedule_period_id, day_of_week, block_kind,
                  scheduling_mode, row_version, series_id
           FROM academic_timetable_blocks
           WHERE id = $1 AND is_active FOR UPDATE"#,
    )
    .bind(block_id)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or_else(stale_block)
}

async fn ensure_draft_version_id(
    transaction: &mut Transaction<'_, Postgres>,
    version_id: Uuid,
) -> Result<(), AppError> {
    let editable: bool = sqlx::query_scalar(
        r#"SELECT version.status = 'draft' AND term.status NOT IN ('closed', 'cancelled')
           FROM academic_timetable_versions version
           JOIN academic_terms term ON term.id = version.academic_term_id
           WHERE version.id = $1 FOR UPDATE OF version, term"#,
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
    let assignments: Vec<InstructorAssignment> = sqlx::query_as(
        r#"SELECT assignment.teacher_id, assignment.role
           FROM academic_timetable_block_groups block_group
           JOIN learning_group_teachers assignment
             ON assignment.learning_group_id = block_group.learning_group_id
           JOIN academic_timetable_versions version ON version.id = $2
           WHERE block_group.id = $1
             AND assignment.teacher_id = ANY($3)
             AND assignment.starts_on <= version.effective_from
             AND (assignment.ends_on IS NULL OR assignment.ends_on >= version.effective_from)
           ORDER BY assignment.teacher_id"#,
    )
    .bind(block_group_id)
    .bind(timetable_version_id)
    .bind(instructor_ids)
    .fetch_all(&mut **transaction)
    .await?;
    if assignments.len() != instructor_ids.len() {
        return Err(AppError::ValidationError(
            "ครูที่เลือกต้องเป็นครูของกลุ่มเรียนในวันที่รุ่นตารางเริ่มใช้".to_string(),
        ));
    }
    sqlx::query("DELETE FROM academic_timetable_block_group_instructors WHERE block_group_id = $1")
        .bind(block_group_id)
        .execute(&mut **transaction)
        .await?;
    for (index, teacher_id) in instructor_ids.iter().enumerate() {
        let assignment = assignments
            .iter()
            .find(|assignment| assignment.teacher_id == *teacher_id)
            .expect("validated instructor set must contain every selected teacher");
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
    let version: VersionContext = sqlx::query_as(
        r#"SELECT version.academic_term_id, version.academic_year_id,
                  version.bell_schedule_id, version.status,
                  term.status AS term_status
           FROM academic_timetable_versions version
           JOIN academic_terms term ON term.id = version.academic_term_id
           WHERE version.id = $1 AND version.academic_term_id = $2
           FOR UPDATE OF version, term"#,
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
    if matches!(version.term_status.as_str(), "closed" | "cancelled") {
        return Err(AppError::Conflict(
            "ภาคเรียนนี้ปิดแล้ว ไม่สามารถแก้ตารางสอนได้".to_string(),
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

async fn ensure_version_offering_target(
    transaction: &mut Transaction<'_, Postgres>,
    version_id: Uuid,
    offering_id: Uuid,
) -> Result<(), AppError> {
    let exists: bool = sqlx::query_scalar(
        r#"SELECT EXISTS (
               SELECT 1 FROM academic_timetable_version_targets
               WHERE timetable_version_id = $1 AND learning_offering_id = $2
           )"#,
    )
    .bind(version_id)
    .bind(offering_id)
    .fetch_one(&mut **transaction)
    .await?;
    if exists {
        Ok(())
    } else {
        Err(AppError::ValidationError(
            "รายการเปิดสอนไม่อยู่ในเป้าหมายของรุ่นตารางสอน".to_string(),
        ))
    }
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
    table: &str,
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
    let changed = sqlx::query(&query)
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
