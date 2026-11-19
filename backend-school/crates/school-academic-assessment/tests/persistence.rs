use school_academic_assessment::assessment::models::AssessmentPlanListQuery;
use school_academic_assessment::assessment::services;
use school_academic_assessment::policy::assessment::{
    require_assessment_plan_access, require_assessment_plan_list_access, AssessmentAction,
};
use school_authorization::AcademicResourceListFilter;
use school_authorization::ActorContext;
use school_errors::AppError;
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

async fn retained_math_context(pool: &sqlx::PgPool) -> (Uuid, Uuid) {
    let rows:Vec<(Uuid,Uuid)>=sqlx::query_as("SELECT detail.learning_offering_id,detail.academic_term_id FROM course_offering_details detail JOIN subjects subject ON subject.id=detail.subject_id JOIN academic_terms term ON term.id=detail.academic_term_id WHERE subject.code='MATH-CORE' AND term.status='active'").fetch_all(pool).await.unwrap();
    assert_eq!(
        rows.len(),
        1,
        "select the retained Math context explicitly, independent of other migrated plans"
    );
    rows[0]
}
async fn assign_current_offerings(
    pool: &sqlx::PgPool,
    term: Uuid,
    teacher: Uuid,
    offerings: &[Uuid],
) {
    let count=sqlx::query("WITH groups AS (INSERT INTO learning_groups(id,learning_offering_id,academic_term_id,academic_year_id,code,name) SELECT uuidv7(),id,academic_term_id,academic_year_id,'ASSESS-'||id::text,'Current assessment group' FROM learning_offerings WHERE academic_term_id=$1 AND id=ANY($3) RETURNING id,academic_term_id,academic_year_id) INSERT INTO learning_group_teachers(id,learning_group_id,academic_term_id,academic_year_id,teacher_id,role) SELECT uuidv7(),id,academic_term_id,academic_year_id,$2,'primary' FROM groups").bind(term).bind(teacher).bind(offerings).execute(pool).await.unwrap().rows_affected();
    assert_eq!(count, offerings.len() as u64);
}
#[tokio::test]
async fn assessment_list_uses_exact_current_registry_and_filters_before_limit() {
    let pool = populated_canonical_pool("assessment_list_current_source").await;
    let teacher = Uuid::from_u128(0x50000000000000000000000000000002);
    let other = Uuid::from_u128(0x50000000000000000000000000000003);
    let (source, _) = retained_math_context(&pool).await;
    let retained: String = sqlx::query_scalar(
        "SELECT jsonb_agg(to_jsonb(plan) ORDER BY id)::text FROM course_assessment_plans plan",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let term = Uuid::now_v7();
    sqlx::query("INSERT INTO academic_terms(school_days,id,academic_year_id,name,sequence_no,code,term_type,included_in_year_result,blocks_year_closure,status,bell_schedule_id) SELECT 'MON,TUE,WED,THU,FRI',$1,offering.academic_year_id,'Assessment list',1001,'ASSESS-LIST','custom',false,false,'planning',term.bell_schedule_id FROM learning_offerings offering JOIN academic_terms term ON term.id=offering.academic_term_id WHERE offering.id=$2").bind(term).bind(source).execute(&pool).await.unwrap();
    // Current registry membership replaced publication selection in migration157.
    // Both assigned targets sort beyond500 unrelated current courses.
    let offerings:Vec<(Uuid,i32)>=sqlx::query_as("WITH pins AS (INSERT INTO subject_versions SELECT (jsonb_populate_record(NULL::subject_versions,to_jsonb(version)||jsonb_build_object('id',uuidv7(),'version_no',(SELECT max(version_no) FROM subject_versions WHERE subject_id=version.subject_id)+n,'name_th','Assessment pin '||n,'effective_from',current_date-100))).* FROM course_offering_details detail JOIN subject_versions version ON version.id=detail.subject_version_id,generate_series(1,504) n WHERE detail.learning_offering_id=$2 RETURNING id,subject_id,version_no,credit), inserted AS (INSERT INTO learning_offerings(id,academic_term_id,academic_year_id,kind,code_snapshot,name_snapshot,weekly_period_target,owning_organization_unit_id) SELECT pin.id,$1,source.academic_year_id,'course',CASE WHEN pin.version_no=(SELECT max(version_no) FROM pins) THEN 'ZZ-CURRENT' ELSE 'AA-'||lpad(pin.version_no::text,6,'0') END,'Offering pin '||pin.version_no,1,source.owning_organization_unit_id FROM pins pin CROSS JOIN learning_offerings source WHERE source.id=$2 RETURNING id,academic_term_id,academic_year_id) INSERT INTO course_offering_details(learning_offering_id,academic_term_id,academic_year_id,subject_version_id,subject_id,credit,hours,assessment_total_score) SELECT inserted.id,inserted.academic_term_id,inserted.academic_year_id,pin.id,pin.subject_id,pin.credit,detail.hours,detail.assessment_total_score FROM inserted JOIN pins pin ON pin.id=inserted.id CROSS JOIN course_offering_details detail WHERE detail.learning_offering_id=$2 RETURNING learning_offering_id,(SELECT version_no FROM pins WHERE id=subject_version_id)").bind(term).bind(source).fetch_all(&pool).await.unwrap();
    assert_eq!(offerings.len(), 504);
    let old = offerings.iter().min_by_key(|(_, v)| v).unwrap().0;
    let (current, revision) = *offerings.iter().max_by_key(|(_, v)| v).unwrap();
    let no_plan = offerings
        .iter()
        .find(|(_, v)| *v == revision - 1)
        .unwrap()
        .0;
    assign_current_offerings(&pool, term, other, &[old]).await;
    sqlx::query("INSERT INTO course_assessment_plans(id,academic_term_id,academic_year_id,subject_version_id,learning_offering_id,assessment_coordinator_id) SELECT uuidv7(),academic_term_id,academic_year_id,subject_version_id,learning_offering_id,$2 FROM course_offering_details WHERE learning_offering_id=$1").bind(old).bind(other).execute(&pool).await.unwrap();
    let school = AcademicResourceListFilter {
        includes_school_owned: true,
        ..Default::default()
    };
    let unfiltered = services::list_assessment_plans(&pool, &list_query(term), &school, teacher)
        .await
        .unwrap();
    assert_eq!(unfiltered.len(), 500);
    assert!(
        unfiltered
            .iter()
            .any(|p| p.offering_id == old && p.plan_id.is_some()),
        "a current registry course needs no publication"
    );
    assert!(
        !unfiltered
            .iter()
            .any(|p| [current, no_plan].contains(&p.offering_id)),
        "both target codes really lie beyond the unfiltered limit"
    );
    assign_current_offerings(&pool, term, teacher, &[current, no_plan]).await;
    let assigned = actor(teacher, &[codes::ACADEMIC_ASSESSMENT_READ_ASSIGNED]);
    let access = require_assessment_plan_list_access(&pool, &assigned, AssessmentAction::Read)
        .await
        .unwrap();
    let mut rows = services::list_assessment_plans(&pool, &list_query(term), &access, teacher)
        .await
        .unwrap();
    rows.sort_by_key(|p| p.offering_id);
    let mut expected = vec![current, no_plan];
    expected.sort_unstable();
    assert_eq!(
        rows.iter().map(|p| p.offering_id).collect::<Vec<_>>(),
        expected,
        "assigned scope must filter before LIMIT500"
    );
    let mut instructor = list_query(term);
    instructor.instructor_id = Some(teacher);
    let mut filtered = services::list_assessment_plans(&pool, &instructor, &school, teacher)
        .await
        .unwrap()
        .into_iter()
        .map(|p| p.offering_id)
        .collect::<Vec<_>>();
    filtered.sort_unstable();
    assert_eq!(
        filtered, expected,
        "instructor scope must also filter before LIMIT500"
    );
    assert!(rows.iter().all(|p| p.plan_id.is_none()));
    let pinned = rows.iter().find(|p| p.offering_id == no_plan).unwrap();
    assert_eq!(
        pinned.subject_version_display_label,
        format!("Assessment pin 503 · v{}", revision - 1)
    );
    assert_eq!(
        pinned.offering_name,
        format!("Offering pin {}", revision - 1)
    );
    assert_eq!(rows[0].subject_id, rows[1].subject_id);
    let mut connection = pool.acquire().await.unwrap();
    let year: Uuid = sqlx::query_scalar("SELECT academic_year_id FROM academic_terms WHERE id=$1")
        .bind(term)
        .fetch_one(&mut *connection)
        .await
        .unwrap();
    let ids = school_academic_delivery::services::current::course_offering_ids_on_connection(
        &mut connection,
        year,
        term,
    )
    .await
    .unwrap();
    assert_eq!(ids.len(), 504);
    for (year, term) in [(Uuid::now_v7(), term), (year, Uuid::now_v7())] {
        assert!(
            school_academic_delivery::services::current::course_offering_ids_on_connection(
                &mut connection,
                year,
                term
            )
            .await
            .unwrap()
            .is_empty()
        );
    }
    drop(connection);
    // Remove only this test's isolated current rows; retained predecessor plans
    // stay byte-identical and cannot become an empty context's fallback.
    sqlx::query("DELETE FROM course_assessment_plans WHERE academic_term_id=$1")
        .bind(term)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM learning_groups WHERE academic_term_id=$1")
        .bind(term)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM learning_offerings WHERE academic_term_id=$1")
        .bind(term)
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        services::list_assessment_plans(&pool, &list_query(term), &school, teacher)
            .await
            .unwrap()
            .is_empty()
    );
    let after: String = sqlx::query_scalar(
        "SELECT jsonb_agg(to_jsonb(plan) ORDER BY id)::text FROM course_assessment_plans plan",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(retained, after);
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
async fn assessment_coordinator_is_explicit_and_authority_uses_current_active_assignments() {
    let pool = populated_canonical_pool("assessment_current_assignments").await;
    let primary = Uuid::from_u128(0x50000000000000000000000000000002);
    let secondary = Uuid::from_u128(0x50000000000000000000000000000003);
    let (source, _) = retained_math_context(&pool).await;
    let term = Uuid::now_v7();
    let offering = Uuid::now_v7();
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO academic_terms(school_days,id,academic_year_id,name,sequence_no,code,term_type,included_in_year_result,blocks_year_closure,status,bell_schedule_id) SELECT term.school_days,$1,offering.academic_year_id,'Current assessment',1000,'ASSESS-CURRENT','custom',false,false,'planning',term.bell_schedule_id FROM learning_offerings offering JOIN academic_terms term ON term.id=offering.academic_term_id WHERE offering.id=$2").bind(term).bind(source).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO learning_offerings(id,academic_term_id,academic_year_id,kind,code_snapshot,name_snapshot,weekly_period_target,owning_organization_unit_id) SELECT $1,$2,academic_year_id,'course','ASSESS-CURRENT','Current assessment',1,owning_organization_unit_id FROM learning_offerings WHERE id=$3").bind(offering).bind(term).bind(source).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO course_offering_details(learning_offering_id,academic_term_id,academic_year_id,subject_version_id,subject_id,credit,hours,assessment_total_score) SELECT $1,$2,academic_year_id,subject_version_id,subject_id,credit,hours,assessment_total_score FROM course_offering_details WHERE learning_offering_id=$3").bind(offering).bind(term).bind(source).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    assign_current_offerings(&pool, term, primary, &[offering]).await;
    let unsaved = services::get_plan_detail(&pool, offering).await.unwrap();
    assert!(unsaved.id.is_none());
    assert!(unsaved.assessment_coordinator_id.is_none());
    assert_eq!(unsaved.coordinator_candidates.len(), 1);
    assert_eq!(unsaved.coordinator_candidates[0].teacher_id, primary);
    assert_eq!(unsaved.suggested_coordinator_id, Some(primary));
    let plan_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM course_assessment_plans WHERE learning_offering_id=$1",
    )
    .bind(offering)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        plan_count, 0,
        "reading current teachers must not initialize a coordinator or plan"
    );
    sqlx::query("INSERT INTO course_assessment_plans(id,academic_term_id,academic_year_id,subject_version_id,learning_offering_id,assessment_coordinator_id) SELECT uuidv7(),academic_term_id,academic_year_id,subject_version_id,learning_offering_id,$2 FROM course_offering_details WHERE learning_offering_id=$1").bind(offering).bind(primary).execute(&pool).await.unwrap();
    assert_assessment_assignment_scope(&pool, term, offering, primary, true).await;
    sqlx::query("DELETE FROM learning_group_teachers WHERE academic_term_id=$1")
        .bind(term)
        .execute(&pool)
        .await
        .unwrap();
    assert_assessment_assignment_scope(&pool, term, offering, primary, false).await;
    let removed = services::get_plan_detail(&pool, offering).await.unwrap();
    assert_eq!(removed.assessment_coordinator_id, Some(primary));
    assert!(removed.coordinator_candidates.is_empty());
    sqlx::query("INSERT INTO learning_group_teachers(id,learning_group_id,academic_term_id,academic_year_id,teacher_id,role) SELECT uuidv7(),id,academic_term_id,academic_year_id,$2,'primary' FROM learning_groups WHERE learning_offering_id=$1").bind(offering).bind(secondary).execute(&pool).await.unwrap();
    assert_assessment_assignment_scope(&pool, term, offering, secondary, true).await;
    let changed = services::get_plan_detail(&pool, offering).await.unwrap();
    assert_eq!(
        changed.assessment_coordinator_id,
        Some(primary),
        "current assignment does not silently replace the saved coordinator"
    );
    assert_eq!(changed.coordinator_candidates[0].teacher_id, secondary);
    sqlx::query("UPDATE users SET status='inactive' WHERE id=$1")
        .bind(secondary)
        .execute(&pool)
        .await
        .unwrap();
    assert_assessment_assignment_scope(&pool, term, offering, secondary, false).await;
    assert!(services::get_plan_detail(&pool, offering)
        .await
        .unwrap()
        .coordinator_candidates
        .is_empty());
    sqlx::query("UPDATE users SET status='active' WHERE id=$1")
        .bind(secondary)
        .execute(&pool)
        .await
        .unwrap();
    assert_assessment_assignment_scope(&pool, term, offering, secondary, true).await;
    assert_assessment_assignment_scope(&pool, term, offering, primary, false).await;
    sqlx::query("DELETE FROM learning_group_teachers WHERE academic_term_id=$1")
        .bind(term)
        .execute(&pool)
        .await
        .unwrap();
    assert_assessment_assignment_scope(&pool, term, offering, secondary, false).await;
    let empty = services::get_plan_detail(&pool, offering).await.unwrap();
    assert_eq!(empty.assessment_coordinator_id, Some(primary));
    assert!(empty.coordinator_candidates.is_empty());
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
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM learning_groups WHERE learning_offering_id=$1",
        )
        .bind(offering)
        .fetch_one(&pool)
        .await
        .unwrap(),
        2
    );
    let assignments: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT teacher.teacher_id,teacher.role FROM learning_groups learning_group JOIN learning_group_teachers teacher ON teacher.learning_group_id=learning_group.id WHERE learning_group.learning_offering_id=$1 ORDER BY teacher.teacher_id",
    )
    .bind(offering)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        assignments,
        vec![(primary, "primary".into()), (secondary, "primary".into())]
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
    // Every normalized current group contributes immediately; no publication or
    // teacher episode date may suppress this new allocation.
    assign_current_offerings(&pool, term, primary, &[offering]).await;
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
    let (math, term) = retained_math_context(&pool).await;
    let (science, science_owner): (Uuid, Uuid) = sqlx::query_as(
        "SELECT id,owning_organization_unit_id FROM learning_offerings
         WHERE academic_term_id=$1 AND kind='course' AND code_snapshot='SCI-CORE'",
    )
    .bind(term)
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
    assert_eq!(pool.options().get_max_connections(), 1);
    let (offering, term) = retained_math_context(&pool).await;
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

mod codes {
    include!("persistence/permission_codes_generated.rs");
}
