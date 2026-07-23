use crate::error::AppError;
use crate::modules::academic::models::timetable::{
    ConflictInfo, CreateBatchTimetableEntriesRequest, CreateTimetableEntryRequest,
    MoveValidityCell, SwapTimetableEntriesRequest, TimetableValidationResponse,
    ValidateMovesRequest,
};
use sqlx::PgPool;
use uuid::Uuid;

mod entries;
mod instructors;
mod occupancy;
mod shared;

pub use entries::{
    create_entry, delete_entry, fetch_entry_by_id, list_entries,
    resolve_classroom_course_semester_id, update_entry,
};
#[allow(unused_imports)]
pub use instructors::{
    add_entry_instructor, get_my_activity_for_entry, hide_instructor_from_slot,
    hide_instructor_from_slot_period, remove_entry_instructor, restore_instructor_to_slot,
    AddInstructorResult, MyActivityForEntry, MyActivityInstructor, RemoveInstructorResult,
};
pub use occupancy::get_occupancy;
#[allow(unused_imports)]
pub use occupancy::OccupancyRow;
pub use shared::{
    BatchBlockedCell, BatchCreateOutcome, BatchDeletedEntry, BatchExcludedInstructor,
    BatchInstructorConflict, BatchSkippedCell, CreateEntryOutcome, SwapConflictInfo, SwapOutcome,
    TimetableFilter, UpdateEntryOutcome,
};
use shared::{MoveCellKey, MoveEntryRefs, MoveEntryRow, MoveSourceRow, SwapEntryRow};

/// ตรวจ conflict ของ entry ที่กำลังจะสร้าง (instructor + classroom + room)
pub async fn validate_entry(
    pool: &PgPool,
    payload: &CreateTimetableEntryRequest,
) -> Result<TimetableValidationResponse, AppError> {
    let mut conflicts = Vec::new();

    // Unified instructor conflict check via junction
    let candidate_instructors: Vec<Uuid> = if let Some(cc_id) = payload.classroom_course_id {
        sqlx::query_scalar(
            "SELECT instructor_id FROM classroom_course_instructors WHERE classroom_course_id = $1",
        )
        .bind(cc_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default()
    } else if let Some(slot_id) = payload.activity_slot_id {
        let mode: Option<String> = sqlx::query_scalar(
            "SELECT ac.scheduling_mode FROM activity_slots s JOIN activity_catalog ac ON ac.id = s.activity_catalog_id WHERE s.id = $1",
        )
        .bind(slot_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();
        if mode.as_deref() == Some("independent") {
            if let Some(cls_id) = payload.classroom_id {
                sqlx::query_scalar(
                    "SELECT instructor_id FROM activity_slot_classroom_assignments
                     WHERE slot_id = $1 AND classroom_id = $2",
                )
                .bind(slot_id)
                .bind(cls_id)
                .fetch_all(pool)
                .await
                .unwrap_or_default()
            } else {
                Vec::new()
            }
        } else {
            sqlx::query_scalar("SELECT user_id FROM activity_slot_instructors WHERE slot_id = $1")
                .bind(slot_id)
                .fetch_all(pool)
                .await
                .unwrap_or_default()
        }
    } else {
        Vec::new()
    };

    if !candidate_instructors.is_empty() {
        let conflict_instructors: Vec<(String,)> = sqlx::query_as(
            r#"SELECT DISTINCT concat(u.first_name, ' ', u.last_name)
               FROM academic_timetable_entries te
               JOIN timetable_entry_instructors tei ON tei.entry_id = te.id
               JOIN users u ON u.id = tei.instructor_id
               WHERE tei.instructor_id = ANY($1)
                 AND te.day_of_week = $2
                 AND te.period_id = $3
                 AND te.is_active = true"#,
        )
        .bind(&candidate_instructors)
        .bind(&payload.day_of_week)
        .bind(payload.period_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        for (name,) in &conflict_instructors {
            conflicts.push(ConflictInfo {
                conflict_type: "INSTRUCTOR_CONFLICT".to_string(),
                message: format!("{} มีสอนในคาบนี้อยู่แล้ว", name),
                existing_entry: None,
            });
        }
    }

    // Classroom conflict check (resolves classroom_id from course if needed)
    let classroom_for_check: Option<Uuid> = if let Some(course_id) = payload.classroom_course_id {
        let cls: Option<Uuid> =
            sqlx::query_scalar("SELECT classroom_id FROM classroom_courses WHERE id = $1")
                .bind(course_id)
                .fetch_optional(pool)
                .await
                .ok()
                .flatten();
        if cls.is_none() {
            return Err(AppError::NotFound("Classroom course not found".to_string()));
        }
        cls
    } else {
        payload.classroom_id
    };

    if let Some(cls_id) = classroom_for_check {
        let classroom_conflict: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM academic_timetable_entries te
                LEFT JOIN classroom_courses cc ON te.classroom_course_id = cc.id
                WHERE (te.classroom_id = $1 OR cc.classroom_id = $1)
                  AND te.day_of_week = $2
                  AND te.period_id = $3
                  AND te.is_active = true
            )
            "#,
        )
        .bind(cls_id)
        .bind(&payload.day_of_week)
        .bind(payload.period_id)
        .fetch_one(pool)
        .await
        .unwrap_or(false);

        if classroom_conflict {
            conflicts.push(ConflictInfo {
                conflict_type: "CLASSROOM_CONFLICT".to_string(),
                message: "ห้องเรียนนี้มีตารางในคาบนี้อยู่แล้ว".to_string(),
                existing_entry: None,
            });
        }
    }

    // Room conflict
    if let Some(room_id) = payload.room_id {
        let has_conflict: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM academic_timetable_entries
                WHERE room_id = $1
                  AND day_of_week = $2
                  AND period_id = $3
                  AND is_active = true
            )
            "#,
        )
        .bind(room_id)
        .bind(&payload.day_of_week)
        .bind(payload.period_id)
        .fetch_one(pool)
        .await
        .unwrap_or(false);

        if has_conflict {
            conflicts.push(ConflictInfo {
                conflict_type: "ROOM_CONFLICT".to_string(),
                message: "ห้องเรียนถูกใช้ในคาบนี้อยู่แล้ว".to_string(),
                existing_entry: None,
            });
        }
    }

    Ok(TimetableValidationResponse {
        is_valid: conflicts.is_empty(),
        conflicts,
    })
}

/// Swap 2 entries ใน day/period กัน
/// 3-step transaction เพื่อ bypass trigger race (migration 097)
pub async fn swap_entries(
    pool: &PgPool,
    body: SwapTimetableEntriesRequest,
) -> Result<SwapOutcome, AppError> {
    let entries: Vec<SwapEntryRow> = sqlx::query_as(
        r#"SELECT id, day_of_week, period_id, room_id, classroom_id, academic_semester_id, batch_id
           FROM academic_timetable_entries
           WHERE id = ANY($1) AND is_active = true"#,
    )
    .bind([body.entry_a_id, body.entry_b_id])
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    if entries.len() != 2 {
        return Err(AppError::NotFound(
            "Entry not found or inactive".to_string(),
        ));
    }

    // Block: ถ้า entry ใด entry หนึ่งสร้างจาก batch (pinned) → ไม่ให้สลับ
    if entries.iter().any(|e| e.6.is_some()) {
        return Err(AppError::BadRequest(
            "คาบที่สร้างจาก Batch ไม่สามารถสลับได้ (ลบก่อนแล้ว batch ใหม่แทน)".to_string(),
        ));
    }

    let (a, b) = if entries[0].0 == body.entry_a_id {
        (&entries[0], &entries[1])
    } else {
        (&entries[1], &entries[0])
    };

    let (a_id, a_day, a_period, a_room, a_classroom, semester_id, _) = a.clone();
    let (b_id, b_day, b_period, b_room, b_classroom, _, _) = b.clone();

    let make_conflict = |reason: String| -> SwapConflictInfo {
        SwapConflictInfo {
            reason,
            semester_id,
            a_id,
            a_day: a_day.clone(),
            a_period,
            a_room,
            b_id,
            b_day: b_day.clone(),
            b_period,
        }
    };

    // Validate: each entry's classroom must be free at new position (excluding swap partner)
    let a_target_conflict: Option<(String,)> = sqlx::query_as(
        r#"SELECT cr.name FROM academic_timetable_entries te
           LEFT JOIN class_rooms cr ON cr.id = te.classroom_id
           WHERE te.classroom_id = $1 AND te.day_of_week = $2 AND te.period_id = $3
             AND te.is_active = true AND te.id NOT IN ($4, $5)
           LIMIT 1"#,
    )
    .bind(a_classroom)
    .bind(&b_day)
    .bind(b_period)
    .bind(a_id)
    .bind(b_id)
    .fetch_optional(pool)
    .await
    .unwrap_or(None);

    if let Some((name,)) = a_target_conflict {
        return Ok(SwapOutcome::Conflict(make_conflict(format!(
            "ห้อง {} ไม่ว่างที่ตำแหน่งปลายทางของ entry A",
            name
        ))));
    }

    let b_target_conflict: Option<(String,)> = sqlx::query_as(
        r#"SELECT cr.name FROM academic_timetable_entries te
           LEFT JOIN class_rooms cr ON cr.id = te.classroom_id
           WHERE te.classroom_id = $1 AND te.day_of_week = $2 AND te.period_id = $3
             AND te.is_active = true AND te.id NOT IN ($4, $5)
           LIMIT 1"#,
    )
    .bind(b_classroom)
    .bind(&a_day)
    .bind(a_period)
    .bind(a_id)
    .bind(b_id)
    .fetch_optional(pool)
    .await
    .unwrap_or(None);

    if let Some((name,)) = b_target_conflict {
        return Ok(SwapOutcome::Conflict(make_conflict(format!(
            "ห้อง {} ไม่ว่างที่ตำแหน่งปลายทางของ entry B",
            name
        ))));
    }

    // Room conflict (if rooms set)
    if let Some(a_room_id) = a_room {
        let conflict: Option<(String,)> = sqlx::query_as(
            r#"SELECT r.code FROM academic_timetable_entries te
               JOIN rooms r ON r.id = te.room_id
               WHERE te.room_id = $1 AND te.day_of_week = $2 AND te.period_id = $3
                 AND te.is_active = true AND te.id NOT IN ($4, $5)
               LIMIT 1"#,
        )
        .bind(a_room_id)
        .bind(&b_day)
        .bind(b_period)
        .bind(a_id)
        .bind(b_id)
        .fetch_optional(pool)
        .await
        .unwrap_or(None);
        if let Some((code,)) = conflict {
            return Ok(SwapOutcome::Conflict(make_conflict(format!(
                "ห้อง {} ถูกใช้ที่ตำแหน่งปลายทางของ entry A",
                code
            ))));
        }
    }
    if let Some(b_room_id) = b_room {
        let conflict: Option<(String,)> = sqlx::query_as(
            r#"SELECT r.code FROM academic_timetable_entries te
               JOIN rooms r ON r.id = te.room_id
               WHERE te.room_id = $1 AND te.day_of_week = $2 AND te.period_id = $3
                 AND te.is_active = true AND te.id NOT IN ($4, $5)
               LIMIT 1"#,
        )
        .bind(b_room_id)
        .bind(&a_day)
        .bind(a_period)
        .bind(a_id)
        .bind(b_id)
        .fetch_optional(pool)
        .await
        .unwrap_or(None);
        if let Some((code,)) = conflict {
            return Ok(SwapOutcome::Conflict(make_conflict(format!(
                "ห้อง {} ถูกใช้ที่ตำแหน่งปลายทางของ entry B",
                code
            ))));
        }
    }

    // Instructor conflict — each entry's instructors must be free at new position (excluding partner)
    let a_instr_conflict: Option<(String,)> = sqlx::query_as(
        r#"SELECT concat(u.first_name, ' ', u.last_name)
           FROM timetable_entry_instructors tei_self
           JOIN users u ON u.id = tei_self.instructor_id
           WHERE tei_self.entry_id = $1
             AND EXISTS (
                 SELECT 1 FROM timetable_entry_instructors tei_other
                 JOIN academic_timetable_entries te_other ON te_other.id = tei_other.entry_id
                 WHERE tei_other.instructor_id = tei_self.instructor_id
                   AND te_other.day_of_week = $2 AND te_other.period_id = $3
                   AND te_other.is_active = true
                   AND te_other.id NOT IN ($1, $4)
             )
           LIMIT 1"#,
    )
    .bind(a_id)
    .bind(&b_day)
    .bind(b_period)
    .bind(b_id)
    .fetch_optional(pool)
    .await
    .unwrap_or(None);

    if let Some((name,)) = a_instr_conflict {
        return Ok(SwapOutcome::Conflict(make_conflict(format!(
            "ครู {} จะติดคาบที่ตำแหน่งปลายทางของ entry A",
            name
        ))));
    }

    let b_instr_conflict: Option<(String,)> = sqlx::query_as(
        r#"SELECT concat(u.first_name, ' ', u.last_name)
           FROM timetable_entry_instructors tei_self
           JOIN users u ON u.id = tei_self.instructor_id
           WHERE tei_self.entry_id = $1
             AND EXISTS (
                 SELECT 1 FROM timetable_entry_instructors tei_other
                 JOIN academic_timetable_entries te_other ON te_other.id = tei_other.entry_id
                 WHERE tei_other.instructor_id = tei_self.instructor_id
                   AND te_other.day_of_week = $2 AND te_other.period_id = $3
                   AND te_other.is_active = true
                   AND te_other.id NOT IN ($1, $4)
             )
           LIMIT 1"#,
    )
    .bind(b_id)
    .bind(&a_day)
    .bind(a_period)
    .bind(a_id)
    .fetch_optional(pool)
    .await
    .unwrap_or(None);

    if let Some((name,)) = b_instr_conflict {
        return Ok(SwapOutcome::Conflict(make_conflict(format!(
            "ครู {} จะติดคาบที่ตำแหน่งปลายทางของ entry B",
            name
        ))));
    }

    // 3-step transaction to bypass trigger race (migration 097)
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    sqlx::query("UPDATE academic_timetable_entries SET is_active = false WHERE id = $1")
        .bind(a_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::InternalServerError(format!("swap step 1: {}", e)))?;

    sqlx::query(
        "UPDATE academic_timetable_entries SET day_of_week = $1, period_id = $2, updated_at = NOW() WHERE id = $3",
    )
    .bind(&a_day)
    .bind(a_period)
    .bind(b_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| AppError::InternalServerError(format!("swap step 2: {}", e)))?;

    sqlx::query(
        "UPDATE academic_timetable_entries SET day_of_week = $1, period_id = $2, is_active = true, updated_at = NOW() WHERE id = $3",
    )
    .bind(&b_day)
    .bind(b_period)
    .bind(a_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| AppError::InternalServerError(format!("swap step 3: {}", e)))?;

    tx.commit()
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok(SwapOutcome::Swapped { semester_id })
}

/// Validate ทุก cell ในตารางว่า entry ที่ระบุย้ายไปได้ไหม
/// Frontend ใช้ผลลัพธ์ colorize drop targets ก่อน user release
pub async fn validate_moves(
    pool: &PgPool,
    body: ValidateMovesRequest,
) -> Result<Vec<MoveValidityCell>, AppError> {
    let src: Option<MoveSourceRow> = sqlx::query_as(
        r#"SELECT day_of_week, period_id, classroom_id, room_id, academic_semester_id, id
           FROM academic_timetable_entries WHERE id = $1 AND is_active = true"#,
    )
    .bind(body.entry_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    let (src_day, src_period, src_classroom, src_room, src_semester, _) = match src {
        Some(v) => v,
        None => return Err(AppError::NotFound("Entry not found".to_string())),
    };

    let all_entries: Vec<MoveEntryRow> = sqlx::query_as(
        r#"SELECT id, day_of_week, period_id, classroom_id, room_id
           FROM academic_timetable_entries
           WHERE academic_semester_id = $1 AND is_active = true"#,
    )
    .bind(src_semester)
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    let src_instructors: Vec<Uuid> = sqlx::query_scalar(
        "SELECT instructor_id FROM timetable_entry_instructors WHERE entry_id = $1",
    )
    .bind(body.entry_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let other_ids: Vec<Uuid> = all_entries.iter().map(|e| e.0).collect();
    let other_instructors_flat: Vec<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT entry_id, instructor_id FROM timetable_entry_instructors WHERE entry_id = ANY($1)",
    )
    .bind(&other_ids)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    use std::collections::HashMap;
    let mut by_entry: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
    for (eid, iid) in &other_instructors_flat {
        by_entry.entry(*eid).or_default().push(*iid);
    }

    let mut cell_entries: HashMap<MoveCellKey, MoveEntryRefs<'_>> = HashMap::new();
    for e in &all_entries {
        cell_entries.entry((e.1.clone(), e.2)).or_default().push(e);
    }

    let periods: Vec<(Uuid,)> = sqlx::query_as(
        r#"SELECT p.id FROM academic_periods p
           JOIN academic_semesters sem ON sem.academic_year_id = p.academic_year_id
           WHERE sem.id = $1
           ORDER BY p.order_index"#,
    )
    .bind(src_semester)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let days = ["MON", "TUE", "WED", "THU", "FRI", "SAT", "SUN"];
    let mut cells: Vec<MoveValidityCell> = Vec::new();

    for day in days.iter() {
        for (pid,) in &periods {
            let key = (day.to_string(), *pid);

            if *day == src_day && *pid == src_period {
                cells.push(MoveValidityCell {
                    day_of_week: day.to_string(),
                    period_id: *pid,
                    state: "source".to_string(),
                    target_entry_id: None,
                    valid: false,
                    reason: String::new(),
                });
                continue;
            }

            let occupants: MoveEntryRefs<'_> = cell_entries.get(&key).cloned().unwrap_or_default();
            let others: MoveEntryRefs<'_> = occupants
                .iter()
                .filter(|e| e.0 != body.entry_id)
                .copied()
                .collect();

            if others.is_empty() {
                let mut valid = true;
                let mut reason = String::new();

                if all_entries.iter().any(|e| {
                    e.0 != body.entry_id && e.3 == src_classroom && e.1 == *day && e.2 == *pid
                }) {
                    valid = false;
                    reason = "ห้องเรียนมี entry อื่น".to_string();
                }

                if valid {
                    for iid in &src_instructors {
                        if all_entries.iter().any(|e| {
                            e.0 != body.entry_id
                                && e.1 == *day
                                && e.2 == *pid
                                && by_entry.get(&e.0).is_some_and(|ids| ids.contains(iid))
                        }) {
                            valid = false;
                            reason = "ครูติดคาบ".to_string();
                            break;
                        }
                    }
                }

                if valid {
                    if let Some(r) = src_room {
                        if all_entries.iter().any(|e| {
                            e.0 != body.entry_id && e.4 == Some(r) && e.1 == *day && e.2 == *pid
                        }) {
                            valid = false;
                            reason = "ห้องถูกใช้".to_string();
                        }
                    }
                }

                cells.push(MoveValidityCell {
                    day_of_week: day.to_string(),
                    period_id: *pid,
                    state: "empty".to_string(),
                    target_entry_id: None,
                    valid,
                    reason,
                });
            } else {
                let target = others[0];
                let target_id = target.0;
                let mut valid = true;
                let mut reason = String::new();

                if all_entries.iter().any(|e| {
                    e.0 != body.entry_id
                        && e.0 != target_id
                        && e.3 == src_classroom
                        && e.1 == *day
                        && e.2 == *pid
                }) {
                    valid = false;
                    reason = "ห้องของต้นทางถูกใช้ที่คาบนี้".to_string();
                }
                if valid
                    && all_entries.iter().any(|e| {
                        e.0 != body.entry_id
                            && e.0 != target_id
                            && e.3 == target.3
                            && e.1 == src_day
                            && e.2 == src_period
                    })
                {
                    valid = false;
                    reason = "ห้องของปลายทางถูกใช้ที่คาบต้นทาง".to_string();
                }

                if valid {
                    for iid in &src_instructors {
                        if all_entries.iter().any(|e| {
                            e.0 != body.entry_id
                                && e.0 != target_id
                                && e.1 == *day
                                && e.2 == *pid
                                && by_entry.get(&e.0).is_some_and(|ids| ids.contains(iid))
                        }) {
                            valid = false;
                            reason = "ครูต้นทางติดคาบปลายทาง".to_string();
                            break;
                        }
                    }
                }
                if valid {
                    let target_instr: Vec<Uuid> =
                        by_entry.get(&target_id).cloned().unwrap_or_default();
                    for iid in &target_instr {
                        if all_entries.iter().any(|e| {
                            e.0 != body.entry_id
                                && e.0 != target_id
                                && e.1 == src_day
                                && e.2 == src_period
                                && by_entry.get(&e.0).is_some_and(|ids| ids.contains(iid))
                        }) {
                            valid = false;
                            reason = "ครูปลายทางติดคาบต้นทาง".to_string();
                            break;
                        }
                    }
                }

                if valid {
                    if let Some(r) = src_room {
                        if all_entries.iter().any(|e| {
                            e.0 != body.entry_id
                                && e.0 != target_id
                                && e.4 == Some(r)
                                && e.1 == *day
                                && e.2 == *pid
                        }) {
                            valid = false;
                            reason = "ห้องต้นทางถูกใช้ที่คาบปลายทาง".to_string();
                        }
                    }
                }
                if valid {
                    if let Some(r) = target.4 {
                        if all_entries.iter().any(|e| {
                            e.0 != body.entry_id
                                && e.0 != target_id
                                && e.4 == Some(r)
                                && e.1 == src_day
                                && e.2 == src_period
                        }) {
                            valid = false;
                            reason = "ห้องปลายทางถูกใช้ที่คาบต้นทาง".to_string();
                        }
                    }
                }

                cells.push(MoveValidityCell {
                    day_of_week: day.to_string(),
                    period_id: *pid,
                    state: "occupied".to_string(),
                    target_entry_id: Some(target_id),
                    valid,
                    reason,
                });
            }
        }
    }

    Ok(cells)
}

/// ลบ entries ทั้งหมดที่ match (slot_id, day, semester) — return rows affected
pub async fn delete_entries_by_slot(
    pool: &PgPool,
    slot_id: Uuid,
    day_of_week: &str,
    semester_id: Uuid,
) -> Result<u64, AppError> {
    let result = sqlx::query(
        r#"
        DELETE FROM academic_timetable_entries
        WHERE activity_slot_id = $1
          AND day_of_week = $2
          AND academic_semester_id = $3
        "#,
    )
    .bind(slot_id)
    .bind(day_of_week)
    .bind(semester_id)
    .execute(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to batch delete entries: {}", e);
        AppError::InternalServerError("Failed to batch delete entries".to_string())
    })?;
    Ok(result.rows_affected())
}

/// ลบ entries ทั้ง batch (จากการ create แบบ batch) — return (rows_affected, semester_id)
pub async fn delete_batch_group(
    pool: &PgPool,
    batch_id: Uuid,
) -> Result<(u64, Option<Uuid>), AppError> {
    let semester_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT academic_semester_id FROM academic_timetable_entries WHERE batch_id = $1 LIMIT 1",
    )
    .bind(batch_id)
    .fetch_optional(pool)
    .await
    .unwrap_or(None);

    let result = sqlx::query("DELETE FROM academic_timetable_entries WHERE batch_id = $1")
        .bind(batch_id)
        .execute(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to delete batch group {}: {}", batch_id, e);
            AppError::InternalServerError("Failed to delete batch group".to_string())
        })?;

    Ok((result.rows_affected(), semester_id))
}

/// Batch create — สร้าง entries หลายห้อง × หลายวัน × หลายคาบ ในคำสั่งเดียว
/// Conflict resolution logic ครอบคลุม:
/// - sync batch (slot.scheduling_mode = synchronized): block ถ้ามี classroom conflict; exclude instructor ถ้า no-force
/// - text/independent batch: skip ถ้า no-force, overwrite ถ้า force
/// - instructor-only entries (no slot, no subject): create teacher-only entries แยก
pub async fn create_batch_entries(
    pool: &PgPool,
    user_id: Option<Uuid>,
    payload: CreateBatchTimetableEntriesRequest,
) -> Result<BatchCreateOutcome, AppError> {
    let force = payload.force.unwrap_or(false);

    // ต้องเลือกห้องอย่างน้อย 1 หรือ ครูอย่างน้อย 1
    if payload.classroom_ids.is_empty() && payload.instructor_ids.is_empty() {
        return Err(AppError::BadRequest(
            "ต้องเลือกห้องเรียน หรือ ครู อย่างน้อย 1 อย่าง".to_string(),
        ));
    }

    // Validate slot participation + instructor exists (sync)
    if let Some(slot_id) = payload.activity_slot_id {
        let non_participating: Vec<(String,)> = sqlx::query_as(
            r#"SELECT cr.name FROM class_rooms cr
               WHERE cr.id = ANY($1)
                 AND NOT EXISTS (SELECT 1 FROM activity_slot_classrooms
                                 WHERE slot_id = $2 AND classroom_id = cr.id)"#,
        )
        .bind(&payload.classroom_ids)
        .bind(slot_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();
        if !non_participating.is_empty() {
            let names: Vec<String> = non_participating.into_iter().map(|(n,)| n).collect();
            return Err(AppError::BadRequest(format!(
                "ห้องต่อไปนี้ยังไม่ได้อยู่ในกิจกรรม: {} — เพิ่มห้องที่ Course Planning ก่อน",
                names.join(", ")
            )));
        }
        let missing_teacher: Vec<(String,)> = sqlx::query_as(
            r#"SELECT cr.name
               FROM class_rooms cr, activity_slots s
               JOIN activity_catalog ac ON ac.id = s.activity_catalog_id
               WHERE s.id = $2 AND cr.id = ANY($1)
                 AND CASE WHEN ac.scheduling_mode = 'independent' THEN
                          NOT EXISTS(SELECT 1 FROM activity_slot_classroom_assignments
                                     WHERE slot_id = $2 AND classroom_id = cr.id)
                         ELSE NOT EXISTS(SELECT 1 FROM activity_slot_instructors
                                         WHERE slot_id = $2) END"#,
        )
        .bind(&payload.classroom_ids)
        .bind(slot_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();
        if !missing_teacher.is_empty() {
            let names: Vec<String> = missing_teacher.into_iter().map(|(n,)| n).collect();
            return Err(AppError::BadRequest(format!(
                "กิจกรรมนี้ยังไม่ได้กำหนดครูผู้สอน (กระทบ: {}) — เพิ่มครูที่หน้า Activities ก่อน",
                names.join(", ")
            )));
        }
    }

    // ===== Determine batch type =====
    let is_sync_batch = if let Some(slot_id) = payload.activity_slot_id {
        let mode: Option<String> = sqlx::query_scalar(
            "SELECT ac.scheduling_mode FROM activity_slots s
             JOIN activity_catalog ac ON ac.id = s.activity_catalog_id WHERE s.id = $1",
        )
        .bind(slot_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();
        mode.as_deref() == Some("synchronized")
    } else {
        false
    };

    // ===== Resolve candidate instructors =====
    let mut candidate_instructors: Vec<Uuid> = if let Some(slot_id) = payload.activity_slot_id {
        if is_sync_batch {
            sqlx::query_scalar("SELECT user_id FROM activity_slot_instructors WHERE slot_id = $1")
                .bind(slot_id)
                .fetch_all(pool)
                .await
                .unwrap_or_default()
        } else {
            sqlx::query_scalar(
                "SELECT instructor_id FROM activity_slot_classroom_assignments
                 WHERE slot_id = $1 AND classroom_id = ANY($2)",
            )
            .bind(slot_id)
            .bind(&payload.classroom_ids)
            .fetch_all(pool)
            .await
            .unwrap_or_default()
        }
    } else if let Some(subject_id) = payload.subject_id {
        sqlx::query_scalar(
            "SELECT DISTINCT cci.instructor_id FROM classroom_course_instructors cci
             JOIN classroom_courses cc ON cc.id = cci.classroom_course_id
             WHERE cc.classroom_id = ANY($1) AND cc.subject_id = $2",
        )
        .bind(&payload.classroom_ids)
        .bind(subject_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default()
    } else {
        Vec::new()
    };
    for id in &payload.instructor_ids {
        if !candidate_instructors.contains(id) {
            candidate_instructors.push(*id);
        }
    }

    // ===== Pre-fetch existing entries that COULD conflict =====
    #[derive(sqlx::FromRow, Clone)]
    struct ExistingEntry {
        id: Uuid,
        classroom_id: Option<Uuid>,
        classroom_name: Option<String>,
        day_of_week: String,
        period_id: Uuid,
        period_name: Option<String>,
        room_id: Option<Uuid>,
        #[allow(dead_code)]
        title: Option<String>,
        entry_type: String,
        #[allow(dead_code)]
        activity_slot_id: Option<Uuid>,
        scheduling_mode: Option<String>,
        display_title: String,
        instructor_ids: Vec<Uuid>,
        instructor_names: Vec<String>,
    }

    let existing: Vec<ExistingEntry> = sqlx::query_as::<_, ExistingEntry>(
        r#"
        SELECT te.id, te.classroom_id, cr.name AS classroom_name,
               te.day_of_week, te.period_id,
               COALESCE(ap.name, 'คาบ ' || ap.order_index::text) AS period_name,
               te.room_id, te.title, te.entry_type,
               te.activity_slot_id, ac.scheduling_mode,
               COALESCE(s.name_th, te.title, '(ไม่ระบุ)') AS display_title,
               COALESCE(ARRAY_AGG(DISTINCT tei.instructor_id) FILTER (WHERE tei.instructor_id IS NOT NULL), '{}'::uuid[]) AS instructor_ids,
               COALESCE(ARRAY_AGG(DISTINCT concat(u.first_name, ' ', u.last_name)) FILTER (WHERE u.id IS NOT NULL), '{}'::text[]) AS instructor_names
          FROM academic_timetable_entries te
          LEFT JOIN class_rooms cr ON cr.id = te.classroom_id
          LEFT JOIN academic_periods ap ON ap.id = te.period_id
          LEFT JOIN classroom_courses cc ON cc.id = te.classroom_course_id
          LEFT JOIN subjects s ON s.id = cc.subject_id
          LEFT JOIN activity_slots aslot ON aslot.id = te.activity_slot_id
          LEFT JOIN activity_catalog ac ON ac.id = aslot.activity_catalog_id
          LEFT JOIN timetable_entry_instructors tei ON tei.entry_id = te.id
          LEFT JOIN users u ON u.id = tei.instructor_id
         WHERE te.is_active = true
           AND te.day_of_week = ANY($1)
           AND te.period_id = ANY($2)
           AND (te.activity_slot_id IS DISTINCT FROM $3 OR te.activity_slot_id IS NULL)
         GROUP BY te.id, cr.name, ap.name, ap.order_index, s.name_th, ac.scheduling_mode
        "#,
    )
    .bind(&payload.days_of_week)
    .bind(&payload.period_ids)
    .bind(payload.activity_slot_id)
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::InternalServerError(format!("fetch existing entries: {}", e)))?;

    // Pre-fetch classroom/period names
    let classroom_names: std::collections::HashMap<Uuid, String> =
        sqlx::query_as::<_, (Uuid, String)>("SELECT id, name FROM class_rooms WHERE id = ANY($1)")
            .bind(&payload.classroom_ids)
            .fetch_all(pool)
            .await
            .unwrap_or_default()
            .into_iter()
            .collect();
    let period_labels: std::collections::HashMap<Uuid, String> = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT id, COALESCE(name, 'คาบ ' || order_index::text) FROM academic_periods WHERE id = ANY($1)",
    )
    .bind(&payload.period_ids)
    .fetch_all(pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .collect();

    let day_label = |d: &str| -> String {
        match d {
            "MON" => "จันทร์".to_string(),
            "TUE" => "อังคาร".to_string(),
            "WED" => "พุธ".to_string(),
            "THU" => "พฤหัสฯ".to_string(),
            "FRI" => "ศุกร์".to_string(),
            "SAT" => "เสาร์".to_string(),
            "SUN" => "อาทิตย์".to_string(),
            _ => d.to_string(),
        }
    };

    let mut skipped: Vec<BatchSkippedCell> = Vec::new();
    let mut blocked: Vec<BatchBlockedCell> = Vec::new();
    let mut deleted: Vec<BatchDeletedEntry> = Vec::new();
    let mut excluded_instructors_map: std::collections::HashMap<
        Uuid,
        (String, Vec<BatchInstructorConflict>),
    > = std::collections::HashMap::new();
    let mut entries_to_delete: Vec<Uuid> = Vec::new();
    let mut insert_tuples: Vec<(Uuid, String, Uuid)> = Vec::new();

    let instructors_attach_to_classroom =
        payload.activity_slot_id.is_some() || payload.subject_id.is_some();

    for cr_id in &payload.classroom_ids {
        for day in &payload.days_of_week {
            for p_id in &payload.period_ids {
                let cell_conflicts: Vec<&ExistingEntry> = existing
                    .iter()
                    .filter(|e| {
                        if e.day_of_week != *day || e.period_id != *p_id {
                            return false;
                        }
                        e.classroom_id == Some(*cr_id)
                            || (payload.room_id.is_some() && e.room_id == payload.room_id)
                            || (instructors_attach_to_classroom
                                && e.instructor_ids
                                    .iter()
                                    .any(|i| candidate_instructors.contains(i)))
                    })
                    .collect();

                if cell_conflicts.is_empty() {
                    insert_tuples.push((*cr_id, day.clone(), *p_id));
                    continue;
                }

                let has_sync_conflict = cell_conflicts
                    .iter()
                    .any(|e| e.scheduling_mode.as_deref() == Some("synchronized"));

                if is_sync_batch {
                    let cell_cls_name = classroom_names
                        .get(cr_id)
                        .cloned()
                        .unwrap_or_else(|| "?".to_string());
                    let cell_period = period_labels.get(p_id).cloned().unwrap_or_default();
                    let cell_day = day_label(day);

                    let classroom_busy = cell_conflicts
                        .iter()
                        .find(|e| e.classroom_id == Some(*cr_id));
                    if let Some(blocker) = classroom_busy {
                        if force {
                            if blocker.scheduling_mode.as_deref() == Some("synchronized") {
                                blocked.push(BatchBlockedCell {
                                    classroom_id: *cr_id,
                                    classroom_name: Some(cell_cls_name.clone()),
                                    day_of_week: day.clone(),
                                    period_id: *p_id,
                                    period_name: Some(cell_period.clone()),
                                    reason: "SYNC_VS_SYNC".to_string(),
                                    message: format!(
                                        "{} {} {}: ทับกิจกรรม sync '{}' — sync ทับ sync ไม่ได้",
                                        cell_cls_name, cell_day, cell_period, blocker.display_title
                                    ),
                                });
                                continue;
                            }
                            entries_to_delete.push(blocker.id);
                            deleted.push(BatchDeletedEntry {
                                id: blocker.id,
                                classroom_name: Some(cell_cls_name.clone()),
                                day_of_week: day.clone(),
                                period_id: *p_id,
                                period_name: Some(cell_period.clone()),
                                title: blocker.display_title.clone(),
                                entry_type: blocker.entry_type.clone(),
                                instructor_names: blocker.instructor_names.clone(),
                            });
                            insert_tuples.push((*cr_id, day.clone(), *p_id));
                        } else {
                            blocked.push(BatchBlockedCell {
                                classroom_id: *cr_id,
                                classroom_name: Some(cell_cls_name.clone()),
                                day_of_week: day.clone(),
                                period_id: *p_id,
                                period_name: Some(cell_period.clone()),
                                reason: "STUDENT_BUSY".to_string(),
                                message: format!(
                                    "{} {} {}: นักเรียนติด '{}' — ลบของเดิมก่อน",
                                    cell_cls_name, cell_day, cell_period, blocker.display_title
                                ),
                            });
                        }
                        continue;
                    }
                    let room_busy = cell_conflicts.iter().find(|e| {
                        payload.room_id.is_some()
                            && e.room_id == payload.room_id
                            && e.classroom_id != Some(*cr_id)
                    });
                    if let Some(blocker) = room_busy {
                        if force {
                            entries_to_delete.push(blocker.id);
                            deleted.push(BatchDeletedEntry {
                                id: blocker.id,
                                classroom_name: blocker.classroom_name.clone(),
                                day_of_week: day.clone(),
                                period_id: *p_id,
                                period_name: blocker.period_name.clone(),
                                title: blocker.display_title.clone(),
                                entry_type: blocker.entry_type.clone(),
                                instructor_names: blocker.instructor_names.clone(),
                            });
                            insert_tuples.push((*cr_id, day.clone(), *p_id));
                        } else {
                            skipped.push(BatchSkippedCell {
                                classroom_id: Some(*cr_id),
                                classroom_name: Some(cell_cls_name.clone()),
                                day_of_week: day.clone(),
                                period_id: *p_id,
                                period_name: Some(cell_period.clone()),
                                reason: "ROOM_BUSY".to_string(),
                                message: format!(
                                    "{} {} {}: ห้องสอนถูกใช้โดย '{}' อยู่ — ข้ามไม่ลง",
                                    cell_cls_name, cell_day, cell_period, blocker.display_title
                                ),
                            });
                        }
                        continue;
                    }
                    let mut conflicting_instructors: Vec<(Uuid, String)> = Vec::new();
                    for e in &cell_conflicts {
                        for (idx, iid) in e.instructor_ids.iter().enumerate() {
                            if candidate_instructors.contains(iid) {
                                let name = e.instructor_names.get(idx).cloned().unwrap_or_default();
                                conflicting_instructors.push((*iid, name));
                                if force && !entries_to_delete.contains(&e.id) {
                                    entries_to_delete.push(e.id);
                                    deleted.push(BatchDeletedEntry {
                                        id: e.id,
                                        classroom_name: e.classroom_name.clone(),
                                        day_of_week: day.clone(),
                                        period_id: *p_id,
                                        period_name: e.period_name.clone(),
                                        title: e.display_title.clone(),
                                        entry_type: e.entry_type.clone(),
                                        instructor_names: e.instructor_names.clone(),
                                    });
                                }
                            }
                        }
                    }
                    if !force {
                        for (iid, _name) in &conflicting_instructors {
                            let Some(conf_entry) = cell_conflicts
                                .iter()
                                .find(|e| e.instructor_ids.contains(iid))
                            else {
                                continue;
                            };
                            let entry_record =
                                excluded_instructors_map.entry(*iid).or_insert_with(|| {
                                    let nm = cell_conflicts
                                        .iter()
                                        .filter_map(|e| {
                                            e.instructor_ids
                                                .iter()
                                                .position(|x| x == iid)
                                                .and_then(|idx| e.instructor_names.get(idx))
                                        })
                                        .next()
                                        .cloned()
                                        .unwrap_or_default();
                                    (nm, Vec::new())
                                });
                            entry_record.1.push(BatchInstructorConflict {
                                day_of_week: day.clone(),
                                period_id: *p_id,
                                period_name: conf_entry.period_name.clone(),
                                existing_title: conf_entry.display_title.clone(),
                            });
                        }
                    }
                    insert_tuples.push((*cr_id, day.clone(), *p_id));
                } else {
                    let cell_cls_name = classroom_names
                        .get(cr_id)
                        .cloned()
                        .unwrap_or_else(|| "?".to_string());
                    let cell_period = period_labels.get(p_id).cloned().unwrap_or_default();
                    let cell_day = day_label(day);

                    if has_sync_conflict {
                        let sync_blocker_title = cell_conflicts
                            .iter()
                            .find(|e| e.scheduling_mode.as_deref() == Some("synchronized"))
                            .map(|e| e.display_title.clone())
                            .unwrap_or_else(|| "กิจกรรม sync".to_string());
                        blocked.push(BatchBlockedCell {
                            classroom_id: *cr_id,
                            classroom_name: Some(cell_cls_name.clone()),
                            day_of_week: day.clone(),
                            period_id: *p_id,
                            period_name: Some(cell_period.clone()),
                            reason: "SYNC_ACTIVITY_PRESENT".to_string(),
                            message: format!(
                                "{} {} {}: มีกิจกรรม sync '{}' อยู่ — ลบกิจกรรม sync ก่อน",
                                cell_cls_name, cell_day, cell_period, sync_blocker_title
                            ),
                        });
                        continue;
                    }
                    if force {
                        for e in &cell_conflicts {
                            if !entries_to_delete.contains(&e.id) {
                                entries_to_delete.push(e.id);
                                deleted.push(BatchDeletedEntry {
                                    id: e.id,
                                    classroom_name: e.classroom_name.clone(),
                                    day_of_week: day.clone(),
                                    period_id: *p_id,
                                    period_name: e.period_name.clone(),
                                    title: e.display_title.clone(),
                                    entry_type: e.entry_type.clone(),
                                    instructor_names: e.instructor_names.clone(),
                                });
                            }
                        }
                        insert_tuples.push((*cr_id, day.clone(), *p_id));
                    } else {
                        let primary = &cell_conflicts[0];
                        let (reason, message) = if primary.classroom_id == Some(*cr_id) {
                            let r = if primary.entry_type == "COURSE" {
                                "CLASSROOM_COURSE"
                            } else {
                                "CLASSROOM_ACTIVITY"
                            };
                            (
                                r,
                                format!(
                                    "{} {} {}: ห้องนี้มี '{}' อยู่แล้ว — ข้ามไม่ลง",
                                    cell_cls_name, cell_day, cell_period, primary.display_title
                                ),
                            )
                        } else if let Some(busy_instr) = primary
                            .instructor_ids
                            .iter()
                            .enumerate()
                            .find(|(_, iid)| candidate_instructors.contains(iid))
                        {
                            let instr_name = primary
                                .instructor_names
                                .get(busy_instr.0)
                                .map(|s| s.as_str())
                                .unwrap_or("ครู");
                            (
                                "INSTRUCTOR_BUSY",
                                format!(
                                    "{} {} {}: ครู {} ติดสอน '{}' (ที่ {}) — ข้ามไม่ลง",
                                    cell_cls_name,
                                    cell_day,
                                    cell_period,
                                    instr_name,
                                    primary.display_title,
                                    primary.classroom_name.as_deref().unwrap_or("?")
                                ),
                            )
                        } else {
                            (
                                "ROOM_BUSY",
                                format!(
                                    "{} {} {}: ห้องสอนถูกใช้โดย '{}' ที่ {} — ข้ามไม่ลง",
                                    cell_cls_name,
                                    cell_day,
                                    cell_period,
                                    primary.display_title,
                                    primary.classroom_name.as_deref().unwrap_or("?")
                                ),
                            )
                        };
                        skipped.push(BatchSkippedCell {
                            classroom_id: Some(*cr_id),
                            classroom_name: Some(cell_cls_name.clone()),
                            day_of_week: day.clone(),
                            period_id: *p_id,
                            period_name: Some(cell_period.clone()),
                            reason: reason.to_string(),
                            message,
                        });
                    }
                }
            }
        }
    }

    let effective_instructors: Vec<Uuid> = if is_sync_batch && !force {
        payload
            .instructor_ids
            .iter()
            .filter(|i| !excluded_instructors_map.contains_key(i))
            .copied()
            .collect()
    } else {
        payload.instructor_ids.clone()
    };

    // ===== Execute transaction =====
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    let batch_uuid = Uuid::new_v4();

    if !entries_to_delete.is_empty() {
        sqlx::query("DELETE FROM academic_timetable_entries WHERE id = ANY($1)")
            .bind(&entries_to_delete)
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::InternalServerError(format!("delete overwrite: {}", e)))?;
        entries_to_delete.clear();
    }

    let mut inserted_count: i64 = 0;
    if !insert_tuples.is_empty() {
        let cr_arr: Vec<Uuid> = insert_tuples.iter().map(|(c, _, _)| *c).collect();
        let d_arr: Vec<String> = insert_tuples.iter().map(|(_, d, _)| d.clone()).collect();
        let p_arr: Vec<Uuid> = insert_tuples.iter().map(|(_, _, p)| *p).collect();

        let result = sqlx::query(
            r#"
            WITH cc_map AS (
                SELECT cc.id AS cc_id, cc.classroom_id AS cr_id, s.name_th AS course_name
                FROM classroom_courses cc
                JOIN subjects s ON cc.subject_id = s.id
                WHERE $8::uuid IS NOT NULL
                  AND cc.subject_id = $8
                  AND cc.classroom_id = ANY($5)
            ),
            new_entries AS (
                INSERT INTO academic_timetable_entries (
                    id, classroom_id, academic_semester_id, day_of_week, period_id, room_id,
                    entry_type, title, is_active, created_by, updated_by,
                    classroom_course_id, note, activity_slot_id, batch_id
                )
                SELECT gen_random_uuid(), t.c, $1, t.d, t.p, $2,
                    CASE WHEN cc_map.cc_id IS NOT NULL THEN 'COURSE' ELSE $3 END,
                    COALESCE(cc_map.course_name, $4),
                    true, $9, $9,
                    cc_map.cc_id, $10, $11, $12
                FROM UNNEST($5::uuid[], $6::text[], $7::uuid[]) AS t(c, d, p)
                LEFT JOIN cc_map ON cc_map.cr_id = t.c
                ON CONFLICT DO NOTHING
                RETURNING id, classroom_id, classroom_course_id
            ),
            slot_mode AS (
                SELECT ac.scheduling_mode AS mode
                FROM activity_slots s
                JOIN activity_catalog ac ON ac.id = s.activity_catalog_id
                WHERE $11::uuid IS NOT NULL AND s.id = $11
            ),
            tei_inserts AS (
                INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
                SELECT ne.id, cci.instructor_id, cci.role
                    FROM new_entries ne
                    JOIN classroom_course_instructors cci ON cci.classroom_course_id = ne.classroom_course_id
                    WHERE ne.classroom_course_id IS NOT NULL
                UNION ALL
                SELECT ne.id, asca.instructor_id, 'primary'
                    FROM new_entries ne
                    JOIN activity_slot_classroom_assignments asca
                        ON asca.slot_id = $11 AND asca.classroom_id = ne.classroom_id
                    WHERE (SELECT mode FROM slot_mode) = 'independent'
                UNION ALL
                SELECT ne.id, i.v, 'primary'
                    FROM new_entries ne
                    CROSS JOIN UNNEST($13::uuid[]) AS i(v)
                    WHERE (SELECT mode FROM slot_mode) = 'synchronized'
                ON CONFLICT DO NOTHING
                RETURNING entry_id
            )
            SELECT COUNT(*) FROM new_entries
            "#,
        )
        .bind(payload.academic_semester_id)
        .bind(payload.room_id)
        .bind(&payload.entry_type)
        .bind(&payload.title)
        .bind(&cr_arr)
        .bind(&d_arr)
        .bind(&p_arr)
        .bind(payload.subject_id)
        .bind(user_id)
        .bind(&payload.note)
        .bind(payload.activity_slot_id)
        .bind(batch_uuid)
        .bind(&effective_instructors)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| {
            tracing::error!("Failed bulk classroom batch INSERT: {}", e);
            AppError::InternalServerError("Failed to batch create entries".to_string())
        })?;
        inserted_count = sqlx::Row::try_get::<i64, _>(&result, 0).unwrap_or(0);
    }

    // === INSTRUCTOR-only entries — skip ถ้าเป็น SLOT mode (attach ผ่าน CTE ด้านบนแล้ว) ===
    if !payload.instructor_ids.is_empty() && payload.activity_slot_id.is_none() {
        let instr_names: std::collections::HashMap<Uuid, String> =
            sqlx::query_as::<_, (Uuid, String)>(
                "SELECT id, concat(first_name, ' ', last_name) FROM users WHERE id = ANY($1)",
            )
            .bind(&payload.instructor_ids)
            .fetch_all(pool)
            .await
            .unwrap_or_default()
            .into_iter()
            .collect();

        let mut entry_ids: Vec<Uuid> = Vec::new();
        let mut instr_ids: Vec<Uuid> = Vec::new();
        let mut days: Vec<String> = Vec::new();
        let mut periods: Vec<Uuid> = Vec::new();
        for i_id in &payload.instructor_ids {
            for d in &payload.days_of_week {
                for p_id in &payload.period_ids {
                    let busy = existing.iter().find(|e| {
                        e.day_of_week == *d
                            && e.period_id == *p_id
                            && e.instructor_ids.contains(i_id)
                    });
                    if let Some(blocker) = busy {
                        if !force {
                            let instr_name = instr_names
                                .get(i_id)
                                .cloned()
                                .unwrap_or_else(|| "ครู".to_string());
                            let p_name = period_labels.get(p_id).cloned().unwrap_or_default();
                            skipped.push(BatchSkippedCell {
                                classroom_id: None,
                                classroom_name: None,
                                day_of_week: d.clone(),
                                period_id: *p_id,
                                period_name: Some(p_name.clone()),
                                reason: "INSTRUCTOR_BUSY".to_string(),
                                message: format!(
                                    "ครู {} {} {}: ติดสอน '{}' ที่ {} อยู่ — ไม่สร้างคาบครูเปล่า",
                                    instr_name,
                                    day_label(d),
                                    p_name,
                                    blocker.display_title,
                                    blocker.classroom_name.as_deref().unwrap_or("?")
                                ),
                            });
                            continue;
                        }
                        if !entries_to_delete.contains(&blocker.id) {
                            entries_to_delete.push(blocker.id);
                            deleted.push(BatchDeletedEntry {
                                id: blocker.id,
                                classroom_name: blocker.classroom_name.clone(),
                                day_of_week: d.clone(),
                                period_id: *p_id,
                                period_name: blocker.period_name.clone(),
                                title: blocker.display_title.clone(),
                                entry_type: blocker.entry_type.clone(),
                                instructor_names: blocker.instructor_names.clone(),
                            });
                        }
                    }
                    entry_ids.push(Uuid::new_v4());
                    instr_ids.push(*i_id);
                    days.push(d.clone());
                    periods.push(*p_id);
                }
            }
        }
        if !entries_to_delete.is_empty() {
            sqlx::query("DELETE FROM academic_timetable_entries WHERE id = ANY($1)")
                .bind(&entries_to_delete)
                .execute(&mut *tx)
                .await
                .map_err(|e| {
                    AppError::InternalServerError(format!("delete teacher conflicts: {}", e))
                })?;
            entries_to_delete.clear();
        }

        sqlx::query(
            r#"INSERT INTO academic_timetable_entries (
                id, classroom_id, academic_semester_id, day_of_week, period_id, room_id,
                entry_type, title, is_active, created_by, updated_by,
                classroom_course_id, note, activity_slot_id, batch_id
            )
            SELECT id, NULL, $1, day, period, $2, $3, $4, true, $5, $5, NULL, $6, NULL, $7
            FROM UNNEST($8::uuid[], $9::text[], $10::uuid[]) AS t(id, day, period)
            ON CONFLICT DO NOTHING"#,
        )
        .bind(payload.academic_semester_id)
        .bind(payload.room_id)
        .bind(&payload.entry_type)
        .bind(&payload.title)
        .bind(user_id)
        .bind(&payload.note)
        .bind(batch_uuid)
        .bind(&entry_ids)
        .bind(&days)
        .bind(&periods)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

        sqlx::query(
            r#"INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
               SELECT id, instr, 'primary'
               FROM UNNEST($1::uuid[], $2::uuid[]) AS t(id, instr)
               ON CONFLICT DO NOTHING"#,
        )
        .bind(&entry_ids)
        .bind(&instr_ids)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    }

    tx.commit()
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    let excluded_instructors: Vec<BatchExcludedInstructor> = excluded_instructors_map
        .into_iter()
        .map(|(iid, (name, conflicts))| BatchExcludedInstructor {
            instructor_id: iid,
            instructor_name: name,
            conflicting_at: conflicts,
        })
        .collect();

    Ok(BatchCreateOutcome {
        inserted_count,
        skipped,
        blocked,
        deleted,
        excluded_instructors,
        semester_id: payload.academic_semester_id,
    })
}
