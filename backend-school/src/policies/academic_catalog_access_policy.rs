pub use school_academic_http::policies::academic_catalog_access_policy::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::academic::cutover_test_support::{
        apply_migrations_through, seed_academic_cutover_fixture, CutoverFixture,
    };
    use school_authorization::{AcademicResourceAccess, ActorContext};
    use school_errors::AppError;
    use school_permissions::registry::codes;
    use school_test_db::create_named_test_pool;
    use uuid::Uuid;

    const ACTOR_ID: &str = "50000000-0000-0000-0000-000000000002";
    const ROOT_UNIT_ID: &str = "c5e06a47-ebf6-40f6-bbf9-59c509e842f2";
    const CHILD_UNIT_ID: &str = "c2000000-0000-0000-0000-000000000001";

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
    async fn catalog_policy_enforces_school_unit_tree_and_action_boundaries() {
        let pool = create_named_test_pool("academic_catalog_access_policy").await;
        apply_migrations_through(&pool, 40).await.unwrap();
        seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
            .await
            .unwrap();
        apply_migrations_through(&pool, 43).await.unwrap();

        sqlx::raw_sql(
            r#"
            INSERT INTO organization_units (id, code, name, parent_unit_id, category, unit_type)
            VALUES (
                'c2000000-0000-0000-0000-000000000001',
                'FIXTURE-CHILD', 'หน่วยงานลูกทดสอบ',
                'c5e06a47-ebf6-40f6-bbf9-59c509e842f2', 'academic', 'unit'
            );

            INSERT INTO organization_members (
                id, user_id, organization_unit_id, position_code, started_at
            ) VALUES (
                'c3000000-0000-0000-0000-000000000001',
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
                'academic_catalog.manage.organization_unit',
                'academic_catalog.manage.organization_tree'
            )
            ON CONFLICT DO NOTHING;

            UPDATE subjects
            SET owning_organization_unit_id =
                CASE WHEN code = 'MATH-CORE'
                     THEN 'c5e06a47-ebf6-40f6-bbf9-59c509e842f2'::uuid
                     ELSE 'c2000000-0000-0000-0000-000000000001'::uuid
                END;
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        let root_subject_id: Uuid =
            sqlx::query_scalar("SELECT id FROM subjects WHERE code = 'MATH-CORE'")
                .fetch_one(&pool)
                .await
                .unwrap();
        let child_subject_id: Uuid =
            sqlx::query_scalar("SELECT id FROM subjects WHERE code = 'SCI-CORE'")
                .fetch_one(&pool)
                .await
                .unwrap();
        let school_activity_id: Uuid =
            sqlx::query_scalar("SELECT id FROM activities ORDER BY id LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();

        let school_reader = actor(&[codes::ACADEMIC_CATALOG_READ_SCHOOL]);
        assert_eq!(
            academic_catalog_access(
                &pool,
                &school_reader,
                CatalogResourceRef::Subject(root_subject_id),
                CatalogAction::Read,
            )
            .await
            .unwrap(),
            AcademicResourceAccess::School
        );
        assert_eq!(
            academic_catalog_access(
                &pool,
                &school_reader,
                CatalogResourceRef::Activity(school_activity_id),
                CatalogAction::Read,
            )
            .await
            .unwrap(),
            AcademicResourceAccess::School
        );
        assert_eq!(
            academic_catalog_access(
                &pool,
                &school_reader,
                CatalogResourceRef::Subject(root_subject_id),
                CatalogAction::Manage,
            )
            .await
            .unwrap(),
            AcademicResourceAccess::None
        );

        let unit_manager = actor(&[codes::ACADEMIC_CATALOG_MANAGE_ORGANIZATION_UNIT]);
        assert_eq!(
            academic_catalog_access(
                &pool,
                &unit_manager,
                CatalogResourceRef::Subject(root_subject_id),
                CatalogAction::Manage,
            )
            .await
            .unwrap(),
            AcademicResourceAccess::OrganizationUnit
        );
        assert_eq!(
            academic_catalog_access(
                &pool,
                &unit_manager,
                CatalogResourceRef::Subject(child_subject_id),
                CatalogAction::Read,
            )
            .await
            .unwrap(),
            AcademicResourceAccess::None
        );
        assert_eq!(
            academic_catalog_access(
                &pool,
                &unit_manager,
                CatalogResourceRef::Activity(school_activity_id),
                CatalogAction::Read,
            )
            .await
            .unwrap(),
            AcademicResourceAccess::None
        );

        let tree_manager = actor(&[codes::ACADEMIC_CATALOG_MANAGE_ORGANIZATION_TREE]);
        assert_eq!(
            academic_catalog_access(
                &pool,
                &tree_manager,
                CatalogResourceRef::Subject(child_subject_id),
                CatalogAction::Manage,
            )
            .await
            .unwrap(),
            AcademicResourceAccess::OrganizationTree
        );

        let unrelated = actor(&[]);
        assert_eq!(
            academic_catalog_access(
                &pool,
                &unrelated,
                CatalogResourceRef::Subject(root_subject_id),
                CatalogAction::Read,
            )
            .await
            .unwrap(),
            AcademicResourceAccess::None
        );
        assert!(matches!(
            require_academic_catalog_list_access(&pool, &unrelated, CatalogAction::Read).await,
            Err(AppError::Forbidden(_))
        ));
        assert!(matches!(
            require_academic_catalog_access(
                &pool,
                &unrelated,
                CatalogResourceRef::Subject(root_subject_id),
                CatalogAction::Read,
            )
            .await,
            Err(AppError::Forbidden(_))
        ));
        require_academic_catalog_list_access(&pool, &school_reader, CatalogAction::Read)
            .await
            .unwrap();
        require_academic_catalog_access(
            &pool,
            &school_reader,
            CatalogResourceRef::Subject(root_subject_id),
            CatalogAction::Read,
        )
        .await
        .unwrap();

        assert_eq!(
            unit_manager.user_id,
            Uuid::parse_str(ACTOR_ID).unwrap(),
            "fixture actor must remain stable"
        );
        assert_eq!(
            Uuid::parse_str(ROOT_UNIT_ID).unwrap().to_string(),
            ROOT_UNIT_ID
        );
        assert_eq!(
            Uuid::parse_str(CHILD_UNIT_ID).unwrap().to_string(),
            CHILD_UNIT_ID
        );
    }
}
