pub use school_academic_http::policies::learning_offering_access_policy::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::academic::cutover_test_support::{
        apply_migrations_through, apply_phase_b_runtime_migrations, seed_academic_cutover_fixture,
        seed_learning_offering_policy_science_course, CutoverFixture,
    };
    use crate::modules::academic::delivery::services::offerings;
    use school_academic_delivery::models::LearningOfferingQuery;
    use school_authorization::{AcademicResourceAccess, ActorContext};
    use school_errors::AppError;
    use school_permissions::registry::codes;
    use school_test_db::{create_named_test_pool, run_test_migrations};
    use uuid::Uuid;

    const ACTOR_ID: &str = "50000000-0000-0000-0000-000000000002";
    const ROOT_UNIT_ID: &str = "c5e06a47-ebf6-40f6-bbf9-59c509e842f2";
    const CHILD_UNIT_ID: &str = "2b8c8ef9-c752-4939-9615-9ffd2c7c93f1";

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
        seed_learning_offering_policy_science_course(&pool)
            .await
            .unwrap();
        apply_migrations_through(&pool, 41).await.unwrap();

        sqlx::raw_sql(
            r#"
            UPDATE organization_units
            SET parent_unit_id = 'c5e06a47-ebf6-40f6-bbf9-59c509e842f2'
            WHERE code = 'SUBJ-MA';

            INSERT INTO organization_members (
                id, user_id, organization_unit_id, position_code, started_at
            ) VALUES (
                'c3000000-0000-0000-0000-000000000003',
                '50000000-0000-0000-0000-000000000002',
                'c5e06a47-ebf6-40f6-bbf9-59c509e842f2', 'head', '2020-01-01'
            );

            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        apply_phase_b_runtime_migrations(&pool).await.unwrap();
        // Current runtime queries require the complete schema, not a historical cutoff.
        run_test_migrations(&pool).await;

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
			         '2b8c8ef9-c752-4939-9615-9ffd2c7c93f1'
			   ORDER BY id
			   LIMIT 1"#,
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        let school_offering_id: Uuid = sqlx::query_scalar(
            r#"SELECT offering.id FROM learning_offerings offering
               JOIN organization_units owner ON owner.id = offering.owning_organization_unit_id
               WHERE owner.code = 'SUBJ-SC' ORDER BY offering.id LIMIT 1"#,
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
        assert!(learning_offering_owner_allowed(
            &union_filter,
            Uuid::parse_str(ROOT_UNIT_ID).unwrap()
        ));
        assert!(learning_offering_owner_allowed(
            &union_filter,
            Uuid::parse_str(CHILD_UNIT_ID).unwrap()
        ));
        assert!(!learning_offering_owner_allowed(
            &union_filter,
            Uuid::new_v4()
        ));

        let assigned_filter =
            require_learning_offering_list_access(&pool, &assigned_reader, OfferingAction::Read)
                .await
                .unwrap();
        let assigned_term_id: Uuid =
            sqlx::query_scalar("SELECT academic_term_id FROM learning_offerings WHERE id = $1")
                .bind(assigned_offering_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        let assigned_values = offerings::list(
            &pool,
            LearningOfferingQuery {
                academic_term_id: assigned_term_id,
            },
            &assigned_filter,
        )
        .await
        .unwrap();
        assert!(assigned_values
            .iter()
            .any(|offering| offering.id == assigned_offering_id));

        let denied = actor(&[]);
        assert!(matches!(
            require_learning_offering_list_access(&pool, &denied, OfferingAction::Read).await,
            Err(AppError::Forbidden(_))
        ));
        assert!(require_learning_offering_access(
            &pool,
            &assigned_manager,
            assigned_offering_id,
            OfferingAction::Manage,
        )
        .await
        .is_ok());

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

        let unassigned_offering_id = school_offering_id;
        assert!(matches!(
            require_learning_offering_batch_access(
                &pool,
                &assigned_manager,
                &[assigned_offering_id, unassigned_offering_id],
                OfferingAction::Manage,
            )
            .await,
            Err(AppError::Forbidden(_))
        ));
        let school_manager = actor(&[codes::LEARNING_OFFERING_MANAGE_SCHOOL]);
        require_learning_offering_batch_access(
            &pool,
            &school_manager,
            &[assigned_offering_id, unassigned_offering_id],
            OfferingAction::Manage,
        )
        .await
        .expect("school management scope must authorize every affected offering together");
    }
}
