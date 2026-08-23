use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::middleware::permission::ActorContext;
use crate::permissions::registry::codes;
use crate::policies::resource_access_policy::{
    self, AcademicResourceAccess, AcademicResourceListFilter, AcademicResourcePermissions,
};

const OFFERING_READ_ASSIGNED_PERMISSIONS: &[&str] = &[
    codes::LEARNING_OFFERING_READ_ASSIGNED,
    codes::LEARNING_OFFERING_MANAGE_ASSIGNED,
];
const OFFERING_READ_UNIT_PERMISSIONS: &[&str] = &[
    codes::LEARNING_OFFERING_READ_ORGANIZATION_UNIT,
    codes::LEARNING_OFFERING_MANAGE_ORGANIZATION_UNIT,
];
const OFFERING_READ_TREE_PERMISSIONS: &[&str] = &[
    codes::LEARNING_OFFERING_READ_ORGANIZATION_TREE,
    codes::LEARNING_OFFERING_MANAGE_ORGANIZATION_TREE,
];
const OFFERING_READ_SCHOOL_PERMISSIONS: &[&str] = &[
    codes::LEARNING_OFFERING_READ_SCHOOL,
    codes::LEARNING_OFFERING_MANAGE_SCHOOL,
];
const OFFERING_MANAGE_ASSIGNED_PERMISSIONS: &[&str] = &[codes::LEARNING_OFFERING_MANAGE_ASSIGNED];
const OFFERING_MANAGE_UNIT_PERMISSIONS: &[&str] =
    &[codes::LEARNING_OFFERING_MANAGE_ORGANIZATION_UNIT];
const OFFERING_MANAGE_TREE_PERMISSIONS: &[&str] =
    &[codes::LEARNING_OFFERING_MANAGE_ORGANIZATION_TREE];
const OFFERING_MANAGE_SCHOOL_PERMISSIONS: &[&str] = &[codes::LEARNING_OFFERING_MANAGE_SCHOOL];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OfferingAction {
    Read,
    Manage,
}

pub async fn learning_offering_list_access(
    pool: &PgPool,
    actor: &ActorContext,
    action: OfferingAction,
) -> Result<AcademicResourceListFilter, AppError> {
    resource_access_policy::resolve_academic_resource_list_filter(
        pool,
        actor,
        offering_permissions(action),
    )
    .await
}

pub async fn learning_offering_access(
    pool: &PgPool,
    actor: &ActorContext,
    offering_id: Uuid,
    action: OfferingAction,
) -> Result<AcademicResourceAccess, AppError> {
    let target: Option<(Option<Uuid>, bool)> = sqlx::query_as(
        r#"
        SELECT offering.owning_organization_unit_id,
               EXISTS (
                   SELECT 1
                   FROM learning_groups learning_group
                   JOIN learning_group_teachers teacher
                     ON teacher.learning_group_id = learning_group.id
                   WHERE learning_group.learning_offering_id = offering.id
                     AND teacher.teacher_id = $2
               ) AS actor_is_assigned
        FROM learning_offerings offering
        WHERE offering.id = $1
        "#,
    )
    .bind(offering_id)
    .bind(actor.user_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| {
        tracing::error!(
            reason = "learning_offering_access_target_query_failed",
            database_error = %error
        );
        AppError::InternalServerError("ไม่สามารถตรวจสอบสิทธิ์การเปิดสอนได้".to_string())
    })?;

    let Some((owning_organization_unit_id, actor_is_assigned)) = target else {
        return Err(AppError::NotFound("ไม่พบการเปิดสอน".to_string()));
    };

    let filter = learning_offering_list_access(pool, actor, action).await?;
    Ok(resource_access_policy::academic_resource_access_for(
        &filter,
        owning_organization_unit_id,
        actor_is_assigned,
    ))
}

fn offering_permissions(action: OfferingAction) -> AcademicResourcePermissions {
    match action {
        OfferingAction::Read => AcademicResourcePermissions {
            assigned: OFFERING_READ_ASSIGNED_PERMISSIONS,
            organization_unit: OFFERING_READ_UNIT_PERMISSIONS,
            organization_tree: OFFERING_READ_TREE_PERMISSIONS,
            school: OFFERING_READ_SCHOOL_PERMISSIONS,
        },
        OfferingAction::Manage => AcademicResourcePermissions {
            assigned: OFFERING_MANAGE_ASSIGNED_PERMISSIONS,
            organization_unit: OFFERING_MANAGE_UNIT_PERMISSIONS,
            organization_tree: OFFERING_MANAGE_TREE_PERMISSIONS,
            school: OFFERING_MANAGE_SCHOOL_PERMISSIONS,
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
    const ROOT_UNIT_ID: &str = "c5e06a47-ebf6-40f6-bbf9-59c509e842f2";
    const CHILD_UNIT_ID: &str = "c2000000-0000-0000-0000-000000000003";

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
    async fn offering_policy_unions_assignment_unit_and_tree_without_expanding_school_access() {
        let pool = create_named_test_pool("learning_offering_access_policy").await;
        apply_migrations_through(&pool, 40).await.unwrap();
        seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
            .await
            .unwrap();
        apply_migrations_through(&pool, 41).await.unwrap();

        sqlx::raw_sql(
            r#"
            INSERT INTO organization_units (id, code, name, parent_unit_id, category, unit_type)
            VALUES (
                'c2000000-0000-0000-0000-000000000003',
                'FIXTURE-OFFERING-CHILD', 'หน่วยงานการเปิดสอนลูกทดสอบ',
                'c5e06a47-ebf6-40f6-bbf9-59c509e842f2', 'academic', 'unit'
            );

            INSERT INTO organization_members (
                id, user_id, organization_unit_id, position_code, started_at
            ) VALUES (
                'c3000000-0000-0000-0000-000000000003',
                '50000000-0000-0000-0000-000000000002',
                'c5e06a47-ebf6-40f6-bbf9-59c509e842f2', 'head', '2020-01-01'
            );

            UPDATE subjects
            SET owning_organization_unit_id =
                'c5e06a47-ebf6-40f6-bbf9-59c509e842f2'
            WHERE code = 'MATH-CORE';

            UPDATE activities
            SET owning_organization_unit_id =
				CASE WHEN id = (SELECT selected.id FROM activities selected ORDER BY selected.id LIMIT 1)
                     THEN 'c2000000-0000-0000-0000-000000000003'::uuid
                     ELSE NULL
                END;
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        apply_migrations_through(&pool, 43).await.unwrap();

        sqlx::raw_sql(
            r#"
            INSERT INTO organization_permission_grants (
                organization_unit_id, permission_id, created_by, position_code
            )
            SELECT 'c5e06a47-ebf6-40f6-bbf9-59c509e842f2', permission.id,
                   '50000000-0000-0000-0000-000000000002', 'head'
            FROM permissions permission
            WHERE permission.code IN (
                'learning_offering.read.organization_unit',
                'learning_offering.read.organization_tree',
                'learning_offering.manage.organization_unit',
                'learning_offering.manage.organization_tree'
            )
            ON CONFLICT DO NOTHING;
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        let assigned_offering_id: Uuid = sqlx::query_scalar(
            r#"SELECT offering.id
               FROM learning_offerings offering
               JOIN learning_groups learning_group
                 ON learning_group.learning_offering_id = offering.id
               JOIN learning_group_teachers teacher
                 ON teacher.learning_group_id = learning_group.id
               WHERE teacher.teacher_id = $1
				 AND offering.owning_organization_unit_id =
				     'c5e06a47-ebf6-40f6-bbf9-59c509e842f2'
               ORDER BY offering.id
               LIMIT 1"#,
        )
        .bind(Uuid::parse_str(ACTOR_ID).unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
        let child_offering_id: Uuid = sqlx::query_scalar(
            r#"SELECT id
			   FROM learning_offerings
			   WHERE owning_organization_unit_id =
			         'c2000000-0000-0000-0000-000000000003'
			   ORDER BY id
			   LIMIT 1"#,
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        let school_offering_id: Uuid = sqlx::query_scalar(
            "SELECT id FROM learning_offerings WHERE owning_organization_unit_id IS NULL ORDER BY id LIMIT 1",
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        let assigned_reader = actor(&[codes::LEARNING_OFFERING_READ_ASSIGNED]);
        assert_eq!(
            learning_offering_access(
                &pool,
                &assigned_reader,
                assigned_offering_id,
                OfferingAction::Read,
            )
            .await
            .unwrap(),
            AcademicResourceAccess::Assigned
        );
        assert_eq!(
            learning_offering_access(
                &pool,
                &assigned_reader,
                assigned_offering_id,
                OfferingAction::Manage,
            )
            .await
            .unwrap(),
            AcademicResourceAccess::None
        );

        let assigned_manager = actor(&[codes::LEARNING_OFFERING_MANAGE_ASSIGNED]);
        assert_eq!(
            learning_offering_access(
                &pool,
                &assigned_manager,
                assigned_offering_id,
                OfferingAction::Manage,
            )
            .await
            .unwrap(),
            AcademicResourceAccess::Assigned
        );

        let tree_reader = actor(&[codes::LEARNING_OFFERING_READ_ORGANIZATION_TREE]);
        assert_eq!(
            learning_offering_access(&pool, &tree_reader, child_offering_id, OfferingAction::Read,)
                .await
                .unwrap(),
            AcademicResourceAccess::OrganizationTree
        );
        assert_eq!(
            learning_offering_access(
                &pool,
                &tree_reader,
                school_offering_id,
                OfferingAction::Read,
            )
            .await
            .unwrap(),
            AcademicResourceAccess::None
        );

        let union_actor = actor(&[
            codes::LEARNING_OFFERING_READ_ASSIGNED,
            codes::LEARNING_OFFERING_READ_ORGANIZATION_UNIT,
            codes::LEARNING_OFFERING_READ_ORGANIZATION_TREE,
        ]);
        let union_filter = learning_offering_list_access(&pool, &union_actor, OfferingAction::Read)
            .await
            .unwrap();
        assert_eq!(union_filter.assigned_actor_id, Some(union_actor.user_id));
        assert_eq!(
            union_filter.organization_unit_ids,
            vec![Uuid::parse_str(ROOT_UNIT_ID).unwrap()]
        );
        assert!(union_filter
            .organization_tree_unit_ids
            .contains(&Uuid::parse_str(ROOT_UNIT_ID).unwrap()));
        assert!(union_filter
            .organization_tree_unit_ids
            .contains(&Uuid::parse_str(CHILD_UNIT_ID).unwrap()));
        assert!(!union_filter.includes_school_owned);

        let school_reader = actor(&[codes::LEARNING_OFFERING_READ_SCHOOL]);
        assert_eq!(
            learning_offering_access(
                &pool,
                &school_reader,
                school_offering_id,
                OfferingAction::Read,
            )
            .await
            .unwrap(),
            AcademicResourceAccess::School
        );
    }
}
