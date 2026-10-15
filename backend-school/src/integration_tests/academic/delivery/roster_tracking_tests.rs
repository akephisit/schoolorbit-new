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
    let pool = prepare_delivery_runtime_fixture(name).await;
    let context = planning_runtime_context(&pool).await;
    let today: NaiveDate = sqlx::query_scalar("SELECT (now() AT TIME ZONE 'Asia/Bangkok')::date")
        .fetch_one(&pool)
        .await
        .unwrap();
    let start: NaiveDate = sqlx::query_scalar("SELECT start_date FROM academic_terms WHERE id=$1")
        .bind(context.term_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let day = today.max(start);
    sqlx::query(
        "UPDATE academic_years SET status='closed' WHERE id<>$1 AND status IN ('active','closing')",
    )
    .bind(context.year_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE academic_years SET status='active',end_date=$2 WHERE id=$1")
        .bind(context.year_id)
        .bind(day + Duration::days(180))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE academic_terms SET planned_end_date=$2 WHERE id=$1")
        .bind(context.term_id)
        .bind(day + Duration::days(60))
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
    let offering = offerings::get(&pool, offering.id).await.unwrap();
    offerings::publish(
        &pool,
        context.teacher_id,
        offering.id,
        PublishLearningOfferingRequest {
            row_version: offering.row_version,
            idempotency_key: Uuid::new_v4(),
        },
    )
    .await
    .unwrap();
    for id in &ids {
        let group = groups::get(&pool, *id).await.unwrap();
        let preview = groups::preview_roster(&pool, *id).await.unwrap();
        let group = groups::apply_roster(
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
        groups::publish_roster(
            &pool,
            context.teacher_id,
            *id,
            PublishRosterRequest {
                row_version: group.row_version,
                idempotency_key: Uuid::new_v4(),
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
async fn room_tracking_transfers_reentries_and_deactivation_preserve_manual_and_shared_history() {
    let f = fixture("roster_tracking_history").await;
    let old = members(&f, f.groups[0]).await[0].clone();
    let shared = members(&f, f.groups[2]).await[0].clone();
    let plan = Uuid::new_v4();
    let phase = Uuid::new_v4();
    let item = Uuid::new_v4();
    let score = Uuid::new_v4();
    sqlx::query("INSERT INTO course_assessment_plans(id,academic_term_id,subject_version_id,learning_offering_id,academic_year_id) SELECT $1,g.academic_term_id,d.subject_version_id,g.learning_offering_id,g.academic_year_id FROM learning_groups g JOIN course_offering_details d ON d.learning_offering_id=g.learning_offering_id WHERE g.id=$2").bind(plan).bind(f.groups[0]).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO course_assessment_phases(id,plan_id,phase_code,max_score,exam_arrangement) VALUES($1,$2,'before_midterm',25,'none')").bind(phase).bind(plan).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO learning_group_score_items(id,learning_group_id,learning_offering_id,course_assessment_plan_id,assessment_phase_id,academic_term_id,academic_year_id,name,max_score) SELECT $1,id,learning_offering_id,$2,$3,academic_term_id,academic_year_id,'Synthetic score',10 FROM learning_groups WHERE id=$4").bind(item).bind(plan).bind(phase).bind(f.groups[0]).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO learning_group_student_scores(id,learning_group_id,learning_offering_id,academic_term_id,academic_year_id,score_item_id,student_academic_year_id,score) SELECT $1,id,learning_offering_id,academic_term_id,academic_year_id,$2,$3,7.25 FROM learning_groups WHERE id=$4").bind(score).bind(item).bind(old.student_academic_year_id).bind(f.groups[0]).execute(&f.pool).await.unwrap();
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
    assert!(history
        .iter()
        .any(|m| m.id == old.id && m.left_at == Some(f.day)));
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
    school_students::services::delete_student(&f.pool, old.student_id, f.actor)
        .await
        .unwrap();
    assert!(members(&f, f.groups[0])
        .await
        .iter()
        .all(|m| m.left_at.is_some()));
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
    sqlx::query("UPDATE academic_terms SET status='closed',closed_on=planned_end_date WHERE id=(SELECT academic_term_id FROM learning_groups WHERE id=$1)").bind(id).execute(&f.pool).await.unwrap();
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
        assert!(history
            .iter()
            .any(|m| m.left_at.is_none() && m.joined_at == begins));
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
async fn room_tracking_membership_end_is_bounded_by_both_offering_and_term() {
    let f = fixture("room_tracking_end_bound").await;
    enable(&f, f.groups[0]).await;
    let group = groups::get(&f.pool, f.groups[0]).await.unwrap();
    let change = change_sets::create_change_set(
        &f.pool,
        f.actor,
        CreateAcademicTermChangeSetRequest {
            academic_term_id: group.academic_term_id,
            reason: "dated boundary fixture".into(),
            idempotency_key: Uuid::new_v4(),
        },
    )
    .await
    .unwrap();
    sqlx::query("UPDATE learning_offerings o SET ends_on=$2,stop_reason='dated boundary fixture',stopped_at=now(),stopped_by=$3,stop_change_set_id=$4 FROM learning_groups g WHERE g.learning_offering_id=o.id AND g.id=$1")
        .bind(f.groups[0]).bind(f.day + Duration::days(90)).bind(f.actor).bind(change.id).execute(&f.pool).await.unwrap();
    let before = members(&f, f.groups[0]).await;
    let student_year = before[0].student_academic_year_id;
    move_student(&f, f.room, None, student_year, f.day + Duration::days(75))
        .await
        .unwrap();
    let after = members(&f, f.groups[0]).await;
    let ended = after.iter().find(|m| m.id == before[0].id).unwrap();
    assert_eq!(ended.left_at, Some(f.day + Duration::days(60)));
}
