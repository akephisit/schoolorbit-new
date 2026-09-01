use sqlx::{types::Json, FromRow, PgPool};
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::academic::models::timetable::{
    ApplyTemplateRequest, ClearTimetableRequest, CreateTemplateRequest, FromCurrentRequest,
    TemplateApplyResult, TemplateWithEntries, TimetableTemplate, TimetableTemplateEntry,
    TimetableTemplateTargetSelector, UpdateTemplateRequest,
};

use super::{timetable_block_conflicts::map_write_error, timetable_block_queries};

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
    let version_exists: bool = sqlx::query_scalar(
        r#"SELECT EXISTS(
               SELECT 1 FROM academic_timetable_versions
               WHERE id = $1 AND academic_term_id = $2 AND status <> 'cancelled'
           )"#,
    )
    .bind(request.timetable_version_id)
    .bind(request.academic_term_id)
    .fetch_one(&mut *transaction)
    .await?;
    if !version_exists {
        return Err(AppError::NotFound(
            "ไม่พบรุ่นตารางสอนในภาคเรียนที่ระบุ".to_string(),
        ));
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
           SELECT gen_random_uuid(), $2, block.day_of_week,
                  CASE WHEN block.block_kind = 'STRUCTURAL'
                       THEN block.structural_kind ELSE block.block_kind END,
                  block.title,
                  coalesce((
                      SELECT jsonb_agg(
                          instructor.instructor_id
                          ORDER BY instructor.display_order, instructor.instructor_id
                      )
                      FROM academic_timetable_block_group_instructors instructor
                      WHERE instructor.block_group_id = block_group.id
                  ), (
                      SELECT jsonb_agg(target.teacher_id ORDER BY target.teacher_id)
                      FROM academic_timetable_block_teachers target
                      WHERE target.block_id = block.id AND target.is_active
                  ), '[]'::jsonb),
                  coalesce(block_group.room_id, homeroom_target.room_id), period.order_index,
                  CASE
                      WHEN block_group.learning_group_id IS NULL THEN 'structural'
                      ELSE offering.kind::text
                  END,
                  coalesce(course_detail.subject_id, activity_detail.activity_id),
                  learning_group.code,
                  CASE
                      WHEN block_group.learning_group_id IS NOT NULL THEN '{}'::jsonb
                      WHEN homeroom.id IS NOT NULL THEN jsonb_build_object(
                          'gradeLevelId', homeroom.grade_level_id,
                          'studyProgramId', homeroom.study_program_id,
                          'roomNumber', homeroom.room_number
                      )
                      ELSE jsonb_build_object('instructorOnly', true)
                  END,
                  jsonb_build_object('sourceAcademicTermId', block.academic_term_id,
                                     'sourceBlockId', block.id)
           FROM academic_timetable_blocks block
           JOIN bell_schedule_periods period ON period.id = block.bell_schedule_period_id
           LEFT JOIN academic_timetable_block_groups block_group
             ON block_group.block_id = block.id AND block_group.is_active
           LEFT JOIN academic_timetable_block_homerooms homeroom_target
             ON homeroom_target.block_id = block.id AND homeroom_target.is_active
           LEFT JOIN learning_groups learning_group
             ON learning_group.id = block_group.learning_group_id
           LEFT JOIN learning_offerings offering ON offering.id = block.learning_offering_id
           LEFT JOIN course_offering_details course_detail
             ON course_detail.learning_offering_id = offering.id
           LEFT JOIN activity_offering_details activity_detail
             ON activity_detail.learning_offering_id = offering.id
           LEFT JOIN homerooms homeroom ON homeroom.id = homeroom_target.homeroom_id
           WHERE block.academic_term_id = $1
             AND block.timetable_version_id = $4
             AND block.is_active
             AND block.scheduling_mode IS DISTINCT FROM 'synchronized'
             AND CASE WHEN block.block_kind = 'STRUCTURAL'
                      THEN block.structural_kind ELSE block.block_kind END = ANY($3)"#,
    )
    .bind(request.academic_term_id)
    .bind(template_id)
    .bind(&entry_types)
    .bind(request.timetable_version_id)
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
    let series_id = Uuid::new_v4();
    let mut transaction = pool.begin().await?;
    let (academic_year_id, bell_schedule_id, version_effective_from): (
        Uuid,
        Uuid,
        chrono::NaiveDate,
    ) = sqlx::query_as(
        r#"SELECT version.academic_year_id, version.bell_schedule_id, version.effective_from
           FROM academic_timetable_versions version
           JOIN academic_terms term ON term.id = version.academic_term_id
           WHERE version.id = $1
             AND version.academic_term_id = $2
             AND version.status = 'draft'
             AND term.status <> 'closed'
           FOR UPDATE OF version"#,
    )
    .bind(request.timetable_version_id)
    .bind(request.academic_term_id)
    .fetch_optional(&mut *transaction)
    .await?
    .ok_or_else(|| AppError::Conflict("เพิ่มแม่แบบได้เฉพาะรุ่นตารางฉบับร่างในภาคเรียนที่เปิดอยู่".to_string()))?;
    let mut block_ids = Vec::with_capacity(template.entries.len());
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
        let instructor_ids = match learning_group_id {
            Some(group_id) => {
                eligible_template_group_instructors(
                    &mut transaction,
                    group_id,
                    version_effective_from,
                    &entry.instructor_ids,
                )
                .await?
            }
            None => entry.instructor_ids,
        };
        let block_id = Uuid::new_v4();
        if let Some(group_id) = learning_group_id {
            if instructor_ids.is_empty() {
                return Err(AppError::ValidationError(format!(
                    "แม่แบบคาบ {} ไม่มีครูที่ยังอยู่ในทีมสอนของกลุ่มเป้าหมาย",
                    entry.bell_period_order_index
                )));
            }
            let (offering_id, offering_kind, scheduling_mode): (Uuid, String, Option<String>) =
                sqlx::query_as(
                    r#"SELECT learning_group.learning_offering_id,
                              offering.kind::text,
                              activity_detail.scheduling_mode::text
                       FROM learning_groups learning_group
                       JOIN learning_offerings offering
                         ON offering.id = learning_group.learning_offering_id
                       LEFT JOIN activity_offering_details activity_detail
                         ON activity_detail.learning_offering_id = offering.id
                       WHERE learning_group.id = $1
                         AND learning_group.academic_term_id = $2"#,
                )
                .bind(group_id)
                .bind(request.academic_term_id)
                .fetch_one(&mut *transaction)
                .await?;
            if scheduling_mode.as_deref() == Some("synchronized") {
                return Err(AppError::ValidationError(
                    "กิจกรรมแบบพร้อมกันต้องสร้างจากช่วงกิจกรรมหลัก ไม่รองรับการแตกเป็นรายกลุ่มจากแม่แบบ"
                        .to_string(),
                ));
            }
            sqlx::query(
                r#"INSERT INTO academic_timetable_blocks (
                       id, timetable_version_id, academic_term_id, academic_year_id,
                       bell_schedule_id, bell_schedule_period_id, day_of_week,
                       block_kind, scheduling_mode, learning_offering_id,
                       note, series_id, created_by, updated_by
                   ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'independent', $9,
                             NULL, $10, $11, $11)"#,
            )
            .bind(block_id)
            .bind(request.timetable_version_id)
            .bind(request.academic_term_id)
            .bind(academic_year_id)
            .bind(bell_schedule_id)
            .bind(period_id)
            .bind(&entry.day_of_week)
            .bind(if offering_kind == "course" {
                "COURSE"
            } else {
                "ACTIVITY"
            })
            .bind(offering_id)
            .bind(series_id)
            .bind(actor_user_id)
            .execute(&mut *transaction)
            .await
            .map_err(map_write_error)?;
            let block_group_id = Uuid::new_v4();
            sqlx::query(
                r#"INSERT INTO academic_timetable_block_groups (
                       id, block_id, learning_group_id, learning_offering_id,
                       academic_term_id, academic_year_id, room_id, created_by, updated_by
                   ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $8)"#,
            )
            .bind(block_group_id)
            .bind(block_id)
            .bind(group_id)
            .bind(offering_id)
            .bind(request.academic_term_id)
            .bind(academic_year_id)
            .bind(entry.room_id)
            .bind(actor_user_id)
            .execute(&mut *transaction)
            .await
            .map_err(map_write_error)?;
            for (index, instructor_id) in instructor_ids.iter().enumerate() {
                let role: String = sqlx::query_scalar(
                    r#"SELECT assignment.role::text
                       FROM learning_group_teachers assignment
                       WHERE assignment.learning_group_id = $1
                         AND assignment.teacher_id = $2
                         AND assignment.starts_on <= $3
                         AND (assignment.ends_on IS NULL OR assignment.ends_on >= $3)
                       ORDER BY CASE assignment.role WHEN 'primary' THEN 1 ELSE 2 END
                       LIMIT 1"#,
                )
                .bind(group_id)
                .bind(instructor_id)
                .bind(version_effective_from)
                .fetch_one(&mut *transaction)
                .await?;
                sqlx::query(
                    r#"INSERT INTO academic_timetable_block_group_instructors (
                           id, block_group_id, instructor_id, role, display_order
                       ) VALUES (gen_random_uuid(), $1, $2, $3, $4)"#,
                )
                .bind(block_group_id)
                .bind(instructor_id)
                .bind(role)
                .bind((index + 1) as i32)
                .execute(&mut *transaction)
                .await
                .map_err(map_write_error)?;
            }
        } else {
            let structural_kind = match entry.entry_type.as_str() {
                "break" => "BREAK",
                "homeroom" => "HOMEROOM",
                "academic" => "ACADEMIC",
                _ => "OTHER",
            };
            sqlx::query(
                r#"INSERT INTO academic_timetable_blocks (
                       id, timetable_version_id, academic_term_id, academic_year_id,
                       bell_schedule_id, bell_schedule_period_id, day_of_week,
                       block_kind, structural_kind, title, series_id, created_by, updated_by
                   ) VALUES ($1, $2, $3, $4, $5, $6, $7,
                             'STRUCTURAL', $8, $9, $10, $11, $11)"#,
            )
            .bind(block_id)
            .bind(request.timetable_version_id)
            .bind(request.academic_term_id)
            .bind(academic_year_id)
            .bind(bell_schedule_id)
            .bind(period_id)
            .bind(&entry.day_of_week)
            .bind(structural_kind)
            .bind(entry.title.as_deref().unwrap_or("กิจกรรมโรงเรียน"))
            .bind(series_id)
            .bind(actor_user_id)
            .execute(&mut *transaction)
            .await
            .map_err(map_write_error)?;
            if let Some(homeroom_id) = homeroom_id {
                sqlx::query(
                    r#"INSERT INTO academic_timetable_block_homerooms (
                           id, block_id, homeroom_id, academic_term_id, academic_year_id,
                           target_kind, room_id, created_by, updated_by
                       ) VALUES (gen_random_uuid(), $1, $2, $3, $4,
                                 'structural', $5, $6, $6)"#,
                )
                .bind(block_id)
                .bind(homeroom_id)
                .bind(request.academic_term_id)
                .bind(academic_year_id)
                .bind(entry.room_id)
                .bind(actor_user_id)
                .execute(&mut *transaction)
                .await
                .map_err(map_write_error)?;
            }
            for instructor_id in instructor_ids {
                sqlx::query(
                    r#"INSERT INTO academic_timetable_block_teachers (
                           id, block_id, teacher_id, academic_term_id, academic_year_id,
                           created_by, updated_by
                       ) VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, $5)"#,
                )
                .bind(block_id)
                .bind(instructor_id)
                .bind(request.academic_term_id)
                .bind(academic_year_id)
                .bind(actor_user_id)
                .execute(&mut *transaction)
                .await
                .map_err(map_write_error)?;
            }
        }
        block_ids.push(block_id);
    }
    transaction.commit().await?;
    Ok(TemplateApplyResult {
        applied: block_ids.len(),
        entry_ids: block_ids,
    })
}

async fn eligible_template_group_instructors(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    learning_group_id: Uuid,
    effective_from: chrono::NaiveDate,
    selected_ids: &[Uuid],
) -> Result<Vec<Uuid>, AppError> {
    let mut requested_ids = Vec::new();
    for requested_id in selected_ids.iter().copied() {
        if !requested_ids.contains(&requested_id) {
            requested_ids.push(requested_id);
        }
    }
    if requested_ids.is_empty() {
        return Ok(Vec::new());
    }
    let eligible_ids: Vec<Uuid> = sqlx::query_scalar(
        r#"SELECT DISTINCT assignment.teacher_id
           FROM learning_group_teachers assignment
           JOIN users teacher ON teacher.id = assignment.teacher_id
           WHERE assignment.learning_group_id = $1
             AND assignment.starts_on <= $2
             AND (assignment.ends_on IS NULL OR assignment.ends_on >= $2)
             AND assignment.teacher_id = ANY($3)
             AND teacher.user_type = 'staff'
             AND teacher.status = 'active'
           ORDER BY assignment.teacher_id"#,
    )
    .bind(learning_group_id)
    .bind(effective_from)
    .bind(&requested_ids)
    .fetch_all(&mut **transaction)
    .await?;
    if eligible_ids.len() == requested_ids.len() {
        Ok(requested_ids)
    } else {
        Ok(Vec::new())
    }
}

pub async fn clear_timetable(
    pool: &PgPool,
    actor_user_id: Uuid,
    request: ClearTimetableRequest,
) -> Result<Vec<crate::modules::academic::models::timetable_block::TimetableBlock>, AppError> {
    let entry_types = canonical_entry_types(request.entry_types)?;
    let mut transaction = pool.begin().await?;
    let is_draft: bool = sqlx::query_scalar(
        r#"SELECT EXISTS (
               SELECT 1 FROM academic_timetable_versions
               WHERE id = $1 AND academic_term_id = $2 AND status = 'draft'
           )"#,
    )
    .bind(request.timetable_version_id)
    .bind(request.academic_term_id)
    .fetch_one(&mut *transaction)
    .await?;
    if !is_draft {
        return Err(AppError::Conflict("ล้างได้เฉพาะรุ่นตารางฉบับร่าง".to_string()));
    }
    let ids: Vec<Uuid> = sqlx::query_scalar(
        r#"SELECT id FROM academic_timetable_blocks
           WHERE academic_term_id = $1
             AND timetable_version_id = $3
             AND CASE WHEN block_kind = 'STRUCTURAL'
                      THEN structural_kind ELSE block_kind END = ANY($2)
             AND is_active
           ORDER BY id FOR UPDATE"#,
    )
    .bind(request.academic_term_id)
    .bind(&entry_types)
    .bind(request.timetable_version_id)
    .fetch_all(&mut *transaction)
    .await?;
    sqlx::query(
        r#"UPDATE academic_timetable_blocks
           SET is_active = false, updated_by = $3,
               row_version = row_version + 1, updated_at = now()
           WHERE academic_term_id = $1
             AND timetable_version_id = $4
             AND CASE WHEN block_kind = 'STRUCTURAL'
                      THEN structural_kind ELSE block_kind END = ANY($2)
             AND is_active"#,
    )
    .bind(request.academic_term_id)
    .bind(&entry_types)
    .bind(actor_user_id)
    .bind(request.timetable_version_id)
    .execute(&mut *transaction)
    .await
    .map_err(map_write_error)?;
    transaction.commit().await?;
    timetable_block_queries::get_blocks(pool, &ids).await
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
