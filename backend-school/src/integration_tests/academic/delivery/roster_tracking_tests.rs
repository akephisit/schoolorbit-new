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
                starts_on: day,
                delivery_version_id: None,
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
    let revision = create_runtime_change_set_at(
        &pool,
        context.teacher_id,
        context.term_id,
        day,
        "tracking-initial-plan",
    )
    .await;
    let offering = offerings::create_for_delivery(
        &pool,
        context.teacher_id,
        Some(revision.target_delivery_version_id),
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
    publish_plan(&pool, context.teacher_id, revision.id).await;
    Fixture {
        pool,
        actor: context.teacher_id,
        room: context.homeroom_id,
        other,
        day,
        groups: ids,
    }
}
async fn publish_plan(pool: &PgPool, actor: Uuid, id: Uuid) -> AcademicTermChangeSet {
    let preview = publication_preview(pool, id).await.unwrap();
    assert!(
        !preview
            .findings
            .iter()
            .any(|finding| finding.severity == AcademicChangeFindingSeverity::Blocking),
        "{:?}",
        preview.findings
    );
    change_sets::publish_change_set(
        pool,
        actor,
        id,
        PublishAcademicTermChangeSetRequest {
            row_version: preview.change_set_row_version,
            target_delivery_version_row_version: preview.target_delivery_version_row_version,
            effective_from: preview.effective_from,
            preview_hash: preview.preview_hash,
            acknowledged_warning_codes: preview
                .findings
                .iter()
                .filter(|finding| finding.severity == AcademicChangeFindingSeverity::Warning)
                .map(|finding| finding.code)
                .collect(),
            idempotency_key: Uuid::now_v7(),
        },
    )
    .await
    .unwrap()
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
    // A's real Delivery publication initializes this canonical plan. Reuse its
    // identity when constructing retained score history rather than inserting a
    // second plan for an already initialized offering.
    let historical_plan: Uuid = sqlx::query_scalar(
        "SELECT plan.id FROM course_assessment_plans plan JOIN learning_groups g ON g.learning_offering_id=plan.learning_offering_id WHERE g.id=$1",
    ).bind(f.groups[0]).fetch_one(&f.pool).await.unwrap();
    let (plan, phase) =
        super::room_scope_tests::canonical_score_phase(&f.pool, f.groups[0], f.actor).await;
    assert_eq!(plan, historical_plan);
    let item = Uuid::new_v4();
    let score = Uuid::new_v4();
    sqlx::query("INSERT INTO learning_group_score_items(id,learning_group_id,learning_offering_id,course_assessment_plan_id,assessment_phase_id,academic_term_id,academic_year_id,name,max_score) SELECT $1,id,learning_offering_id,$2,$3,academic_term_id,academic_year_id,'Synthetic score',10 FROM learning_groups WHERE id=$4").bind(item).bind(plan).bind(phase).bind(f.groups[0]).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO learning_group_student_scores(id,learning_group_id,learning_offering_id,academic_term_id,academic_year_id,score_item_id,student_academic_year_id,score) SELECT $1,id,learning_offering_id,academic_term_id,academic_year_id,$2,$3,7.25 FROM learning_groups WHERE id=$4").bind(score).bind(item).bind(old.student_academic_year_id).bind(f.groups[0]).execute(&f.pool).await.unwrap();
    let assessment_before = super::room_scope_tests::assessment_facts(&f.pool, f.groups[0]).await;
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
    school_students::services::delete_student(
        &f.pool,
        old.student_id,
        f.actor,
        &crate::http_state::PLACEMENT_ROSTERS,
    )
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
    assert_eq!(
        assessment_before,
        super::room_scope_tests::assessment_facts(&f.pool, f.groups[0]).await
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
async fn room_tracking_membership_end_is_bounded_by_both_offering_and_placement() {
    for (name, offering_days, placement_days) in [
        ("room_tracking_placement_end_bound", 90, 75),
        ("room_tracking_offering_end_bound", 60, 75),
    ] {
        let f = fixture(name).await;
        enable(&f, f.groups[0]).await;
        let group = groups::get(&f.pool, f.groups[0]).await.unwrap();
        let offering_end = f.day + Duration::days(offering_days);
        let change = create_runtime_change_set_at(
            &f.pool,
            f.actor,
            group.academic_term_id,
            offering_end.succ_opt().unwrap(),
            "tracking-stop-plan",
        )
        .await;
        let change = change_sets::upsert_change_item(
            &f.pool,
            f.actor,
            change.id,
            UpsertAcademicTermChangeItemRequest::StopOffering {
                change_set_row_version: change.row_version,
                item_row_version: None,
                learning_offering_id: group.learning_offering_id,
            },
        )
        .await
        .unwrap();
        let raw_offering: serde_json::Value =
            sqlx::query_scalar("SELECT to_jsonb(o) FROM learning_offerings o WHERE id=$1")
                .bind(group.learning_offering_id)
                .fetch_one(&f.pool)
                .await
                .unwrap();
        publish_plan(&f.pool, f.actor, change.id).await;
        let after_publication: serde_json::Value =
            sqlx::query_scalar("SELECT to_jsonb(o) FROM learning_offerings o WHERE id=$1")
                .bind(group.learning_offering_id)
                .fetch_one(&f.pool)
                .await
                .unwrap();
        assert_eq!(
            raw_offering, after_publication,
            "publication preserves the offering registry provenance"
        );
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
        assert_eq!(
            ended.left_at,
            Some(offering_end.min(expected_placement_end))
        );
        assert_eq!(ended.joined_at, original.joined_at);
        assert_eq!(ended.learning_group_id, original.learning_group_id);
        assert_eq!(ended.published_at, original.published_at);
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
        assert_eq!(ended.left_at, Some(offering_end.min(actual_placement_end)));
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

#[tokio::test]
async fn room_tracking_departures_choose_the_last_actual_slice_across_a_recurring_offering_gap() {
    use school_academic_delivery::services::{attendance_roster, publication, versions};

    let f = fixture_with_curriculum_catalog("roster_tracking_recurring_gap", true).await;
    let group = groups::get(&f.pool, f.groups[0]).await.unwrap();
    let offering = group.learning_offering_id;
    let stop_on = f.day + Duration::days(10);
    let resume_on = f.day + Duration::days(20);
    let final_stop_on = f.day + Duration::days(40);
    let second_user = Uuid::now_v7();
    let second_year = Uuid::now_v7();
    sqlx::query("INSERT INTO users(id,username,password_hash,first_name,last_name,user_type,status) VALUES($1,$1::text,'fixture-not-login','Recurring gap','Second learner','student','active')")
        .bind(second_user).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO student_academic_years(id,student_id,academic_year_id,grade_level_id,study_program_id,status) SELECT $1,$2,academic_year_id,grade_level_id,study_program_id,'active' FROM homerooms WHERE id=$3")
        .bind(second_year).bind(second_user).bind(f.room).execute(&f.pool).await.unwrap();
    let room = homeroom_roster::get_roster(&f.pool, f.room).await.unwrap();
    homeroom_roster::mutate_roster(
        &f.pool,
        &RoomRosterTracking,
        f.actor,
        f.room,
        MutateHomeroomRosterRequest {
            revision: room.revision,
            action: HomeroomRosterAction::Add,
            selections: vec![HomeroomRosterSelection {
                student_academic_year_id: second_year,
                student_year_row_version: 1,
                placement_id: None,
                placement_row_version: None,
            }],
            effective_date: f.day,
            target_homeroom_id: None,
            reason: "Synthetic recurring-gap learner".into(),
        },
    )
    .await
    .unwrap();
    enable(&f, group.id).await;
    let original = members(&f, group.id).await;
    assert_eq!(
        original.len(),
        2,
        "exercise two distinct placement boundaries"
    );
    let first = original
        .iter()
        .find(|member| member.student_academic_year_id != second_year)
        .unwrap()
        .clone();
    let second = original
        .iter()
        .find(|member| member.student_academic_year_id == second_year)
        .unwrap()
        .clone();
    let student_years = vec![
        first.student_academic_year_id,
        second.student_academic_year_id,
    ];
    let placements: Vec<(Uuid, Uuid, serde_json::Value)> = sqlx::query_as(
        "SELECT p.id,p.student_academic_year_id,to_jsonb(p) FROM homeroom_placements p WHERE p.student_academic_year_id=ANY($1) AND p.homeroom_id=$2 AND p.status='current' ORDER BY p.id",
    ).bind(&student_years).bind(f.room).fetch_all(&f.pool).await.unwrap();
    assert_eq!(placements.len(), 2);
    let placement_ids = placements.iter().map(|row| row.0).collect::<Vec<_>>();
    let earlier_history: serde_json::Value = sqlx::query_scalar(
        "SELECT COALESCE(jsonb_agg(to_jsonb(p) ORDER BY id),'[]'::jsonb) FROM homeroom_placements p WHERE NOT(id=ANY($1))",
    ).bind(&placement_ids).fetch_one(&f.pool).await.unwrap();
    assert!(!earlier_history.as_array().unwrap().is_empty());
    let manual_history = serde_json::to_value(members(&f, f.groups[3]).await).unwrap();
    let raw_offering: serde_json::Value =
        sqlx::query_scalar("SELECT to_jsonb(o) FROM learning_offerings o WHERE id=$1")
            .bind(offering)
            .fetch_one(&f.pool)
            .await
            .unwrap();
    let a = versions::latest_published_for_term(&f.pool, group.academic_term_id)
        .await
        .unwrap()
        .unwrap();

    let b = create_runtime_change_set_at(
        &f.pool,
        f.actor,
        group.academic_term_id,
        stop_on,
        "tracking-gap-stop-B",
    )
    .await;
    let b = change_sets::upsert_change_item(
        &f.pool,
        f.actor,
        b.id,
        UpsertAcademicTermChangeItemRequest::StopOffering {
            change_set_row_version: b.row_version,
            item_row_version: None,
            learning_offering_id: offering,
        },
    )
    .await
    .unwrap();
    let b = publish_plan(&f.pool, f.actor, b.id).await;
    let c = create_runtime_change_set_at(
        &f.pool,
        f.actor,
        group.academic_term_id,
        resume_on,
        "tracking-gap-resume-C",
    )
    .await;
    assert_eq!(
        c.base_delivery_version_id,
        Some(b.target_delivery_version_id)
    );
    assert_eq!(c.reference_date, resume_on);
    let program: Uuid = sqlx::query_scalar("SELECT study_program_id FROM homerooms WHERE id=$1")
        .bind(f.room)
        .fetch_one(&f.pool)
        .await
        .unwrap();
    let preview = offerings::preview_from_curriculum(
        &f.pool,
        PreviewCurriculumOfferingsRequest {
            starts_on: resume_on,
            delivery_version_id: Some(c.target_delivery_version_id),
            academic_term_id: group.academic_term_id,
            study_program_ids: vec![program],
        },
    )
    .await
    .unwrap();
    assert!(
        preview
            .proposals
            .iter()
            .any(|proposal| proposal.existing_offering_id == Some(offering)),
        "explicit reactivation must select the preserved registry identity"
    );
    let choices = preview
        .proposals
        .iter()
        .map(|proposal| CurriculumPreparationChoice {
            proposal_id: proposal.proposal_id.clone(),
            action: if proposal.existing_offering_id == Some(offering) {
                PreparationAction::DeferGroups
            } else {
                PreparationAction::Skip
            },
            groups: vec![],
        })
        .collect();
    let applied = offerings::apply_from_curriculum(&f.pool, f.actor, ApplyCurriculumOfferingsRequest {
        starts_on: resume_on,
        academic_term_id: group.academic_term_id,
        study_program_ids: vec![program],
        delivery_version_id: Some(c.target_delivery_version_id),
        source_hash: preview.source_hash,
        idempotency_key: Uuid::now_v7(),
        choices,
    }).await.expect("an explicit reviewed reactivation must not require the offering already be available in its gap");
    assert_eq!(applied.offering_ids, vec![offering]);
    let mut c = change_sets::get_change_set(&f.pool, c.id).await.unwrap();
    // B cut off every old assignment. Construct C's exact teachers through public
    // commands for all four groups; no live episode is silently recaptured.
    for id in &f.groups {
        c = change_sets::upsert_change_item(
            &f.pool,
            f.actor,
            c.id,
            UpsertAcademicTermChangeItemRequest::AddGroupTeacher {
                change_set_row_version: c.row_version,
                item_row_version: None,
                learning_group_id: *id,
                teacher_id: f.actor,
                teacher_role: LearningTeacherRole::Primary,
            },
        )
        .await
        .unwrap();
    }
    let c = publish_plan(&f.pool, f.actor, c.id).await;
    let new_teachers: i64 = sqlx::query_scalar("SELECT count(*) FROM learning_group_teachers WHERE learning_group_id=ANY($1) AND started_by_change_set_id=$2 AND teacher_id=$3 AND role='primary'")
        .bind(&f.groups).bind(c.id).bind(f.actor).fetch_one(&f.pool).await.unwrap();
    assert_eq!(new_teachers, 4);
    let d = create_runtime_change_set_at(
        &f.pool,
        f.actor,
        group.academic_term_id,
        final_stop_on,
        "tracking-gap-stop-D",
    )
    .await;
    let d = change_sets::upsert_change_item(
        &f.pool,
        f.actor,
        d.id,
        UpsertAcademicTermChangeItemRequest::StopOffering {
            change_set_row_version: d.row_version,
            item_row_version: None,
            learning_offering_id: offering,
        },
    )
    .await
    .unwrap();
    let d = publish_plan(&f.pool, f.actor, d.id).await;
    assert!(b.publication_no < c.publication_no && c.publication_no < d.publication_no);
    let spans: Vec<(Uuid, NaiveDate, Option<NaiveDate>)> = sqlx::query_as(
        "SELECT delivery_version_id,starts_on,ends_on FROM academic_delivery_effective_offering_spans WHERE learning_offering_id=$1 ORDER BY starts_on,delivery_version_id",
    ).bind(offering).fetch_all(&f.pool).await.unwrap();
    assert_eq!(
        spans,
        vec![
            (a, f.day, Some(stop_on - Duration::days(1))),
            (
                c.target_delivery_version_id,
                resume_on,
                Some(final_stop_on - Duration::days(1))
            )
        ]
    );
    let mut connection = f.pool.acquire().await.unwrap();
    for day in [stop_on, resume_on - Duration::days(1)] {
        let selected =
            publication::applicable_on_connection(&mut connection, group.academic_term_id, day)
                .await
                .unwrap()
                .unwrap();
        assert_eq!(selected.id, b.target_delivery_version_id);
        let available: bool =
            sqlx::query_scalar("SELECT academic_delivery_offering_available($1,$2)")
                .bind(offering)
                .bind(day)
                .fetch_one(&mut *connection)
                .await
                .unwrap();
        assert!(!available);
        let membership_count: i64 = sqlx::query_scalar("SELECT count(*) FROM learning_group_students m JOIN academic_delivery_effective_offering_spans span ON span.learning_offering_id=$1 WHERE m.learning_group_id=$2 AND m.student_academic_year_id=ANY($3) AND m.membership_status<>'removed' AND m.joined_at<=$4 AND (m.left_at IS NULL OR m.left_at>=$4) AND span.starts_on<=$4 AND (span.ends_on IS NULL OR span.ends_on>=$4)")
            .bind(offering).bind(group.id).bind(&student_years).bind(day).fetch_one(&mut *connection).await.unwrap();
        assert_eq!(
            membership_count, 0,
            "canonical spans must preserve the recurring gap"
        );
    }
    let selected =
        publication::applicable_on_connection(&mut connection, group.academic_term_id, resume_on)
            .await
            .unwrap()
            .unwrap();
    assert_eq!(selected.id, c.target_delivery_version_id);
    let resumed = attendance_roster::pinned_membership_spans(
        &mut connection,
        selected.id,
        &[group.id],
        resume_on,
        resume_on,
        None,
    )
    .await
    .unwrap();
    assert_eq!(
        resumed.len(),
        2,
        "the actual C source admits both retained learners after reactivation"
    );
    drop(connection);

    for (original, removal, expected_left) in [
        (
            &first,
            stop_on + Duration::days(5),
            stop_on - Duration::days(1),
        ),
        (
            &second,
            final_stop_on + Duration::days(5),
            final_stop_on - Duration::days(1),
        ),
    ] {
        move_student(&f, f.room, None, original.student_academic_year_id, removal)
            .await
            .unwrap();
        let actual = members(&f, group.id)
            .await
            .into_iter()
            .find(|member| member.id == original.id)
            .unwrap();
        let mut expected = serde_json::to_value(original).unwrap();
        expected["membershipStatus"] = serde_json::json!("ended");
        expected["leftAt"] = serde_json::json!(expected_left);
        expected["rowVersion"] = serde_json::json!(original.row_version + 1);
        assert_eq!(
            serde_json::to_value(&actual).unwrap(),
            expected,
            "departure must keep the accepted ID, joined date and every unrelated roster fact"
        );
        let (placement_id, _, placement_before) = placements
            .iter()
            .find(|row| row.1 == original.student_academic_year_id)
            .unwrap();
        let placement_after: serde_json::Value =
            sqlx::query_scalar("SELECT to_jsonb(p) FROM homeroom_placements p WHERE id=$1")
                .bind(placement_id)
                .fetch_one(&f.pool)
                .await
                .unwrap();
        let mut expected_placement = placement_before.clone();
        expected_placement["status"] = serde_json::json!("ended");
        expected_placement["end_date"] = serde_json::json!(removal - Duration::days(1));
        expected_placement["row_version"] =
            serde_json::json!(placement_before["row_version"].as_i64().unwrap() + 1);
        expected_placement["updated_at"] = placement_after["updated_at"].clone();
        assert_eq!(placement_after, expected_placement);
    }
    assert_eq!(members(&f, group.id).await.len(), 2);
    let preserved: serde_json::Value = sqlx::query_scalar("SELECT COALESCE(jsonb_agg(to_jsonb(p) ORDER BY id),'[]'::jsonb) FROM homeroom_placements p WHERE NOT(id=ANY($1))")
        .bind(&placement_ids).fetch_one(&f.pool).await.unwrap();
    assert_eq!(preserved, earlier_history);
    assert_eq!(
        serde_json::to_value(members(&f, f.groups[3]).await).unwrap(),
        manual_history
    );
    let offering_after: serde_json::Value =
        sqlx::query_scalar("SELECT to_jsonb(o) FROM learning_offerings o WHERE id=$1")
            .bind(offering)
            .fetch_one(&f.pool)
            .await
            .unwrap();
    assert_eq!(offering_after, raw_offering);
}
