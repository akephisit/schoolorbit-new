use super::room_scope_tests::{extra_room, group, publish, removal};
use super::*;
use school_academic_delivery::services::{attendance_roster, audiences, roster_tracking};
use serde_json::Value;

struct ScopedFixture {
    pool: PgPool,
    context: RuntimeContext,
    old_room: Uuid,
    offering: Uuid,
    combined: Uuid,
    omitted: Uuid,
    old_source: Uuid,
    new_source: Uuid,
    boundary: NaiveDate,
}

async fn fixture(name: &str) -> ScopedFixture {
    fixture_with_effective(name, false).await
}

async fn fixture_with_effective(name: &str, current: bool) -> ScopedFixture {
    let pool = prepare_delivery_runtime_fixture(name).await;
    let context = planning_runtime_context(&pool).await;
    let today: NaiveDate = sqlx::query_scalar("SELECT (now() AT TIME ZONE 'Asia/Bangkok')::date")
        .fetch_one(&pool)
        .await
        .unwrap();
    if current {
        sqlx::query("UPDATE academic_years SET start_date=LEAST(start_date,$2) WHERE id=$1")
            .bind(context.year_id)
            .bind(today - Duration::days(60))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE academic_terms SET start_date=$2 WHERE id=$1")
            .bind(context.term_id)
            .bind(today - Duration::days(30))
            .execute(&pool)
            .await
            .unwrap();
    }
    let starts: NaiveDate = sqlx::query_scalar("SELECT start_date FROM academic_terms WHERE id=$1")
        .bind(context.term_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let future = if current {
        today
    } else {
        today.max(starts) + Duration::days(2)
    };
    sqlx::query("UPDATE academic_years SET end_date=GREATEST(end_date,$2) WHERE id=$1")
        .bind(context.year_id)
        .bind(future + Duration::days(90))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE academic_terms SET planned_end_date=GREATEST(planned_end_date,$2) WHERE id=$1",
    )
    .bind(context.term_id)
    .bind(future + Duration::days(60))
    .execute(&pool)
    .await
    .unwrap();
    let old_room = extra_room(&pool, &context, "CONSUMER-M4").await;
    let first = create_runtime_change_set(
        &pool,
        context.teacher_id,
        context.term_id,
        1,
        "consumer-first",
    )
    .await;
    let CreateLearningOfferingRequest::Course(mut request) = course_request(&context) else {
        unreachable!()
    };
    request.targets = vec![OfferingTargetInput {
        target_kind: OfferingTargetKind::GradeProgram,
        homeroom_id: None,
        grade_level_id: context.grade_level_id,
        study_program_id: context.study_program_id,
    }];
    let offering = offerings::create_for_delivery(
        &pool,
        context.teacher_id,
        Some(first.target_delivery_version_id),
        CreateLearningOfferingRequest::Course(request),
    )
    .await
    .unwrap();
    let combined = group(
        &pool,
        &context,
        offering.id,
        "M4 and M6 combined",
        vec![old_room, context.homeroom_id],
    )
    .await;
    let omitted = group(&pool, &context, offering.id, "M4 only", vec![old_room]).await;
    publish(&pool, context.teacher_id, first.id).await;
    let next = create_runtime_change_set(
        &pool,
        context.teacher_id,
        context.term_id,
        (future - starts).num_days(),
        "consumer-restrict",
    )
    .await;
    change_sets::remove_draft_offering_homeroom(
        &pool,
        context.teacher_id,
        next.id,
        offering.id,
        old_room,
        removal(&pool, next.id).await,
    )
    .await
    .unwrap();
    let boundary = next.reference_date;
    // A selected draft does not constrain existing operational registry flows.
    let mut connection = pool.acquire().await.unwrap();
    assert!(
        audiences::dated_constraint_on_connection(&mut connection, offering.id, boundary)
            .await
            .unwrap()
            .is_none()
    );
    drop(connection);
    publish(&pool, context.teacher_id, next.id).await;
    ScopedFixture {
        pool,
        context,
        old_room,
        offering: offering.id,
        combined,
        omitted,
        old_source: first.target_delivery_version_id,
        new_source: next.target_delivery_version_id,
        boundary,
    }
}

async fn student(f: &ScopedFixture, room: Uuid, starts: NaiveDate) -> (Uuid, Uuid) {
    let student = Uuid::new_v4();
    let year = Uuid::new_v4();
    sqlx::query("INSERT INTO users(id,username,password_hash,first_name,last_name,user_type,status) VALUES($1,$1::text,'not-a-login','Synthetic','Scope consumer','student','active')")
        .bind(student).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO student_academic_years(id,student_id,academic_year_id,grade_level_id,study_program_id,status) VALUES($1,$2,$3,$4,$5,'active')")
        .bind(year).bind(student).bind(f.context.year_id).bind(f.context.grade_level_id).bind(f.context.study_program_id)
        .execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO homeroom_placements(id,student_academic_year_id,academic_year_id,homeroom_id,start_date,status,enrollment_type) VALUES($1,$2,$3,$4,$5,'current','new')")
        .bind(Uuid::new_v4()).bind(year).bind(f.context.year_id).bind(room).bind(starts)
        .execute(&f.pool).await.unwrap();
    (student, year)
}

async fn member(f: &ScopedFixture, group: Uuid, student: (Uuid, Uuid), source: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO learning_group_students(id,learning_group_id,academic_term_id,academic_year_id,student_academic_year_id,student_id,membership_status,roster_source,joined_at,published_at) VALUES($1,$2,$3,$4,$5,$6,'active',$7,$8,now())")
        .bind(id).bind(group).bind(f.context.term_id).bind(f.context.year_id)
        .bind(student.1).bind(student.0).bind(source).bind(f.boundary - Duration::days(1))
        .execute(&f.pool).await.unwrap();
    id
}

async fn historical_facts(pool: &PgPool) -> Value {
    sqlx::query_scalar("SELECT jsonb_build_object('members',(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM learning_group_students t),'scores',(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM learning_group_student_scores t),'sessions',(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM attendance_sessions t),'attendanceRecords',(SELECT jsonb_agg(to_jsonb(t) ORDER BY session_id,student_id) FROM attendance_records t),'results',(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM academic_course_results t))")
        .fetch_one(pool).await.unwrap()
}

#[tokio::test]
async fn delivery_room_scope_consumer_dated_boundary_manual_group_gate_and_legacy_none() {
    let f = fixture("room_scope_consumer_manual").await;
    let before = f.boundary - Duration::days(1);
    let mut connection = f.pool.acquire().await.unwrap();
    assert!(
        audiences::dated_constraint_on_connection(&mut connection, f.offering, before)
            .await
            .unwrap()
            .is_none()
    );
    let today: NaiveDate = sqlx::query_scalar("SELECT (now() AT TIME ZONE 'Asia/Bangkok')::date")
        .fetch_one(&mut *connection)
        .await
        .unwrap();
    assert!(today < f.boundary);
    assert!(
        audiences::dated_constraint_on_connection(&mut connection, f.offering, today)
            .await
            .unwrap()
            .is_none(),
        "publishing a future scope must not activate it early"
    );
    let scoped = audiences::dated_constraint_on_connection(&mut connection, f.offering, f.boundary)
        .await
        .unwrap()
        .unwrap();
    assert!(scoped.allows_group(f.combined));
    assert!(!scoped.allows_group(f.omitted));
    assert!(!scoped.allows_homeroom(f.combined, Some(f.old_room)));
    assert!(
        audiences::pinned_constraint_on_connection(&mut connection, f.old_source, f.offering)
            .await
            .unwrap()
            .is_none()
    );
    drop(connection);
    // Operational manual additions require a published roster, independently of scope.
    sqlx::query("UPDATE learning_groups SET roster_status='published',roster_published_at=now() WHERE id=ANY($1)")
        .bind(vec![f.combined, f.omitted]).execute(&f.pool).await.unwrap();
    let outside = student(&f, f.old_room, before).await;
    let retained = groups::get(&f.pool, f.combined).await.unwrap();
    let added = roster_memberships::add_membership(
        &f.pool,
        f.context.teacher_id,
        f.combined,
        AddDatedRosterMembershipRequest {
            group_row_version: retained.row_version,
            student_academic_year_id: outside.1,
            joined_at: f.boundary,
        },
    )
    .await
    .unwrap();
    assert_eq!(added.roster_source, "operational_change");
    let permitted_before = student(&f, f.old_room, before).await;
    let omitted = groups::get(&f.pool, f.omitted).await.unwrap();
    roster_memberships::add_membership(
        &f.pool,
        f.context.teacher_id,
        f.omitted,
        AddDatedRosterMembershipRequest {
            group_row_version: omitted.row_version,
            student_academic_year_id: permitted_before.1,
            joined_at: before,
        },
    )
    .await
    .unwrap();
    let denied = student(&f, f.old_room, before).await;
    let omitted = groups::get(&f.pool, f.omitted).await.unwrap();
    let facts = historical_facts(&f.pool).await;
    assert!(matches!(
        roster_memberships::add_membership(
            &f.pool,
            f.context.teacher_id,
            f.omitted,
            AddDatedRosterMembershipRequest {
                group_row_version: omitted.row_version,
                student_academic_year_id: denied.1,
                joined_at: f.boundary
            }
        )
        .await,
        Err(AppError::Forbidden(_))
    ));
    assert_eq!(facts, historical_facts(&f.pool).await);
}

#[tokio::test]
async fn delivery_room_scope_consumer_old_and_new_pinned_attendance_preserve_manual_facts() {
    let f = fixture("room_scope_consumer_attendance").await;
    let before = f.boundary - Duration::days(1);
    let mut expected_old = Vec::new();
    let mut expected_new = Vec::new();
    for (source, retained) in [
        ("placement", false),
        ("homeroom_tracking", false),
        ("migration_homeroom_snapshot", false),
        ("self_registration", false),
        ("manual_add", true),
        ("operational_change", true),
        ("legacy_external_fact", true),
    ] {
        let pupil = student(&f, f.old_room, before).await;
        member(&f, f.combined, pupil, source).await;
        expected_old.push(pupil.0);
        if retained {
            expected_new.push(pupil.0);
        }
        member(&f, f.omitted, pupil, source).await;
    }
    let kept = student(&f, f.context.homeroom_id, before).await;
    member(&f, f.combined, kept, "placement").await;
    expected_old.push(kept.0);
    expected_new.push(kept.0);
    let frozen = Uuid::new_v4();
    sqlx::query("INSERT INTO attendance_sessions(id,academic_term_id,date,kind,source_key,title,teacher_ids,learning_group_id,offering_id,start_time,end_time,saved_at,saved_by,roster) VALUES($1,$2,$3,'lesson',$1::text,'Frozen scope history',$4,$5,$6,'08:00','08:50',now(),$7,$8)")
        .bind(frozen).bind(f.context.term_id).bind(before).bind(vec![f.context.teacher_id])
        .bind(f.combined).bind(f.offering).bind(f.context.teacher_id)
        .bind(sqlx::types::Json(serde_json::json!([{"studentId":kept.0,"studentAcademicYearId":kept.1,"classNumber":null}])))
        .execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO attendance_records(session_id,student_id,student_academic_year_id,result,origin,updated_by) VALUES($1,$2,$3,'present','teacher',$4)")
        .bind(frozen).bind(kept.0).bind(kept.1).bind(f.context.teacher_id).execute(&f.pool).await.unwrap();

    // Transfer boundary uses the latest dated placement, not today's room.
    let moving = student(&f, f.old_room, before).await;
    member(&f, f.combined, moving, "placement").await;
    sqlx::query("UPDATE homeroom_placements SET status='ended',end_date=$2 WHERE student_academic_year_id=$1")
        .bind(moving.1).bind(f.boundary).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO homeroom_placements(id,student_academic_year_id,academic_year_id,homeroom_id,start_date,status,enrollment_type) VALUES($1,$2,$3,$4,$5,'current','transfer')")
        .bind(Uuid::new_v4()).bind(moving.1).bind(f.context.year_id).bind(f.context.homeroom_id).bind(f.boundary)
        .execute(&f.pool).await.unwrap();
    expected_old.push(moving.0);
    expected_new.push(moving.0);
    let facts = historical_facts(&f.pool).await;
    let mut connection = f.pool.acquire().await.unwrap();
    for (source, mut expected) in [(f.old_source, expected_old), (f.new_source, expected_new)] {
        let mut actual = attendance_roster::pinned_students_on_connection(
            &mut connection,
            source,
            &[f.combined],
            f.boundary,
        )
        .await
        .unwrap()
        .into_iter()
        .map(|row| row.student_id)
        .collect::<Vec<_>>();
        expected.sort();
        actual.sort();
        assert_eq!(actual, expected);
    }
    assert!(
        attendance_roster::pinned_students_on_connection(
            &mut connection,
            f.new_source,
            &[f.omitted],
            f.boundary
        )
        .await
        .unwrap()
        .is_empty(),
        "no manual/legacy provenance bypasses an omitted group"
    );
    let spans = attendance_roster::pinned_membership_spans(
        &mut connection,
        f.new_source,
        &[f.combined],
        before,
        f.boundary,
        Some(moving.0),
    )
    .await
    .unwrap();
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].joined_at, f.boundary);
    drop(connection);
    assert_eq!(
        facts,
        historical_facts(&f.pool).await,
        "attendance projection never rewrites frozen/history ledgers"
    );
}

#[tokio::test]
async fn delivery_room_scope_consumer_tracking_refuses_only_new_joins_and_preserves_scores() {
    let f = fixture_with_effective("room_scope_consumer_tracking", true).await;
    let before = f.boundary - Duration::days(1);
    let existing = student(&f, f.old_room, before).await;
    let existing_member = member(&f, f.combined, existing, "homeroom_tracking").await;
    let denied = student(&f, f.old_room, before).await;
    let allowed = student(&f, f.context.homeroom_id, f.boundary + Duration::days(1)).await;
    let plan = Uuid::new_v4();
    let phase = Uuid::new_v4();
    let item = Uuid::new_v4();
    let score = Uuid::new_v4();
    sqlx::query("INSERT INTO course_assessment_plans(id,academic_term_id,subject_version_id,learning_offering_id,academic_year_id) SELECT $1,g.academic_term_id,d.subject_version_id,g.learning_offering_id,g.academic_year_id FROM learning_groups g JOIN course_offering_details d ON d.learning_offering_id=g.learning_offering_id WHERE g.id=$2")
        .bind(plan).bind(f.combined).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO course_assessment_phases(id,plan_id,phase_code,max_score,exam_arrangement) VALUES($1,$2,'before_midterm',25,'none')")
        .bind(phase).bind(plan).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO learning_group_score_items(id,learning_group_id,learning_offering_id,course_assessment_plan_id,assessment_phase_id,academic_term_id,academic_year_id,name,max_score) SELECT $1,id,learning_offering_id,$2,$3,academic_term_id,academic_year_id,'Preserved score',10 FROM learning_groups WHERE id=$4")
        .bind(item).bind(plan).bind(phase).bind(f.combined).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO learning_group_student_scores(id,learning_group_id,learning_offering_id,academic_term_id,academic_year_id,score_item_id,student_academic_year_id,score) SELECT $1,id,learning_offering_id,academic_term_id,academic_year_id,$2,$3,7.25 FROM learning_groups WHERE id=$4")
        .bind(score).bind(item).bind(existing.1).bind(f.combined).execute(&f.pool).await.unwrap();
    let member_before: Value =
        sqlx::query_scalar("SELECT to_jsonb(t) FROM learning_group_students t WHERE id=$1")
            .bind(existing_member)
            .fetch_one(&f.pool)
            .await
            .unwrap();
    let score_before: Value =
        sqlx::query_scalar("SELECT to_jsonb(t) FROM learning_group_student_scores t WHERE id=$1")
            .bind(score)
            .fetch_one(&f.pool)
            .await
            .unwrap();
    sqlx::query("INSERT INTO learning_group_roster_tracking(learning_group_id,mode,effective_from) VALUES($1,'homeroom',$2)")
        .bind(f.combined).bind(before).execute(&f.pool).await.unwrap();
    let mut transaction = f.pool.begin().await.unwrap();
    roster_tracking::sync_group(
        &mut transaction,
        f.context.teacher_id,
        f.combined,
        Some(&[existing.1, denied.1, allowed.1]),
    )
    .await
    .unwrap();
    transaction.commit().await.unwrap();
    assert_eq!(
        member_before,
        sqlx::query_scalar::<_, Value>(
            "SELECT to_jsonb(t) FROM learning_group_students t WHERE id=$1"
        )
        .bind(existing_member)
        .fetch_one(&f.pool)
        .await
        .unwrap()
    );
    assert_eq!(
        score_before,
        sqlx::query_scalar::<_, Value>(
            "SELECT to_jsonb(t) FROM learning_group_student_scores t WHERE id=$1"
        )
        .bind(score)
        .fetch_one(&f.pool)
        .await
        .unwrap()
    );
    let joined: Vec<Uuid> = sqlx::query_scalar(
        "SELECT student_id FROM learning_group_students WHERE learning_group_id=$1",
    )
    .bind(f.combined)
    .fetch_all(&f.pool)
    .await
    .unwrap();
    assert!(joined.contains(&allowed.0));
    assert!(!joined.contains(&denied.0));
}

#[tokio::test]
async fn delivery_room_scope_consumer_explicit_empty_never_falls_back_for_pinned_attendance() {
    let f = fixture("room_scope_consumer_empty").await;
    let pupil = student(&f, f.context.homeroom_id, f.boundary - Duration::days(1)).await;
    member(&f, f.combined, pupil, "manual_add").await;
    let start: NaiveDate = sqlx::query_scalar("SELECT start_date FROM academic_terms WHERE id=$1")
        .bind(f.context.term_id)
        .fetch_one(&f.pool)
        .await
        .unwrap();
    let revision = create_runtime_change_set(
        &f.pool,
        f.context.teacher_id,
        f.context.term_id,
        (f.boundary - start).num_days() + 1,
        "consumer-empty",
    )
    .await;
    change_sets::remove_draft_offering(
        &f.pool,
        f.context.teacher_id,
        revision.id,
        f.offering,
        removal(&f.pool, revision.id).await,
    )
    .await
    .unwrap();
    publish(&f.pool, f.context.teacher_id, revision.id).await;
    let facts = historical_facts(&f.pool).await;
    let mut connection = f.pool.acquire().await.unwrap();
    let empty = audiences::pinned_constraint_on_connection(
        &mut connection,
        revision.target_delivery_version_id,
        f.offering,
    )
    .await
    .unwrap()
    .unwrap();
    assert!(empty.offering.is_none());
    assert!(attendance_roster::pinned_students_on_connection(
        &mut connection,
        revision.target_delivery_version_id,
        &[f.combined],
        revision.reference_date
    )
    .await
    .unwrap()
    .is_empty());
    assert_eq!(
        attendance_roster::pinned_students_on_connection(
            &mut connection,
            f.old_source,
            &[f.combined],
            revision.reference_date
        )
        .await
        .unwrap()
        .len(),
        1
    );
    drop(connection);
    assert_eq!(facts, historical_facts(&f.pool).await);
}

#[tokio::test]
async fn delivery_room_scope_consumer_roster_candidates_preserve_existing_and_refuse_omitted_group()
{
    let f = fixture_with_effective("room_scope_consumer_roster", true).await;
    let before = f.boundary - Duration::days(1);
    let existing = student(&f, f.old_room, before).await;
    let identity = member(&f, f.combined, existing, "placement").await;
    let denied = student(&f, f.old_room, before).await;
    let kept = student(&f, f.context.homeroom_id, before).await;
    let preview = groups::preview_roster(&f.pool, f.combined).await.unwrap();
    assert!(preview
        .students
        .iter()
        .any(|row| row.student_id == existing.0 && row.proposed_active));
    assert!(preview
        .students
        .iter()
        .any(|row| row.student_id == kept.0 && row.proposed_active));
    assert!(preview
        .students
        .iter()
        .all(|row| row.student_id != denied.0));
    let group = groups::get(&f.pool, f.combined).await.unwrap();
    groups::apply_roster(
        &f.pool,
        f.context.teacher_id,
        f.combined,
        ApplyRosterRequest {
            row_version: group.row_version,
            source_hash: preview.source_hash,
            overrides: vec![],
        },
    )
    .await
    .unwrap();
    let preserved: (String, Option<NaiveDate>) =
        sqlx::query_as("SELECT membership_status,left_at FROM learning_group_students WHERE id=$1")
            .bind(identity)
            .fetch_one(&f.pool)
            .await
            .unwrap();
    assert_eq!(preserved, ("active".into(), None));
    let facts = historical_facts(&f.pool).await;
    assert!(matches!(
        groups::preview_roster(&f.pool, f.omitted).await,
        Err(AppError::Conflict(_))
    ));
    let group = groups::get(&f.pool, f.omitted).await.unwrap();
    assert!(matches!(
        groups::apply_roster(
            &f.pool,
            f.context.teacher_id,
            f.omitted,
            ApplyRosterRequest {
                row_version: group.row_version,
                source_hash: "irrelevant-denied-scope".into(),
                overrides: vec![],
            }
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    assert_eq!(facts, historical_facts(&f.pool).await);
}

#[tokio::test]
async fn delivery_room_scope_consumer_timetable_adapters_retain_the_published_delivery_source() {
    let pool = prepare_delivery_runtime_fixture("room_scope_consumer_timetable_source").await;
    let (term, date, source): (Uuid, NaiveDate, Uuid) = sqlx::query_as("SELECT academic_term_id,effective_from,delivery_version_id FROM academic_timetable_versions WHERE status='published' ORDER BY effective_from,id LIMIT 1")
        .fetch_one(&pool).await.unwrap();
    let mut connection = pool.acquire().await.unwrap();
    let spans = school_academic_timetable::services::attendance::published_spans(
        &mut connection,
        term,
        date,
        date,
    )
    .await
    .unwrap();
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].delivery_version_id, source);
    let daily = school_academic_timetable::services::attendance::source_blocks_on_connection(
        &mut connection,
        term,
        date,
    )
    .await
    .unwrap();
    assert_eq!(daily.delivery_version_id, source);
}

#[tokio::test]
async fn delivery_room_scope_consumer_tracking_batches_future_dates_across_publication_boundary() {
    let f = fixture("room_scope_consumer_tracking_future_batch").await;
    let before = f.boundary - Duration::days(1);
    let later = f.boundary + Duration::days(1);
    let last = f.boundary + Duration::days(2);
    let allowed_before = student(&f, f.old_room, before).await;
    let denied_at = student(&f, f.old_room, f.boundary).await;
    let denied_later = student(&f, f.old_room, later).await;
    let denied_last = student(&f, f.old_room, last).await;
    let allowed_later = student(&f, f.context.homeroom_id, later).await;
    let dates = [before, f.boundary, later, last];
    let mut connection = f.pool.acquire().await.unwrap();
    let scopes = audiences::dated_group_constraints_for_dates_on_connection(
        &mut connection,
        &[f.combined],
        &dates,
    )
    .await
    .unwrap();
    assert_eq!(scopes.len(), dates.len());
    assert!(scopes[&before].constraint_for_group(f.combined).is_none());
    for date in [f.boundary, later, last] {
        let scope = scopes[&date].constraint_for_group(f.combined).unwrap();
        assert_eq!(scope.delivery_version_id, f.new_source);
        assert!(!scope.allows_homeroom(f.combined, Some(f.old_room)));
        assert!(scope.allows_homeroom(f.combined, Some(f.context.homeroom_id)));
    }
    drop(connection);
    sqlx::query("INSERT INTO learning_group_roster_tracking(learning_group_id,mode,effective_from) VALUES($1,'homeroom',$2)")
        .bind(f.combined).bind(before).execute(&f.pool).await.unwrap();
    let selected = [
        allowed_before.1,
        denied_at.1,
        denied_later.1,
        denied_last.1,
        allowed_later.1,
    ];
    let mut transaction = f.pool.begin().await.unwrap();
    roster_tracking::sync_group(
        &mut transaction,
        f.context.teacher_id,
        f.combined,
        Some(&selected),
    )
    .await
    .unwrap();
    transaction.commit().await.unwrap();
    let mut joined: Vec<Uuid> = sqlx::query_scalar(
        "SELECT student_id FROM learning_group_students WHERE learning_group_id=$1",
    )
    .bind(f.combined)
    .fetch_all(&f.pool)
    .await
    .unwrap();
    let mut expected = vec![allowed_before.0, allowed_later.0];
    joined.sort();
    expected.sort();
    assert_eq!(joined, expected);
}
