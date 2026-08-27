use crate::error::AppError;
use crate::modules::menu::models::{
    AcademicMenuTemplateApplyResult, AcademicMenuTemplateMove, AcademicMenuTemplatePreview,
    AcademicMenuTemplateSection,
};
use chrono::{DateTime, SecondsFormat, Utc};
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::{FromRow, PgConnection, PgPool};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Clone, Copy)]
struct RecommendedSectionDefinition {
    code: &'static str,
    name: &'static str,
    name_en: &'static str,
    icon: &'static str,
    display_order: i32,
}

const ACADEMIC_WORKSPACE_CODE: &str = "academic";
const RECOMMENDED_SECTIONS: [RecommendedSectionDefinition; 7] = [
    RecommendedSectionDefinition {
        code: "academic_curriculum",
        name: "งานหลักสูตรและกลุ่มสาระ",
        name_en: "Curriculum and Learning Areas",
        icon: "book-open",
        display_order: 10,
    },
    RecommendedSectionDefinition {
        code: "academic_delivery",
        name: "งานจัดการเรียนการสอน",
        name_en: "Teaching and Learning Delivery",
        icon: "calendar-days",
        display_order: 20,
    },
    RecommendedSectionDefinition {
        code: "academic_registry",
        name: "งานทะเบียนนักเรียน",
        name_en: "Student Registry",
        icon: "users",
        display_order: 30,
    },
    RecommendedSectionDefinition {
        code: "academic_assessment",
        name: "งานวัดผลและประเมินผล",
        name_en: "Measurement and Evaluation",
        icon: "badge-check",
        display_order: 40,
    },
    RecommendedSectionDefinition {
        code: "academic_activities",
        name: "งานกิจกรรมพัฒนาผู้เรียน",
        name_en: "Learner Development Activities",
        icon: "sparkles",
        display_order: 50,
    },
    RecommendedSectionDefinition {
        code: "academic_supervision",
        name: "งานนิเทศและพัฒนาการสอน",
        name_en: "Instructional Supervision",
        icon: "clipboard-check",
        display_order: 60,
    },
    RecommendedSectionDefinition {
        code: "academic_admission",
        name: "งานรับนักเรียน",
        name_en: "Student Admission",
        icon: "clipboard-list",
        display_order: 70,
    },
];

#[derive(Debug, FromRow)]
struct RecommendedRouteRow {
    id: Uuid,
    name: String,
    current_group_id: Option<Uuid>,
    current_group_name: Option<String>,
    display_order: i32,
    updated_at: DateTime<Utc>,
    recommended_workspace_code: String,
    recommended_group_code: String,
    recommended_display_order: i32,
}

#[derive(Debug, FromRow)]
struct ExistingSectionRow {
    id: Uuid,
    code: String,
    name: String,
    workspace_code: String,
    display_order: i32,
    updated_at: DateTime<Utc>,
}

#[derive(Serialize)]
struct RevisionRoute<'a> {
    id: Uuid,
    current_group_id: Option<Uuid>,
    display_order: i32,
    updated_at: String,
    recommended_workspace_code: &'a str,
    recommended_group_code: &'a str,
    recommended_display_order: i32,
}

#[derive(Serialize)]
struct RevisionSection<'a> {
    code: &'a str,
    id: Option<Uuid>,
    name: Option<&'a str>,
    workspace_code: Option<&'a str>,
    display_order: Option<i32>,
    updated_at: Option<String>,
}

#[derive(Serialize)]
struct RevisionSnapshot<'a> {
    recommendations_ready: bool,
    routes: Vec<RevisionRoute<'a>>,
    sections: Vec<RevisionSection<'a>>,
}

pub async fn preview_academic_template(
    pool: &PgPool,
) -> Result<AcademicMenuTemplatePreview, AppError> {
    let mut connection = pool.acquire().await?;
    build_preview(&mut connection, false).await
}

pub async fn apply_academic_template(
    pool: &PgPool,
    expected_revision: &str,
) -> Result<AcademicMenuTemplateApplyResult, AppError> {
    let mut transaction = pool.begin().await?;
    let preview = build_preview(&mut transaction, true).await?;
    if preview.revision != expected_revision {
        return Err(AppError::Conflict(
            "ข้อมูลเมนูเปลี่ยนแล้ว กรุณาตรวจสอบรายการอีกครั้ง".to_string(),
        ));
    }
    if !preview.recommendations_ready {
        return Err(AppError::BadRequest(
            "คำแนะนำเมนูยังไม่พร้อม กรุณาซิงก์เส้นทางล่าสุดก่อน".to_string(),
        ));
    }

    let created_section_count = create_missing_sections(&mut transaction).await?;
    let target_ids = load_target_group_ids(&mut transaction).await?;
    let mut item_ids = Vec::with_capacity(preview.moves.len());
    let mut group_ids = Vec::with_capacity(preview.moves.len());
    let mut display_orders = Vec::with_capacity(preview.moves.len());
    for menu_move in &preview.moves {
        let group_id = target_ids
            .get(&menu_move.target_group_code)
            .copied()
            .ok_or_else(|| {
                AppError::InternalServerError(
                    "Recommended academic menu section is unavailable".to_string(),
                )
            })?;
        item_ids.push(menu_move.menu_item_id);
        group_ids.push(group_id);
        display_orders.push(menu_move.target_order);
    }

    let moved_count = if item_ids.is_empty() {
        0
    } else {
        sqlx::query(
            "UPDATE menu_items AS item
             SET group_id = updates.group_id,
                 display_order = updates.display_order,
                 updated_at = NOW()
             FROM UNNEST($1::uuid[], $2::uuid[], $3::int4[])
                  AS updates(id, group_id, display_order)
             WHERE item.id = updates.id
               AND item.managed_by = 'frontend'
               AND item.recommended_workspace_code = 'academic'",
        )
        .bind(&item_ids)
        .bind(&group_ids)
        .bind(&display_orders)
        .execute(&mut *transaction)
        .await?
        .rows_affected()
    };

    let applied_preview = build_preview(&mut transaction, false).await?;
    transaction.commit().await?;

    Ok(AcademicMenuTemplateApplyResult {
        revision: applied_preview.revision,
        created_section_count,
        moved_count,
    })
}

async fn build_preview(
    connection: &mut PgConnection,
    lock_rows: bool,
) -> Result<AcademicMenuTemplatePreview, AppError> {
    let route_query = if lock_rows {
        "SELECT item.id, item.name, item.group_id AS current_group_id,
                current_group.name AS current_group_name, item.display_order,
                item.updated_at, item.recommended_workspace_code,
                item.recommended_group_code, item.recommended_display_order
         FROM menu_items AS item
         LEFT JOIN menu_groups AS current_group ON current_group.id = item.group_id
         WHERE item.managed_by = 'frontend'
           AND item.recommended_workspace_code = 'academic'
           AND item.recommended_group_code IS NOT NULL
           AND item.recommended_display_order IS NOT NULL
         ORDER BY item.id
         FOR UPDATE OF item"
    } else {
        "SELECT item.id, item.name, item.group_id AS current_group_id,
                current_group.name AS current_group_name, item.display_order,
                item.updated_at, item.recommended_workspace_code,
                item.recommended_group_code, item.recommended_display_order
         FROM menu_items AS item
         LEFT JOIN menu_groups AS current_group ON current_group.id = item.group_id
         WHERE item.managed_by = 'frontend'
           AND item.recommended_workspace_code = 'academic'
           AND item.recommended_group_code IS NOT NULL
           AND item.recommended_display_order IS NOT NULL
         ORDER BY item.id"
    };
    let routes = sqlx::query_as::<_, RecommendedRouteRow>(route_query)
        .fetch_all(&mut *connection)
        .await?;

    let section_codes: Vec<&str> = RECOMMENDED_SECTIONS
        .iter()
        .map(|section| section.code)
        .collect();
    let section_query = if lock_rows {
        "SELECT id, code, name, workspace_code, display_order, updated_at
         FROM menu_groups
         WHERE code = ANY($1::varchar[])
         ORDER BY code
         FOR UPDATE"
    } else {
        "SELECT id, code, name, workspace_code, display_order, updated_at
         FROM menu_groups
         WHERE code = ANY($1::varchar[])
         ORDER BY code"
    };
    let sections = sqlx::query_as::<_, ExistingSectionRow>(section_query)
        .bind(&section_codes)
        .fetch_all(&mut *connection)
        .await?;
    let section_by_code: HashMap<&str, &ExistingSectionRow> = sections
        .iter()
        .map(|section| (section.code.as_str(), section))
        .collect();

    let incomplete_route_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM menu_items
         WHERE managed_by = 'frontend'
           AND (path LIKE '/staff/academic/%' OR path = '/staff/students')
           AND (recommended_workspace_code IS NULL
                OR recommended_group_code IS NULL
                OR recommended_display_order IS NULL)",
    )
    .fetch_one(&mut *connection)
    .await?;
    let has_unknown_target = routes.iter().any(|route| {
        !RECOMMENDED_SECTIONS
            .iter()
            .any(|definition| definition.code == route.recommended_group_code)
    });
    let recommendations_ready =
        !routes.is_empty() && incomplete_route_count == 0 && !has_unknown_target;

    let untouched_custom_item_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM menu_items WHERE managed_by IN ('school', 'integration')",
    )
    .fetch_one(&mut *connection)
    .await?;
    let untouched_non_academic_route_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM menu_items
         WHERE managed_by = 'frontend'
           AND recommended_workspace_code IS DISTINCT FROM 'academic'",
    )
    .fetch_one(&mut *connection)
    .await?;

    let sections_to_create = RECOMMENDED_SECTIONS
        .iter()
        .filter(|definition| !section_by_code.contains_key(definition.code))
        .map(|definition| AcademicMenuTemplateSection {
            code: definition.code.to_string(),
            name: definition.name.to_string(),
            workspace_code: ACADEMIC_WORKSPACE_CODE.to_string(),
            display_order: definition.display_order,
        })
        .collect();

    let moves = routes
        .iter()
        .filter_map(|route| {
            let definition = RECOMMENDED_SECTIONS
                .iter()
                .find(|definition| definition.code == route.recommended_group_code)?;
            let target_group_id = section_by_code
                .get(definition.code)
                .map(|section| section.id);
            (target_group_id.is_none()
                || route.current_group_id != target_group_id
                || route.display_order != route.recommended_display_order)
                .then(|| AcademicMenuTemplateMove {
                    menu_item_id: route.id,
                    menu_item_name: route.name.clone(),
                    current_group_name: route.current_group_name.clone(),
                    target_group_code: definition.code.to_string(),
                    target_group_name: section_by_code
                        .get(definition.code)
                        .map(|section| section.name.clone())
                        .unwrap_or_else(|| definition.name.to_string()),
                    current_order: route.display_order,
                    target_order: route.recommended_display_order,
                })
        })
        .collect();

    let revision_snapshot = RevisionSnapshot {
        recommendations_ready,
        routes: routes
            .iter()
            .map(|route| RevisionRoute {
                id: route.id,
                current_group_id: route.current_group_id,
                display_order: route.display_order,
                updated_at: route.updated_at.to_rfc3339_opts(SecondsFormat::Nanos, true),
                recommended_workspace_code: &route.recommended_workspace_code,
                recommended_group_code: &route.recommended_group_code,
                recommended_display_order: route.recommended_display_order,
            })
            .collect(),
        sections: RECOMMENDED_SECTIONS
            .iter()
            .map(|definition| {
                let section = section_by_code.get(definition.code).copied();
                RevisionSection {
                    code: definition.code,
                    id: section.map(|value| value.id),
                    name: section.map(|value| value.name.as_str()),
                    workspace_code: section.map(|value| value.workspace_code.as_str()),
                    display_order: section.map(|value| value.display_order),
                    updated_at: section
                        .map(|value| value.updated_at.to_rfc3339_opts(SecondsFormat::Nanos, true)),
                }
            })
            .collect(),
    };
    let serialized = serde_json::to_vec(&revision_snapshot).map_err(|error| {
        AppError::InternalServerError(format!("Failed to create menu revision: {error}"))
    })?;
    let revision = hex::encode(Sha256::digest(serialized));

    Ok(AcademicMenuTemplatePreview {
        revision,
        recommendations_ready,
        sections_to_create,
        moves,
        untouched_custom_item_count,
        untouched_non_academic_route_count,
    })
}

async fn create_missing_sections(connection: &mut PgConnection) -> Result<u64, AppError> {
    let codes: Vec<&str> = RECOMMENDED_SECTIONS
        .iter()
        .map(|section| section.code)
        .collect();
    let names: Vec<&str> = RECOMMENDED_SECTIONS
        .iter()
        .map(|section| section.name)
        .collect();
    let names_en: Vec<&str> = RECOMMENDED_SECTIONS
        .iter()
        .map(|section| section.name_en)
        .collect();
    let icons: Vec<&str> = RECOMMENDED_SECTIONS
        .iter()
        .map(|section| section.icon)
        .collect();
    let display_orders: Vec<i32> = RECOMMENDED_SECTIONS
        .iter()
        .map(|section| section.display_order)
        .collect();
    let workspace_codes = vec![ACADEMIC_WORKSPACE_CODE; RECOMMENDED_SECTIONS.len()];

    sqlx::query(
        "INSERT INTO menu_groups
            (code, name, name_en, icon, display_order, is_active, workspace_code)
         SELECT code, name, name_en, icon, display_order, true, workspace_code
         FROM UNNEST(
             $1::varchar[], $2::varchar[], $3::varchar[], $4::varchar[],
             $5::int4[], $6::varchar[]
         ) AS section(code, name, name_en, icon, display_order, workspace_code)
         ON CONFLICT (code) DO NOTHING",
    )
    .bind(&codes)
    .bind(&names)
    .bind(&names_en)
    .bind(&icons)
    .bind(&display_orders)
    .bind(&workspace_codes)
    .execute(connection)
    .await
    .map(|result| result.rows_affected())
    .map_err(AppError::from)
}

async fn load_target_group_ids(
    connection: &mut PgConnection,
) -> Result<HashMap<String, Uuid>, AppError> {
    let codes: Vec<&str> = RECOMMENDED_SECTIONS
        .iter()
        .map(|section| section.code)
        .collect();
    let rows: Vec<(String, Uuid)> =
        sqlx::query_as("SELECT code, id FROM menu_groups WHERE code = ANY($1::varchar[])")
            .bind(&codes)
            .fetch_all(connection)
            .await?;
    Ok(rows.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::AppError;
    use crate::test_helpers::{create_named_test_pool, run_test_migrations};
    use sqlx::PgPool;
    use uuid::Uuid;

    async fn test_pool(test_name: &str) -> PgPool {
        let pool = create_named_test_pool(test_name).await;
        run_test_migrations(&pool).await;
        pool
    }

    async fn insert_template_fixture(pool: &PgPool) -> (Uuid, Uuid, Uuid) {
        let custom_group_id: Uuid = sqlx::query_scalar(
            "INSERT INTO menu_groups
                (code, name, workspace_code, display_order, is_active)
             VALUES ('school_custom_academic', 'งานวิชาการที่โรงเรียนจัดเอง',
                     'academic', 88, true)
             RETURNING id",
        )
        .fetch_one(pool)
        .await
        .expect("custom group should exist");

        let ids: Vec<Uuid> = sqlx::query_scalar(
            "INSERT INTO menu_items
                (code, name, path, icon, group_id, required_permission, user_type,
                 display_order, is_active, managed_by, recommended_workspace_code,
                 recommended_group_code, recommended_display_order)
             VALUES
                ('staff-academic-core', 'ชื่อเมนูของโรงเรียน', '/staff/academic/core',
                 'school-icon', $1, 'academic_term.read.school', 'staff', 77,
                 false, 'frontend', 'academic', 'academic_delivery', 10),
                ('school-custom-link', 'คู่มือโรงเรียน', '/school-guide', NULL, $1,
                 NULL, 'staff', 78, true, 'school', 'academic',
                 'academic_delivery', 20),
                ('integration-custom-link', 'ระบบภายนอก', '/integration', NULL, $1,
                 NULL, 'staff', 79, true, 'integration', 'academic',
                 'academic_delivery', 30)
             RETURNING id",
        )
        .bind(custom_group_id)
        .fetch_all(pool)
        .await
        .expect("menu fixtures should exist");

        (custom_group_id, ids[1], ids[2])
    }

    #[tokio::test]
    async fn preview_and_apply_move_only_frontend_academic_routes() {
        let pool = test_pool("academic_menu_template_apply").await;
        let (custom_group_id, school_item_id, integration_item_id) =
            insert_template_fixture(&pool).await;

        let preview = preview_academic_template(&pool)
            .await
            .expect("preview should load");
        assert!(preview.recommendations_ready);
        assert_eq!(preview.moves.len(), 1);
        assert_eq!(preview.moves[0].menu_item_name, "ชื่อเมนูของโรงเรียน");
        assert_eq!(preview.moves[0].target_group_code, "academic_delivery");
        assert_eq!(preview.moves[0].target_order, 10);
        assert_eq!(preview.untouched_custom_item_count, 2);

        let applied = apply_academic_template(&pool, &preview.revision)
            .await
            .expect("template should apply");
        assert_eq!(applied.moved_count, 1);

        let route: (
            Uuid,
            i32,
            String,
            Option<String>,
            bool,
            String,
            Option<String>,
            String,
        ) = sqlx::query_as(
            "SELECT group_id, display_order, name, icon, is_active, path,
                        required_permission, user_type
                 FROM menu_items
                 WHERE code = 'staff-academic-core'",
        )
        .fetch_one(&pool)
        .await
        .expect("moved route should load");
        assert_ne!(route.0, custom_group_id);
        assert_eq!(route.1, 10);
        assert_eq!(route.2, "ชื่อเมนูของโรงเรียน");
        assert_eq!(route.3.as_deref(), Some("school-icon"));
        assert!(!route.4);
        assert_eq!(route.5, "/staff/academic/core");
        assert_eq!(route.6.as_deref(), Some("academic_term.read.school"));
        assert_eq!(route.7, "staff");

        for item_id in [school_item_id, integration_item_id] {
            let group_id: Uuid =
                sqlx::query_scalar("SELECT group_id FROM menu_items WHERE id = $1")
                    .bind(item_id)
                    .fetch_one(&pool)
                    .await
                    .expect("custom item should load");
            assert_eq!(group_id, custom_group_id);
        }
    }

    #[tokio::test]
    async fn apply_rejects_a_stale_revision_without_moving_items() {
        let pool = test_pool("academic_menu_template_stale").await;
        let (custom_group_id, _, _) = insert_template_fixture(&pool).await;
        let preview = preview_academic_template(&pool)
            .await
            .expect("preview should load");

        sqlx::query(
            "UPDATE menu_items
             SET display_order = 76, updated_at = NOW()
             WHERE code = 'staff-academic-core'",
        )
        .execute(&pool)
        .await
        .expect("concurrent customization should succeed");

        let error = apply_academic_template(&pool, &preview.revision)
            .await
            .expect_err("stale revision must fail");
        assert!(matches!(error, AppError::Conflict(_)));

        let group_id: Uuid = sqlx::query_scalar(
            "SELECT group_id FROM menu_items WHERE code = 'staff-academic-core'",
        )
        .fetch_one(&pool)
        .await
        .expect("route should load");
        assert_eq!(group_id, custom_group_id);
    }

    #[tokio::test]
    async fn applying_an_unchanged_template_is_idempotent() {
        let pool = test_pool("academic_menu_template_idempotent").await;
        insert_template_fixture(&pool).await;
        let preview = preview_academic_template(&pool)
            .await
            .expect("preview should load");

        let first = apply_academic_template(&pool, &preview.revision)
            .await
            .expect("first apply should succeed");
        assert_eq!(first.moved_count, 1);

        let second_preview = preview_academic_template(&pool)
            .await
            .expect("second preview should load");
        assert!(second_preview.moves.is_empty());
        let second = apply_academic_template(&pool, &second_preview.revision)
            .await
            .expect("second apply should succeed");
        assert_eq!(second.moved_count, 0);
    }

    #[tokio::test]
    async fn apply_recreates_a_missing_recommended_section() {
        let pool = test_pool("academic_menu_template_missing_section").await;
        sqlx::query("DELETE FROM menu_groups WHERE code = 'academic_activities'")
            .execute(&pool)
            .await
            .expect("unused target section should be removable");

        sqlx::query(
            "INSERT INTO menu_items
                (code, name, path, display_order, managed_by,
                 recommended_workspace_code, recommended_group_code,
                 recommended_display_order)
             VALUES ('staff-academic-activities', 'กิจกรรม',
                     '/staff/academic/catalog/activities', 90, 'frontend',
                     'academic', 'academic_activities', 10)",
        )
        .execute(&pool)
        .await
        .expect("route should exist");

        let preview = preview_academic_template(&pool)
            .await
            .expect("preview should load");
        assert_eq!(preview.sections_to_create.len(), 1);
        assert_eq!(preview.sections_to_create[0].code, "academic_activities");

        let applied = apply_academic_template(&pool, &preview.revision)
            .await
            .expect("apply should recreate the section");
        assert_eq!(applied.created_section_count, 1);
        assert_eq!(applied.moved_count, 1);
    }
}
