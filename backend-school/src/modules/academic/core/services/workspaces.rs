use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::middleware::permission::ActorContext;
use crate::permissions::registry::codes;
use crate::policies::resource_access_policy::{
    academic_resource_access_for, AcademicResourceAccess, AcademicResourceListFilter,
};

use super::super::models::{AcademicSetupWorkspace, CurriculumProgramWorkspace};
use super::{bell_schedules, curriculum, years_terms};

const MAX_WORKSPACE_PROGRAMS: usize = 500;
const MAX_WORKSPACE_REQUIREMENTS: usize = 10_000;
const MAX_SETUP_YEARS: usize = 100;
const MAX_SETUP_TERMS: usize = 2_000;
const MAX_SETUP_BELL_SCHEDULES: usize = 1_000;

#[derive(sqlx::FromRow)]
struct CurriculumVersionAccessRow {
    owning_organization_unit_id: Option<Uuid>,
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
