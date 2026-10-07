use std::collections::HashMap;

use sqlx::PgPool;
use uuid::Uuid;

use school_authorization::AcademicResourceListFilter;
use school_authorization::ActorContext;
use school_errors::AppError;
use school_permissions::registry::codes;

use crate::models::GradeLevelLookupItem;

use super::super::models::{
    AcademicSetupWorkspace, CurriculumCatalogVersionOption, CurriculumCreateOptions,
    CurriculumLevelView, CurriculumManagementOptions, CurriculumOverview, CurriculumOverviewItem,
};
use super::{bell_schedules, curriculum, years_terms};

const MAX_SETUP_YEARS: usize = 100;
const MAX_SETUP_TERMS: usize = 2_000;
const MAX_SETUP_BELL_SCHEDULES: usize = 1_000;
const MAX_CURRICULUM_OPTION_GRADES: usize = 500;
const MAX_CURRICULUM_CATALOG_OPTIONS: usize = 5_000;

#[derive(sqlx::FromRow)]
struct WorkspaceGradeLevelRow {
    id: Uuid,
    level_type: String,
    year: i32,
}

pub async fn curriculum_overview(
    pool: &PgPool,
    filter: &AcademicResourceListFilter,
) -> Result<CurriculumOverview, AppError> {
    let editions = curriculum::list(pool, filter).await?;
    let ids = editions.iter().map(|e| e.id).collect::<Vec<_>>();
    let counts:Vec<(Uuid,i64,i64)>=sqlx::query_as("WITH visible_levels AS (SELECT l.id,l.edition_id FROM published_curriculum_levels l UNION ALL SELECT l.id,l.edition_id FROM curriculum_levels l JOIN curriculum_editions e ON e.id=l.edition_id WHERE e.current_publication_id IS NULL),visible_programs AS(SELECT p.id,p.curriculum_level_id,p.status FROM published_study_programs p UNION ALL SELECT p.id,p.curriculum_level_id,p.status FROM study_programs p JOIN curriculum_levels l ON l.id=p.curriculum_level_id JOIN curriculum_editions e ON e.id=l.edition_id WHERE e.current_publication_id IS NULL) SELECT l.edition_id,count(DISTINCT l.id),count(p.id) FROM visible_levels l LEFT JOIN visible_programs p ON p.curriculum_level_id=l.id AND p.status<>'archived' WHERE l.edition_id=ANY($1) GROUP BY l.edition_id").bind(&ids).fetch_all(pool).await?;
    let counts = counts
        .into_iter()
        .map(|(id, l, p)| (id, (l, p)))
        .collect::<HashMap<_, _>>();
    let items = editions
        .into_iter()
        .map(|edition| {
            let (level_count, study_program_count) =
                counts.get(&edition.id).copied().unwrap_or((0, 0));
            CurriculumOverviewItem {
                edition,
                level_count,
                study_program_count,
            }
        })
        .collect();
    Ok(CurriculumOverview { items })
}

fn workspace_grade_level_item(row: WorkspaceGradeLevelRow) -> GradeLevelLookupItem {
    let (name, code, short_name, order_base) = match row.level_type.as_str() {
        "kindergarten" => (
            format!("อนุบาลปีที่ {}", row.year),
            format!("K{}", row.year),
            format!("อ.{}", row.year),
            1,
        ),
        "primary" => (
            format!("ประถมศึกษาปีที่ {}", row.year),
            format!("P{}", row.year),
            format!("ป.{}", row.year),
            2,
        ),
        "secondary" => (
            format!("มัธยมศึกษาปีที่ {}", row.year),
            format!("M{}", row.year),
            format!("ม.{}", row.year),
            3,
        ),
        _ => (
            format!("Other {}", row.year),
            format!("O{}", row.year),
            format!("?{}", row.year),
            4,
        ),
    };
    GradeLevelLookupItem {
        id: row.id,
        code,
        name,
        short_name: Some(short_name),
        level_type: row.level_type,
        level_order: order_base * 100 + row.year,
    }
}

pub async fn curriculum_create_options(
    pool: &PgPool,
    filter: &AcademicResourceListFilter,
) -> Result<CurriculumCreateOptions, AppError> {
    require_filter_scope(filter)?;
    Ok(CurriculumCreateOptions {
        grade_levels: active_workspace_grade_levels(pool).await?,
    })
}
pub async fn curriculum_level_views(
    pool: &PgPool,
    edition_id: Uuid,
) -> Result<Vec<CurriculumLevelView>, AppError> {
    Ok(curriculum::list_levels(pool, edition_id)
        .await?
        .into_iter()
        .map(|level| CurriculumLevelView { level })
        .collect())
}

pub async fn curriculum_management_options(
    pool: &PgPool,
    version_id: Uuid,
    filter: &AcademicResourceListFilter,
) -> Result<CurriculumManagementOptions, AppError> {
    require_curriculum_version_access(pool, version_id, filter).await?;
    let grade_levels = active_workspace_grade_levels(pool).await?;
    let owner_ids = filter.allowed_organization_unit_ids();
    let catalog_versions = sqlx::query_as::<_, CurriculumCatalogVersionOption>(
        r#"
        SELECT option.id, option.resource_kind, option.code, option.name,
               option.version_no, option.effective_from, option.effective_until
        FROM (
            SELECT version.id, 'course'::text AS resource_kind,
                   subject.code, version.name_th AS name, version.version_no,
                   version.effective_from, version.effective_until
            FROM subject_versions version
            JOIN subjects subject ON subject.id = version.subject_id
            WHERE version.status = 'published'
              AND ($1 OR subject.owning_organization_unit_id = ANY($2))
            UNION ALL
            SELECT version.id, 'activity'::text AS resource_kind,
                   activity.code, version.name, version.version_no,
                   version.effective_from, version.effective_until
            FROM activity_versions version
            JOIN activities activity ON activity.id = version.activity_id
            WHERE version.status = 'published'
              AND ($1 OR activity.owning_organization_unit_id = ANY($2))
        ) option
        ORDER BY option.resource_kind, option.code, option.version_no DESC, option.id
        LIMIT $3
        "#,
    )
    .bind(filter.includes_school_owned)
    .bind(owner_ids)
    .bind((MAX_CURRICULUM_CATALOG_OPTIONS + 1) as i64)
    .fetch_all(pool)
    .await?;
    ensure_workspace_size(
        catalog_versions.len(),
        MAX_CURRICULUM_CATALOG_OPTIONS,
        "จำนวนตัวเลือกวิชาและกิจกรรมสำหรับหลักสูตร",
    )?;
    Ok(CurriculumManagementOptions {
        grade_levels,
        catalog_versions,
    })
}

fn require_filter_scope(filter: &AcademicResourceListFilter) -> Result<(), AppError> {
    if filter.includes_school_owned {
        Ok(())
    } else {
        Err(AppError::Forbidden("ไม่มีสิทธิ์เข้าถึงทรัพยากรนี้".to_string()))
    }
}

async fn require_curriculum_version_access(
    pool: &PgPool,
    level_id: Uuid,
    filter: &AcademicResourceListFilter,
) -> Result<(), AppError> {
    require_filter_scope(filter)?;
    curriculum::get_level(pool, level_id).await?;
    Ok(())
}

async fn active_workspace_grade_levels(
    pool: &PgPool,
) -> Result<Vec<GradeLevelLookupItem>, AppError> {
    let rows = sqlx::query_as::<_, WorkspaceGradeLevelRow>(
        r#"
        SELECT id, level_type, year
        FROM grade_levels
        WHERE is_active = true
        ORDER BY CASE level_type
                    WHEN 'kindergarten' THEN 1
                    WHEN 'primary' THEN 2
                    WHEN 'secondary' THEN 3
                    ELSE 4
                 END,
                 year,
                 id
        LIMIT $1
        "#,
    )
    .bind((MAX_CURRICULUM_OPTION_GRADES + 1) as i64)
    .fetch_all(pool)
    .await?;
    ensure_workspace_size(
        rows.len(),
        MAX_CURRICULUM_OPTION_GRADES,
        "จำนวนระดับชั้นสำหรับจัดการหลักสูตร",
    )?;
    Ok(rows.into_iter().map(workspace_grade_level_item).collect())
}

pub async fn setup_workspace(
    pool: &PgPool,
    actor: &ActorContext,
) -> Result<AcademicSetupWorkspace, AppError> {
    actor.require_any_permission(&[
        codes::ACADEMIC_YEAR_READ_SCHOOL,
        codes::ACADEMIC_YEAR_MANAGE_SCHOOL,
    ])?;
    actor.require_any_permission(&[
        codes::ACADEMIC_TERM_READ_SCHOOL,
        codes::ACADEMIC_TERM_MANAGE_SCHOOL,
    ])?;

    let years = years_terms::list_years(pool).await?;
    ensure_workspace_size(years.len(), MAX_SETUP_YEARS, "จำนวนปีการศึกษา")?;
    let terms = years_terms::list_all_terms(pool).await?;
    ensure_workspace_size(terms.len(), MAX_SETUP_TERMS, "จำนวนภาคเรียน")?;
    let bell_schedules = bell_schedules::list_all(pool).await?;
    ensure_workspace_size(
        bell_schedules.len(),
        MAX_SETUP_BELL_SCHEDULES,
        "จำนวนตารางคาบ",
    )?;
    Ok(AcademicSetupWorkspace {
        years,
        terms,
        bell_schedules,
    })
}

fn ensure_workspace_size(actual: usize, maximum: usize, label: &str) -> Result<(), AppError> {
    if actual > maximum {
        Err(AppError::ValidationError(format!(
            "{label}มากเกินขีดจำกัด {maximum} รายการ"
        )))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{ensure_workspace_size, workspace_grade_level_item, WorkspaceGradeLevelRow};
    use uuid::Uuid;
    #[test]
    fn level_grade_choices_have_human_names_and_secondary_order() {
        let item = workspace_grade_level_item(WorkspaceGradeLevelRow {
            id: Uuid::new_v4(),
            level_type: "secondary".into(),
            year: 2,
        });
        assert_eq!(item.name, "มัธยมศึกษาปีที่ 2");
        assert_eq!(item.short_name.as_deref(), Some("ม.2"));
        assert_eq!(item.level_order, 302);
    }

    #[test]
    fn oversized_workspace_collections_are_rejected() {
        assert!(ensure_workspace_size(2, 2, "รายการทดสอบ").is_ok());
        assert!(matches!(
            ensure_workspace_size(3, 2, "รายการทดสอบ"),
            Err(school_errors::AppError::ValidationError(_))
        ));
    }
}
