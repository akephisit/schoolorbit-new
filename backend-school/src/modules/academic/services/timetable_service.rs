use crate::error::AppError;
use crate::modules::academic::models::timetable::CreateBatchTimetableEntriesRequest;
use sqlx::PgPool;
use uuid::Uuid;

mod entries;
mod instructors;
mod moves_and_swaps;
mod occupancy;
mod shared;
mod validation;

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
pub use moves_and_swaps::{swap_entries, validate_moves};
pub use occupancy::get_occupancy;
#[allow(unused_imports)]
pub use occupancy::OccupancyRow;
#[allow(unused_imports)]
pub use shared::{
    BatchBlockedCell, BatchCreateOutcome, BatchDeletedEntry, BatchExcludedInstructor,
    BatchInstructorConflict, BatchSkippedCell, CreateEntryOutcome, SwapConflictInfo, SwapOutcome,
    TimetableFilter, UpdateEntryOutcome,
};
pub use validation::validate_entry;

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
