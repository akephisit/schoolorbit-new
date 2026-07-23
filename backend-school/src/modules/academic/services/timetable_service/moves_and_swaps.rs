use crate::error::AppError;
use crate::modules::academic::models::timetable::{
    MoveValidityCell, SwapTimetableEntriesRequest, ValidateMovesRequest,
};
use sqlx::PgPool;
use uuid::Uuid;

use super::shared::{
    MoveCellKey, MoveEntryRefs, MoveEntryRow, MoveSourceRow, SwapConflictInfo, SwapEntryRow,
    SwapOutcome,
};

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
