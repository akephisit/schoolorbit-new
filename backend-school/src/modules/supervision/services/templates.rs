use std::collections::HashMap;

use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::supervision::models::{
    CreateSupervisionTemplateRequest, CreateSupervisionTemplateSectionRequest,
    CreateSupervisionTemplateStepRequest, SupervisionTemplate, SupervisionTemplateItem,
    SupervisionTemplateItemType, SupervisionTemplateSection, SupervisionTemplateStatus,
    SupervisionTemplateStep, SupervisionTemplateStepActorKind, UpdateSupervisionTemplateRequest,
};

use super::shared::{
    parse_step_action_kind, parse_step_actor_kind, parse_template_item_type, parse_template_status,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TemplateSectionBulkRow {
    pub(super) id: Uuid,
    pub(super) title: String,
    pub(super) description: Option<String>,
    pub(super) sort_order: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TemplateItemBulkRow {
    pub(super) section_id: Uuid,
    pub(super) label: String,
    pub(super) description: Option<String>,
    pub(super) item_type: SupervisionTemplateItemType,
    pub(super) required: bool,
    pub(super) sort_order: i32,
}

#[derive(Debug, sqlx::FromRow)]
struct SupervisionTemplateRow {
    id: Uuid,
    title: String,
    description: Option<String>,
    status: String,
    rating_min: i32,
    rating_max: i32,
    created_by: Option<Uuid>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
struct SupervisionTemplateSectionRow {
    id: Uuid,
    template_id: Uuid,
    title: String,
    description: Option<String>,
    sort_order: i32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
struct SupervisionTemplateItemRow {
    id: Uuid,
    section_id: Uuid,
    label: String,
    description: Option<String>,
    item_type: String,
    required: bool,
    sort_order: i32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
struct SupervisionTemplateStepRow {
    id: Uuid,
    template_id: Uuid,
    step_order: i32,
    step_code: String,
    label: String,
    actor_kind: String,
    actor_permission: Option<String>,
    organization_position_code: Option<String>,
    action_kind: String,
    required: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

pub async fn list_templates(pool: &PgPool) -> Result<Vec<SupervisionTemplate>, AppError> {
    let rows = sqlx::query_as::<_, SupervisionTemplateRow>(
        r#"
        SELECT id, title, description, status, rating_min, rating_max,
               created_by, created_at, updated_at
        FROM supervision_templates
        ORDER BY created_at DESC
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to list supervision templates: {}", error);
        AppError::InternalServerError("ไม่สามารถดึงแบบประเมินนิเทศได้".to_string())
    })?;

    let mut templates = Vec::with_capacity(rows.len());
    for row in rows {
        templates.push(get_template(pool, row.id).await?);
    }
    Ok(templates)
}

pub async fn get_template(pool: &PgPool, id: Uuid) -> Result<SupervisionTemplate, AppError> {
    let template_row = sqlx::query_as::<_, SupervisionTemplateRow>(
        r#"
        SELECT id, title, description, status, rating_min, rating_max,
               created_by, created_at, updated_at
        FROM supervision_templates
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to get supervision template: {}", error);
        AppError::InternalServerError("ไม่สามารถดึงแบบประเมินนิเทศได้".to_string())
    })?
    .ok_or_else(|| AppError::NotFound("ไม่พบแบบประเมินนิเทศ".to_string()))?;

    let section_rows = sqlx::query_as::<_, SupervisionTemplateSectionRow>(
        r#"
        SELECT id, template_id, title, description, sort_order, created_at, updated_at
        FROM supervision_template_sections
        WHERE template_id = $1
        ORDER BY sort_order, created_at
        "#,
    )
    .bind(id)
    .fetch_all(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to get supervision template sections: {}", error);
        AppError::InternalServerError("ไม่สามารถดึงหมวดแบบประเมินนิเทศได้".to_string())
    })?;

    let item_rows = sqlx::query_as::<_, SupervisionTemplateItemRow>(
        r#"
        SELECT i.id, i.section_id, i.label, i.description, i.item_type,
               i.required, i.sort_order, i.created_at, i.updated_at
        FROM supervision_template_items i
        JOIN supervision_template_sections s ON i.section_id = s.id
        WHERE s.template_id = $1
        ORDER BY s.sort_order, i.sort_order, i.created_at
        "#,
    )
    .bind(id)
    .fetch_all(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to get supervision template items: {}", error);
        AppError::InternalServerError("ไม่สามารถดึงหัวข้อแบบประเมินนิเทศได้".to_string())
    })?;

    let step_rows = sqlx::query_as::<_, SupervisionTemplateStepRow>(
        r#"
        SELECT id, template_id, step_order, step_code, label, actor_kind,
               actor_permission, organization_position_code, action_kind,
               required, created_at, updated_at
        FROM supervision_template_steps
        WHERE template_id = $1
        ORDER BY step_order, created_at
        "#,
    )
    .bind(id)
    .fetch_all(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to get supervision template steps: {}", error);
        AppError::InternalServerError("ไม่สามารถดึงขั้นตอนแบบประเมินนิเทศได้".to_string())
    })?;

    template_from_rows(template_row, section_rows, item_rows, step_rows)
}

pub async fn create_template(
    pool: &PgPool,
    input: CreateSupervisionTemplateRequest,
    created_by: Uuid,
) -> Result<SupervisionTemplate, AppError> {
    validate_template_input(
        input.rating_min,
        input.rating_max,
        &input.sections,
        &input.steps,
    )?;

    let mut tx = pool.begin().await.map_err(|error| {
        tracing::error!(
            "Failed to begin create supervision template transaction: {}",
            error
        );
        AppError::InternalServerError("ไม่สามารถเริ่มสร้างแบบประเมินนิเทศได้".to_string())
    })?;

    let status = input.status.unwrap_or(SupervisionTemplateStatus::Draft);
    let template_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO supervision_templates (
            title, description, status, rating_min, rating_max, created_by
        )
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING id
        "#,
    )
    .bind(&input.title)
    .bind(&input.description)
    .bind(status.as_str())
    .bind(input.rating_min)
    .bind(input.rating_max)
    .bind(created_by)
    .fetch_one(&mut *tx)
    .await
    .map_err(|error| {
        tracing::error!("Failed to create supervision template: {}", error);
        AppError::InternalServerError("ไม่สามารถสร้างแบบประเมินนิเทศได้".to_string())
    })?;

    insert_template_sections(&mut tx, template_id, &input.sections).await?;
    insert_template_steps(&mut tx, template_id, &input.steps).await?;

    tx.commit().await.map_err(|error| {
        tracing::error!(
            "Failed to commit create supervision template transaction: {}",
            error
        );
        AppError::InternalServerError("ไม่สามารถบันทึกแบบประเมินนิเทศได้".to_string())
    })?;

    get_template(pool, template_id).await
}

pub async fn update_template(
    pool: &PgPool,
    id: Uuid,
    input: UpdateSupervisionTemplateRequest,
) -> Result<SupervisionTemplate, AppError> {
    let current = get_template(pool, id).await?;
    let title = input.title.unwrap_or(current.title);
    let description = input.description.or(current.description);
    let status = input.status.unwrap_or(current.status);
    let rating_min = input.rating_min.unwrap_or(current.rating_min);
    let rating_max = input.rating_max.unwrap_or(current.rating_max);

    if let Some(sections) = &input.sections {
        validate_template_input(
            rating_min,
            rating_max,
            sections,
            input.steps.as_deref().unwrap_or(&[]),
        )?;
    } else if rating_min >= rating_max {
        return Err(AppError::ValidationError(
            "คะแนนต่ำสุดต้องน้อยกว่าคะแนนสูงสุด".to_string(),
        ));
    }

    let mut tx = pool.begin().await.map_err(|error| {
        tracing::error!(
            "Failed to begin update supervision template transaction: {}",
            error
        );
        AppError::InternalServerError("ไม่สามารถเริ่มแก้ไขแบบประเมินนิเทศได้".to_string())
    })?;

    sqlx::query(
        r#"
        UPDATE supervision_templates
        SET title = $2, description = $3, status = $4,
            rating_min = $5, rating_max = $6
        WHERE id = $1
        "#,
    )
    .bind(id)
    .bind(&title)
    .bind(&description)
    .bind(status.as_str())
    .bind(rating_min)
    .bind(rating_max)
    .execute(&mut *tx)
    .await
    .map_err(|error| {
        tracing::error!("Failed to update supervision template: {}", error);
        AppError::InternalServerError("ไม่สามารถแก้ไขแบบประเมินนิเทศได้".to_string())
    })?;

    if let Some(sections) = input.sections {
        sqlx::query("DELETE FROM supervision_template_sections WHERE template_id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|error| {
                tracing::error!("Failed to clear supervision template sections: {}", error);
                AppError::InternalServerError("ไม่สามารถแก้ไขหมวดแบบประเมินนิเทศได้".to_string())
            })?;
        insert_template_sections(&mut tx, id, &sections).await?;
    }

    if let Some(steps) = input.steps {
        sqlx::query("DELETE FROM supervision_template_steps WHERE template_id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|error| {
                tracing::error!("Failed to clear supervision template steps: {}", error);
                AppError::InternalServerError("ไม่สามารถแก้ไขขั้นตอนแบบประเมินนิเทศได้".to_string())
            })?;
        insert_template_steps(&mut tx, id, &steps).await?;
    }

    tx.commit().await.map_err(|error| {
        tracing::error!(
            "Failed to commit update supervision template transaction: {}",
            error
        );
        AppError::InternalServerError("ไม่สามารถบันทึกแบบประเมินนิเทศได้".to_string())
    })?;

    get_template(pool, id).await
}

fn validate_template_input(
    rating_min: i32,
    rating_max: i32,
    sections: &[CreateSupervisionTemplateSectionRequest],
    steps: &[CreateSupervisionTemplateStepRequest],
) -> Result<(), AppError> {
    if rating_min >= rating_max {
        return Err(AppError::ValidationError(
            "คะแนนต่ำสุดต้องน้อยกว่าคะแนนสูงสุด".to_string(),
        ));
    }

    if sections.is_empty() {
        return Err(AppError::ValidationError(
            "แบบประเมินต้องมีอย่างน้อย 1 หมวด".to_string(),
        ));
    }

    if sections.iter().all(|section| section.items.is_empty()) {
        return Err(AppError::ValidationError(
            "แบบประเมินต้องมีอย่างน้อย 1 หัวข้อ".to_string(),
        ));
    }

    for step in steps {
        match step.actor_kind {
            SupervisionTemplateStepActorKind::Permission if step.actor_permission.is_none() => {
                return Err(AppError::ValidationError(
                    "ขั้นตอนแบบ permission ต้องระบุ actorPermission".to_string(),
                ));
            }
            SupervisionTemplateStepActorKind::OrganizationPosition
                if step.organization_position_code.is_none() =>
            {
                return Err(AppError::ValidationError(
                    "ขั้นตอนแบบ organizationPosition ต้องระบุ organizationPositionCode".to_string(),
                ));
            }
            _ => {}
        }
    }

    Ok(())
}

async fn insert_template_sections(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    template_id: Uuid,
    sections: &[CreateSupervisionTemplateSectionRequest],
) -> Result<(), AppError> {
    let (section_rows, item_rows) = build_template_section_bulk_rows(sections);
    bulk_insert_template_sections(tx, template_id, &section_rows).await?;
    bulk_insert_template_items(tx, &item_rows).await?;

    Ok(())
}

pub(super) fn build_template_section_bulk_rows(
    sections: &[CreateSupervisionTemplateSectionRequest],
) -> (Vec<TemplateSectionBulkRow>, Vec<TemplateItemBulkRow>) {
    let mut section_rows = Vec::with_capacity(sections.len());
    let mut item_rows = Vec::new();

    for section in sections {
        let section_id = Uuid::new_v4();
        section_rows.push(TemplateSectionBulkRow {
            id: section_id,
            title: section.title.clone(),
            description: section.description.clone(),
            sort_order: section.sort_order,
        });

        item_rows.extend(section.items.iter().map(|item| TemplateItemBulkRow {
            section_id,
            label: item.label.clone(),
            description: item.description.clone(),
            item_type: item.item_type,
            required: item.required,
            sort_order: item.sort_order,
        }));
    }

    (section_rows, item_rows)
}

async fn bulk_insert_template_sections(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    template_id: Uuid,
    rows: &[TemplateSectionBulkRow],
) -> Result<(), AppError> {
    if rows.is_empty() {
        return Ok(());
    }

    let mut builder = QueryBuilder::new(
        r#"
        INSERT INTO supervision_template_sections (
            id, template_id, title, description, sort_order
        )
        "#,
    );
    builder.push_values(rows, |mut row_builder, row| {
        row_builder
            .push_bind(row.id)
            .push_bind(template_id)
            .push_bind(&row.title)
            .push_bind(&row.description)
            .push_bind(row.sort_order);
    });

    builder.build().execute(&mut **tx).await.map_err(|error| {
        tracing::error!(
            "Failed to bulk insert supervision template sections: {}",
            error
        );
        AppError::InternalServerError("ไม่สามารถบันทึกหมวดแบบประเมินนิเทศได้".to_string())
    })?;

    Ok(())
}

async fn bulk_insert_template_items(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    rows: &[TemplateItemBulkRow],
) -> Result<(), AppError> {
    if rows.is_empty() {
        return Ok(());
    }

    let mut builder = QueryBuilder::new(
        r#"
        INSERT INTO supervision_template_items (
            section_id, label, description, item_type, required, sort_order
        )
        "#,
    );
    builder.push_values(rows, |mut row_builder, row| {
        row_builder
            .push_bind(row.section_id)
            .push_bind(&row.label)
            .push_bind(&row.description)
            .push_bind(row.item_type.as_str())
            .push_bind(row.required)
            .push_bind(row.sort_order);
    });

    builder.build().execute(&mut **tx).await.map_err(|error| {
        tracing::error!(
            "Failed to bulk insert supervision template items: {}",
            error
        );
        AppError::InternalServerError("ไม่สามารถบันทึกหัวข้อแบบประเมินนิเทศได้".to_string())
    })?;

    Ok(())
}

async fn insert_template_steps(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    template_id: Uuid,
    steps: &[CreateSupervisionTemplateStepRequest],
) -> Result<(), AppError> {
    if steps.is_empty() {
        return Ok(());
    }

    let step_orders: Vec<i32> = steps.iter().map(|step| step.step_order).collect();
    let step_codes: Vec<String> = steps.iter().map(|step| step.step_code.clone()).collect();
    let labels: Vec<String> = steps.iter().map(|step| step.label.clone()).collect();
    let actor_kinds: Vec<String> = steps
        .iter()
        .map(|step| step.actor_kind.as_str().to_string())
        .collect();
    let actor_permissions: Vec<Option<String>> = steps
        .iter()
        .map(|step| step.actor_permission.clone())
        .collect();
    let organization_position_codes: Vec<Option<String>> = steps
        .iter()
        .map(|step| step.organization_position_code.clone())
        .collect();
    let action_kinds: Vec<String> = steps
        .iter()
        .map(|step| step.action_kind.as_str().to_string())
        .collect();
    let required_flags: Vec<bool> = steps.iter().map(|step| step.required).collect();

    sqlx::query(
        r#"
        INSERT INTO supervision_template_steps (
            template_id, step_order, step_code, label, actor_kind, actor_permission,
            organization_position_code, action_kind, required
        )
        SELECT $1, step_order, step_code, label, actor_kind, actor_permission,
               organization_position_code, action_kind, required
        FROM UNNEST(
            $2::int4[], $3::text[], $4::text[], $5::text[], $6::text[],
            $7::text[], $8::text[], $9::bool[]
        ) AS rows(
            step_order, step_code, label, actor_kind, actor_permission,
            organization_position_code, action_kind, required
        )
        "#,
    )
    .bind(template_id)
    .bind(&step_orders)
    .bind(&step_codes)
    .bind(&labels)
    .bind(&actor_kinds)
    .bind(&actor_permissions)
    .bind(&organization_position_codes)
    .bind(&action_kinds)
    .bind(&required_flags)
    .execute(&mut **tx)
    .await
    .map_err(|error| {
        tracing::error!("Failed to insert supervision template steps: {}", error);
        AppError::InternalServerError("ไม่สามารถบันทึกขั้นตอนแบบประเมินนิเทศได้".to_string())
    })?;

    Ok(())
}

fn template_from_rows(
    row: SupervisionTemplateRow,
    section_rows: Vec<SupervisionTemplateSectionRow>,
    item_rows: Vec<SupervisionTemplateItemRow>,
    step_rows: Vec<SupervisionTemplateStepRow>,
) -> Result<SupervisionTemplate, AppError> {
    let mut items_by_section: HashMap<Uuid, Vec<SupervisionTemplateItem>> = HashMap::new();
    for item_row in item_rows {
        let section_id = item_row.section_id;
        items_by_section
            .entry(section_id)
            .or_default()
            .push(template_item_from_row(item_row)?);
    }

    let mut sections = Vec::with_capacity(section_rows.len());
    for section_row in section_rows {
        let section_id = section_row.id;
        sections.push(SupervisionTemplateSection {
            id: section_row.id,
            template_id: section_row.template_id,
            title: section_row.title,
            description: section_row.description,
            sort_order: section_row.sort_order,
            created_at: section_row.created_at,
            updated_at: section_row.updated_at,
            items: items_by_section.remove(&section_id).unwrap_or_default(),
        });
    }

    let mut steps = Vec::with_capacity(step_rows.len());
    for step_row in step_rows {
        steps.push(template_step_from_row(step_row)?);
    }

    Ok(SupervisionTemplate {
        id: row.id,
        title: row.title,
        description: row.description,
        status: parse_template_status(&row.status)?,
        rating_min: row.rating_min,
        rating_max: row.rating_max,
        created_by: row.created_by,
        created_at: row.created_at,
        updated_at: row.updated_at,
        sections,
        steps,
    })
}

fn template_item_from_row(
    row: SupervisionTemplateItemRow,
) -> Result<SupervisionTemplateItem, AppError> {
    Ok(SupervisionTemplateItem {
        id: row.id,
        section_id: row.section_id,
        label: row.label,
        description: row.description,
        item_type: parse_template_item_type(&row.item_type)?,
        required: row.required,
        sort_order: row.sort_order,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

fn template_step_from_row(
    row: SupervisionTemplateStepRow,
) -> Result<SupervisionTemplateStep, AppError> {
    Ok(SupervisionTemplateStep {
        id: row.id,
        template_id: row.template_id,
        step_order: row.step_order,
        step_code: row.step_code,
        label: row.label,
        actor_kind: parse_step_actor_kind(&row.actor_kind)?,
        actor_permission: row.actor_permission,
        organization_position_code: row.organization_position_code,
        action_kind: parse_step_action_kind(&row.action_kind)?,
        required: row.required,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::supervision::models::CreateSupervisionTemplateItemRequest;
    #[test]
    fn template_bulk_rows_preserve_section_item_relationships() {
        let (section_rows, item_rows) =
            build_template_section_bulk_rows(&[CreateSupervisionTemplateSectionRequest {
                title: "ด้านการจัดกิจกรรม".to_string(),
                description: Some("ตรวจแผนและกิจกรรมการเรียนรู้".to_string()),
                sort_order: 1,
                items: vec![
                    CreateSupervisionTemplateItemRequest {
                        label: "จัดกิจกรรมตามแผน".to_string(),
                        description: None,
                        item_type: SupervisionTemplateItemType::Rating,
                        required: true,
                        sort_order: 1,
                    },
                    CreateSupervisionTemplateItemRequest {
                        label: "ข้อเสนอแนะ".to_string(),
                        description: Some("บันทึกเพิ่มเติม".to_string()),
                        item_type: SupervisionTemplateItemType::Text,
                        required: false,
                        sort_order: 2,
                    },
                ],
            }]);

        assert_eq!(section_rows.len(), 1);
        assert_eq!(item_rows.len(), 2);
        assert_ne!(section_rows[0].id, Uuid::nil());
        assert!(item_rows
            .iter()
            .all(|item| item.section_id == section_rows[0].id));
        assert_eq!(item_rows[0].item_type, SupervisionTemplateItemType::Rating);
        assert_eq!(item_rows[1].item_type, SupervisionTemplateItemType::Text);
    }
}
