use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::middleware::permission::ActorContext;
use crate::permissions::registry::codes;
use crate::policies::resource_access_policy::{
    self, AcademicResourceAccess, AcademicResourceListFilter, AcademicResourcePermissions,
};

const NO_PERMISSIONS: &[&str] = &[];
const CURRICULUM_READ_UNIT_PERMISSIONS: &[&str] = &[
    codes::ACADEMIC_CURRICULUM_READ_ORGANIZATION_UNIT,
    codes::ACADEMIC_CURRICULUM_MANAGE_ORGANIZATION_UNIT,
];
const CURRICULUM_READ_TREE_PERMISSIONS: &[&str] = &[
    codes::ACADEMIC_CURRICULUM_READ_ORGANIZATION_TREE,
    codes::ACADEMIC_CURRICULUM_MANAGE_ORGANIZATION_TREE,
];
const CURRICULUM_READ_SCHOOL_PERMISSIONS: &[&str] = &[
    codes::ACADEMIC_CURRICULUM_READ_SCHOOL,
    codes::ACADEMIC_CURRICULUM_MANAGE_SCHOOL,
];
const CURRICULUM_MANAGE_UNIT_PERMISSIONS: &[&str] =
    &[codes::ACADEMIC_CURRICULUM_MANAGE_ORGANIZATION_UNIT];
const CURRICULUM_MANAGE_TREE_PERMISSIONS: &[&str] =
    &[codes::ACADEMIC_CURRICULUM_MANAGE_ORGANIZATION_TREE];
const CURRICULUM_MANAGE_SCHOOL_PERMISSIONS: &[&str] = &[codes::ACADEMIC_CURRICULUM_MANAGE_SCHOOL];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CurriculumAction {
    Read,
    Manage,
}

pub async fn academic_curriculum_list_access(
    pool: &PgPool,
    actor: &ActorContext,
    action: CurriculumAction,
) -> Result<AcademicResourceListFilter, AppError> {
    resource_access_policy::resolve_academic_resource_list_filter(
        pool,
        actor,
        curriculum_permissions(action),
    )
    .await
}

pub async fn academic_curriculum_access(
    pool: &PgPool,
    actor: &ActorContext,
    curriculum_id: Uuid,
    action: CurriculumAction,
) -> Result<AcademicResourceAccess, AppError> {
    let owning_organization_unit_id: Option<Uuid> =
        sqlx::query_scalar("SELECT owning_organization_unit_id FROM curricula WHERE id = $1")
            .bind(curriculum_id)
            .fetch_optional(pool)
            .await
            .map_err(|error| {
                tracing::error!(
                    reason = "academic_curriculum_owner_query_failed",
                    database_error = %error
                );
                AppError::InternalServerError("ไม่สามารถตรวจสอบเจ้าของหลักสูตรได้".to_string())
            })?
            .ok_or_else(|| AppError::NotFound("ไม่พบหลักสูตร".to_string()))?;

    let filter = academic_curriculum_list_access(pool, actor, action).await?;
    Ok(resource_access_policy::academic_resource_access_for(
        &filter,
        owning_organization_unit_id,
        false,
    ))
}

pub async fn require_academic_curriculum_list_access(
    pool: &PgPool,
    actor: &ActorContext,
    action: CurriculumAction,
) -> Result<AcademicResourceListFilter, AppError> {
    let filter = academic_curriculum_list_access(pool, actor, action).await?;
    if !filter.includes_school_owned
        && filter.organization_unit_ids.is_empty()
        && filter.organization_tree_unit_ids.is_empty()
        && filter.assigned_actor_id.is_none()
    {
        Err(AppError::Forbidden("ไม่มีสิทธิ์เข้าถึงหลักสูตร".to_string()))
    } else {
        Ok(filter)
    }
}

pub async fn require_academic_curriculum_access(
    pool: &PgPool,
    actor: &ActorContext,
    curriculum_id: Uuid,
    action: CurriculumAction,
) -> Result<(), AppError> {
    if academic_curriculum_access(pool, actor, curriculum_id, action).await?
        == AcademicResourceAccess::None
    {
        Err(AppError::Forbidden("ไม่มีสิทธิ์เข้าถึงทรัพยากรนี้".to_string()))
    } else {
        Ok(())
    }
}

fn curriculum_permissions(action: CurriculumAction) -> AcademicResourcePermissions {
    match action {
        CurriculumAction::Read => AcademicResourcePermissions {
            assigned: NO_PERMISSIONS,
            organization_unit: CURRICULUM_READ_UNIT_PERMISSIONS,
            organization_tree: CURRICULUM_READ_TREE_PERMISSIONS,
            school: CURRICULUM_READ_SCHOOL_PERMISSIONS,
        },
        CurriculumAction::Manage => AcademicResourcePermissions {
            assigned: NO_PERMISSIONS,
            organization_unit: CURRICULUM_MANAGE_UNIT_PERMISSIONS,
            organization_tree: CURRICULUM_MANAGE_TREE_PERMISSIONS,
            school: CURRICULUM_MANAGE_SCHOOL_PERMISSIONS,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::academic::cutover_test_support::{
        apply_migrations_through, seed_academic_cutover_fixture, CutoverFixture,
    };
    use crate::permissions::registry::codes;
    use crate::test_helpers::create_named_test_pool;

    const ACTOR_ID: &str = "50000000-0000-0000-0000-000000000002";
    const CURRICULUM_ID: &str = "30000000-0000-0000-0000-000000000001";

    fn actor(permissions: &[&str]) -> ActorContext {
        ActorContext {
            user_id: Uuid::parse_str(ACTOR_ID).unwrap(),
            permissions: permissions
                .iter()
                .map(|permission| permission.to_string())
                .collect(),
        }
    }

    #[tokio::test]
    async fn curriculum_policy_separates_read_manage_unit_and_tree_scope() {
        let pool = create_named_test_pool("academic_curriculum_access_policy").await;
        apply_migrations_through(&pool, 40).await.unwrap();
        seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
            .await
            .unwrap();
        apply_migrations_through(&pool, 43).await.unwrap();

        sqlx::raw_sql(
            r#"
            INSERT INTO organization_units (id, code, name, parent_unit_id, category, unit_type)
            VALUES (
                'c2000000-0000-0000-0000-000000000002',
                'FIXTURE-CURRICULUM-CHILD', 'หน่วยงานหลักสูตรลูกทดสอบ',
                'c5e06a47-ebf6-40f6-bbf9-59c509e842f2', 'academic', 'unit'
            );

            INSERT INTO organization_members (
                id, user_id, organization_unit_id, position_code, started_at
            ) VALUES (
                'c3000000-0000-0000-0000-000000000002',
                '50000000-0000-0000-0000-000000000002',
                'c5e06a47-ebf6-40f6-bbf9-59c509e842f2', 'head', '2020-01-01'
            );

            INSERT INTO organization_permission_grants (
                organization_unit_id, permission_id, created_by, position_code
            )
            SELECT 'c5e06a47-ebf6-40f6-bbf9-59c509e842f2', permission.id,
                   '50000000-0000-0000-0000-000000000002', 'head'
            FROM permissions permission
            WHERE permission.code IN (
                'academic_curriculum.read.organization_unit',
                'academic_curriculum.read.organization_tree',
                'academic_curriculum.manage.organization_unit',
                'academic_curriculum.manage.organization_tree'
            )
            ON CONFLICT DO NOTHING;

            UPDATE curricula
            SET owning_organization_unit_id = 'c5e06a47-ebf6-40f6-bbf9-59c509e842f2'
            WHERE id = '30000000-0000-0000-0000-000000000001';
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        let curriculum_id = Uuid::parse_str(CURRICULUM_ID).unwrap();
        let unit_reader = actor(&[codes::ACADEMIC_CURRICULUM_READ_ORGANIZATION_UNIT]);
        assert_eq!(
            academic_curriculum_access(&pool, &unit_reader, curriculum_id, CurriculumAction::Read,)
                .await
                .unwrap(),
            AcademicResourceAccess::OrganizationUnit
        );
        assert_eq!(
            academic_curriculum_access(
                &pool,
                &unit_reader,
                curriculum_id,
                CurriculumAction::Manage,
            )
            .await
            .unwrap(),
            AcademicResourceAccess::None
        );

        let unit_manager = actor(&[codes::ACADEMIC_CURRICULUM_MANAGE_ORGANIZATION_UNIT]);
        assert_eq!(
            academic_curriculum_access(
                &pool,
                &unit_manager,
                curriculum_id,
                CurriculumAction::Manage,
            )
            .await
            .unwrap(),
            AcademicResourceAccess::OrganizationUnit
        );

        sqlx::query(
            r#"UPDATE curricula
               SET owning_organization_unit_id = 'c2000000-0000-0000-0000-000000000002'
               WHERE id = $1"#,
        )
        .bind(curriculum_id)
        .execute(&pool)
        .await
        .unwrap();

        let tree_reader = actor(&[codes::ACADEMIC_CURRICULUM_READ_ORGANIZATION_TREE]);
        assert_eq!(
            academic_curriculum_access(&pool, &tree_reader, curriculum_id, CurriculumAction::Read,)
                .await
                .unwrap(),
            AcademicResourceAccess::OrganizationTree
        );
        assert_eq!(
            academic_curriculum_access(
                &pool,
                &tree_reader,
                curriculum_id,
                CurriculumAction::Manage,
            )
            .await
            .unwrap(),
            AcademicResourceAccess::None
        );

        let school_reader = actor(&[codes::ACADEMIC_CURRICULUM_READ_SCHOOL]);
        assert_eq!(
            academic_curriculum_access(
                &pool,
                &school_reader,
                curriculum_id,
                CurriculumAction::Read,
            )
            .await
            .unwrap(),
            AcademicResourceAccess::School
        );
        assert_eq!(
            academic_curriculum_access(
                &pool,
                &school_reader,
                curriculum_id,
                CurriculumAction::Manage,
            )
            .await
            .unwrap(),
            AcademicResourceAccess::None
        );

        let unrelated = actor(&[]);
        assert!(matches!(
            require_academic_curriculum_list_access(&pool, &unrelated, CurriculumAction::Read,)
                .await,
            Err(AppError::Forbidden(_))
        ));
        assert!(matches!(
            require_academic_curriculum_access(
                &pool,
                &unrelated,
                curriculum_id,
                CurriculumAction::Read,
            )
            .await,
            Err(AppError::Forbidden(_))
        ));
        require_academic_curriculum_list_access(&pool, &school_reader, CurriculumAction::Read)
            .await
            .unwrap();
        require_academic_curriculum_access(
            &pool,
            &school_reader,
            curriculum_id,
            CurriculumAction::Read,
        )
        .await
        .unwrap();
    }
}
