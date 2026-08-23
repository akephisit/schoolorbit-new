use sqlx::{types::Json, FromRow, PgPool};
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::academic::models::timetable::{
    ApplyTemplateRequest, ClearTimetableRequest, CreateTemplateRequest, FromCurrentRequest,
    TemplateApplyResult, TemplateWithEntries, TimetableTemplate, TimetableTemplateEntry,
    TimetableTemplateTargetSelector, UpdateTemplateRequest,
};
use crate::modules::academic::services::timetable_service;

#[derive(Debug, FromRow)]
struct TemplateRow {
    id: Uuid,
    name: String,
    description: Option<String>,
    created_by: Option<Uuid>,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, FromRow)]
struct TemplateEntryRow {
    id: Uuid,
    template_id: Uuid,
    day_of_week: String,
    bell_period_order_index: i32,
    entry_type: String,
    title: Option<String>,
    resource_kind: String,
    stable_resource_id: Option<Uuid>,
    learning_group_code: Option<String>,
    target_selector: Json<TimetableTemplateTargetSelector>,
    instructor_ids: Json<Vec<Uuid>>,
    room_id: Option<Uuid>,
}

impl From<TemplateRow> for TimetableTemplate {
    fn from(row: TemplateRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
            description: row.description,
            created_by: row.created_by,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

impl From<TemplateEntryRow> for TimetableTemplateEntry {
    fn from(row: TemplateEntryRow) -> Self {
        Self {
            id: row.id,
            template_id: row.template_id,
            day_of_week: row.day_of_week,
            bell_period_order_index: row.bell_period_order_index,
            entry_type: row.entry_type.to_ascii_lowercase(),
            title: row.title,
            resource_kind: row.resource_kind,
            stable_resource_id: row.stable_resource_id,
            learning_group_code: row.learning_group_code,
            target_selector: row.target_selector.0,
            instructor_ids: row.instructor_ids.0,
            room_id: row.room_id,
        }
    }
}

pub async fn list_templates(pool: &PgPool) -> Result<Vec<TimetableTemplate>, AppError> {
    let rows: Vec<TemplateRow> = sqlx::query_as(
        r#"SELECT id, name, description, created_by, created_at, updated_at
           FROM timetable_templates
           ORDER BY updated_at DESC, id"#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(Into::into).collect())
}

pub async fn get_template(pool: &PgPool, id: Uuid) -> Result<TemplateWithEntries, AppError> {
    let template: TemplateRow = sqlx::query_as(
        r#"SELECT id, name, description, created_by, created_at, updated_at
           FROM timetable_templates WHERE id = $1"#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบแม่แบบตารางสอน".to_string()))?;
    let rows: Vec<TemplateEntryRow> = sqlx::query_as(
        r#"SELECT id, template_id, day_of_week, bell_period_order_index,
                  entry_type, title, resource_kind, stable_resource_id,
                  learning_group_code, target_selector, instructor_ids, room_id
           FROM timetable_template_entries
           WHERE template_id = $1
           ORDER BY day_of_week, bell_period_order_index, id"#,
    )
    .bind(id)
    .fetch_all(pool)
    .await?;
    let entries = rows.into_iter().map(Into::into).collect();
    Ok(TemplateWithEntries {
        template: template.into(),
        entries,
    })
}

pub async fn create_template(
    pool: &PgPool,
    actor_user_id: Uuid,
    request: CreateTemplateRequest,
) -> Result<TimetableTemplate, AppError> {
    let name = require_name(&request.name)?;
    let row: TemplateRow = sqlx::query_as(
        r#"INSERT INTO timetable_templates (id, name, description, created_by)
           VALUES ($1, $2, $3, $4)
           RETURNING id, name, description, created_by, created_at, updated_at"#,
    )
    .bind(Uuid::new_v4())
    .bind(name)
    .bind(request.description.as_deref())
    .bind(actor_user_id)
    .fetch_one(pool)
    .await?;
    Ok(row.into())
}

pub async fn update_template(
    pool: &PgPool,
    id: Uuid,
    request: UpdateTemplateRequest,
) -> Result<TimetableTemplate, AppError> {
    let name = request.name.as_deref().map(require_name).transpose()?;
    let row: TemplateRow = sqlx::query_as(
        r#"UPDATE timetable_templates
           SET name = coalesce($2, name),
               description = coalesce($3, description),
               updated_at = now()
           WHERE id = $1
           RETURNING id, name, description, created_by, created_at, updated_at"#,
    )
    .bind(id)
    .bind(name)
    .bind(request.description.as_deref())
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบแม่แบบตารางสอน".to_string()))?;
    Ok(row.into())
}

pub async fn delete_template(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    let result = sqlx::query("DELETE FROM timetable_templates WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        Err(AppError::NotFound("ไม่พบแม่แบบตารางสอน".to_string()))
    } else {
        Ok(())
    }
}

pub async fn from_current(
    pool: &PgPool,
    actor_user_id: Uuid,
    request: FromCurrentRequest,
) -> Result<TemplateWithEntries, AppError> {
    let name = require_name(&request.name)?;
    let entry_types = canonical_entry_types(request.entry_types)?;
    let mut transaction = pool.begin().await?;
    let term_exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM academic_terms WHERE id = $1)")
            .bind(request.academic_term_id)
            .fetch_one(&mut *transaction)
            .await?;
    if !term_exists {
        return Err(AppError::NotFound("ไม่พบภาคเรียน".to_string()));
    }
    let template_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO timetable_templates (id, name, description, created_by)
           VALUES ($1, $2, $3, $4)"#,
    )
    .bind(template_id)
    .bind(name)
    .bind(request.description.as_deref())
    .bind(actor_user_id)
    .execute(&mut *transaction)
    .await?;
    sqlx::query(
        r#"INSERT INTO timetable_template_entries (
               id, template_id, day_of_week, entry_type, title, instructor_ids,
               room_id, bell_period_order_index, resource_kind, stable_resource_id,
               learning_group_code, target_selector, migration_provenance
           )
           SELECT gen_random_uuid(), $2, entry.day_of_week, entry.entry_type, entry.title,
                  CASE WHEN entry.learning_group_id IS NULL THEN coalesce((
                      SELECT jsonb_agg(instructor.instructor_id ORDER BY instructor.instructor_id)
                      FROM timetable_entry_instructors instructor
                      WHERE instructor.entry_id = entry.id
                  ), '[]'::jsonb) ELSE '[]'::jsonb END,
                  entry.room_id, period.order_index,
                  CASE
                      WHEN entry.learning_group_id IS NULL THEN 'structural'
                      ELSE offering.kind::text
                  END,
                  coalesce(course_detail.subject_id, activity_detail.activity_id),
                  learning_group.code,
                  CASE
                      WHEN entry.learning_group_id IS NOT NULL THEN '{}'::jsonb
                      WHEN homeroom.id IS NOT NULL THEN jsonb_build_object(
                          'gradeLevelId', homeroom.grade_level_id,
                          'studyProgramId', homeroom.study_program_id,
                          'roomNumber', homeroom.room_number
                      )
                      ELSE jsonb_build_object('instructorOnly', true)
                  END,
                  jsonb_build_object('sourceAcademicTermId', entry.academic_term_id,
                                     'sourceEntryId', entry.id)
           FROM academic_timetable_entries entry
           JOIN bell_schedule_periods period ON period.id = entry.bell_schedule_period_id
           LEFT JOIN learning_groups learning_group ON learning_group.id = entry.learning_group_id
           LEFT JOIN learning_offerings offering ON offering.id = entry.learning_offering_id
           LEFT JOIN course_offering_details course_detail
             ON course_detail.learning_offering_id = offering.id
           LEFT JOIN activity_offering_details activity_detail
             ON activity_detail.learning_offering_id = offering.id
           LEFT JOIN homerooms homeroom ON homeroom.id = entry.homeroom_id
           WHERE entry.academic_term_id = $1
             AND entry.is_active
             AND entry.entry_type = ANY($3)"#,
    )
    .bind(request.academic_term_id)
    .bind(template_id)
    .bind(&entry_types)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    get_template(pool, template_id).await
}

pub async fn apply_template(
    pool: &PgPool,
    actor_user_id: Uuid,
    template_id: Uuid,
    request: ApplyTemplateRequest,
) -> Result<TemplateApplyResult, AppError> {
    let template = get_template(pool, template_id).await?;
    let batch_id = Uuid::new_v4();
    let mut transaction = pool.begin().await?;
    let mut entry_ids = Vec::with_capacity(template.entries.len());
    for entry in template.entries {
        let period_id: Uuid = sqlx::query_scalar(
            r#"SELECT period.id
               FROM academic_terms term
               JOIN bell_schedule_periods period
                 ON period.bell_schedule_id = term.bell_schedule_id
               WHERE term.id = $1
                 AND period.order_index = $2
                 AND period.is_active
               ORDER BY period.id
               LIMIT 1"#,
        )
        .bind(request.academic_term_id)
        .bind(entry.bell_period_order_index)
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or_else(|| {
            AppError::ValidationError(format!(
                "ไม่พบคาบลำดับ {} ใน bell schedule เป้าหมาย",
                entry.bell_period_order_index
            ))
        })?;
        let learning_group_id = resolve_target_group(
            &mut transaction,
            request.academic_term_id,
            &entry.resource_kind,
            entry.stable_resource_id,
            entry.learning_group_code.as_deref(),
        )
        .await?;
        let homeroom_id = if learning_group_id.is_none() {
            resolve_target_homeroom(
                &mut transaction,
                request.academic_term_id,
                &entry.target_selector,
            )
            .await?
        } else {
            None
        };
        let create_request =
            crate::modules::academic::models::timetable::CreateTimetableEntryRequest {
                academic_term_id: request.academic_term_id,
                learning_group_id,
                homeroom_id,
                day_of_week: entry.day_of_week,
                bell_schedule_period_id: period_id,
                room_id: entry.room_id,
                note: None,
                entry_type: entry.entry_type,
                title: entry.title,
                instructor_ids: entry.instructor_ids,
            };
        entry_ids.push(
            timetable_service::create_entry_in_tx(
                &mut transaction,
                actor_user_id,
                Some(batch_id),
                &create_request,
            )
            .await?,
        );
    }
    transaction.commit().await?;
    Ok(TemplateApplyResult {
        applied: entry_ids.len(),
        entry_ids,
    })
}

pub async fn clear_timetable(
    pool: &PgPool,
    actor_user_id: Uuid,
    request: ClearTimetableRequest,
) -> Result<Vec<crate::modules::academic::models::timetable::TimetableEntry>, AppError> {
    let entry_types = canonical_entry_types(request.entry_types)?;
    let mut transaction = pool.begin().await?;
    let term_status: String =
        sqlx::query_scalar("SELECT status FROM academic_terms WHERE id = $1 FOR SHARE")
            .bind(request.academic_term_id)
            .fetch_optional(&mut *transaction)
            .await?
            .ok_or_else(|| AppError::NotFound("ไม่พบภาคเรียน".to_string()))?;
    if matches!(term_status.as_str(), "closed" | "archived") {
        return Err(AppError::Conflict(
            "ภาคเรียนนี้ปิดแล้ว ไม่สามารถล้างตารางสอนได้".to_string(),
        ));
    }
    let ids: Vec<Uuid> = sqlx::query_scalar(
        r#"SELECT id FROM academic_timetable_entries
           WHERE academic_term_id = $1 AND entry_type = ANY($2) AND is_active
           ORDER BY id FOR UPDATE"#,
    )
    .bind(request.academic_term_id)
    .bind(&entry_types)
    .fetch_all(&mut *transaction)
    .await?;
    sqlx::query(
        r#"UPDATE academic_timetable_entries
           SET is_active = false, updated_by = $3,
               row_version = row_version + 1, updated_at = now()
           WHERE academic_term_id = $1 AND entry_type = ANY($2) AND is_active"#,
    )
    .bind(request.academic_term_id)
    .bind(&entry_types)
    .bind(actor_user_id)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    let mut entries = Vec::with_capacity(ids.len());
    for id in ids {
        entries.push(timetable_service::get_entry(pool, id).await?);
    }
    Ok(entries)
}

async fn resolve_target_group(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    academic_term_id: Uuid,
    resource_kind: &str,
    stable_resource_id: Option<Uuid>,
    group_code: Option<&str>,
) -> Result<Option<Uuid>, AppError> {
    if resource_kind == "structural" {
        return Ok(None);
    }
    let stable_resource_id = stable_resource_id.ok_or_else(|| {
        AppError::ValidationError("แม่แบบขาด stable resource identity".to_string())
    })?;
    let group_code =
        group_code.ok_or_else(|| AppError::ValidationError("แม่แบบขาดรหัสกลุ่มเรียน".to_string()))?;
    let group_id: Option<Uuid> = match resource_kind {
        "course" => {
            sqlx::query_scalar(
                r#"SELECT learning_group.id
                   FROM learning_groups learning_group
                   JOIN course_offering_details detail
                     ON detail.learning_offering_id = learning_group.learning_offering_id
                   WHERE learning_group.academic_term_id = $1
                     AND detail.subject_id = $2
                     AND learning_group.code = $3
                   ORDER BY learning_group.id LIMIT 1"#,
            )
            .bind(academic_term_id)
            .bind(stable_resource_id)
            .bind(group_code)
            .fetch_optional(&mut **transaction)
            .await?
        }
        "activity" => {
            sqlx::query_scalar(
                r#"SELECT learning_group.id
                   FROM learning_groups learning_group
                   JOIN activity_offering_details detail
                     ON detail.learning_offering_id = learning_group.learning_offering_id
                   WHERE learning_group.academic_term_id = $1
                     AND detail.activity_id = $2
                     AND learning_group.code = $3
                   ORDER BY learning_group.id LIMIT 1"#,
            )
            .bind(academic_term_id)
            .bind(stable_resource_id)
            .bind(group_code)
            .fetch_optional(&mut **transaction)
            .await?
        }
        _ => {
            return Err(AppError::ValidationError(
                "ชนิด resource ในแม่แบบไม่ถูกต้อง".to_string(),
            ));
        }
    };
    group_id.map(Some).ok_or_else(|| {
        AppError::ValidationError(format!(
            "ไม่พบกลุ่มเรียนรหัส {group_code} ที่ตรงกับแม่แบบในภาคเรียนเป้าหมาย"
        ))
    })
}

async fn resolve_target_homeroom(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    academic_term_id: Uuid,
    selector: &TimetableTemplateTargetSelector,
) -> Result<Option<Uuid>, AppError> {
    if selector.instructor_only {
        return Ok(None);
    }
    let grade_level_id = selector
        .grade_level_id
        .ok_or_else(|| AppError::ValidationError("แม่แบบขาด gradeLevelId".to_string()))?;
    let study_program_id = selector
        .study_program_id
        .ok_or_else(|| AppError::ValidationError("แม่แบบขาด studyProgramId".to_string()))?;
    let room_number = selector
        .room_number
        .as_deref()
        .ok_or_else(|| AppError::ValidationError("แม่แบบขาด roomNumber".to_string()))?;
    sqlx::query_scalar(
        r#"SELECT homeroom.id
           FROM homerooms homeroom
           JOIN academic_terms term ON term.academic_year_id = homeroom.academic_year_id
           WHERE term.id = $1
             AND homeroom.grade_level_id = $2
             AND homeroom.study_program_id = $3
             AND homeroom.room_number = $4
           ORDER BY homeroom.id LIMIT 1"#,
    )
    .bind(academic_term_id)
    .bind(grade_level_id)
    .bind(study_program_id)
    .bind(room_number)
    .fetch_optional(&mut **transaction)
    .await?
    .map(Some)
    .ok_or_else(|| {
        AppError::ValidationError("ไม่พบห้องประจำชั้นที่ตรงกับแม่แบบในปีการศึกษาเป้าหมาย".to_string())
    })
}

fn canonical_entry_types(entry_types: Option<Vec<String>>) -> Result<Vec<String>, AppError> {
    let values = entry_types.unwrap_or_else(|| {
        vec![
            "course".to_string(),
            "activity".to_string(),
            "break".to_string(),
            "homeroom".to_string(),
            "academic".to_string(),
        ]
    });
    let mut normalized = Vec::new();
    for value in values {
        let value = value.trim().to_ascii_uppercase();
        if !["COURSE", "ACTIVITY", "BREAK", "HOMEROOM", "ACADEMIC"].contains(&value.as_str()) {
            return Err(AppError::ValidationError(
                "ชนิดรายการในแม่แบบไม่ถูกต้อง".to_string(),
            ));
        }
        if !normalized.contains(&value) {
            normalized.push(value);
        }
    }
    Ok(normalized)
}

fn require_name(name: &str) -> Result<&str, AppError> {
    let name = name.trim();
    if name.is_empty() {
        Err(AppError::ValidationError("ต้องระบุชื่อแม่แบบ".to_string()))
    } else {
        Ok(name)
    }
}

#[cfg(test)]
mod tests {
    use super::canonical_entry_types;

    #[test]
    fn template_entry_types_use_canonical_values() {
        assert_eq!(
            canonical_entry_types(Some(vec!["course".to_string(), "COURSE".to_string()])).unwrap(),
            vec!["COURSE"]
        );
    }
}
