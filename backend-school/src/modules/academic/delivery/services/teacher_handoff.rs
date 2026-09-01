use std::collections::{BTreeMap, BTreeSet, HashMap};

use chrono::NaiveDate;
use serde::Serialize;
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::academic::delivery::models::{
    AcademicTermChangeActionKind, ApplyTeacherHandoffRequest, ApplyTeacherHandoffResponse,
    PreviewTeacherHandoffRequest, TeacherHandoffConflict, TeacherHandoffConflictKind,
    TeacherHandoffEntryPreview, TeacherHandoffEntryVersion, TeacherHandoffInstructorPreview,
    TeacherHandoffMode, TeacherHandoffPreview,
};
use crate::modules::academic::services::effective_teacher_service::{
    eligible_teacher_ids_for_group, project_effective_assignments_in_tx,
};

use super::{stable_hash, validate_row_version};

#[derive(Debug, FromRow)]
struct HandoffContextRow {
    change_set_row_version: i64,
    effective_from: NaiveDate,
    academic_term_id: Uuid,
    academic_year_id: Uuid,
    target_timetable_version_id: Uuid,
    target_timetable_version_row_version: i64,
}

#[derive(Debug, FromRow)]
struct TeacherChangeItemRow {
    id: Uuid,
    action_kind: AcademicTermChangeActionKind,
    learning_group_id: Uuid,
    learning_group_teacher_id: Option<Uuid>,
    teacher_id: Uuid,
}

#[derive(Clone, Debug, FromRow)]
struct HandoffEntryRow {
    id: Uuid,
    row_version: i64,
    learning_group_id: Uuid,
    learning_group_label: String,
    offering_label: String,
    day_of_week: String,
    bell_schedule_period_id: Uuid,
    period_label: String,
    room_label: Option<String>,
}

#[derive(Clone, Debug, FromRow)]
struct EntryInstructorRow {
    entry_id: Uuid,
    instructor_id: Uuid,
    display_name: String,
    role: String,
}

#[derive(Debug, FromRow)]
struct OccupiedInstructorRow {
    entry_id: Uuid,
    instructor_id: Uuid,
    day_of_week: String,
    bell_schedule_period_id: Uuid,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct NormalizedApplyRequest {
    change_set_id: Uuid,
    change_set_row_version: i64,
    target_timetable_version_row_version: i64,
    teacher_change_item_id: Uuid,
    entries: Vec<(Uuid, i64)>,
    mode: TeacherHandoffMode,
    instructor_ids: Vec<Uuid>,
    preview_hash: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TeacherHandoffAuditPayload<'a> {
    change_set_id: Uuid,
    teacher_change_item_id: Uuid,
    timetable_version_id: Uuid,
    effective_from: NaiveDate,
    before_instructors: &'a [TeacherHandoffInstructorPreview],
    after_instructors: &'a [TeacherHandoffInstructorPreview],
    before_row_version: i64,
    after_row_version: i64,
}

#[derive(Debug)]
pub struct TeacherHandoffApplyOutcome {
    pub response: ApplyTeacherHandoffResponse,
    pub academic_term_id: Uuid,
}

pub async fn preview(
    pool: &PgPool,
    change_set_id: Uuid,
    request: PreviewTeacherHandoffRequest,
) -> Result<TeacherHandoffPreview, AppError> {
    let mut transaction = pool.begin().await?;
    let preview = preview_in_tx(&mut transaction, change_set_id, &request, false).await?;
    transaction.commit().await?;
    Ok(preview)
}

pub async fn apply(
    pool: &PgPool,
    actor_user_id: Uuid,
    change_set_id: Uuid,
    request: ApplyTeacherHandoffRequest,
) -> Result<TeacherHandoffApplyOutcome, AppError> {
    validate_row_version(request.change_set_row_version)?;
    validate_row_version(request.target_timetable_version_row_version)?;
    if request.preview_hash.len() != 64
        || !request
            .preview_hash
            .bytes()
            .all(|value| value.is_ascii_hexdigit() && !value.is_ascii_uppercase())
    {
        return Err(AppError::ValidationError("previewHash ไม่ถูกต้อง".to_string()));
    }
    let entries = canonical_entry_versions(&request.entries)?;
    if entries.is_empty() {
        return Err(AppError::ValidationError(
            "เลือกคาบที่ต้องการส่งมอบอย่างน้อยหนึ่งคาบ".to_string(),
        ));
    }
    let instructor_ids = canonical_ids(&request.instructor_ids);
    let request_hash = stable_hash(&NormalizedApplyRequest {
        change_set_id,
        change_set_row_version: request.change_set_row_version,
        target_timetable_version_row_version: request.target_timetable_version_row_version,
        teacher_change_item_id: request.teacher_change_item_id,
        entries: entries.clone(),
        mode: request.mode,
        instructor_ids: instructor_ids.clone(),
        preview_hash: request.preview_hash.clone(),
    })?;

    let mut transaction = pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(format!(
            "academic-teacher-handoff:{}",
            request.idempotency_key
        ))
        .execute(&mut *transaction)
        .await?;
    if let Some((stored_hash, sqlx::types::Json(mut response), academic_term_id)) =
        sqlx::query_as::<_, (String, sqlx::types::Json<ApplyTeacherHandoffResponse>, Uuid)>(
            r#"SELECT request_hash::text, response_snapshot, academic_term_id
           FROM academic_teacher_handoff_runs
           WHERE idempotency_key = $1"#,
        )
        .bind(request.idempotency_key)
        .fetch_optional(&mut *transaction)
        .await?
    {
        if stored_hash != request_hash {
            return Err(AppError::Conflict(
                "idempotencyKey นี้ถูกใช้กับคำขอส่งมอบคาบอื่นแล้ว".to_string(),
            ));
        }
        response.replayed = true;
        transaction.commit().await?;
        return Ok(TeacherHandoffApplyOutcome {
            response,
            academic_term_id,
        });
    }

    let entry_ids = entries.iter().map(|(id, _)| *id).collect::<Vec<_>>();
    let preview_request = PreviewTeacherHandoffRequest {
        change_set_row_version: request.change_set_row_version,
        target_timetable_version_row_version: request.target_timetable_version_row_version,
        teacher_change_item_id: request.teacher_change_item_id,
        entry_ids: entry_ids.clone(),
        mode: request.mode,
        instructor_ids,
    };
    let preview = preview_in_tx(&mut transaction, change_set_id, &preview_request, true).await?;
    if preview.preview_hash.as_deref() != Some(request.preview_hash.as_str()) {
        return Err(AppError::Conflict(
            "ข้อมูลเปลี่ยนหลังการตรวจตัวอย่าง กรุณาตรวจอีกครั้ง".to_string(),
        ));
    }
    if !preview.can_apply {
        return Err(AppError::Conflict(
            "ยังมีคาบชนหรือครูไม่พร้อม จึงยังส่งมอบคาบไม่ได้".to_string(),
        ));
    }
    if request.mode == TeacherHandoffMode::Manual {
        return Err(AppError::ValidationError(
            "โหมดจัดเองไม่ส่งคำขอ apply ให้แก้ในหน้าตารางสอน".to_string(),
        ));
    }
    let context: (Uuid, Uuid, Uuid, NaiveDate) = sqlx::query_as(
        r#"SELECT academic_term_id, academic_year_id,
                  target_timetable_version_id, effective_from
           FROM academic_term_change_sets WHERE id = $1"#,
    )
    .bind(change_set_id)
    .fetch_one(&mut *transaction)
    .await?;
    let expected_versions = entries.into_iter().collect::<BTreeMap<_, _>>();
    for entry in &preview.proposed_entries {
        if expected_versions.get(&entry.entry_id) != Some(&entry.row_version) {
            return Err(AppError::Conflict(
                "คาบเรียนถูกแก้ไขหลังเลือก กรุณาตรวจรายการใหม่".to_string(),
            ));
        }
    }

    sqlx::query(
        "DELETE FROM academic_timetable_block_group_instructors WHERE block_group_id = ANY($1)",
    )
    .bind(&entry_ids)
    .execute(&mut *transaction)
    .await?;
    for entry in &preview.proposed_entries {
        for (index, instructor) in entry.after_instructors.iter().enumerate() {
            sqlx::query(
                r#"INSERT INTO academic_timetable_block_group_instructors (
                       id, block_group_id, instructor_id, role, display_order
                   ) VALUES ($1, $2, $3, $4, $5)"#,
            )
            .bind(Uuid::new_v4())
            .bind(entry.entry_id)
            .bind(instructor.instructor_id)
            .bind(&instructor.role)
            .bind(
                i32::try_from(index + 1).map_err(|_| {
                    AppError::ValidationError("จำนวนครูผู้สอนมากเกินกว่าที่รองรับ".to_string())
                })?,
            )
            .execute(&mut *transaction)
            .await?;
        }
        let updated = sqlx::query(
            r#"UPDATE academic_timetable_block_groups
               SET row_version = row_version + 1, updated_by = $1, updated_at = now()
               WHERE id = $2 AND row_version = $3"#,
        )
        .bind(actor_user_id)
        .bind(entry.entry_id)
        .bind(entry.row_version)
        .execute(&mut *transaction)
        .await?;
        if updated.rows_affected() != 1 {
            return Err(AppError::Conflict(
                "คาบเรียนถูกแก้ไขระหว่างส่งมอบ กรุณาลองใหม่".to_string(),
            ));
        }
        let audit_payload = TeacherHandoffAuditPayload {
            change_set_id,
            teacher_change_item_id: request.teacher_change_item_id,
            timetable_version_id: preview.target_timetable_version_id,
            effective_from: context.3,
            before_instructors: &entry.before_instructors,
            after_instructors: &entry.after_instructors,
            before_row_version: entry.row_version,
            after_row_version: entry.row_version + 1,
        };
        sqlx::query(
            r#"INSERT INTO academic_audit_events (
                   event_code, entity_type, entity_id, academic_year_id,
                   academic_term_id, actor_user_id, payload
               ) VALUES (
                   'academic_teacher_handoff.applied',
                   'academic_timetable_block_group', $1, $2, $3, $4, $5
               )"#,
        )
        .bind(entry.entry_id)
        .bind(context.1)
        .bind(context.0)
        .bind(actor_user_id)
        .bind(sqlx::types::Json(&audit_payload))
        .execute(&mut *transaction)
        .await?;
    }
    let updated_entries = preview
        .proposed_entries
        .iter()
        .map(|entry| TeacherHandoffEntryVersion {
            entry_id: entry.entry_id,
            row_version: entry.row_version + 1,
        })
        .collect::<Vec<_>>();
    let response = ApplyTeacherHandoffResponse {
        handoff: preview,
        updated_entries,
        replayed: false,
    };
    sqlx::query(
        r#"INSERT INTO academic_teacher_handoff_runs (
               idempotency_key, change_set_id, teacher_change_item_id,
               timetable_version_id, academic_term_id, academic_year_id,
               request_hash, selected_entry_ids, response_snapshot, applied_by
           ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)"#,
    )
    .bind(request.idempotency_key)
    .bind(change_set_id)
    .bind(request.teacher_change_item_id)
    .bind(context.2)
    .bind(context.0)
    .bind(context.1)
    .bind(&request_hash)
    .bind(&entry_ids)
    .bind(sqlx::types::Json(&response))
    .bind(actor_user_id)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(TeacherHandoffApplyOutcome {
        response,
        academic_term_id: context.0,
    })
}

async fn preview_in_tx(
    transaction: &mut Transaction<'_, Postgres>,
    change_set_id: Uuid,
    request: &PreviewTeacherHandoffRequest,
    lock_for_apply: bool,
) -> Result<TeacherHandoffPreview, AppError> {
    validate_row_version(request.change_set_row_version)?;
    validate_row_version(request.target_timetable_version_row_version)?;
    let context_lock = if lock_for_apply {
        "FOR UPDATE OF change_set, version"
    } else {
        "FOR SHARE OF change_set, version"
    };
    let context: HandoffContextRow = sqlx::query_as(&format!(
        r#"SELECT change_set.row_version AS change_set_row_version,
                  change_set.effective_from, change_set.academic_term_id,
                  change_set.academic_year_id,
                  change_set.target_timetable_version_id,
                  version.row_version AS target_timetable_version_row_version
           FROM academic_term_change_sets change_set
           JOIN academic_timetable_versions version
             ON version.id = change_set.target_timetable_version_id
           WHERE change_set.id = $1
             AND change_set.status = 'draft'
             AND version.status = 'draft'
             AND version.change_set_id = change_set.id
           {context_lock}"#,
    ))
    .bind(change_set_id)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or_else(|| AppError::Conflict("ชุดการเปลี่ยนแปลงหรือรุ่นตารางไม่ใช่แบบร่างแล้ว".to_string()))?;
    if context.change_set_row_version != request.change_set_row_version
        || context.target_timetable_version_row_version
            != request.target_timetable_version_row_version
    {
        return Err(AppError::Conflict(
            "ชุดการเปลี่ยนแปลงหรือรุ่นตารางถูกแก้ไขแล้ว".to_string(),
        ));
    }
    let item_lock = if lock_for_apply {
        "FOR UPDATE"
    } else {
        "FOR SHARE"
    };
    let item: TeacherChangeItemRow = sqlx::query_as(&format!(
        r#"SELECT id, action_kind, learning_group_id,
                  learning_group_teacher_id, teacher_id
           FROM academic_term_change_items
           WHERE id = $1 AND change_set_id = $2
             AND action_kind IN (
                 'add_group_teacher',
                 'adjust_group_teacher_role',
                 'stop_group_teacher'
             )
           {item_lock}"#,
    ))
    .bind(request.teacher_change_item_id)
    .bind(change_set_id)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบรายการเปลี่ยนครู".to_string()))?;
    if request.mode != TeacherHandoffMode::Manual
        && item.action_kind != AcademicTermChangeActionKind::StopGroupTeacher
    {
        return Err(AppError::ValidationError(
            "การส่งมอบคาบอัตโนมัติใช้ได้กับรายการหยุดครูเท่านั้น".to_string(),
        ));
    }
    validate_mode(request.mode, &request.instructor_ids)?;

    let all_affected = load_affected_entries(
        transaction,
        context.target_timetable_version_id,
        item.learning_group_id,
        item.teacher_id,
        lock_for_apply,
    )
    .await?;
    let requested_entry_ids = canonical_ids(&request.entry_ids);
    let affected_ids = all_affected
        .iter()
        .map(|entry| entry.id)
        .collect::<BTreeSet<_>>();
    if requested_entry_ids
        .iter()
        .any(|entry_id| !affected_ids.contains(entry_id))
    {
        return Err(AppError::ValidationError(
            "คาบที่เลือกไม่ได้รับผลจากรายการหยุดครูนี้".to_string(),
        ));
    }
    let selected_ids = if requested_entry_ids.is_empty() {
        affected_ids.iter().copied().collect::<Vec<_>>()
    } else {
        requested_entry_ids
    };
    let selected_set = selected_ids.iter().copied().collect::<BTreeSet<_>>();
    let all_entry_ids = all_affected
        .iter()
        .map(|entry| entry.id)
        .collect::<Vec<_>>();
    let before_by_entry = load_instructors(transaction, &all_entry_ids, lock_for_apply).await?;
    let replacement_ids = canonical_ids(&request.instructor_ids);
    let display_names = load_staff_names(transaction, &replacement_ids).await?;
    let projected =
        project_effective_assignments_in_tx(transaction, change_set_id, &[item.learning_group_id])
            .await?;
    let eligible_ids = eligible_teacher_ids_for_group(&projected, item.learning_group_id);
    let route = timetable_route(
        context.academic_year_id,
        context.academic_term_id,
        context.target_timetable_version_id,
        item.learning_group_id,
    );
    let mut conflicts = Vec::new();
    if replacement_ids.len() != request.instructor_ids.len() {
        conflicts.push(TeacherHandoffConflict {
            kind: TeacherHandoffConflictKind::DuplicateInstructor,
            message: "รายชื่อครูใหม่มีข้อมูลซ้ำ".to_string(),
            entry_ids: selected_ids.clone(),
            instructor_ids: replacement_ids.clone(),
            timetable_route: route.clone(),
        });
    }
    let ineligible = replacement_ids
        .iter()
        .filter(|id| !eligible_ids.contains(id))
        .copied()
        .collect::<Vec<_>>();
    if !ineligible.is_empty() {
        conflicts.push(TeacherHandoffConflict {
            kind: TeacherHandoffConflictKind::IneligibleInstructor,
            message: "ครูที่เลือกไม่ได้อยู่ในทีมสอนที่มีผลในวันที่เริ่มใช้".to_string(),
            entry_ids: selected_ids.clone(),
            instructor_ids: ineligible,
            timetable_route: route.clone(),
        });
    }

    let mut affected_entries = Vec::new();
    let mut proposed_entries = Vec::new();
    for entry in &all_affected {
        let before = before_by_entry.get(&entry.id).cloned().unwrap_or_default();
        let after =
            if selected_set.contains(&entry.id) && request.mode != TeacherHandoffMode::Manual {
                proposed_instructors(&before, item.teacher_id, &replacement_ids, &display_names)?
            } else {
                before.clone()
            };
        let preview = entry_preview(entry, before, after);
        affected_entries.push(preview.clone());
        if selected_set.contains(&entry.id) && request.mode != TeacherHandoffMode::Manual {
            proposed_entries.push(preview);
        }
    }
    if request.mode != TeacherHandoffMode::Manual {
        append_instructor_collisions(
            transaction,
            context.target_timetable_version_id,
            &proposed_entries,
            &selected_set,
            &route,
            &mut conflicts,
        )
        .await?;
    }
    conflicts.sort_by_key(|conflict| {
        (
            format!("{:?}", conflict.kind),
            conflict.entry_ids.clone(),
            conflict.instructor_ids.clone(),
        )
    });
    let preview_hash = if request.mode == TeacherHandoffMode::Manual {
        None
    } else {
        Some(stable_hash(&(
            change_set_id,
            context.change_set_row_version,
            context.target_timetable_version_id,
            context.target_timetable_version_row_version,
            context.effective_from,
            item.id,
            item.learning_group_teacher_id,
            item.teacher_id,
            request.mode,
            &replacement_ids,
            &proposed_entries,
            &conflicts,
        ))?)
    };
    let can_apply = request.mode != TeacherHandoffMode::Manual
        && !proposed_entries.is_empty()
        && conflicts.is_empty();
    Ok(TeacherHandoffPreview {
        change_set_id,
        change_set_row_version: context.change_set_row_version,
        teacher_change_item_id: item.id,
        target_timetable_version_id: context.target_timetable_version_id,
        target_timetable_version_row_version: context.target_timetable_version_row_version,
        mode: request.mode,
        affected_entries,
        proposed_entries,
        conflicts,
        preview_hash,
        can_apply,
        timetable_route: route,
    })
}

async fn load_affected_entries(
    transaction: &mut Transaction<'_, Postgres>,
    version_id: Uuid,
    group_id: Uuid,
    teacher_id: Uuid,
    lock_for_apply: bool,
) -> Result<Vec<HandoffEntryRow>, AppError> {
    let lock = if lock_for_apply {
        "FOR UPDATE OF block_group"
    } else {
        "FOR SHARE OF block_group"
    };
    sqlx::query_as(&format!(
        r#"SELECT block_group.id, block_group.row_version, block_group.learning_group_id,
                  concat_ws(' · ', nullif(learning_group.code, ''), learning_group.name)
                    AS learning_group_label,
                  concat_ws(' · ', nullif(offering.code_snapshot, ''), offering.name_snapshot)
                    AS offering_label,
                  block.day_of_week, block.bell_schedule_period_id,
                  period.name AS period_label,
                  CASE WHEN room.id IS NULL THEN NULL
                       ELSE concat_ws(' · ', nullif(room.code, ''), room.name_th)
                  END AS room_label
           FROM academic_timetable_block_groups block_group
           JOIN academic_timetable_blocks block ON block.id = block_group.block_id
           JOIN learning_groups learning_group ON learning_group.id = block_group.learning_group_id
           JOIN learning_offerings offering ON offering.id = block_group.learning_offering_id
           JOIN bell_schedule_periods period ON period.id = block.bell_schedule_period_id
           LEFT JOIN rooms room ON room.id = block_group.room_id
           WHERE block.timetable_version_id = $1
             AND block_group.learning_group_id = $2
             AND block.is_active
             AND block_group.is_active
             AND EXISTS (
                 SELECT 1 FROM academic_timetable_block_group_instructors instructor
                 WHERE instructor.block_group_id = block_group.id
                   AND instructor.instructor_id = $3
             )
           ORDER BY block.day_of_week, period.order_index, block_group.id
           {lock}"#,
    ))
    .bind(version_id)
    .bind(group_id)
    .bind(teacher_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(AppError::from)
}

async fn load_instructors(
    transaction: &mut Transaction<'_, Postgres>,
    entry_ids: &[Uuid],
    lock_for_apply: bool,
) -> Result<HashMap<Uuid, Vec<TeacherHandoffInstructorPreview>>, AppError> {
    if entry_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let lock = if lock_for_apply {
        "FOR UPDATE OF instructor"
    } else {
        "FOR SHARE OF instructor"
    };
    let rows: Vec<EntryInstructorRow> = sqlx::query_as(&format!(
        r#"SELECT instructor.block_group_id AS entry_id, instructor.instructor_id,
                  coalesce(
                      nullif(concat_ws(' ',
                          nullif(concat(coalesce(user_account.title, ''), user_account.first_name), ''),
                          nullif(user_account.last_name, '')
                      ), ''),
                      user_account.username,
                      user_account.email
                  ) AS display_name,
                  instructor.role::text AS role
           FROM academic_timetable_block_group_instructors instructor
           JOIN users user_account ON user_account.id = instructor.instructor_id
           WHERE instructor.block_group_id = ANY($1)
           ORDER BY instructor.block_group_id,
                    CASE instructor.role WHEN 'primary' THEN 1 ELSE 2 END,
                    instructor.display_order,
                    instructor.instructor_id
           {lock}"#,
    ))
    .bind(entry_ids)
    .fetch_all(&mut **transaction)
    .await?;
    let mut grouped = HashMap::<Uuid, Vec<TeacherHandoffInstructorPreview>>::new();
    for row in rows {
        grouped
            .entry(row.entry_id)
            .or_default()
            .push(TeacherHandoffInstructorPreview {
                instructor_id: row.instructor_id,
                display_name: row.display_name,
                role: row.role,
            });
    }
    Ok(grouped)
}

async fn load_staff_names(
    transaction: &mut Transaction<'_, Postgres>,
    teacher_ids: &[Uuid],
) -> Result<HashMap<Uuid, String>, AppError> {
    if teacher_ids.is_empty() {
        return Ok(HashMap::new());
    }
    Ok(sqlx::query_as::<_, (Uuid, String)>(
        r#"SELECT id,
                  coalesce(
                      nullif(concat_ws(' ',
                          nullif(concat(coalesce(title, ''), first_name), ''),
                          nullif(last_name, '')
                      ), ''),
                      username,
                      email
                  )
           FROM users WHERE id = ANY($1)"#,
    )
    .bind(teacher_ids)
    .fetch_all(&mut **transaction)
    .await?
    .into_iter()
    .collect())
}

fn proposed_instructors(
    before: &[TeacherHandoffInstructorPreview],
    stopped_teacher_id: Uuid,
    replacements: &[Uuid],
    display_names: &HashMap<Uuid, String>,
) -> Result<Vec<TeacherHandoffInstructorPreview>, AppError> {
    let removed = before
        .iter()
        .find(|instructor| instructor.instructor_id == stopped_teacher_id)
        .ok_or_else(|| AppError::Conflict("คาบที่เลือกไม่มีครูที่จะหยุดแล้ว".to_string()))?;
    let mut result = before
        .iter()
        .filter(|instructor| instructor.instructor_id != stopped_teacher_id)
        .cloned()
        .collect::<Vec<_>>();
    let needs_primary =
        removed.role == "primary" && !result.iter().any(|instructor| instructor.role == "primary");
    for (index, teacher_id) in replacements.iter().enumerate() {
        if result
            .iter()
            .any(|instructor| instructor.instructor_id == *teacher_id)
        {
            continue;
        }
        result.push(TeacherHandoffInstructorPreview {
            instructor_id: *teacher_id,
            display_name: display_names.get(teacher_id).cloned().unwrap_or_default(),
            role: if needs_primary && index == 0 {
                "primary".to_string()
            } else {
                "secondary".to_string()
            },
        });
    }
    result.sort_by_key(|instructor| {
        (
            if instructor.role == "primary" { 0 } else { 1 },
            instructor.instructor_id,
        )
    });
    Ok(result)
}

async fn append_instructor_collisions(
    transaction: &mut Transaction<'_, Postgres>,
    version_id: Uuid,
    proposed_entries: &[TeacherHandoffEntryPreview],
    selected_ids: &BTreeSet<Uuid>,
    route: &str,
    conflicts: &mut Vec<TeacherHandoffConflict>,
) -> Result<(), AppError> {
    let proposed_ids = proposed_entries
        .iter()
        .flat_map(|entry| {
            entry
                .after_instructors
                .iter()
                .map(|instructor| instructor.instructor_id)
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if proposed_ids.is_empty() {
        return Ok(());
    }
    let occupied: Vec<OccupiedInstructorRow> = sqlx::query_as(
        r#"SELECT block_group.id AS entry_id, instructor.instructor_id,
                  block.day_of_week, block.bell_schedule_period_id
           FROM academic_timetable_block_groups block_group
           JOIN academic_timetable_blocks block ON block.id = block_group.block_id
           JOIN academic_timetable_block_group_instructors instructor
             ON instructor.block_group_id = block_group.id
           WHERE block.timetable_version_id = $1
             AND block.is_active
             AND block_group.is_active
             AND instructor.instructor_id = ANY($2)
           ORDER BY block_group.id, instructor.instructor_id"#,
    )
    .bind(version_id)
    .bind(&proposed_ids)
    .fetch_all(&mut **transaction)
    .await?;
    for proposed in proposed_entries {
        for instructor in &proposed.after_instructors {
            let mut collided_entry_ids = occupied
                .iter()
                .filter(|occupied| {
                    occupied.instructor_id == instructor.instructor_id
                        && occupied.day_of_week == proposed.day_of_week
                        && occupied.bell_schedule_period_id == proposed.bell_schedule_period_id
                        && occupied.entry_id != proposed.entry_id
                        && !selected_ids.contains(&occupied.entry_id)
                })
                .map(|occupied| occupied.entry_id)
                .collect::<Vec<_>>();
            collided_entry_ids.extend(
                proposed_entries
                    .iter()
                    .filter(|other| {
                        other.entry_id != proposed.entry_id
                            && other.day_of_week == proposed.day_of_week
                            && other.bell_schedule_period_id == proposed.bell_schedule_period_id
                            && other
                                .after_instructors
                                .iter()
                                .any(|value| value.instructor_id == instructor.instructor_id)
                    })
                    .map(|other| other.entry_id),
            );
            collided_entry_ids.sort_unstable();
            collided_entry_ids.dedup();
            if !collided_entry_ids.is_empty() {
                let mut entry_ids = vec![proposed.entry_id];
                entry_ids.extend(collided_entry_ids);
                entry_ids.sort_unstable();
                entry_ids.dedup();
                conflicts.push(TeacherHandoffConflict {
                    kind: TeacherHandoffConflictKind::InstructorCollision,
                    message: format!("{} มีคาบอื่นในวันและคาบเดียวกัน", instructor.display_name),
                    entry_ids,
                    instructor_ids: vec![instructor.instructor_id],
                    timetable_route: route.to_string(),
                });
            }
        }
    }
    conflicts.dedup();
    Ok(())
}

fn entry_preview(
    entry: &HandoffEntryRow,
    before_instructors: Vec<TeacherHandoffInstructorPreview>,
    after_instructors: Vec<TeacherHandoffInstructorPreview>,
) -> TeacherHandoffEntryPreview {
    TeacherHandoffEntryPreview {
        entry_id: entry.id,
        row_version: entry.row_version,
        learning_group_id: entry.learning_group_id,
        learning_group_label: entry.learning_group_label.clone(),
        offering_label: entry.offering_label.clone(),
        day_of_week: entry.day_of_week.clone(),
        bell_schedule_period_id: entry.bell_schedule_period_id,
        period_label: entry.period_label.clone(),
        room_label: entry.room_label.clone(),
        before_instructors,
        after_instructors,
    }
}

fn validate_mode(mode: TeacherHandoffMode, instructor_ids: &[Uuid]) -> Result<(), AppError> {
    match mode {
        TeacherHandoffMode::AssignOne if instructor_ids.len() != 1 => Err(
            AppError::ValidationError("โหมดครูคนเดียวต้องเลือกครูหนึ่งคน".to_string()),
        ),
        TeacherHandoffMode::AssignCoteachers if instructor_ids.is_empty() => Err(
            AppError::ValidationError("โหมดครูร่วมต้องเลือกครูอย่างน้อยหนึ่งคน".to_string()),
        ),
        TeacherHandoffMode::Manual if !instructor_ids.is_empty() => Err(AppError::ValidationError(
            "โหมดจัดเองต้องไม่ส่งรายชื่อครู".to_string(),
        )),
        _ => Ok(()),
    }
}

fn canonical_ids(values: &[Uuid]) -> Vec<Uuid> {
    values
        .iter()
        .copied()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn canonical_entry_versions(
    values: &[TeacherHandoffEntryVersion],
) -> Result<Vec<(Uuid, i64)>, AppError> {
    let mut entries = BTreeMap::new();
    for value in values {
        validate_row_version(value.row_version)?;
        if entries.insert(value.entry_id, value.row_version).is_some() {
            return Err(AppError::ValidationError(
                "รายการคาบที่ส่งมอบมีข้อมูลซ้ำ".to_string(),
            ));
        }
    }
    Ok(entries.into_iter().collect())
}

fn timetable_route(
    academic_year_id: Uuid,
    academic_term_id: Uuid,
    timetable_version_id: Uuid,
    learning_group_id: Uuid,
) -> String {
    format!(
        "/staff/academic/timetable?academicYearId={academic_year_id}&academicTermId={academic_term_id}&timetableVersionId={timetable_version_id}&view=group&ownerId={learning_group_id}"
    )
}
