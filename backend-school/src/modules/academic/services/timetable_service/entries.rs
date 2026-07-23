use crate::error::AppError;
use crate::modules::academic::models::timetable::{
    ConflictInfo, CreateTimetableEntryRequest, TimetableEntry, UpdateTimetableEntryRequest,
};
use sqlx::PgPool;
use uuid::Uuid;

pub use super::shared::{CreateEntryOutcome, TimetableFilter, UpdateEntryOutcome};
use super::validate_entry;

/// SELECT clause พร้อม joins ที่ใช้ร่วมระหว่าง list_entries และ fetch_entry_by_id
/// แก้ตรงนี้ที่เดียวเมื่อต้องเพิ่มฟิลด์ joined
const ENTRY_SELECT_WITH_JOINS: &str = r#"
SELECT
    te.*,
    s.code   AS subject_code,
    s.name_th AS subject_name_th,
    s.group_id AS subject_group_id,
    sg.name_th AS subject_group_name,
    sg.display_order AS subject_group_display_order,
    (SELECT ARRAY_AGG(concat(u2.first_name, ' ', u2.last_name) ORDER BY tei2.role, tei2.created_at, tei2.instructor_id)
     FROM timetable_entry_instructors tei2
     JOIN users u2 ON u2.id = tei2.instructor_id
     WHERE tei2.entry_id = te.id) AS instructor_names,
    (SELECT ARRAY_AGG(tei_id.instructor_id ORDER BY tei_id.role, tei_id.created_at, tei_id.instructor_id)
     FROM timetable_entry_instructors tei_id
     WHERE tei_id.entry_id = te.id) AS instructor_ids,
    (SELECT ARRAY_AGG(tei_role.role ORDER BY tei_role.role, tei_role.created_at, tei_role.instructor_id)
     FROM timetable_entry_instructors tei_role
     WHERE tei_role.entry_id = te.id) AS instructor_roles,
    (SELECT ARRAY_AGG(teacher_subject_group.subject_group_id ORDER BY tei_sg.role, tei_sg.created_at, tei_sg.instructor_id)
     FROM timetable_entry_instructors tei_sg
     LEFT JOIN LATERAL (
         SELECT teacher_sg.id AS subject_group_id
         FROM organization_members om
         JOIN organization_units ou ON ou.id = om.organization_unit_id
         JOIN subject_groups teacher_sg ON teacher_sg.id = ou.subject_group_id
         WHERE om.user_id = tei_sg.instructor_id
           AND om.ended_at IS NULL
           AND ou.is_active = true
           AND ou.unit_type = 'subject_group'
           AND teacher_sg.is_active = true
         ORDER BY om.is_primary DESC, om.started_at DESC, ou.display_order, ou.name
         LIMIT 1
     ) teacher_subject_group ON true
     WHERE tei_sg.entry_id = te.id) AS instructor_subject_group_ids,
    (SELECT ARRAY_AGG(teacher_subject_group.name_th ORDER BY tei_sg.role, tei_sg.created_at, tei_sg.instructor_id)
     FROM timetable_entry_instructors tei_sg
     LEFT JOIN LATERAL (
         SELECT teacher_sg.name_th
         FROM organization_members om
         JOIN organization_units ou ON ou.id = om.organization_unit_id
         JOIN subject_groups teacher_sg ON teacher_sg.id = ou.subject_group_id
         WHERE om.user_id = tei_sg.instructor_id
           AND om.ended_at IS NULL
           AND ou.is_active = true
           AND ou.unit_type = 'subject_group'
           AND teacher_sg.is_active = true
         ORDER BY om.is_primary DESC, om.started_at DESC, ou.display_order, ou.name
         LIMIT 1
     ) teacher_subject_group ON true
     WHERE tei_sg.entry_id = te.id) AS instructor_subject_group_names,
    (SELECT ARRAY_AGG(teacher_subject_group.display_order ORDER BY tei_sg.role, tei_sg.created_at, tei_sg.instructor_id)
     FROM timetable_entry_instructors tei_sg
     LEFT JOIN LATERAL (
         SELECT teacher_sg.display_order
         FROM organization_members om
         JOIN organization_units ou ON ou.id = om.organization_unit_id
         JOIN subject_groups teacher_sg ON teacher_sg.id = ou.subject_group_id
         WHERE om.user_id = tei_sg.instructor_id
           AND om.ended_at IS NULL
           AND ou.is_active = true
           AND ou.unit_type = 'subject_group'
           AND teacher_sg.is_active = true
         ORDER BY om.is_primary DESC, om.started_at DESC, ou.display_order, ou.name
         LIMIT 1
     ) teacher_subject_group ON true
     WHERE tei_sg.entry_id = te.id) AS instructor_subject_group_display_orders,
    (SELECT concat(u3.first_name, ' ', u3.last_name)
     FROM timetable_entry_instructors tei3
     JOIN users u3 ON u3.id = tei3.instructor_id
     WHERE tei3.entry_id = te.id
     ORDER BY tei3.role, tei3.created_at, tei3.instructor_id
     LIMIT 1) AS instructor_name,
    cr.name  AS classroom_name,
    r.code   AS room_code,
    ap.name  AS period_name,
    ap.order_index AS period_order_index,
    ap.start_time,
    ap.end_time,
    asl_ac.name AS activity_slot_name,
    asl_ac.activity_type AS activity_type,
    asl_ac.scheduling_mode AS activity_scheduling_mode
FROM academic_timetable_entries te
LEFT JOIN classroom_courses cc ON te.classroom_course_id = cc.id
LEFT JOIN subjects s ON cc.subject_id = s.id
LEFT JOIN subject_groups sg ON sg.id = s.group_id
LEFT JOIN class_rooms cr ON te.classroom_id = cr.id
JOIN academic_periods ap ON te.period_id = ap.id
LEFT JOIN rooms r ON te.room_id = r.id
LEFT JOIN activity_slots asl ON te.activity_slot_id = asl.id
LEFT JOIN activity_catalog asl_ac ON asl.activity_catalog_id = asl_ac.id
"#;

/// List timetable entries ตาม filter — single query path สำหรับทุกมุมมอง
pub async fn list_entries(
    pool: &PgPool,
    filter: TimetableFilter,
) -> Result<Vec<TimetableEntry>, AppError> {
    let mut sql = String::from(ENTRY_SELECT_WITH_JOINS);
    sql.push_str(" WHERE te.is_active = true");

    let mut idx = 0u32;

    if filter.classroom_id.is_some() {
        idx += 1;
        sql.push_str(&format!(" AND te.classroom_id = ${idx}"));
    }

    if filter.student_id.is_some() {
        idx += 1;
        sql.push_str(&format!(
            " AND te.classroom_id IN (SELECT class_room_id FROM student_class_enrollments WHERE student_id = ${idx} AND status = 'active')"
        ));
    }

    if filter.instructor_id.is_some() {
        idx += 1;
        if filter.include_team_ghosts {
            // Ghost mode: รวม cell ที่ instructor อยู่ในทีมของ course (cci) หรือถูก assign ใน tei
            sql.push_str(&format!(
                " AND (EXISTS (SELECT 1 FROM classroom_course_instructors cci \
                       WHERE cci.classroom_course_id = te.classroom_course_id AND cci.instructor_id = ${idx}) \
                    OR EXISTS (SELECT 1 FROM timetable_entry_instructors tei \
                       WHERE tei.entry_id = te.id AND tei.instructor_id = ${idx}))"
            ));
        } else {
            sql.push_str(&format!(
                " AND EXISTS (SELECT 1 FROM timetable_entry_instructors tei WHERE tei.entry_id = te.id AND tei.instructor_id = ${idx})"
            ));
        }
    }

    if filter.room_id.is_some() {
        idx += 1;
        sql.push_str(&format!(" AND te.room_id = ${idx}"));
    }

    if filter.academic_semester_id.is_some() {
        idx += 1;
        sql.push_str(&format!(" AND te.academic_semester_id = ${idx}"));
    }

    if filter.day_of_week.is_some() {
        idx += 1;
        sql.push_str(&format!(" AND te.day_of_week = ${idx}"));
    }

    if filter.entry_type.is_some() {
        idx += 1;
        sql.push_str(&format!(" AND te.entry_type = ${idx}"));
    }

    sql.push_str(" ORDER BY te.day_of_week, ap.order_index");

    let mut q = sqlx::query_as::<_, TimetableEntry>(&sql);
    if let Some(v) = filter.classroom_id {
        q = q.bind(v);
    }
    if let Some(v) = filter.student_id {
        q = q.bind(v);
    }
    if let Some(v) = filter.instructor_id {
        q = q.bind(v);
    }
    if let Some(v) = filter.room_id {
        q = q.bind(v);
    }
    if let Some(v) = filter.academic_semester_id {
        q = q.bind(v);
    }
    if let Some(v) = filter.day_of_week {
        q = q.bind(v);
    }
    if let Some(v) = filter.entry_type {
        q = q.bind(v);
    }

    q.fetch_all(pool).await.map_err(|e| {
        tracing::error!("Failed to fetch timetable entries: {}", e);
        AppError::InternalServerError("Failed to fetch timetable".to_string())
    })
}

/// Fetch 1 entry พร้อม joined fields (subject, classroom, room, period, instructors)
/// ใช้กับ patch events เพื่อให้ frontend ได้ entry ครบ, patch ได้ทันทีไม่ต้อง re-fetch
pub async fn fetch_entry_by_id(pool: &PgPool, entry_id: Uuid) -> Option<TimetableEntry> {
    let mut sql = String::from(ENTRY_SELECT_WITH_JOINS);
    sql.push_str(" WHERE te.id = $1");

    sqlx::query_as::<_, TimetableEntry>(&sql)
        .bind(entry_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
}

pub async fn resolve_classroom_course_semester_id(
    pool: &PgPool,
    classroom_course_id: Uuid,
) -> Result<Option<Uuid>, AppError> {
    sqlx::query_scalar("SELECT academic_semester_id FROM classroom_courses WHERE id = $1")
        .bind(classroom_course_id)
        .fetch_optional(pool)
        .await
        .map_err(AppError::from)
}

/// สร้าง entry ใหม่ — รวม validate + lookup + insert + populate junction ใน transaction
pub async fn create_entry(
    pool: &PgPool,
    user_id: Option<Uuid>,
    payload: CreateTimetableEntryRequest,
) -> Result<CreateEntryOutcome, AppError> {
    // 1. Validate conflicts
    let validation = validate_entry(pool, &payload).await?;
    if !validation.is_valid {
        return Ok(CreateEntryOutcome::Conflict(validation.conflicts));
    }

    // 2. Lookup classroom/semester/type/title ตาม entry source (course vs activity slot)
    let (classroom_id_val, academic_semester_id, entry_type, title, activity_slot_id) =
        if let Some(course_id) = payload.classroom_course_id {
            let info: Option<(Uuid, Uuid)> = sqlx::query_as(
                "SELECT classroom_id, academic_semester_id FROM classroom_courses WHERE id = $1",
            )
            .bind(course_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| AppError::InternalServerError(e.to_string()))?;

            match info {
                Some((cls, sem)) => (cls, sem, "COURSE".to_string(), None::<String>, None::<Uuid>),
                None => return Err(AppError::NotFound("Classroom course not found".to_string())),
            }
        } else if let Some(slot_id) = payload.activity_slot_id {
            let cls = payload.classroom_id.ok_or_else(|| {
                AppError::BadRequest("classroom_id required for activity entry".to_string())
            })?;
            let sem = payload.academic_semester_id.ok_or_else(|| {
                AppError::BadRequest("academic_semester_id required for activity entry".to_string())
            })?;

            // Validate: classroom participates in this slot
            let participates: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM activity_slot_classrooms
                 WHERE slot_id = $1 AND classroom_id = $2)",
            )
            .bind(slot_id)
            .bind(cls)
            .fetch_one(pool)
            .await
            .unwrap_or(false);
            if !participates {
                return Err(AppError::BadRequest(
                    "ห้องนี้ไม่ได้อยู่ในกิจกรรมนี้ — เพิ่มห้องที่ Course Planning ก่อน".to_string(),
                ));
            }

            // Validate: must have instructor
            let has_instructor: bool = sqlx::query_scalar(
                r#"SELECT CASE
                     WHEN ac.scheduling_mode = 'independent' THEN
                         EXISTS(SELECT 1 FROM activity_slot_classroom_assignments
                                WHERE slot_id = $1 AND classroom_id = $2)
                     ELSE
                         EXISTS(SELECT 1 FROM activity_slot_instructors WHERE slot_id = $1)
                   END
                   FROM activity_slots s
                   JOIN activity_catalog ac ON ac.id = s.activity_catalog_id
                   WHERE s.id = $1"#,
            )
            .bind(slot_id)
            .bind(cls)
            .fetch_one(pool)
            .await
            .unwrap_or(false);
            if !has_instructor {
                return Err(AppError::BadRequest(
                    "กิจกรรมนี้ยังไม่ได้กำหนดครูผู้สอน — เพิ่มครูที่หน้า Activities ก่อน".to_string(),
                ));
            }

            // Lookup slot name (from catalog via FK) for title
            let slot_name: Option<String> = sqlx::query_scalar(
                "SELECT ac.name FROM activity_slots s
                 JOIN activity_catalog ac ON ac.id = s.activity_catalog_id
                 WHERE s.id = $1",
            )
            .bind(slot_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| AppError::InternalServerError(e.to_string()))?;

            let title = payload.title.clone().or(slot_name);
            let et = payload
                .entry_type
                .clone()
                .unwrap_or_else(|| "ACTIVITY".to_string());
            (cls, sem, et, title, Some(slot_id))
        } else {
            return Err(AppError::BadRequest(
                "classroom_course_id or activity_slot_id required".to_string(),
            ));
        };

    // 3. Insert + populate junction in transaction
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    let entry = sqlx::query_as::<_, TimetableEntry>(
        r#"
        INSERT INTO academic_timetable_entries (
            id, classroom_course_id, day_of_week, period_id,
            room_id, note, classroom_id, academic_semester_id, entry_type, title, is_active,
            created_by, updated_by, activity_slot_id
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, true, $11, $11, $12)
        RETURNING *, NULL::TEXT AS subject_code, NULL::TEXT AS subject_name_th,
                  NULL::TEXT[] AS instructor_names,
                  NULL::TEXT AS instructor_name, NULL::TEXT AS classroom_name,
                  NULL::TEXT AS room_code, NULL::TEXT AS period_name,
                  NULL::TIME AS start_time, NULL::TIME AS end_time,
                  NULL::TEXT AS activity_slot_name, NULL::TEXT AS activity_type,
                  NULL::TEXT AS activity_scheduling_mode
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(payload.classroom_course_id)
    .bind(&payload.day_of_week)
    .bind(payload.period_id)
    .bind(payload.room_id)
    .bind(&payload.note)
    .bind(classroom_id_val)
    .bind(academic_semester_id)
    .bind(&entry_type)
    .bind(&title)
    .bind(user_id)
    .bind(activity_slot_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| {
        tracing::error!("Failed to create timetable entry: {}", e);
        if e.to_string().contains("unique_entry_per_slot") {
            AppError::BadRequest("This slot is already occupied".to_string())
        } else {
            AppError::InternalServerError("Failed to create timetable entry".to_string())
        }
    })?;

    // Populate timetable_entry_instructors from source (classroom_course หรือ activity_slot)
    if let Some(cc_id) = entry.classroom_course_id {
        sqlx::query(
            "INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
             SELECT $1, instructor_id, role FROM classroom_course_instructors
             WHERE classroom_course_id = $2 ON CONFLICT DO NOTHING",
        )
        .bind(entry.id)
        .bind(cc_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    } else if let Some(slot_id) = entry.activity_slot_id {
        let mode: Option<String> = sqlx::query_scalar(
            "SELECT ac.scheduling_mode FROM activity_slots s JOIN activity_catalog ac ON ac.id = s.activity_catalog_id WHERE s.id = $1",
        )
        .bind(slot_id)
        .fetch_optional(&mut *tx)
        .await
        .ok()
        .flatten();

        if mode.as_deref() == Some("independent") {
            sqlx::query(
                "INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
                 SELECT $1, instructor_id, 'primary'
                 FROM activity_slot_classroom_assignments
                 WHERE slot_id = $2 AND classroom_id = $3 ON CONFLICT DO NOTHING",
            )
            .bind(entry.id)
            .bind(slot_id)
            .bind(entry.classroom_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::InternalServerError(e.to_string()))?;
        } else {
            sqlx::query(
                "INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
                 SELECT $1, user_id, 'primary' FROM activity_slot_instructors
                 WHERE slot_id = $2 ON CONFLICT DO NOTHING",
            )
            .bind(entry.id)
            .bind(slot_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::InternalServerError(e.to_string()))?;
        }
    }

    tx.commit()
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    Ok(CreateEntryOutcome::Created(Box::new(entry)))
}

/// ลบ entry เดียว — return semester_id (None ถ้าไม่เจอ entry)
pub async fn delete_entry(pool: &PgPool, id: Uuid) -> Result<Option<Uuid>, AppError> {
    sqlx::query_scalar(
        "DELETE FROM academic_timetable_entries WHERE id = $1 RETURNING academic_semester_id",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|_| AppError::InternalServerError("Failed to delete entry".to_string()))
}

/// Update entry — รวม fetch existing + validate (exclude self) + update
/// Return Conflict ถ้าเจอ conflict (handler broadcast DropRejected), Updated ถ้าสำเร็จ
pub async fn update_entry(
    pool: &PgPool,
    user_id: Option<Uuid>,
    id: Uuid,
    payload: UpdateTimetableEntryRequest,
) -> Result<UpdateEntryOutcome, AppError> {
    // 1. Fetch existing entry
    let existing_entry = sqlx::query_as::<_, TimetableEntry>(
        r#"
        SELECT te.*, NULL::TEXT as subject_code, NULL::TEXT as subject_name_th, NULL::TEXT as instructor_name,
               NULL::TEXT as classroom_name, NULL::TEXT as room_code, NULL::TEXT as period_name,
               NULL::TIME as start_time, NULL::TIME as end_time,
               NULL::TEXT as activity_slot_name, NULL::TEXT as activity_type
        FROM academic_timetable_entries te WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .map_err(|_| AppError::NotFound("Entry not found".to_string()))?;

    // Block: ถ้า entry สร้างจาก batch (pinned) → ไม่ให้ update
    if existing_entry.batch_id.is_some() {
        return Err(AppError::BadRequest(
            "คาบที่สร้างจาก Batch ไม่สามารถย้าย/เปลี่ยนเนื้อหาได้ (ลบก่อนแล้ว batch ใหม่แทน)".to_string(),
        ));
    }

    // 2. Compute target values (new or fallback to existing)
    let target_classroom_id: Option<Uuid> = payload.classroom_id.or(existing_entry.classroom_id);
    let target_day = payload
        .day_of_week
        .clone()
        .unwrap_or_else(|| existing_entry.day_of_week.clone());
    let target_period = payload.period_id.unwrap_or(existing_entry.period_id);
    let target_room = payload.room_id.or(existing_entry.room_id);

    // 3. Validate conflicts (excluding current entry ID)
    let mut conflict_list: Vec<ConflictInfo> = Vec::new();

    // 3a. Classroom conflict
    let classroom_conflict: Option<(String,)> = sqlx::query_as(
        r#"SELECT cr.name
           FROM academic_timetable_entries te
           LEFT JOIN class_rooms cr ON cr.id = te.classroom_id
           WHERE te.classroom_id = $1
             AND te.day_of_week = $2
             AND te.period_id = $3
             AND te.is_active = true
             AND te.id <> $4
           LIMIT 1"#,
    )
    .bind(target_classroom_id)
    .bind(&target_day)
    .bind(target_period)
    .bind(id)
    .fetch_optional(pool)
    .await
    .unwrap_or(None);

    if let Some((cr_name,)) = classroom_conflict {
        conflict_list.push(ConflictInfo {
            conflict_type: "CLASSROOM_CONFLICT".to_string(),
            message: format!("{} มีตารางในคาบนี้อยู่แล้ว", cr_name),
            existing_entry: None,
        });
    }

    // 3b. Room conflict
    if let Some(room_id) = target_room {
        let room_conflict: Option<(String,)> = sqlx::query_as(
            r#"SELECT r.code
               FROM academic_timetable_entries te
               JOIN rooms r ON r.id = te.room_id
               WHERE te.room_id = $1
                 AND te.day_of_week = $2
                 AND te.period_id = $3
                 AND te.is_active = true
                 AND te.id <> $4
               LIMIT 1"#,
        )
        .bind(room_id)
        .bind(&target_day)
        .bind(target_period)
        .bind(id)
        .fetch_optional(pool)
        .await
        .unwrap_or(None);

        if let Some((room_code,)) = room_conflict {
            conflict_list.push(ConflictInfo {
                conflict_type: "ROOM_CONFLICT".to_string(),
                message: format!("ห้อง {} มีการใช้งานในคาบนี้อยู่แล้ว", room_code),
                existing_entry: None,
            });
        }
    }

    // 3c. Instructor conflict via junction
    let candidate_instructors: Vec<Uuid> = sqlx::query_scalar(
        "SELECT instructor_id FROM timetable_entry_instructors WHERE entry_id = $1",
    )
    .bind(id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    if !candidate_instructors.is_empty() {
        let conflict_instructors: Vec<(String,)> = sqlx::query_as(
            r#"SELECT DISTINCT concat(u.first_name, ' ', u.last_name)
               FROM academic_timetable_entries te
               JOIN timetable_entry_instructors tei ON tei.entry_id = te.id
               JOIN users u ON u.id = tei.instructor_id
               WHERE tei.instructor_id = ANY($1)
                 AND te.day_of_week = $2
                 AND te.period_id = $3
                 AND te.is_active = true
                 AND te.id <> $4"#,
        )
        .bind(&candidate_instructors)
        .bind(&target_day)
        .bind(target_period)
        .bind(id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        for (name,) in &conflict_instructors {
            conflict_list.push(ConflictInfo {
                conflict_type: "INSTRUCTOR_CONFLICT".to_string(),
                message: format!("{} มีสอนในคาบนี้อยู่แล้ว", name),
                existing_entry: None,
            });
        }
    }

    if !conflict_list.is_empty() {
        return Ok(UpdateEntryOutcome::Conflict {
            conflicts: conflict_list,
            existing: Box::new(existing_entry),
        });
    }

    // 4. Update Entry
    let updated_entry = sqlx::query_as::<_, TimetableEntry>(
        r#"
        UPDATE academic_timetable_entries SET
            day_of_week = COALESCE($2, day_of_week),
            period_id = COALESCE($3, period_id),
            room_id = COALESCE($4, room_id),
            note = COALESCE($5, note),
            classroom_course_id = COALESCE($7, classroom_course_id),
            activity_slot_id = COALESCE($8, activity_slot_id),
            classroom_id = COALESCE($9, classroom_id),
            updated_at = NOW(),
            updated_by = $6
        WHERE id = $1
        RETURNING *
        "#,
    )
    .bind(id)
    .bind(payload.day_of_week)
    .bind(payload.period_id)
    .bind(payload.room_id)
    .bind(payload.note)
    .bind(user_id)
    .bind(payload.classroom_course_id)
    .bind(payload.activity_slot_id)
    .bind(payload.classroom_id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to update entry: {}", e);
        let msg = e.to_string();
        if msg.contains("unique_entry_per_slot") {
            AppError::BadRequest("This slot is already occupied".to_string())
        } else if msg.contains("instructor double-book") || msg.contains("ไม่สามารถย้าย")
        {
            AppError::BadRequest(
                msg.split("ERROR:")
                    .last()
                    .unwrap_or(&msg)
                    .trim()
                    .to_string(),
            )
        } else {
            AppError::InternalServerError("Failed to update entry".to_string())
        }
    })?;

    Ok(UpdateEntryOutcome::Updated {
        updated: Box::new(updated_entry),
        existing: Box::new(existing_entry),
    })
}
