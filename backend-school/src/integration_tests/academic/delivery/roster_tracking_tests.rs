use super::*;
use school_academic_core::{models::*, services::homeroom_roster};
use school_academic_delivery::{
    models::{RosterTrackingMode, UpdateRosterTrackingRequest},
    services::roster_tracking::{self, RoomRosterTracking},
};

struct Fixture {
    pool: PgPool,
    actor: Uuid,
    room: Uuid,
    other: Uuid,
    day: NaiveDate,
    groups: Vec<Uuid>,
}

async fn fixture(name: &str) -> Fixture {
    fixture_with_curriculum_catalog(name, false).await
}

async fn fixture_with_curriculum_catalog(name: &str, curriculum_catalog: bool) -> Fixture {
    let pool = prepare_delivery_runtime_fixture(name).await;
    let context = planning_runtime_context(&pool).await;
    let today: NaiveDate = sqlx::query_scalar("SELECT (now() AT TIME ZONE 'Asia/Bangkok')::date")
        .fetch_one(&pool)
        .await
        .unwrap();
    let day = today;
    sqlx::query(
        "UPDATE academic_years SET status='closed' WHERE id<>$1 AND status IN ('active','closing')",
    )
    .bind(context.year_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE academic_years SET status='active' WHERE id=$1")
        .bind(context.year_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE student_academic_years SET status='active' WHERE academic_year_id=$1 AND status='planned'").bind(context.year_id).execute(&pool).await.unwrap();
    sqlx::query("UPDATE homeroom_placements SET status='current' WHERE academic_year_id=$1 AND status='planned'").bind(context.year_id).execute(&pool).await.unwrap();
    let other = Uuid::new_v4();
    sqlx::query("INSERT INTO homerooms(id,code,name,academic_year_id,grade_level_id,room_number,study_program_id,capacity,is_active) SELECT $1,'TRACK-OTHER','ห้องปลายทางทดสอบ',academic_year_id,grade_level_id,'2',study_program_id,40,true FROM homerooms WHERE id=$2").bind(other).bind(context.homeroom_id).execute(&pool).await.unwrap();
    let CreateLearningOfferingRequest::Course(mut request) = course_request(&context) else {
        unreachable!()
    };
    if curriculum_catalog {
        let preview = offerings::preview_from_curriculum(
            &pool,
            PreviewCurriculumOfferingsRequest {
                academic_term_id: context.term_id,
                study_program_ids: vec![context.study_program_id],
            },
        )
        .await
        .unwrap();
        request.subject_version_id = preview
            .proposals
            .iter()
            .find(|proposal| proposal.resource_kind == LearningOfferingKind::Course)
            .expect("recurring-gap reactivation must use an actual curriculum catalog pin")
            .catalog_version_id;
    }
    request.targets.push(OfferingTargetInput {
        target_kind: OfferingTargetKind::Homeroom,
        homeroom_id: Some(other),
        grade_level_id: context.grade_level_id,
        study_program_id: context.study_program_id,
    });
    let offering = offerings::create(
        &pool,
        context.teacher_id,
        CreateLearningOfferingRequest::Course(request),
    )
    .await
    .unwrap();
    let mut ids = Vec::new();
    for (index, rooms) in [
        vec![context.homeroom_id],
        vec![other],
        vec![context.homeroom_id, other],
        vec![context.homeroom_id],
    ]
    .into_iter()
    .enumerate()
    {
        let group = groups::create(
            &pool,
            context.teacher_id,
            offering.id,
            CreateLearningGroupRequest {
                registration_open: false,
                name: format!("Tracking group {index}"),
                description: None,
                capacity: Some(40),
                preferred_room_ids: vec![],
            },
        )
        .await
        .unwrap();
        let group = groups::replace_homerooms(
            &pool,
            context.teacher_id,
            group.id,
            ReplaceLearningGroupHomeroomsRequest {
                row_version: group.row_version,
                homeroom_ids: rooms,
            },
        )
        .await
        .unwrap();
        let group = groups::replace_teachers(
            &pool,
            context.teacher_id,
            group.id,
            ReplaceLearningGroupTeachersRequest {
                row_version: group.row_version,
                teachers: vec![TeacherAssignmentInput {
                    teacher_id: context.teacher_id,
                    role: LearningTeacherRole::Primary,
                }],
            },
        )
        .await
        .unwrap();
        ids.push(group.id);
    }
    for id in &ids {
        let group = groups::get(&pool, *id).await.unwrap();
        let preview = groups::preview_roster(&pool, *id).await.unwrap();
        groups::apply_roster(
            &pool,
            context.teacher_id,
            *id,
            ApplyRosterRequest {
                row_version: group.row_version,
                source_hash: preview.source_hash,
                overrides: vec![],
            },
        )
        .await
        .unwrap();
    }
    Fixture {
        pool,
        actor: context.teacher_id,
        room: context.homeroom_id,
        other,
        day,
        groups: ids,
    }
}
async fn enable(f: &Fixture, id: Uuid) {
    let group = groups::get(&f.pool, id).await.unwrap();
    roster_tracking::update(
        &f.pool,
        f.actor,
        id,
        UpdateRosterTrackingRequest {
            mode: RosterTrackingMode::Homeroom,
            effective_from: Some(f.day),
            row_version: group.row_version,
        },
    )
    .await
    .unwrap();
}
async fn members(f: &Fixture, id: Uuid) -> Vec<super::super::models::DatedRosterMembership> {
    roster_memberships::list_memberships(&f.pool, id)
        .await
        .unwrap()
}
async fn move_student(
    f: &Fixture,
    from: Uuid,
    to: Option<Uuid>,
    student_year: Uuid,
    day: NaiveDate,
) -> Result<HomeroomRoster, AppError> {
    let roster = homeroom_roster::get_roster(&f.pool, from).await.unwrap();
    let student = roster
        .students
        .iter()
        .find(|s| s.student_academic_year_id == student_year)
        .unwrap();
    homeroom_roster::mutate_roster(
        &f.pool,
        &RoomRosterTracking,
        f.actor,
        from,
        MutateHomeroomRosterRequest {
            revision: roster.revision,
            action: if to.is_some() {
                HomeroomRosterAction::Transfer
            } else {
                HomeroomRosterAction::Remove
            },
            selections: vec![HomeroomRosterSelection {
                student_academic_year_id: student_year,
                student_year_row_version: student.student_year_row_version,
                placement_id: Some(student.placement_id),
                placement_row_version: Some(student.row_version),
            }],
            effective_date: day,
            target_homeroom_id: to,
            reason: "Synthetic transfer".into(),
        },
    )
    .await
}

#[tokio::test]
async fn room_tracking_annual_membership_actual_spans_preserve_real_transfer() {
    use school_academic_delivery::services::attendance_roster;
    let f = fixture("roster_tracking_annual_pinned_transfer").await;
    let learner = members(&f, f.groups[0]).await[0].clone();
    let shared = members(&f, f.groups[2]).await[0].clone();
    sqlx::query("UPDATE homeroom_placements SET placement_scope='academic_year',start_date=NULL WHERE student_academic_year_id=$1 AND status='current'")
        .bind(learner.student_academic_year_id).execute(&f.pool).await.unwrap();
    let other_year: Uuid = sqlx::query_scalar("INSERT INTO academic_years(year,name,default_school_days,status) SELECT min(year)-1,'Excluded pinned annual history','MON','closed' FROM academic_years RETURNING id").fetch_one(&f.pool).await.unwrap();
    let other_room: Uuid = sqlx::query_scalar("INSERT INTO homerooms(id,code,name,academic_year_id,grade_level_id,room_number,study_program_id,capacity,is_active) SELECT uuidv7(),'FOREIGN-PIN','Other year pin',$1,grade_level_id,'FOREIGN-PIN',study_program_id,40,true FROM homerooms WHERE id=$2 RETURNING id")
        .bind(other_year).bind(f.room).fetch_one(&f.pool).await.unwrap();
    let other_annual: Uuid = sqlx::query_scalar("INSERT INTO student_academic_years(id,student_id,academic_year_id,grade_level_id,study_program_id,status) SELECT uuidv7(),student_id,$1,grade_level_id,study_program_id,'active' FROM student_academic_years WHERE id=$2 RETURNING id")
        .bind(other_year).bind(learner.student_academic_year_id).fetch_one(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO homeroom_placements(id,student_academic_year_id,academic_year_id,homeroom_id,placement_scope,status,enrollment_type) VALUES(uuidv7(),$1,$2,$3,'academic_year','current','promotion')")
        .bind(other_annual).bind(other_year).bind(other_room).execute(&f.pool).await.unwrap();
    let foreign_before: serde_json::Value = sqlx::query_scalar("SELECT jsonb_build_object('annual',to_jsonb(y),'placements',(SELECT jsonb_agg(to_jsonb(p) ORDER BY id) FROM homeroom_placements p WHERE p.student_academic_year_id=y.id)) FROM student_academic_years y WHERE id=$1")
        .bind(other_annual).fetch_one(&f.pool).await.unwrap();
    for group in &f.groups[..3] {
        enable(&f, *group).await;
    }
    let manual = serde_json::to_value(members(&f, f.groups[3]).await).unwrap();
    let before = f.day + Duration::days(1);
    let old_advisor = sqlx::query_scalar::<_, Uuid>(
        "SELECT user_id FROM homeroom_advisors WHERE homeroom_id=$1 ORDER BY user_id LIMIT 1",
    )
    .bind(f.room)
    .fetch_optional(&f.pool)
    .await
    .unwrap()
    .unwrap_or(f.actor);
    sqlx::query("INSERT INTO homeroom_advisors(homeroom_id,user_id,role) VALUES($1,$2,'primary') ON CONFLICT DO NOTHING").bind(f.room).bind(old_advisor).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO homeroom_advisors(homeroom_id,user_id,role) VALUES($1,$2,'primary')")
        .bind(other_room)
        .bind(old_advisor)
        .execute(&f.pool)
        .await
        .unwrap();
    let new_advisor: Uuid = sqlx::query_scalar("INSERT INTO users(id,username,password_hash,first_name,last_name,user_type,status) VALUES(uuidv7(),'annual-advisor-'||uuidv7()::text,'not-a-login','Synthetic','New advisor','staff','active') RETURNING id").fetch_one(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO homeroom_advisors(homeroom_id,user_id,role) VALUES($1,$2,'primary')")
        .bind(f.other)
        .bind(new_advisor)
        .execute(&f.pool)
        .await
        .unwrap();
    let transfer = before + Duration::days(1);
    let after = transfer + Duration::days(1);
    move_student(
        &f,
        f.room,
        Some(f.other),
        learner.student_academic_year_id,
        transfer,
    )
    .await
    .unwrap();
    for day in [f.day, before, transfer, after] {
        let old = school_academic_core::services::attendance_roster::advisor_for_student(
            &f.pool,
            old_advisor,
            learner.student_id,
            day,
        )
        .await
        .unwrap();
        let new = school_academic_core::services::attendance_roster::advisor_for_student(
            &f.pool,
            new_advisor,
            learner.student_id,
            day,
        )
        .await
        .unwrap();
        assert_eq!(
            old,
            day < transfer,
            "active-year original advisor owns the actual span, despite ended status and unrelated closed-year annual history"
        );
        assert_eq!(
            new,
            day >= transfer,
            "the destination advisor starts on the real transfer day"
        );
    }
    let placements = school_academic_core::services::student_years::list_placements(
        &f.pool,
        learner.student_academic_year_id,
    )
    .await
    .unwrap();
    assert!(placements.iter().any(
        |p| p.placement_scope == HomeroomPlacementScope::AcademicYear
            && p.start_date.is_none()
            && p.end_date == Some(before)
            && p.status == HomeroomPlacementStatus::Ended
    ));
    assert!(
        placements
            .iter()
            .any(|p| p.placement_scope == HomeroomPlacementScope::Dated
                && p.start_date == Some(transfer)
                && p.homeroom_id == f.other
                && p.status == HomeroomPlacementStatus::Current)
    );
    assert!(
        members(&f, f.groups[0])
            .await
            .iter()
            .any(|m| m.id == learner.id && m.left_at == Some(before))
    );
    assert!(
        members(&f, f.groups[2])
            .await
            .iter()
            .any(|m| m.id == shared.id && m.left_at.is_none())
    );
    assert_eq!(
        serde_json::to_value(members(&f, f.groups[3]).await).unwrap(),
        manual
    );
    let mut retained = Vec::new();
    for group in &f.groups[..3] {
        retained.push(serde_json::to_value(members(&f, *group).await).unwrap());
    }
    let mut connection = f.pool.acquire().await.unwrap();
    let spans = attendance_roster::membership_spans(&mut connection, &f.groups[..3], before, after)
        .await
        .unwrap();
    for (group, expected) in [
        (f.groups[0], vec![(learner.joined_at, Some(before))]),
        (f.groups[1], vec![(transfer, None)]),
        (f.groups[2], vec![(shared.joined_at, None)]),
    ] {
        let mut actual: Vec<_> = spans
            .iter()
            .filter(|m| m.student_id == learner.student_id && m.learning_group_id == group)
            .map(|m| (m.joined_at, m.left_at))
            .collect();
        actual.sort();
        assert_eq!(actual, expected, "exact annual/dated partition for {group}");
    }
    for day in [before, transfer, after] {
        let pinned =
            attendance_roster::students_on_connection(&mut connection, &f.groups[..3], day)
                .await
                .unwrap();
        let pupil: Vec<_> = pinned
            .iter()
            .filter(|p| p.student_id == learner.student_id)
            .collect();
        assert_eq!(
            pupil.len(),
            2,
            "one source/destination group plus the shared group on {day}"
        );
        assert!(
            pupil
                .iter()
                .all(|p| p.student_academic_year_id == learner.student_academic_year_id)
        );
        let mut actual: Vec<_> = pupil.iter().map(|p| p.learning_group_id).collect();
        actual.sort();
        let mut expected = vec![
            if day < transfer {
                f.groups[0]
            } else {
                f.groups[1]
            },
            f.groups[2],
        ];
        expected.sort();
        assert_eq!(actual, expected);
        for group in &f.groups[..3] {
            let coverage = spans
                .iter()
                .filter(|m| {
                    m.student_id == learner.student_id
                        && m.learning_group_id == *group
                        && m.joined_at <= day
                        && m.left_at.is_none_or(|end| day <= end)
                })
                .count();
            assert_eq!(
                coverage,
                usize::from(expected.contains(group)),
                "no gap or duplicate coverage on {day}"
            );
        }
    }
    let scoped = attendance_roster::membership_spans_for_student(
        &mut connection,
        &f.groups[..3],
        before,
        after,
        learner.student_id,
    )
    .await
    .unwrap();
    let fields = |m: &attendance_roster::AttendanceMembershipSpan| {
        (m.learning_group_id, m.student_id, m.joined_at, m.left_at)
    };
    assert_eq!(
        scoped.iter().map(fields).collect::<Vec<_>>(),
        spans
            .iter()
            .filter(|m| m.student_id == learner.student_id)
            .map(fields)
            .collect::<Vec<_>>()
    );
    drop(connection);
    for (group, facts) in f.groups[..3].iter().zip(retained) {
        assert_eq!(
            serde_json::to_value(members(&f, *group).await).unwrap(),
            facts,
            "actual projection does not rewrite membership facts"
        );
    }
    let foreign_after: serde_json::Value = sqlx::query_scalar("SELECT jsonb_build_object('annual',to_jsonb(y),'placements',(SELECT jsonb_agg(to_jsonb(p) ORDER BY id) FROM homeroom_placements p WHERE p.student_academic_year_id=y.id)) FROM student_academic_years y WHERE id=$1")
        .bind(other_annual).fetch_one(&f.pool).await.unwrap();
    assert_eq!(foreign_after, foreign_before);
}

#[tokio::test]
async fn room_tracking_transfers_reentries_and_deactivation_preserve_manual_and_shared_history() {
    let f = fixture("roster_tracking_history").await;
    let old = members(&f, f.groups[0]).await[0].clone();
    let shared = members(&f, f.groups[2]).await[0].clone();
    // A's real Delivery publication initializes this canonical plan. Reuse its
    // identity when constructing retained score history rather than inserting a
    // second plan for an already initialized offering.
    let historical_plan: Uuid = sqlx::query_scalar(
        "SELECT plan.id FROM course_assessment_plans plan JOIN learning_groups g ON g.learning_offering_id=plan.learning_offering_id WHERE g.id=$1",
    ).bind(f.groups[0]).fetch_one(&f.pool).await.unwrap();
    let (plan, phase) = canonical_score_phase(&f.pool, f.groups[0], f.actor).await;
    assert_eq!(plan, historical_plan);
    let item = Uuid::new_v4();
    let score = Uuid::new_v4();
    sqlx::query("INSERT INTO learning_group_score_items(id,learning_group_id,learning_offering_id,course_assessment_plan_id,assessment_phase_id,academic_term_id,academic_year_id,name,max_score) SELECT $1,id,learning_offering_id,$2,$3,academic_term_id,academic_year_id,'Synthetic score',10 FROM learning_groups WHERE id=$4").bind(item).bind(plan).bind(phase).bind(f.groups[0]).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO learning_group_student_scores(id,learning_group_id,learning_offering_id,academic_term_id,academic_year_id,score_item_id,student_academic_year_id,score) SELECT $1,id,learning_offering_id,academic_term_id,academic_year_id,$2,$3,7.25 FROM learning_groups WHERE id=$4").bind(score).bind(item).bind(old.student_academic_year_id).bind(f.groups[0]).execute(&f.pool).await.unwrap();
    let assessment_before = assessment_facts(&f.pool, f.groups[0]).await;
    let recorded: serde_json::Value =
        sqlx::query_scalar("SELECT to_jsonb(s) FROM learning_group_student_scores s WHERE id=$1")
            .bind(score)
            .fetch_one(&f.pool)
            .await
            .unwrap();
    let manual = serde_json::to_value(members(&f, f.groups[3]).await).unwrap();
    for id in &f.groups[..3] {
        enable(&f, *id).await;
    }
    assert_eq!(members(&f, f.groups[0]).await[0].id, old.id);
    let group = groups::get(&f.pool, f.groups[0]).await.unwrap();
    assert!(matches!(
        roster_memberships::remove_membership(
            &f.pool,
            f.actor,
            group.id,
            old.id,
            RemoveDatedRosterMembershipRequest {
                group_row_version: group.row_version,
                membership_row_version: old.row_version,
                left_at: f.day
            }
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    move_student(
        &f,
        f.room,
        Some(f.other),
        old.student_academic_year_id,
        f.day + Duration::days(1),
    )
    .await
    .unwrap();
    let ended = members(&f, f.groups[0])
        .await
        .into_iter()
        .find(|m| m.id == old.id)
        .unwrap();
    assert_eq!(ended.left_at, Some(f.day));
    assert_eq!(
        ended.membership_status,
        super::super::models::MembershipStatus::Ended
    );
    assert_eq!(
        members(&f, f.groups[1]).await[0].joined_at,
        f.day + Duration::days(1)
    );
    assert_eq!(members(&f, f.groups[2]).await[0].id, shared.id);
    assert_eq!(
        serde_json::to_value(members(&f, f.groups[3]).await).unwrap(),
        manual
    );
    move_student(
        &f,
        f.other,
        Some(f.room),
        old.student_academic_year_id,
        f.day + Duration::days(2),
    )
    .await
    .unwrap();
    let history = members(&f, f.groups[0]).await;
    assert_eq!(history.len(), 2);
    assert!(
        history
            .iter()
            .any(|m| m.id == old.id && m.left_at == Some(f.day))
    );
    assert!(history.iter().any(|m| m.id != old.id
        && m.joined_at == f.day + Duration::days(2)
        && m.left_at.is_none()));
    let before = serde_json::to_value(&history).unwrap();
    let mut tx = f.pool.begin().await.unwrap();
    roster_tracking::sync_students(&mut tx, f.actor, &[old.student_academic_year_id])
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(
        serde_json::to_value(members(&f, f.groups[0]).await).unwrap(),
        before
    );
    school_students::services::delete_student(
        &f.pool,
        old.student_id,
        f.actor,
        &crate::http_state::PLACEMENT_ROSTERS,
    )
    .await
    .unwrap();
    assert!(
        members(&f, f.groups[0])
            .await
            .iter()
            .all(|m| m.left_at.is_some())
    );
    assert_eq!(
        serde_json::to_value(members(&f, f.groups[3]).await).unwrap(),
        manual
    );
    let retained: serde_json::Value =
        sqlx::query_scalar("SELECT to_jsonb(s) FROM learning_group_student_scores s WHERE id=$1")
            .bind(score)
            .fetch_one(&f.pool)
            .await
            .unwrap();
    assert_eq!(recorded, retained);
    assert_eq!(
        assessment_before,
        assessment_facts(&f.pool, f.groups[0]).await
    );
}

#[tokio::test]
async fn room_tracking_capacity_failure_rolls_back_placement_memberships_and_audit() {
    let f = fixture("roster_tracking_atomic_capacity").await;
    for id in &f.groups[..3] {
        enable(&f, *id).await;
    }
    sqlx::query("UPDATE learning_groups SET capacity=1 WHERE id=$1")
        .bind(f.groups[1])
        .execute(&f.pool)
        .await
        .unwrap();
    let synthetic_user = Uuid::new_v4();
    let synthetic_year = Uuid::new_v4();
    sqlx::query("INSERT INTO users(id,username,password_hash,first_name,last_name,user_type,status) VALUES($1,$2,'fixture-not-login','นักเรียนความจุ','ทดสอบ','student','active')").bind(synthetic_user).bind(format!("tracking-{synthetic_user}")).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO student_academic_years(id,student_id,academic_year_id,grade_level_id,study_program_id,status) SELECT $1,$2,academic_year_id,grade_level_id,study_program_id,'active' FROM homerooms WHERE id=$3").bind(synthetic_year).bind(synthetic_user).bind(f.other).execute(&f.pool).await.unwrap();
    let roster = homeroom_roster::get_roster(&f.pool, f.other).await.unwrap();
    homeroom_roster::mutate_roster(
        &f.pool,
        &RoomRosterTracking,
        f.actor,
        f.other,
        MutateHomeroomRosterRequest {
            revision: roster.revision,
            action: HomeroomRosterAction::Add,
            selections: vec![HomeroomRosterSelection {
                student_academic_year_id: synthetic_year,
                student_year_row_version: 1,
                placement_id: None,
                placement_row_version: None,
            }],
            effective_date: f.day,
            target_homeroom_id: None,
            reason: String::new(),
        },
    )
    .await
    .unwrap();
    let student = members(&f, f.groups[0]).await[0].clone();
    let snapshot: serde_json::Value=sqlx::query_scalar("SELECT jsonb_build_object('placements',(SELECT jsonb_agg(to_jsonb(p) ORDER BY p.id) FROM homeroom_placements p),'memberships',(SELECT jsonb_agg(to_jsonb(m) ORDER BY m.id) FROM learning_group_students m),'audits',(SELECT count(*) FROM academic_audit_events))").fetch_one(&f.pool).await.unwrap();
    assert!(matches!(
        move_student(
            &f,
            f.room,
            Some(f.other),
            student.student_academic_year_id,
            f.day + Duration::days(1)
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    let after: serde_json::Value=sqlx::query_scalar("SELECT jsonb_build_object('placements',(SELECT jsonb_agg(to_jsonb(p) ORDER BY p.id) FROM homeroom_placements p),'memberships',(SELECT jsonb_agg(to_jsonb(m) ORDER BY m.id) FROM learning_group_students m),'audits',(SELECT count(*) FROM academic_audit_events))").fetch_one(&f.pool).await.unwrap();
    assert_eq!(snapshot, after);
}

#[tokio::test]
async fn room_tracking_is_opt_in_revisioned_and_switching_manual_keeps_members() {
    let f = fixture("roster_tracking_mode").await;
    let id = f.groups[0];
    let initial = roster_tracking::get(&f.pool, id).await.unwrap();
    assert_eq!(initial.mode, RosterTrackingMode::Manual);
    enable(&f, id).await;
    assert!(matches!(
        roster_tracking::update(
            &f.pool,
            f.actor,
            id,
            UpdateRosterTrackingRequest {
                mode: RosterTrackingMode::Manual,
                effective_from: None,
                row_version: initial.group_row_version
            }
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    let config = roster_tracking::get(&f.pool, id).await.unwrap();
    let before = serde_json::to_value(members(&f, id).await).unwrap();
    roster_tracking::update(
        &f.pool,
        f.actor,
        id,
        UpdateRosterTrackingRequest {
            mode: RosterTrackingMode::Manual,
            effective_from: None,
            row_version: config.group_row_version,
        },
    )
    .await
    .unwrap();
    assert_eq!(serde_json::to_value(members(&f, id).await).unwrap(), before);
    let config = roster_tracking::get(&f.pool, id).await.unwrap();
    sqlx::query("UPDATE academic_terms SET status='closed' WHERE id=(SELECT academic_term_id FROM learning_groups WHERE id=$1)").bind(id).execute(&f.pool).await.unwrap();
    assert!(matches!(
        roster_tracking::update(
            &f.pool,
            f.actor,
            id,
            UpdateRosterTrackingRequest {
                mode: RosterTrackingMode::Homeroom,
                effective_from: Some(f.day),
                row_version: config.group_row_version
            }
        )
        .await,
        Err(AppError::Conflict(_))
    ));
}

#[tokio::test]
async fn room_tracking_future_cancellation_keeps_identity_and_allows_reentry_on_the_same_start() {
    let f = fixture("roster_tracking_planned").await;
    let year: Uuid = sqlx::query_scalar("SELECT academic_year_id FROM homerooms WHERE id=$1")
        .bind(f.room)
        .fetch_one(&f.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE academic_years SET status='planning' WHERE id=$1")
        .bind(year)
        .execute(&f.pool)
        .await
        .unwrap();
    let user = Uuid::new_v4();
    let student_year = Uuid::new_v4();
    sqlx::query("INSERT INTO users(id,username,password_hash,first_name,last_name,user_type,status) VALUES($1,$2,'fixture-not-login','นักเรียนเตรียมการ','ทดสอบ','student','active')").bind(user).bind(format!("tracking-{user}")).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO student_academic_years(id,student_id,academic_year_id,grade_level_id,study_program_id,status) SELECT $1,$2,academic_year_id,grade_level_id,study_program_id,'planned' FROM homerooms WHERE id=$3").bind(student_year).bind(user).bind(f.other).execute(&f.pool).await.unwrap();
    let group = groups::get(&f.pool, f.groups[1]).await.unwrap();
    let begins = f.day + Duration::days(5);
    roster_tracking::update(
        &f.pool,
        f.actor,
        group.id,
        UpdateRosterTrackingRequest {
            mode: RosterTrackingMode::Homeroom,
            effective_from: Some(begins),
            row_version: group.row_version,
        },
    )
    .await
    .unwrap();
    for attempt in 0..2 {
        if attempt == 1 {
            sqlx::query("UPDATE academic_years SET status='ready' WHERE id=(SELECT academic_year_id FROM learning_groups WHERE id=$1)")
                .bind(group.id).execute(&f.pool).await.unwrap();
        }
        let roster = homeroom_roster::get_roster(&f.pool, f.other).await.unwrap();
        let revision: i64 =
            sqlx::query_scalar("SELECT row_version FROM student_academic_years WHERE id=$1")
                .bind(student_year)
                .fetch_one(&f.pool)
                .await
                .unwrap();
        let roster = homeroom_roster::mutate_roster(
            &f.pool,
            &RoomRosterTracking,
            f.actor,
            f.other,
            MutateHomeroomRosterRequest {
                revision: roster.revision,
                action: HomeroomRosterAction::Add,
                selections: vec![HomeroomRosterSelection {
                    student_academic_year_id: student_year,
                    student_year_row_version: revision,
                    placement_id: None,
                    placement_row_version: None,
                }],
                effective_date: begins,
                target_homeroom_id: None,
                reason: String::new(),
            },
        )
        .await
        .unwrap();
        assert_eq!(
            roster
                .students
                .iter()
                .find(|s| s.student_academic_year_id == student_year)
                .unwrap()
                .status,
            HomeroomPlacementStatus::Planned
        );
        let history = members(&f, group.id).await;
        assert_eq!(history.len(), attempt + 1);
        assert!(
            history
                .iter()
                .any(|m| m.left_at.is_none() && m.joined_at == begins)
        );
        if attempt == 0 {
            let old_id = history[0].id;
            move_student(&f, f.other, None, student_year, f.day + Duration::days(1))
                .await
                .unwrap();
            let cancelled = members(&f, group.id).await;
            assert_eq!(cancelled[0].id, old_id);
            assert_eq!(
                cancelled[0].membership_status,
                super::super::models::MembershipStatus::Removed
            );
        }
    }
}

#[tokio::test]
async fn room_tracking_membership_end_uses_actual_placement_end() {
    let placement_days = 75;
    let f = fixture("room_tracking_placement_end_bound").await;
    enable(&f, f.groups[0]).await;
    {
        let removal_effective_date = f.day + Duration::days(placement_days);
        let expected_placement_end = removal_effective_date - Duration::days(1);
        let before = members(&f, f.groups[0]).await;
        let original = &before[0];
        let placement_before: serde_json::Value = sqlx::query_scalar("SELECT to_jsonb(p) FROM homeroom_placements p WHERE student_academic_year_id=$1 AND homeroom_id=$2 AND status='current'")
            .bind(original.student_academic_year_id).bind(f.room).fetch_one(&f.pool).await.unwrap();
        let placement_id: Uuid = serde_json::from_value(placement_before["id"].clone()).unwrap();
        let placement_ids: Vec<Uuid> = sqlx::query_scalar(
            "SELECT id FROM homeroom_placements WHERE student_academic_year_id=$1 ORDER BY id",
        )
        .bind(original.student_academic_year_id)
        .fetch_all(&f.pool)
        .await
        .unwrap();
        move_student(
            &f,
            f.room,
            None,
            original.student_academic_year_id,
            removal_effective_date,
        )
        .await
        .unwrap();
        let after = members(&f, f.groups[0]).await;
        assert_eq!(
            after.len(),
            before.len(),
            "ending an actual interval preserves membership history"
        );
        let ended = after.iter().find(|m| m.id == original.id).unwrap();
        assert_eq!(ended.left_at, Some(expected_placement_end));
        assert_eq!(ended.joined_at, original.joined_at);
        assert_eq!(ended.learning_group_id, original.learning_group_id);
        assert_eq!(ended.student_id, original.student_id);
        assert_eq!(
            ended.student_academic_year_id,
            original.student_academic_year_id
        );
        assert_eq!(ended.roster_source, original.roster_source);
        assert_eq!(
            ended.membership_status,
            super::super::models::MembershipStatus::Ended
        );
        let placement_after: serde_json::Value =
            sqlx::query_scalar("SELECT to_jsonb(p) FROM homeroom_placements p WHERE id=$1")
                .bind(placement_id)
                .fetch_one(&f.pool)
                .await
                .unwrap();
        let actual_placement_end: NaiveDate =
            serde_json::from_value(placement_after["end_date"].clone()).unwrap();
        assert_eq!(
            actual_placement_end, expected_placement_end,
            "removal begins after the final inclusive day of the original placement"
        );
        assert_eq!(ended.left_at, Some(actual_placement_end));
        let before_updated: chrono::DateTime<Utc> =
            serde_json::from_value(placement_before["updated_at"].clone()).unwrap();
        let after_updated: chrono::DateTime<Utc> =
            serde_json::from_value(placement_after["updated_at"].clone()).unwrap();
        assert!(after_updated >= before_updated);
        let mut expected_placement = placement_before;
        expected_placement["status"] = serde_json::json!("ended");
        expected_placement["end_date"] = serde_json::json!(expected_placement_end);
        expected_placement["row_version"] =
            serde_json::json!(expected_placement["row_version"].as_i64().unwrap() + 1);
        expected_placement["updated_at"] = placement_after["updated_at"].clone();
        assert_eq!(
            placement_after, expected_placement,
            "retain the original placement ID and every unrelated fact"
        );
        assert_eq!(
            placement_ids,
            sqlx::query_scalar::<_, Uuid>(
                "SELECT id FROM homeroom_placements WHERE student_academic_year_id=$1 ORDER BY id"
            )
            .bind(original.student_academic_year_id)
            .fetch_all(&f.pool)
            .await
            .unwrap()
        );
    }
}

async fn canonical_score_phase(pool: &PgPool, group: Uuid, _actor: Uuid) -> (Uuid, Uuid) {
    sqlx::query_as("SELECT p.id,phase.id FROM learning_groups g JOIN course_assessment_plans p ON p.learning_offering_id=g.learning_offering_id JOIN course_assessment_phases phase ON phase.plan_id=p.id WHERE g.id=$1 ORDER BY phase.phase_code,phase.id LIMIT 1").bind(group).fetch_one(pool).await.unwrap()
}
async fn assessment_facts(pool: &PgPool, group: Uuid) -> serde_json::Value {
    sqlx::query_scalar("SELECT jsonb_build_object('items',(SELECT jsonb_agg(to_jsonb(i) ORDER BY id) FROM learning_group_score_items i WHERE learning_group_id=$1),'scores',(SELECT jsonb_agg(to_jsonb(s) ORDER BY id) FROM learning_group_student_scores s WHERE learning_group_id=$1),'confirmations',(SELECT jsonb_agg(to_jsonb(c) ORDER BY id) FROM learning_group_phase_confirmations c WHERE learning_group_id=$1))").bind(group).fetch_one(pool).await.unwrap()
}
