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

async fn assert_assessment_assignment_scope(
    pool: &sqlx::PgPool,
    term: Uuid,
    offering: Uuid,
    teacher: Uuid,
    expected: bool,
) {
    let assigned = actor(teacher, &[codes::ACADEMIC_ASSESSMENT_READ_ASSIGNED]);
    let access = require_assessment_plan_list_access(pool, &assigned, AssessmentAction::Read)
        .await
        .unwrap();
    let plans = services::list_assessment_plans(pool, &list_query(term), &access, teacher)
        .await
        .unwrap();
    assert_eq!(
        plans.iter().any(|plan| plan.offering_id == offering),
        expected
    );
    let detail_access =
        require_assessment_plan_access(pool, &assigned, offering, AssessmentAction::Read).await;
    if expected {
        detail_access.unwrap();
    } else {
        assert!(matches!(detail_access, Err(AppError::Forbidden(_))));
    }
    let mut instructor_query = list_query(term);
    instructor_query.instructor_id = Some(teacher);
    let school = AcademicResourceListFilter {
        includes_school_owned: true,
        ..Default::default()
    };
    let filtered = services::list_assessment_plans(pool, &instructor_query, &school, teacher)
        .await
        .unwrap();
    assert_eq!(
        filtered.iter().any(|plan| plan.offering_id == offering),
        expected
    );
}

#[tokio::test]
async fn assessment_coordinator_uses_latest_published_allocation_after_teaching_ended() {
    let pool = populated_canonical_pool("assessment_allocation_history").await;
    let primary = Uuid::from_u128(0x50000000000000000000000000000002);
    let source: Uuid = sqlx::query_scalar(
        "SELECT learning_offering_id FROM course_assessment_plans ORDER BY id LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let term = Uuid::now_v7();
    let offering = Uuid::now_v7();
    let group = Uuid::now_v7();
    let plan = Uuid::now_v7();
    let mut transaction = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO academic_terms(id,academic_year_id,name,sequence_no,code,term_type,included_in_year_result,blocks_year_closure,status,bell_schedule_id) SELECT $1,offering.academic_year_id,'Allocation history',1000,'ALLOC-HISTORY','custom',false,false,'planning',term.bell_schedule_id FROM learning_offerings offering JOIN academic_terms term ON term.id=offering.academic_term_id WHERE offering.id=$2")
        .bind(term).bind(source).execute(&mut *transaction).await.unwrap();
    sqlx::query("INSERT INTO learning_offerings(id,academic_term_id,academic_year_id,kind,code_snapshot,name_snapshot,status,starts_on,owning_organization_unit_id) SELECT $1,$2,academic_year_id,'course','ALLOC-HISTORY','Allocation history','draft',current_date-100,owning_organization_unit_id FROM learning_offerings WHERE id=$3")
        .bind(offering).bind(term).bind(source).execute(&mut *transaction).await.unwrap();
    sqlx::query("INSERT INTO course_offering_details(learning_offering_id,academic_term_id,academic_year_id,subject_version_id,subject_id,credit,hours,assessment_total_score) SELECT $1,$2,academic_year_id,subject_version_id,subject_id,credit,hours,assessment_total_score FROM course_offering_details WHERE learning_offering_id=$3")
        .bind(offering).bind(term).bind(source).execute(&mut *transaction).await.unwrap();
    sqlx::query("INSERT INTO learning_groups(id,learning_offering_id,academic_term_id,academic_year_id,code,name,status,roster_status) SELECT $1,id,academic_term_id,academic_year_id,'ALLOC-HISTORY','Allocation history','draft','draft' FROM learning_offerings WHERE id=$2")
        .bind(group).bind(offering).execute(&mut *transaction).await.unwrap();
    sqlx::query("INSERT INTO learning_group_teachers(id,learning_group_id,academic_term_id,academic_year_id,teacher_id,role,starts_on,ends_on) SELECT uuidv7(),id,academic_term_id,academic_year_id,$2,'primary',current_date-100,current_date-80 FROM learning_groups WHERE id=$1")
        .bind(group).bind(primary).execute(&mut *transaction).await.unwrap();
    sqlx::query("UPDATE learning_groups SET status='published' WHERE id=$1")
        .bind(group)
        .execute(&mut *transaction)
        .await
        .unwrap();
    sqlx::query("UPDATE learning_offerings SET status='published',published_at=now() WHERE id=$1")
        .bind(offering)
        .execute(&mut *transaction)
        .await
        .unwrap();
    sqlx::query("INSERT INTO course_assessment_plans(id,academic_term_id,academic_year_id,subject_version_id,learning_offering_id,assessment_coordinator_id) SELECT $1,academic_term_id,academic_year_id,subject_version_id,learning_offering_id,$3 FROM course_offering_details WHERE learning_offering_id=$2")
        .bind(plan).bind(offering).bind(primary).execute(&mut *transaction).await.unwrap();
    transaction.commit().await.unwrap();
    // The saved owner is retained, but no graph means no assigned/instructor
    // authority even when an actual teaching episode exists in the same context.
    let unknown = services::get_plan_detail(&pool, offering).await.unwrap();
    assert_eq!(unknown.assessment_coordinator_id, Some(primary));
    assert!(unknown.coordinator_candidates.is_empty());
    assert_assessment_assignment_scope(&pool, term, offering, primary, false).await;
    sqlx::query("INSERT INTO academic_delivery_versions(id,academic_term_id,academic_year_id,effective_from,status,snapshot,published_by,published_at,publication_idempotency_key,publication_request_hash) SELECT uuidv7(),academic_term_id,academic_year_id,current_date-90,'published',academic_capture_delivery_snapshot(academic_term_id,current_date-90,jsonb_build_array(jsonb_build_object('id',id,'weekly_period_target',1))),$2,now(),uuidv7(),repeat('a',64) FROM learning_offerings WHERE id=$1")
        .bind(offering).bind(primary).execute(&pool).await.unwrap();
    let detail = services::get_plan_detail(&pool, offering).await.unwrap();
    assert_eq!(detail.assessment_coordinator_id, Some(primary));
    assert!(
        detail
            .coordinator_candidates
            .iter()
            .any(|candidate| candidate.teacher_id == primary),
        "the latest published allocation retains its teacher after the dated episode ends"
    );
    assert_assessment_assignment_scope(&pool, term, offering, primary, true).await;

    let live_only = Uuid::from_u128(0x50000000000000000000000000000003);
    sqlx::query("UPDATE learning_groups SET status='draft' WHERE id=$1")
        .bind(group)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO learning_group_teachers(id,learning_group_id,academic_term_id,academic_year_id,teacher_id,role,starts_on) SELECT uuidv7(),id,academic_term_id,academic_year_id,$2,'secondary',current_date FROM learning_groups WHERE id=$1")
        .bind(group).bind(live_only).execute(&pool).await.unwrap();
    assert_assessment_assignment_scope(&pool, term, offering, live_only, false).await;
    sqlx::query("UPDATE users SET status='inactive' WHERE id=$1")
        .bind(primary)
        .execute(&pool)
        .await
        .unwrap();
    assert_assessment_assignment_scope(&pool, term, offering, primary, false).await;
    sqlx::query("UPDATE users SET status='active' WHERE id=$1")
        .bind(primary)
        .execute(&pool)
        .await
        .unwrap();
    assert_assessment_assignment_scope(&pool, term, offering, primary, true).await;

    sqlx::query("INSERT INTO academic_delivery_versions(id,academic_term_id,academic_year_id,effective_from,status,snapshot,published_by,published_at,publication_idempotency_key,publication_request_hash) SELECT uuidv7(),academic_term_id,academic_year_id,current_date,'published',academic_capture_delivery_snapshot(academic_term_id,current_date,'[]'::jsonb),$2,now(),uuidv7(),repeat('b',64) FROM learning_offerings WHERE id=$1")
        .bind(offering).bind(primary).execute(&pool).await.unwrap();
    assert_assessment_assignment_scope(&pool, term, offering, primary, false).await;
    assert_assessment_assignment_scope(&pool, term, offering, live_only, false).await;
    let omitted = services::get_plan_detail(&pool, offering).await.unwrap();
    assert_eq!(omitted.assessment_coordinator_id, Some(primary));
    assert!(omitted.coordinator_candidates.is_empty());
}

#[tokio::test]
async fn assessment_candidates_read_canonical_name_projection() {
    let pool = populated_canonical_pool("assessment_canonical_names").await;
    // Migration 146 can add another subject's plan ahead of the retained Math UUID.
    let mut math_plans: Vec<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT plan.learning_offering_id,plan.academic_term_id
         FROM course_assessment_plans plan
         JOIN course_offering_details detail ON detail.learning_offering_id=plan.learning_offering_id
         JOIN subjects subject ON subject.id=detail.subject_id
         JOIN academic_terms term ON term.id=plan.academic_term_id
         WHERE subject.code='MATH-CORE' AND term.status='active'",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(math_plans.len(), 1);
    let (offering, term) = math_plans.pop().unwrap();
    let primary = Uuid::parse_str("50000000-0000-0000-0000-000000000002").unwrap();
    let secondary = Uuid::parse_str("50000000-0000-0000-0000-000000000003").unwrap();
    assert_eq!(sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM learning_groups WHERE learning_offering_id=$1 AND status<>'closed'",
    ).bind(offering).fetch_one(&pool).await.unwrap(), 2);
    let assignments: Vec<(String, Uuid, String)> = sqlx::query_as(
        "SELECT learning_group.status,teacher.teacher_id,teacher.role
         FROM learning_groups learning_group
         JOIN learning_group_teachers teacher ON teacher.learning_group_id=learning_group.id
         WHERE learning_group.learning_offering_id=$1 AND learning_group.status<>'closed'
           AND teacher.starts_on<=(now() AT TIME ZONE 'Asia/Bangkok')::date
           AND (teacher.ends_on IS NULL OR teacher.ends_on>=(now() AT TIME ZONE 'Asia/Bangkok')::date)
         ORDER BY teacher.teacher_id",
    )
    .bind(offering)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        assignments,
        vec![
            ("published".into(), primary, "primary".into()),
            ("published".into(), secondary, "primary".into())
        ]
    );
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
    let today: chrono::NaiveDate =
        sqlx::query_scalar("SELECT (now() AT TIME ZONE 'Asia/Bangkok')::date")
            .fetch_one(&pool)
            .await
            .unwrap();
    let draft_group: Uuid = sqlx::query_scalar(
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
                  inserted_group.academic_year_id,$2,'primary',$3
           FROM inserted_group RETURNING learning_group_id",
    ).bind(offering).bind(primary).bind(today)
            .fetch_one(&pool).await.unwrap();
    let draft: (String, chrono::NaiveDate) = sqlx::query_as(
        "SELECT learning_group.status,teacher.starts_on FROM learning_groups learning_group
         JOIN learning_group_teachers teacher ON teacher.learning_group_id=learning_group.id
         WHERE learning_group.id=$1 AND teacher.teacher_id=$2",
    )
    .bind(draft_group)
    .bind(primary)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(draft, ("draft".into(), today));
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
    // The new draft group remains in the denominator, but not the published allocation.
    assert_eq!(primary_option.learning_group_count, 1);
    assert_eq!(primary_option.primary_learning_group_count, 1);
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
        "SELECT DISTINCT plan.learning_offering_id,plan.academic_term_id FROM course_assessment_plans plan JOIN academic_delivery_latest_published_teacher_allocations allocation ON allocation.learning_offering_id=plan.learning_offering_id ORDER BY plan.learning_offering_id LIMIT 1",
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
