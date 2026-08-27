use std::collections::HashMap;

use chrono::{DateTime, NaiveDate, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::middleware::permission::ActorContext;
use crate::permissions::registry::codes;
use crate::policies::resource_access_policy::{
    academic_resource_access_for, AcademicResourceAccess, AcademicResourceListFilter,
};

use crate::modules::lookup::models::GradeLevelLookupItem;

use super::super::models::{
    AcademicSetupWorkspace, CurriculumDisplayState, CurriculumOverview, CurriculumOverviewItem,
    CurriculumProgramWorkspace, CurriculumVersion, VersionStatus,
};
use super::{bell_schedules, curriculum, years_terms};

const MAX_WORKSPACE_PROGRAMS: usize = 500;
const MAX_WORKSPACE_REQUIREMENTS: usize = 10_000;
const MAX_SETUP_YEARS: usize = 100;
const MAX_SETUP_TERMS: usize = 2_000;
const MAX_SETUP_BELL_SCHEDULES: usize = 1_000;
const MAX_CURRICULUM_OVERVIEW_VERSIONS: usize = 5_000;

#[derive(sqlx::FromRow)]
struct CurriculumVersionAccessRow {
    owning_organization_unit_id: Option<Uuid>,
}

#[derive(sqlx::FromRow)]
struct CurriculumOverviewVersionRow {
    id: Uuid,
    curriculum_id: Uuid,
    version_name: String,
    start_academic_year_id: Uuid,
    end_academic_year_id: Option<Uuid>,
    description: Option<String>,
    status: VersionStatus,
    published_at: Option<DateTime<Utc>>,
    row_version: i64,
    migrated: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    start_academic_year_name: String,
    start_academic_year_date: NaiveDate,
    end_academic_year_name: Option<String>,
    end_academic_year_date: Option<NaiveDate>,
}

impl CurriculumOverviewVersionRow {
    fn version(&self) -> CurriculumVersion {
        CurriculumVersion {
            id: self.id,
            curriculum_id: self.curriculum_id,
            version_name: self.version_name.clone(),
            start_academic_year_id: self.start_academic_year_id,
            end_academic_year_id: self.end_academic_year_id,
            description: self.description.clone(),
            status: self.status,
            published_at: self.published_at,
            row_version: self.row_version,
            migrated: self.migrated,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

#[derive(sqlx::FromRow)]
struct WorkspaceGradeLevelRow {
    id: Uuid,
    level_type: String,
    year: i32,
}

#[derive(sqlx::FromRow)]
struct VersionProgramCountRow {
    curriculum_version_id: Uuid,
    count: i64,
}

pub async fn curriculum_overview(
    pool: &PgPool,
    filter: &AcademicResourceListFilter,
) -> Result<CurriculumOverview, AppError> {
    let curricula = curriculum::list(pool, filter).await?;
    if curricula.is_empty() {
        return Ok(CurriculumOverview { items: Vec::new() });
    }
    let curriculum_ids = curricula
        .iter()
        .map(|curriculum| curriculum.id)
        .collect::<Vec<_>>();
    let versions = sqlx::query_as::<_, CurriculumOverviewVersionRow>(
        r#"
        SELECT version.id, version.curriculum_id, version.version_name,
               version.start_academic_year_id, version.end_academic_year_id,
               version.description, version.status, version.published_at,
               version.row_version,
               version.migration_provenance <> '{}'::jsonb AS migrated,
               version.created_at, version.updated_at,
               starts.name AS start_academic_year_name,
               starts.start_date AS start_academic_year_date,
               ends.name AS end_academic_year_name,
               ends.end_date AS end_academic_year_date
        FROM curriculum_versions version
        JOIN academic_years starts ON starts.id = version.start_academic_year_id
        LEFT JOIN academic_years ends ON ends.id = version.end_academic_year_id
        WHERE version.curriculum_id = ANY($1)
        ORDER BY version.curriculum_id, version.created_at DESC, version.id
        LIMIT $2
        "#,
    )
    .bind(&curriculum_ids)
    .bind((MAX_CURRICULUM_OVERVIEW_VERSIONS + 1) as i64)
    .fetch_all(pool)
    .await?;
    ensure_workspace_size(
        versions.len(),
        MAX_CURRICULUM_OVERVIEW_VERSIONS,
        "จำนวนเวอร์ชันในภาพรวมหลักสูตร",
    )?;
    let version_ids = versions
        .iter()
        .map(|version| version.id)
        .collect::<Vec<_>>();
    let program_counts = if version_ids.is_empty() {
        Vec::new()
    } else {
        sqlx::query_as::<_, VersionProgramCountRow>(
            r#"
            SELECT curriculum_version_id, count(*)::bigint AS count
            FROM study_programs
            WHERE curriculum_version_id = ANY($1) AND status <> 'archived'
            GROUP BY curriculum_version_id
            "#,
        )
        .bind(&version_ids)
        .fetch_all(pool)
        .await?
    };
    let program_counts = program_counts
        .into_iter()
        .map(|row| (row.curriculum_version_id, row.count))
        .collect::<HashMap<_, _>>();
    let grade_level_ids = curricula
        .iter()
        .flat_map(|curriculum| curriculum.grade_level_ids.iter().copied())
        .collect::<Vec<_>>();
    let grade_levels = if grade_level_ids.is_empty() {
        Vec::new()
    } else {
        sqlx::query_as::<_, WorkspaceGradeLevelRow>(
            r#"
            SELECT id, level_type, year
            FROM grade_levels
            WHERE id = ANY($1)
            ORDER BY CASE level_type
                        WHEN 'kindergarten' THEN 1
                        WHEN 'primary' THEN 2
                        WHEN 'secondary' THEN 3
                        ELSE 4
                     END,
                     year,
                     id
            "#,
        )
        .bind(&grade_level_ids)
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(workspace_grade_level_item)
        .collect::<Vec<_>>()
    };
    let today: NaiveDate = sqlx::query_scalar("SELECT CURRENT_DATE")
        .fetch_one(pool)
        .await?;
    let mut versions_by_curriculum: HashMap<Uuid, Vec<&CurriculumOverviewVersionRow>> =
        HashMap::new();
    for version in &versions {
        versions_by_curriculum
            .entry(version.curriculum_id)
            .or_default()
            .push(version);
    }

    let items = curricula
        .into_iter()
        .map(|curriculum| {
            let candidate_versions = versions_by_curriculum
                .get(&curriculum.id)
                .map(Vec::as_slice)
                .unwrap_or_default();
            let (display, display_state, draft_count) =
                select_curriculum_display(candidate_versions, today);
            let resolved_grade_levels = grade_levels
                .iter()
                .filter(|level| curriculum.grade_level_ids.contains(&level.id))
                .cloned()
                .collect();
            CurriculumOverviewItem {
                start_academic_year_name: display
                    .map(|version| version.start_academic_year_name.clone()),
                end_academic_year_name: display
                    .and_then(|version| version.end_academic_year_name.clone()),
                study_program_count: display
                    .and_then(|version| program_counts.get(&version.id).copied())
                    .unwrap_or(0),
                display_version: display.map(CurriculumOverviewVersionRow::version),
                curriculum,
                display_state,
                grade_levels: resolved_grade_levels,
                draft_count,
            }
        })
        .collect();
    Ok(CurriculumOverview { items })
}

fn select_curriculum_display<'a>(
    versions: &[&'a CurriculumOverviewVersionRow],
    today: NaiveDate,
) -> (
    Option<&'a CurriculumOverviewVersionRow>,
    CurriculumDisplayState,
    i64,
) {
    let draft_count = versions
        .iter()
        .filter(|version| version.status == VersionStatus::Draft)
        .count() as i64;
    let published = versions
        .iter()
        .copied()
        .filter(|version| version.status == VersionStatus::Published)
        .collect::<Vec<_>>();
    if let Some(current) = published
        .iter()
        .copied()
        .filter(|version| {
            version.start_academic_year_date <= today
                && version
                    .end_academic_year_date
                    .is_none_or(|end| end >= today)
        })
        .max_by_key(|version| {
            (
                version.start_academic_year_date,
                version.created_at,
                version.id,
            )
        })
    {
        return (Some(current), CurriculumDisplayState::Current, draft_count);
    }
    if let Some(upcoming) = published
        .iter()
        .copied()
        .filter(|version| version.start_academic_year_date > today)
        .min_by_key(|version| {
            (
                version.start_academic_year_date,
                version.created_at,
                version.id,
            )
        })
    {
        return (
            Some(upcoming),
            CurriculumDisplayState::Upcoming,
            draft_count,
        );
    }
    if let Some(expired) = published
        .into_iter()
        .filter(|version| {
            version
                .end_academic_year_date
                .is_some_and(|end| end < today)
        })
        .max_by_key(|version| {
            (
                version.end_academic_year_date,
                version.start_academic_year_date,
                version.created_at,
                version.id,
            )
        })
    {
        return (Some(expired), CurriculumDisplayState::Expired, draft_count);
    }
    (None, CurriculumDisplayState::Unpublished, draft_count)
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

pub async fn program_workspace(
    pool: &PgPool,
    version_id: Uuid,
    filter: &AcademicResourceListFilter,
) -> Result<CurriculumProgramWorkspace, AppError> {
    let access: CurriculumVersionAccessRow = sqlx::query_as(
        r#"SELECT curriculum.owning_organization_unit_id
           FROM curriculum_versions version
           JOIN curricula curriculum ON curriculum.id = version.curriculum_id
           WHERE version.id = $1"#,
    )
    .bind(version_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบเวอร์ชันหลักสูตร".to_string()))?;
    if academic_resource_access_for(filter, access.owning_organization_unit_id, false)
        == AcademicResourceAccess::None
    {
        return Err(AppError::Forbidden("ไม่มีสิทธิ์เข้าถึงทรัพยากรนี้".to_string()));
    }

    let programs = curriculum::list_programs_for_version(pool, version_id).await?;
    ensure_workspace_size(
        programs.len(),
        MAX_WORKSPACE_PROGRAMS,
        "จำนวนแผนการเรียนในเวอร์ชันหลักสูตร",
    )?;
    let program_ids: Vec<Uuid> = programs.iter().map(|program| program.id).collect();
    let requirements = curriculum::list_requirements_for_programs(pool, &program_ids).await?;
    ensure_workspace_size(
        requirements.len(),
        MAX_WORKSPACE_REQUIREMENTS,
        "จำนวนข้อกำหนดในเวอร์ชันหลักสูตร",
    )?;
    Ok(CurriculumProgramWorkspace {
        programs,
        requirements,
    })
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
    use super::ensure_workspace_size;

    #[test]
    fn oversized_workspace_collections_are_rejected() {
        assert!(ensure_workspace_size(2, 2, "รายการทดสอบ").is_ok());
        assert!(matches!(
            ensure_workspace_size(3, 2, "รายการทดสอบ"),
            Err(crate::error::AppError::ValidationError(_))
        ));
    }
}
