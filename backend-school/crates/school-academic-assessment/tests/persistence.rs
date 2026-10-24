use school_academic_assessment::assessment::models::AssessmentPlanListQuery;
use school_academic_assessment::assessment::services;
use school_academic_assessment::policy::assessment::{
    require_assessment_plan_access, require_assessment_plan_list_access, AssessmentAction,
};
use school_authorization::AcademicResourceListFilter;
use school_authorization::ActorContext;
use school_errors::AppError;
use school_permissions::registry::codes;
use school_test_db::cutover_test_support::{
    apply_migrations_through, record_passing_phase_a_reconciliation_marker,
    seed_academic_cutover_fixture, seed_learning_offering_policy_science_course, CutoverFixture,
};
use uuid::Uuid;

async fn populated_canonical_pool(name: &str) -> sqlx::PgPool {
    let pool = school_test_db::create_named_test_pool(name).await;
    apply_migrations_through(&pool, 40).await.unwrap();
    seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
        .await
        .unwrap();
    seed_learning_offering_policy_science_course(&pool)
        .await
        .unwrap();
    apply_migrations_through(&pool, 44).await.unwrap();
    record_passing_phase_a_reconciliation_marker(&pool)
        .await
        .unwrap();
    school_test_db::run_test_migrations(&pool).await;
    pool
}

fn list_query(academic_term_id: Uuid) -> AssessmentPlanListQuery {
    AssessmentPlanListQuery {
        academic_term_id,
        subject_id: None,
        instructor_id: None,
        ready: None,
        exam_arrangement: None,
    }
}

#[tokio::test]
async fn assessment_candidates_read_canonical_name_projection() {
    let pool = populated_canonical_pool("assessment_canonical_names").await;
    let (offering, term): (Uuid, Uuid) = sqlx::query_as(
        "SELECT learning_offering_id, academic_term_id FROM course_assessment_plans ORDER BY id LIMIT 1",
    ).fetch_one(&pool).await.unwrap();
    let primary = Uuid::parse_str("50000000-0000-0000-0000-000000000002").unwrap();
    let secondary = Uuid::parse_str("50000000-0000-0000-0000-000000000003").unwrap();
    sqlx::query(
        "UPDATE users SET title='นาย', first_name='สมชาย', last_name='ทดสอบ',
             title_en='Mr.', first_name_en='Somchai', last_name_en='Example' WHERE id=$1",
    )
    .bind(primary)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE users SET title=NULL, first_name='', last_name='',
             title_en='Ms.', first_name_en='Alice', last_name_en='Example' WHERE id=$1",
    )
    .bind(secondary)
    .execute(&pool)
    .await
    .unwrap();
    // Add a draft group without changing the immutable published assignments.
    sqlx::query(
        "WITH inserted_group AS (
             INSERT INTO learning_groups (
                 id,learning_offering_id,academic_term_id,academic_year_id,code,name,status,roster_status
             ) SELECT uuidv7(),id,academic_term_id,academic_year_id,
                      'NAME-PROJECTION','Name projection fixture','draft','draft'
               FROM learning_offerings WHERE id=$1
             RETURNING id,academic_term_id,academic_year_id
         ) INSERT INTO learning_group_teachers (
             id,learning_group_id,academic_term_id,academic_year_id,teacher_id,role,starts_on
         ) SELECT uuidv7(),inserted_group.id,inserted_group.academic_term_id,
                  inserted_group.academic_year_id,$2,'primary',term.start_date
           FROM inserted_group JOIN academic_terms term ON term.id=inserted_group.academic_term_id",
    ).bind(offering).bind(primary).execute(&pool).await.unwrap();
    sqlx::query(
        "UPDATE course_assessment_plans SET assessment_coordinator_id=$2 WHERE learning_offering_id=$1",
    ).bind(offering).bind(primary).execute(&pool).await.unwrap();
    let plans = services::list_assessment_plans(
        &pool,
        &list_query(term),
        &AcademicResourceListFilter {
            includes_school_owned: true,
            ..Default::default()
        },
        primary,
    )
    .await
    .unwrap();
    let summary = plans
        .iter()
        .find(|plan| plan.offering_id == offering)
        .unwrap();
    assert_eq!(summary.learning_group_count, 3);
    assert_eq!(
        summary.assessment_coordinator_name.as_deref(),
        Some("นายสมชาย ทดสอบ")
    );
    assert_eq!(summary.suggested_coordinator_id, None);
    assert_eq!(summary.suggested_coordinator_name, None);
    let detail = services::get_plan_detail(&pool, offering).await.unwrap();
    assert_eq!(detail.coordinator_candidates.len(), 2);
    let primary_option = detail
        .coordinator_candidates
        .iter()
        .find(|candidate| candidate.teacher_id == primary)
        .unwrap();
    assert_eq!(primary_option.display_name, "นายสมชาย ทดสอบ");
    assert_eq!(primary_option.learning_group_count, 2);
    assert_eq!(primary_option.primary_learning_group_count, 2);
    let secondary_option = detail
        .coordinator_candidates
        .iter()
        .find(|candidate| candidate.teacher_id == secondary)
        .unwrap();
    assert_eq!(secondary_option.display_name, "Ms. Alice Example");
    assert_eq!(secondary_option.learning_group_count, 1);
    assert_eq!(secondary_option.primary_learning_group_count, 1);
    assert_eq!(
        detail.suggested_coordinator_id,
        summary.suggested_coordinator_id
    );
    assert_eq!(
        detail.assessment_coordinator_name,
        summary.assessment_coordinator_name
    );
    assert_eq!(
        services::list_phase_controls(&pool, term)
            .await
            .unwrap()
            .len(),
        4
    );
}

fn actor(user_id: Uuid, permissions: &[&str]) -> ActorContext {
    ActorContext {
        user_id,
        permissions: permissions
            .iter()
            .map(|permission| permission.to_string())
            .collect(),
    }
}

#[tokio::test]
async fn assessment_canonical_lists_preserve_assigned_and_exact_learning_area_scopes() {
    let pool = populated_canonical_pool("assessment_canonical_scopes").await;
    let teacher = Uuid::parse_str("50000000-0000-0000-0000-000000000002").unwrap();
    let term: Uuid =
        sqlx::query_scalar("SELECT academic_term_id FROM course_assessment_plans LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    let (science, science_owner): (Uuid, Uuid) = sqlx::query_as(
        "SELECT id,owning_organization_unit_id FROM learning_offerings
         WHERE academic_term_id=$1 AND kind='course' AND code_snapshot='SCI-CORE'",
    )
    .bind(term)
    .fetch_one(&pool)
    .await
    .unwrap();
    let math: Uuid =
        sqlx::query_scalar("SELECT learning_offering_id FROM course_assessment_plans LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    let parent_code: String = sqlx::query_scalar(
        "SELECT parent.code FROM organization_units unit
         JOIN organization_units parent ON parent.id=unit.parent_unit_id WHERE unit.id=$1",
    )
    .bind(science_owner)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(parent_code, "SCHOOL");
    sqlx::query(
        "INSERT INTO organization_members(user_id,organization_unit_id,position_code,started_at)
         VALUES($1,$2,'member',CURRENT_DATE)",
    )
    .bind(teacher)
    .bind(science_owner)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO organization_permission_grants(organization_unit_id,permission_id,position_code,created_by)
         SELECT $1,id,'member',$2 FROM permissions WHERE code=$3",
    ).bind(science_owner).bind(teacher).bind(codes::ACADEMIC_ASSESSMENT_READ_ORGANIZATION_UNIT)
        .execute(&pool).await.unwrap();

    for (permissions, mut expected) in [
        (
            vec![codes::ACADEMIC_ASSESSMENT_READ_SCHOOL],
            vec![math, science],
        ),
        (vec![codes::ACADEMIC_ASSESSMENT_READ_ASSIGNED], vec![math]),
        (
            vec![codes::ACADEMIC_ASSESSMENT_READ_ORGANIZATION_UNIT],
            vec![science],
        ),
        (
            vec![
                codes::ACADEMIC_ASSESSMENT_READ_ASSIGNED,
                codes::ACADEMIC_ASSESSMENT_READ_ORGANIZATION_UNIT,
            ],
            vec![math, science],
        ),
    ] {
        let actor = actor(teacher, &permissions);
        let access = require_assessment_plan_list_access(&pool, &actor, AssessmentAction::Read)
            .await
            .unwrap();
        let mut actual =
            services::list_assessment_plans(&pool, &list_query(term), &access, teacher)
                .await
                .unwrap()
                .into_iter()
                .map(|plan| plan.offering_id)
                .collect::<Vec<_>>();
        actual.sort_unstable();
        expected.sort_unstable();
        assert_eq!(actual, expected, "permissions: {permissions:?}");
        for offering in expected {
            require_assessment_plan_access(&pool, &actor, offering, AssessmentAction::Read)
                .await
                .unwrap();
        }
    }
    let exact_actor = actor(
        teacher,
        &[codes::ACADEMIC_ASSESSMENT_READ_ORGANIZATION_UNIT],
    );
    assert!(matches!(
        require_assessment_plan_access(&pool, &exact_actor, math, AssessmentAction::Read).await,
        Err(AppError::Forbidden(_))
    ));
    let assigned_actor = actor(teacher, &[codes::ACADEMIC_ASSESSMENT_READ_ASSIGNED]);
    assert!(matches!(
        require_assessment_plan_access(&pool, &assigned_actor, science, AssessmentAction::Read)
            .await,
        Err(AppError::Forbidden(_))
    ));
    for permissions in [vec![], vec![codes::LEARNING_OFFERING_READ_ASSIGNED]] {
        let denied = actor(teacher, &permissions);
        assert!(matches!(
            require_assessment_plan_list_access(&pool, &denied, AssessmentAction::Read).await,
            Err(AppError::Forbidden(_))
        ));
        assert!(matches!(
            require_assessment_plan_access(&pool, &denied, math, AssessmentAction::Read).await,
            Err(AppError::Forbidden(_))
        ));
    }
}

#[tokio::test]
async fn assessment_phase_controls_read_through_the_assessment_crate_boundary() {
    let pool =
        school_test_db::create_named_test_pool("academic_assessment_crate_persistence").await;
    school_test_db::run_test_migrations(&pool).await;

    let controls = services::list_phase_controls(&pool, Uuid::new_v4())
        .await
        .unwrap();

    assert!(controls.is_empty());
    let relation: Option<String> =
        sqlx::query_scalar("SELECT to_regclass('academic_assessment_phase_controls')::text")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        relation.as_deref(),
        Some("academic_assessment_phase_controls")
    );
}

#[tokio::test]
async fn assessment_detail_and_summary_share_the_callers_single_connection() {
    let pool = populated_canonical_pool("assessment_borrowed_connection").await;
    let (offering, term): (Uuid, Uuid) = sqlx::query_as(
        "SELECT learning_offering_id,academic_term_id FROM course_assessment_plans ORDER BY id LIMIT 1",
    ).fetch_one(&pool).await.unwrap();
    let mut connection = pool.acquire().await.unwrap();
    let access = AcademicResourceListFilter {
        includes_school_owned: true,
        ..Default::default()
    };
    let actor = Uuid::from_u128(0x50000000000000000000000000000002);
    let plans = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        services::list_assessment_plans_on_connection(
            &mut connection,
            &list_query(term),
            &access,
            actor,
        ),
    )
    .await
    .unwrap()
    .unwrap();
    let detail = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        services::get_plan_detail_on_connection(&mut connection, offering),
    )
    .await
    .unwrap()
    .unwrap();
    let summary = plans
        .iter()
        .find(|plan| plan.offering_id == offering)
        .unwrap();
    assert_eq!(summary.plan_id, detail.id);
    assert_eq!(detail.offering_id, offering);
    assert_eq!(detail.academic_term_id, term);
    assert!(!detail.phases.is_empty());
    assert!(!detail.coordinator_candidates.is_empty());
}
