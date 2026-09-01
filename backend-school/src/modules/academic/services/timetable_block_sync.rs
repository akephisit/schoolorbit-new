use sqlx::{FromRow, Postgres, Transaction};
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::academic::models::timetable_block::{
    TimetableBlockSyncState, TimetableBlockSyncStatus,
};

#[derive(Debug, FromRow)]
struct SyncBlockContext {
    learning_offering_id: Uuid,
    academic_term_id: Uuid,
    academic_year_id: Uuid,
    effective_from: chrono::NaiveDate,
}

#[derive(Debug, FromRow)]
struct SyncGroupRow {
    id: Uuid,
    row_version: i64,
}

#[derive(Debug, FromRow)]
struct SyncStateRow {
    id: Uuid,
    learning_group_id: Uuid,
    learning_offering_id: Uuid,
    status: String,
    linked_block_group_id: Option<Uuid>,
    conflict_code: Option<String>,
    conflict_message: Option<String>,
    attempted_group_row_version: Option<i64>,
    row_version: i64,
}

pub(crate) async fn sync_offering_groups_in_tx(
    transaction: &mut Transaction<'_, Postgres>,
    block_id: Uuid,
    actor_id: Uuid,
) -> Result<Vec<TimetableBlockSyncState>, AppError> {
    sync_groups_in_tx(transaction, block_id, actor_id, &[], false).await
}

pub(crate) async fn retry_groups_in_tx(
    transaction: &mut Transaction<'_, Postgres>,
    block_id: Uuid,
    actor_id: Uuid,
    learning_group_ids: &[Uuid],
) -> Result<Vec<TimetableBlockSyncState>, AppError> {
    sync_groups_in_tx(transaction, block_id, actor_id, learning_group_ids, false).await
}

pub(crate) async fn restore_group_in_tx(
    transaction: &mut Transaction<'_, Postgres>,
    block_id: Uuid,
    actor_id: Uuid,
    learning_group_id: Uuid,
) -> Result<Vec<TimetableBlockSyncState>, AppError> {
    sqlx::query(
        r#"UPDATE academic_timetable_block_group_sync
           SET status = 'WAITING_FOR_DATA', linked_block_group_id = NULL,
               conflict_code = NULL, conflict_message = NULL,
               row_version = row_version + 1, updated_by = $3, updated_at = now()
           WHERE block_id = $1 AND learning_group_id = $2 AND status = 'EXCLUDED'"#,
    )
    .bind(block_id)
    .bind(learning_group_id)
    .bind(actor_id)
    .execute(&mut **transaction)
    .await?;
    sync_groups_in_tx(transaction, block_id, actor_id, &[learning_group_id], true).await
}

async fn sync_groups_in_tx(
    transaction: &mut Transaction<'_, Postgres>,
    block_id: Uuid,
    actor_id: Uuid,
    selected_group_ids: &[Uuid],
    include_excluded: bool,
) -> Result<Vec<TimetableBlockSyncState>, AppError> {
    let context: SyncBlockContext = sqlx::query_as(
        r#"SELECT block.learning_offering_id, block.academic_term_id,
                  block.academic_year_id, version.effective_from
           FROM academic_timetable_blocks block
           JOIN academic_timetable_versions version ON version.id = block.timetable_version_id
           WHERE block.id = $1
             AND block.is_active
             AND block.block_kind = 'ACTIVITY'
             AND block.scheduling_mode = 'synchronized'
             AND version.status = 'draft'
           FOR UPDATE OF block, version"#,
    )
    .bind(block_id)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or_else(|| AppError::Conflict("ซิงค์ได้เฉพาะช่วงกิจกรรมพร้อมกันในรุ่นแบบร่าง".to_string()))?;

    let selected = canonical_ids(selected_group_ids);
    let groups: Vec<SyncGroupRow> = sqlx::query_as(
        r#"SELECT learning_group.id, learning_group.row_version
           FROM learning_groups learning_group
           WHERE learning_group.learning_offering_id = $1
             AND learning_group.academic_term_id = $2
             AND learning_group.academic_year_id = $3
             AND (cardinality($4::uuid[]) = 0 OR learning_group.id = ANY($4))
           ORDER BY learning_group.id"#,
    )
    .bind(context.learning_offering_id)
    .bind(context.academic_term_id)
    .bind(context.academic_year_id)
    .bind(&selected)
    .fetch_all(&mut **transaction)
    .await?;

    for group in groups {
        let existing_status: Option<String> = sqlx::query_scalar(
            r#"SELECT status FROM academic_timetable_block_group_sync
               WHERE block_id = $1 AND learning_group_id = $2"#,
        )
        .bind(block_id)
        .bind(group.id)
        .fetch_optional(&mut **transaction)
        .await?;
        if existing_status.as_deref() == Some("EXCLUDED") && !include_excluded {
            continue;
        }

        let homeroom_ids: Vec<Uuid> = sqlx::query_scalar(
            r#"SELECT homeroom_id FROM learning_group_homerooms
               WHERE learning_group_id = $1 ORDER BY homeroom_id"#,
        )
        .bind(group.id)
        .fetch_all(&mut **transaction)
        .await?;
        let inside_scope: bool = !homeroom_ids.is_empty()
            && sqlx::query_scalar(
                r#"SELECT count(*) = cardinality($3::uuid[])
                   FROM academic_timetable_block_homerooms target
                   WHERE target.block_id = $1 AND target.is_active
                     AND target.homeroom_id = ANY($3)
                     AND target.academic_year_id = $2"#,
            )
            .bind(block_id)
            .bind(context.academic_year_id)
            .bind(&homeroom_ids)
            .fetch_one(&mut **transaction)
            .await?;
        if !inside_scope {
            remove_group_allocation(transaction, block_id, group.id, actor_id).await?;
            upsert_sync_state(
                transaction,
                block_id,
                &context,
                group.id,
                group.row_version,
                "OUTSIDE_SCOPE",
                None,
                Some("TIMETABLE_SYNC_OUTSIDE_RESERVED_HOMEROOMS"),
                Some("ห้องประจำชั้นของกลุ่มอยู่นอกขอบเขตกิจกรรมที่จองไว้"),
                actor_id,
            )
            .await?;
            continue;
        }

        let teachers: Vec<(Uuid, String)> = sqlx::query_as(
            r#"SELECT assignment.teacher_id, assignment.role
               FROM learning_group_teachers assignment
               JOIN users account ON account.id = assignment.teacher_id
               WHERE assignment.learning_group_id = $1
                 AND assignment.starts_on <= $2
                 AND (assignment.ends_on IS NULL OR assignment.ends_on >= $2)
                 AND account.user_type = 'staff' AND account.status = 'active'
               ORDER BY CASE assignment.role
                            WHEN 'primary' THEN 1
                            WHEN 'secondary' THEN 2
                            ELSE 3
                        END,
                        assignment.starts_on, assignment.id"#,
        )
        .bind(group.id)
        .bind(context.effective_from)
        .fetch_all(&mut **transaction)
        .await?;
        if teachers.is_empty() {
            remove_group_allocation(transaction, block_id, group.id, actor_id).await?;
            upsert_sync_state(
                transaction,
                block_id,
                &context,
                group.id,
                group.row_version,
                "WAITING_FOR_DATA",
                None,
                Some("TIMETABLE_SYNC_MISSING_INSTRUCTOR"),
                Some("กลุ่มกิจกรรมยังไม่มีครูที่มีผลในวันที่รุ่นตารางเริ่มใช้"),
                actor_id,
            )
            .await?;
            continue;
        }

        sqlx::query("SAVEPOINT timetable_sync_group")
            .execute(&mut **transaction)
            .await?;
        let allocation = write_group_allocation(
            transaction,
            block_id,
            &context,
            group.id,
            &teachers,
            actor_id,
        )
        .await;
        match allocation {
            Ok(block_group_id) => {
                sqlx::query("RELEASE SAVEPOINT timetable_sync_group")
                    .execute(&mut **transaction)
                    .await?;
                upsert_sync_state(
                    transaction,
                    block_id,
                    &context,
                    group.id,
                    group.row_version,
                    "LINKED",
                    Some(block_group_id),
                    None,
                    None,
                    actor_id,
                )
                .await?;
            }
            Err(error) => {
                let (code, message) = sync_conflict(&error);
                sqlx::query("ROLLBACK TO SAVEPOINT timetable_sync_group")
                    .execute(&mut **transaction)
                    .await?;
                sqlx::query("RELEASE SAVEPOINT timetable_sync_group")
                    .execute(&mut **transaction)
                    .await?;
                remove_group_allocation(transaction, block_id, group.id, actor_id).await?;
                upsert_sync_state(
                    transaction,
                    block_id,
                    &context,
                    group.id,
                    group.row_version,
                    "CONFLICT",
                    None,
                    Some(code),
                    Some(message),
                    actor_id,
                )
                .await?;
            }
        }
    }

    load_states(transaction, block_id).await
}

async fn write_group_allocation(
    transaction: &mut Transaction<'_, Postgres>,
    block_id: Uuid,
    context: &SyncBlockContext,
    learning_group_id: Uuid,
    teachers: &[(Uuid, String)],
    actor_id: Uuid,
) -> Result<Uuid, sqlx::Error> {
    let room_id: Option<Uuid> = sqlx::query_scalar(
        r#"SELECT room_id FROM learning_group_preferred_rooms
           WHERE learning_group_id = $1
           ORDER BY rank, id LIMIT 1"#,
    )
    .bind(learning_group_id)
    .fetch_optional(&mut **transaction)
    .await?;
    let block_group_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO academic_timetable_block_groups (
               id, block_id, learning_group_id, learning_offering_id,
               academic_term_id, academic_year_id, room_id,
               created_by, updated_by
           ) VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, $6, $7, $7)
           ON CONFLICT (block_id, learning_group_id) DO UPDATE
           SET room_id = EXCLUDED.room_id, is_active = true,
               row_version = academic_timetable_block_groups.row_version + 1,
               updated_by = EXCLUDED.updated_by, updated_at = now()
           RETURNING id"#,
    )
    .bind(block_id)
    .bind(learning_group_id)
    .bind(context.learning_offering_id)
    .bind(context.academic_term_id)
    .bind(context.academic_year_id)
    .bind(room_id)
    .bind(actor_id)
    .fetch_one(&mut **transaction)
    .await?;
    sqlx::query("DELETE FROM academic_timetable_block_group_instructors WHERE block_group_id = $1")
        .bind(block_group_id)
        .execute(&mut **transaction)
        .await?;
    for (index, (teacher_id, role)) in teachers.iter().enumerate() {
        sqlx::query(
            r#"INSERT INTO academic_timetable_block_group_instructors (
                   id, block_group_id, instructor_id, role, display_order
               ) VALUES (gen_random_uuid(), $1, $2, $3, $4)"#,
        )
        .bind(block_group_id)
        .bind(teacher_id)
        .bind(role)
        .bind((index + 1) as i32)
        .execute(&mut **transaction)
        .await?;
    }
    Ok(block_group_id)
}

async fn remove_group_allocation(
    transaction: &mut Transaction<'_, Postgres>,
    block_id: Uuid,
    learning_group_id: Uuid,
    actor_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query(
        r#"UPDATE academic_timetable_block_groups
           SET is_active = false, row_version = row_version + 1,
               updated_by = $3, updated_at = now()
           WHERE block_id = $1 AND learning_group_id = $2 AND is_active"#,
    )
    .bind(block_id)
    .bind(learning_group_id)
    .bind(actor_id)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn upsert_sync_state(
    transaction: &mut Transaction<'_, Postgres>,
    block_id: Uuid,
    context: &SyncBlockContext,
    learning_group_id: Uuid,
    group_row_version: i64,
    status: &str,
    linked_block_group_id: Option<Uuid>,
    conflict_code: Option<&str>,
    conflict_message: Option<&str>,
    actor_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query(
        r#"INSERT INTO academic_timetable_block_group_sync (
               id, block_id, learning_group_id, learning_offering_id,
               academic_term_id, academic_year_id, status, linked_block_group_id,
               conflict_code, conflict_message, attempted_group_row_version,
               created_by, updated_by
           ) VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $11)
           ON CONFLICT (block_id, learning_group_id) DO UPDATE
           SET status = EXCLUDED.status,
               linked_block_group_id = EXCLUDED.linked_block_group_id,
               conflict_code = EXCLUDED.conflict_code,
               conflict_message = EXCLUDED.conflict_message,
               attempted_group_row_version = EXCLUDED.attempted_group_row_version,
               row_version = academic_timetable_block_group_sync.row_version + 1,
               updated_by = EXCLUDED.updated_by, updated_at = now()"#,
    )
    .bind(block_id)
    .bind(learning_group_id)
    .bind(context.learning_offering_id)
    .bind(context.academic_term_id)
    .bind(context.academic_year_id)
    .bind(status)
    .bind(linked_block_group_id)
    .bind(conflict_code)
    .bind(conflict_message)
    .bind(group_row_version)
    .bind(actor_id)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn load_states(
    transaction: &mut Transaction<'_, Postgres>,
    block_id: Uuid,
) -> Result<Vec<TimetableBlockSyncState>, AppError> {
    let rows: Vec<SyncStateRow> = sqlx::query_as(
        r#"SELECT id, learning_group_id, learning_offering_id, status,
                  linked_block_group_id, conflict_code, conflict_message,
                  attempted_group_row_version, row_version
           FROM academic_timetable_block_group_sync
           WHERE block_id = $1 ORDER BY learning_group_id"#,
    )
    .bind(block_id)
    .fetch_all(&mut **transaction)
    .await?;
    rows.into_iter()
        .map(|row| {
            Ok(TimetableBlockSyncState {
                id: row.id,
                learning_group_id: row.learning_group_id,
                learning_offering_id: row.learning_offering_id,
                status: match row.status.as_str() {
                    "LINKED" => TimetableBlockSyncStatus::Linked,
                    "WAITING_FOR_DATA" => TimetableBlockSyncStatus::WaitingForData,
                    "CONFLICT" => TimetableBlockSyncStatus::Conflict,
                    "OUTSIDE_SCOPE" => TimetableBlockSyncStatus::OutsideScope,
                    "EXCLUDED" => TimetableBlockSyncStatus::Excluded,
                    _ => {
                        return Err(AppError::InternalServerError(
                            "สถานะซิงค์ตารางสอนไม่ถูกต้อง".to_string(),
                        ))
                    }
                },
                linked_block_group_id: row.linked_block_group_id,
                conflict_code: row.conflict_code,
                conflict_message: row.conflict_message,
                attempted_group_row_version: row.attempted_group_row_version,
                row_version: row.row_version,
            })
        })
        .collect()
}

fn sync_conflict(error: &sqlx::Error) -> (&'static str, &'static str) {
    let message = match error {
        sqlx::Error::Database(database) => database.message(),
        _ => "",
    };
    match message {
        "ACADEMIC_TIMETABLE_GROUP_CONFLICT" => (
            "TIMETABLE_SYNC_GROUP_CONFLICT",
            "กลุ่มกิจกรรมมีคาบอื่นอยู่ในช่วงเวลานี้",
        ),
        "ACADEMIC_TIMETABLE_HOMEROOM_CONFLICT" => (
            "TIMETABLE_SYNC_HOMEROOM_CONFLICT",
            "ห้องประจำชั้นของกลุ่มมีคาบอื่นอยู่ในช่วงเวลานี้",
        ),
        "ACADEMIC_TIMETABLE_INSTRUCTOR_DOUBLE_BOOKED" => (
            "TIMETABLE_SYNC_TEACHER_CONFLICT",
            "ครูของกลุ่มมีคาบอื่นอยู่ในช่วงเวลานี้",
        ),
        "ACADEMIC_TIMETABLE_ROOM_CONFLICT" => (
            "TIMETABLE_SYNC_ROOM_CONFLICT",
            "ห้องเรียนของกลุ่มถูกใช้ในช่วงเวลานี้",
        ),
        _ => (
            "TIMETABLE_SYNC_WRITE_CONFLICT",
            "ไม่สามารถเชื่อมกลุ่มเข้าช่วงกิจกรรมได้ กรุณาตรวจสอบข้อมูลตารางสอน",
        ),
    }
}

fn canonical_ids(ids: &[Uuid]) -> Vec<Uuid> {
    let mut ids = ids.to_vec();
    ids.sort_unstable();
    ids.dedup();
    ids
}
