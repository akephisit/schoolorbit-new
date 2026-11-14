use super::*;
use school_academic_delivery::services::bulk_group_setup;

struct BulkFixture {
    pool: PgPool,
    context: RuntimeContext,
    offering_id: Uuid,
    group_ids: Vec<Uuid>,
}

async fn fixture(name: &str) -> BulkFixture {
    let pool = prepare_concurrent_delivery_runtime_fixture(name).await;
    let context = planning_runtime_context(&pool).await;
    let offering_id = offerings::create(&pool, context.teacher_id, course_request(&context))
        .await
        .unwrap()
        .id;
    let mut group_ids = Vec::new();
    for name in ["Bulk first", "Bulk second", "Bulk unselected"] {
        let group = groups::create(
            &pool,
            context.teacher_id,
            offering_id,
            CreateLearningGroupRequest {
                registration_open: false,
                name: name.into(),
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
                homeroom_ids: vec![context.homeroom_id],
            },
        )
        .await
        .unwrap();
        group_ids.push(group.id);
    }
    assert!(group_ids.len() >= 3);
    BulkFixture {
        pool,
        context,
        offering_id,
        group_ids,
    }
}

async fn teacher(pool: &PgPool, label: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO users(id,email,username,password_hash,first_name,last_name,user_type,status) VALUES($1,$2,$3,'fixture-not-a-login',$4,'ทดสอบ','staff','active')")
        .bind(id).bind(format!("{id}@bulk.example.invalid")).bind(id.to_string()).bind(label).execute(pool).await.unwrap();
    id
}

async fn request(f: &BulkFixture, teacher_id: Uuid) -> BulkGroupSetupRequest {
    let workspace = current_workspace(&f.pool, f.context.year_id, f.context.term_id)
        .await
        .unwrap();
    let mut selected = Vec::new();
    for id in &f.group_ids[..2] {
        let group = groups::get(&f.pool, *id).await.unwrap();
        selected.push(BulkGroupSetupTarget {
            learning_group_id: *id,
            row_version: group.row_version,
        });
    }
    BulkGroupSetupRequest {
        delivery_row_version: workspace.row_version,
        groups: selected,
        primary_teacher: Some(BulkGroupSetupPrimaryTeacher {
            teacher_id,
            replace_existing_primary: false,
        }),
        roster_tracking: None,
    }
}

async fn facts(f: &BulkFixture) -> String {
    sqlx::query_scalar("SELECT jsonb_build_object('groups',(SELECT jsonb_agg(to_jsonb(g) ORDER BY id) FROM learning_groups g WHERE learning_offering_id=$1),'teachers',(SELECT jsonb_agg(to_jsonb(t) ORDER BY t.id) FROM learning_group_teachers t JOIN learning_groups g ON g.id=t.learning_group_id WHERE g.learning_offering_id=$1),'tracking',(SELECT jsonb_agg(to_jsonb(t) ORDER BY t.learning_group_id) FROM learning_group_roster_tracking t JOIN learning_groups g ON g.id=t.learning_group_id WHERE g.learning_offering_id=$1),'deliveryState',(SELECT to_jsonb(state) FROM academic_delivery_state state WHERE academic_term_id=$2))::text")
        .bind(f.offering_id).bind(f.context.term_id).fetch_one(&f.pool).await.unwrap()
}

#[tokio::test]
async fn bulk_group_setup_preserves_exceptions_and_applies_only_explicit_current_roster_tracking() {
    let f = fixture("bulk_setup_preserve").await;
    let secondary = teacher(&f.pool, "ร่วมสอน").await;
    let assistant = teacher(&f.pool, "ผู้ช่วย").await;
    let selected_teacher = teacher(&f.pool, "ครูหลักชุดใหม่").await;
    let first = groups::get(&f.pool, f.group_ids[0]).await.unwrap();
    let first = groups::replace_teachers(
        &f.pool,
        f.context.teacher_id,
        first.id,
        ReplaceLearningGroupTeachersRequest {
            row_version: first.row_version,
            teachers: vec![
                TeacherAssignmentInput {
                    teacher_id: f.context.teacher_id,
                    role: LearningTeacherRole::Primary,
                },
                TeacherAssignmentInput {
                    teacher_id: secondary,
                    role: LearningTeacherRole::Secondary,
                },
                TeacherAssignmentInput {
                    teacher_id: assistant,
                    role: LearningTeacherRole::Assistant,
                },
            ],
        },
    )
    .await
    .unwrap();
    let preserved: Vec<_> = first
        .teacher_assignments
        .iter()
        .filter(|a| a.role != LearningTeacherRole::Primary)
        .map(|a| (a.id, a.teacher_id, a.role, a.row_version))
        .collect();
    let untouched =
        serde_json::to_value(groups::get(&f.pool, f.group_ids[2]).await.unwrap()).unwrap();
    let roster =
        school_academic_core::services::homeroom_roster::get_roster(&f.pool, f.context.homeroom_id)
            .await
            .unwrap();
    assert!(
        !roster.students.is_empty(),
        "explicit tracking must materialize a real roster"
    );
    let mut input = request(&f, selected_teacher).await;
    let today = sqlx::query_scalar("SELECT (now() AT TIME ZONE 'Asia/Bangkok')::date")
        .fetch_one(&f.pool)
        .await
        .unwrap();
    input.roster_tracking = Some(BulkGroupSetupTrackingInput {
        mode: RosterTrackingMode::Homeroom,
        effective_from: Some(today),
    });
    let before = facts(&f).await;
    let preview = bulk_group_setup::preview(&f.pool, f.offering_id, input.clone())
        .await
        .unwrap();
    assert_eq!(
        facts(&f).await,
        before,
        "preview must be entirely read-only"
    );
    assert_eq!(preview.changed_count, 2);
    assert_eq!(preview.blocked_count, 0);
    assert_eq!(
        preview
            .targets
            .iter()
            .find(|t| t.group.id == first.id)
            .unwrap()
            .primary_teacher_action,
        BulkGroupSetupTeacherAction::PreserveExisting
    );
    let result = bulk_group_setup::apply(
        &crate::adapters::assessment_current::ASSESSMENT_CURRENT,
        &f.pool,
        f.context.teacher_id,
        f.offering_id,
        input.clone(),
    )
    .await
    .unwrap();
    let authoritative_signal: (Uuid, Uuid, i64) = sqlx::query_as(
        "SELECT id,academic_term_id,row_version FROM learning_offerings WHERE id=$1",
    )
    .bind(f.offering_id)
    .fetch_one(&f.pool)
    .await
    .unwrap();
    assert_eq!(
        (
            result.signal.learning_offering_id,
            result.signal.academic_term_id,
            result.signal.row_version
        ),
        authoritative_signal,
        "the accepted command carries the actual offering identity and revision",
    );
    let result = result.response;
    assert!(result.delivery_row_version > input.delivery_row_version);
    for group in &result.groups {
        assert!(
            group.row_version
                > input
                    .groups
                    .iter()
                    .find(|t| t.learning_group_id == group.id)
                    .unwrap()
                    .row_version
        );
        for assignment in group
            .teacher_assignments
            .iter()
            .filter(|assignment| assignment.teacher_id == selected_teacher)
        {
            assert_eq!(
                assignment.id.get_version_num(),
                7,
                "new teacher episodes use UUIDv7 business identities"
            );
        }
    }
    let first_after = result.groups.iter().find(|g| g.id == first.id).unwrap();
    assert!(first_after
        .teacher_assignments
        .iter()
        .any(|a| a.teacher_id == f.context.teacher_id && a.role == LearningTeacherRole::Primary));
    assert_eq!(
        first_after
            .teacher_assignments
            .iter()
            .filter(|a| a.role != LearningTeacherRole::Primary)
            .map(|a| (a.id, a.teacher_id, a.role, a.row_version))
            .collect::<Vec<_>>(),
        preserved
    );
    assert!(
        result
            .groups
            .iter()
            .find(|g| g.id == f.group_ids[1])
            .unwrap()
            .teacher_assignments
            .iter()
            .any(|a| a.teacher_id == selected_teacher && a.role == LearningTeacherRole::Primary)
    );
    assert_eq!(
        serde_json::to_value(groups::get(&f.pool, f.group_ids[2]).await.unwrap()).unwrap(),
        untouched
    );
    let actual: Vec<(Uuid,Uuid,Uuid,NaiveDate,Option<NaiveDate>,String,String)> = sqlx::query_as("SELECT learning_group_id,student_academic_year_id,student_id,joined_at,left_at,membership_status,roster_source FROM learning_group_students WHERE learning_group_id=ANY($1) ORDER BY learning_group_id,student_academic_year_id").bind(&f.group_ids).fetch_all(&f.pool).await.unwrap();
    let mut expected = Vec::new();
    for group in &f.group_ids[..2] {
        for student in &roster.students {
            expected.push((
                *group,
                student.student_academic_year_id,
                student.student_id,
                student.start_date.unwrap_or(today).max(today),
                None,
                "active".into(),
                "homeroom_tracking".into(),
            ));
        }
    }
    expected.sort_by_key(|row| (row.0, row.1));
    assert_eq!(
        actual, expected,
        "only explicitly selected groups receive exact current dated memberships"
    );
    let membership_facts: serde_json::Value = sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(m) ORDER BY id) FROM learning_group_students m WHERE learning_group_id=ANY($1)").bind(&f.group_ids).fetch_one(&f.pool).await.unwrap();
    assert!(
        result
            .roster_tracking
            .iter()
            .all(|config| config.tracking.mode == RosterTrackingMode::Homeroom)
    );
    let mut replace = request(&f, selected_teacher).await;
    replace
        .primary_teacher
        .as_mut()
        .unwrap()
        .replace_existing_primary = true;
    let replacement = bulk_group_setup::apply(
        &crate::adapters::assessment_current::ASSESSMENT_CURRENT,
        &f.pool,
        f.context.teacher_id,
        f.offering_id,
        replace,
    )
    .await
    .unwrap();
    assert!(
        replacement
            .response
            .groups
            .iter()
            .flat_map(|group| &group.teacher_assignments)
            .filter(|assignment| assignment.teacher_id == selected_teacher)
            .all(|assignment| assignment.id.get_version_num() == 7)
    );
    assert_eq!(membership_facts, sqlx::query_scalar::<_,serde_json::Value>("SELECT jsonb_agg(to_jsonb(m) ORDER BY id) FROM learning_group_students m WHERE learning_group_id=ANY($1)").bind(&f.group_ids).fetch_one(&f.pool).await.unwrap(), "teacher-only replacement preserves every saved membership identity, span and status");
    let first_after = replacement
        .response
        .groups
        .iter()
        .find(|g| g.id == first.id)
        .unwrap();
    assert_eq!(
        first_after
            .teacher_assignments
            .iter()
            .filter(|a| a.role != LearningTeacherRole::Primary)
            .map(|a| (a.id, a.teacher_id, a.role, a.row_version))
            .collect::<Vec<_>>(),
        preserved
    );
    assert!(
        first_after
            .teacher_assignments
            .iter()
            .any(|a| a.teacher_id == selected_teacher && a.role == LearningTeacherRole::Primary)
    );
    let no_op = request(&f, selected_teacher).await;
    assert!(matches!(
        bulk_group_setup::apply(
            &crate::adapters::assessment_current::ASSESSMENT_CURRENT,
            &f.pool,
            f.context.teacher_id,
            f.offering_id,
            no_op
        )
        .await,
        Err(AppError::Conflict(_))
    ));
}

#[tokio::test]
async fn bulk_group_setup_rejects_stale_scope_and_any_blocked_target_atomically() {
    let f = fixture("bulk_setup_guards").await;
    let selected_teacher = teacher(&f.pool, "ครูใหม่").await;
    let fresh = request(&f, selected_teacher).await;
    for mut invalid in [
        fresh.clone(),
        fresh.clone(),
        fresh.clone(),
        fresh.clone(),
        fresh.clone(),
    ]
    .into_iter()
    .enumerate()
    {
        match invalid.0 {
            0 => invalid.1.delivery_row_version += 1,
            1 => invalid.1.groups[1].row_version += 1,
            2 => invalid.1.groups[1].learning_group_id = Uuid::new_v4(),
            3 => invalid.1.groups.push(invalid.1.groups[0].clone()),
            _ => invalid.1.groups.clear(),
        }
        let before = facts(&f).await;
        assert!(
            bulk_group_setup::apply(
                &crate::adapters::assessment_current::ASSESSMENT_CURRENT,
                &f.pool,
                f.context.teacher_id,
                f.offering_id,
                invalid.1
            )
            .await
            .is_err()
        );
        assert_eq!(facts(&f).await, before);
    }
    let first = groups::get(&f.pool, f.group_ids[0]).await.unwrap();
    groups::replace_teachers(
        &f.pool,
        f.context.teacher_id,
        first.id,
        ReplaceLearningGroupTeachersRequest {
            row_version: first.row_version,
            teachers: vec![TeacherAssignmentInput {
                teacher_id: selected_teacher,
                role: LearningTeacherRole::Secondary,
            }],
        },
    )
    .await
    .unwrap();
    let input = request(&f, selected_teacher).await;
    let before = facts(&f).await;
    let preview = bulk_group_setup::preview(&f.pool, f.offering_id, input.clone())
        .await
        .unwrap();
    assert_eq!(preview.blocked_count, 1);
    assert_eq!(preview.changed_count, 1);
    assert!(matches!(
        bulk_group_setup::apply(
            &crate::adapters::assessment_current::ASSESSMENT_CURRENT,
            &f.pool,
            f.context.teacher_id,
            f.offering_id,
            input
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    assert_eq!(facts(&f).await, before);
    let mut bad_tracking = request(&f, f.context.teacher_id).await;
    bad_tracking.roster_tracking = Some(BulkGroupSetupTrackingInput {
        mode: RosterTrackingMode::Homeroom,
        effective_from: Some(NaiveDate::from_ymd_opt(2000, 1, 1).unwrap()),
    });
    assert!(
        bulk_group_setup::apply(
            &crate::adapters::assessment_current::ASSESSMENT_CURRENT,
            &f.pool,
            f.context.teacher_id,
            f.offering_id,
            bad_tracking
        )
        .await
        .is_err()
    );
    assert_eq!(facts(&f).await, before);
    sqlx::query("UPDATE users SET status='inactive' WHERE id=$1")
        .bind(selected_teacher)
        .execute(&f.pool)
        .await
        .unwrap();
    assert!(matches!(
        bulk_group_setup::apply(
            &crate::adapters::assessment_current::ASSESSMENT_CURRENT,
            &f.pool,
            f.context.teacher_id,
            f.offering_id,
            request(&f, selected_teacher).await
        )
        .await,
        Err(AppError::ValidationError(_))
    ));
}

#[tokio::test]
async fn bulk_group_setup_concurrent_writers_save_exactly_one_atomic_batch() {
    let f = fixture("bulk_setup_concurrent").await;
    let first = teacher(&f.pool, "ครูหนึ่ง").await;
    let second = teacher(&f.pool, "ครูสอง").await;
    let first_input = request(&f, first).await;
    let second_input = request(&f, second).await;
    let (a, b) = tokio::join!(
        bulk_group_setup::apply(
            &crate::adapters::assessment_current::ASSESSMENT_CURRENT,
            &f.pool,
            f.context.teacher_id,
            f.offering_id,
            first_input
        ),
        bulk_group_setup::apply(
            &crate::adapters::assessment_current::ASSESSMENT_CURRENT,
            &f.pool,
            f.context.teacher_id,
            f.offering_id,
            second_input
        )
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    assert!(matches!(
        if a.is_err() { a } else { b },
        Err(AppError::Conflict(_))
    ));
    let teachers: Vec<Uuid> = sqlx::query_scalar("SELECT teacher_id FROM learning_group_teachers WHERE learning_group_id=ANY($1) AND role='primary' ORDER BY learning_group_id").bind(&f.group_ids[..2]).fetch_all(&f.pool).await.unwrap();
    assert_eq!(teachers.len(), 2);
    assert_eq!(teachers[0], teachers[1]);
}

#[tokio::test]
async fn bulk_group_setup_saved_lists_use_grade_and_natural_room_before_limits() {
    let f = fixture("bulk_setup_room_order").await;
    let mut grade_ids = Vec::new();
    for (kind, year) in [("primary", 1), ("secondary", 1), ("other", 1)] {
        let existing: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM grade_levels WHERE level_type=$1 AND year=$2 ORDER BY id LIMIT 1",
        )
        .bind(kind)
        .bind(year)
        .fetch_optional(&f.pool)
        .await
        .unwrap();
        let grade_id = if let Some(id) = existing {
            id
        } else {
            let id = Uuid::new_v4();
            sqlx::query("INSERT INTO grade_levels(id,level_type,year) VALUES($1,$2,$3)")
                .bind(id)
                .bind(kind)
                .bind(year)
                .execute(&f.pool)
                .await
                .unwrap();
            id
        };
        grade_ids.push(grade_id);
    }
    let mut rooms = Vec::new();
    for (index, room_number, name) in [
        (0, "2", "ห้องสอง"),
        (0, "10", "ห้องสิบ"),
        (0, "A2", "ห้องพิเศษสอง"),
        (0, "A10", "ห้องพิเศษสิบ"),
        (1, "1", "มัธยมหนึ่ง"),
        (2, "1", "อื่นหนึ่ง"),
    ] {
        let proposed_id = Uuid::new_v4();
        let id: Uuid = sqlx::query_scalar("INSERT INTO homerooms(id,code,name,academic_year_id,grade_level_id,room_number,study_program_id,capacity,is_active) VALUES($1,$2,$3,$4,$5,$6,$7,40,true) ON CONFLICT(academic_year_id,grade_level_id,room_number) DO UPDATE SET name=EXCLUDED.name RETURNING id")
            .bind(proposed_id).bind(format!("BULK-{proposed_id}")).bind(name).bind(f.context.year_id).bind(grade_ids[index]).bind(room_number).bind(f.context.study_program_id).fetch_one(&f.pool).await.unwrap();
        rooms.push(id);
    }
    let mut ordered_ids = Vec::new();
    for (name, room_indexes) in [
        ("Z room 2", vec![0]),
        ("ZZ multi-grade", vec![4, 0]),
        ("A room 10", vec![1]),
        ("Z special 2", vec![2]),
        ("A special 10", vec![3]),
        ("A secondary", vec![4]),
        ("A unknown grade", vec![5]),
        ("A no anchor", vec![]),
        ("Z no anchor", vec![]),
    ] {
        let group = groups::create(
            &f.pool,
            f.context.teacher_id,
            f.offering_id,
            CreateLearningGroupRequest {
                registration_open: false,
                name: name.into(),
                description: None,
                capacity: None,
                preferred_room_ids: vec![],
            },
        )
        .await
        .unwrap();
        for index in room_indexes {
            sqlx::query("INSERT INTO learning_group_homerooms(id,learning_group_id,academic_term_id,academic_year_id,homeroom_id,coverage_source) VALUES($1,$2,$3,$4,$5,'manual')")
                .bind(Uuid::new_v4()).bind(group.id).bind(f.context.term_id).bind(f.context.year_id).bind(rooms[index]).execute(&f.pool).await.unwrap();
        }
        ordered_ids.push(group.id);
    }
    let offering_groups = groups::list(&f.pool, f.offering_id).await.unwrap();
    assert_eq!(
        offering_groups
            .iter()
            .filter(|g| ordered_ids.contains(&g.id))
            .map(|g| g.id)
            .collect::<Vec<_>>(),
        ordered_ids
    );
    let term_groups = groups::list_for_term(
        &f.pool,
        f.context.term_id,
        &AcademicResourceListFilter {
            includes_school_owned: true,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(
        term_groups
            .iter()
            .filter(|g| ordered_ids.contains(&g.id))
            .map(|g| g.id)
            .collect::<Vec<_>>(),
        ordered_ids
    );
    // Enough alphabetically early unanchored rows to expose sorting after LIMIT.
    sqlx::query("INSERT INTO learning_groups(id,learning_offering_id,academic_term_id,academic_year_id,code,name,registration_open) SELECT gen_random_uuid(),$1,$2,$3,'LIMIT-'||n,'000 filler '||n,false FROM generate_series(1,505) n")
        .bind(f.offering_id).bind(f.context.term_id).bind(f.context.year_id).execute(&f.pool).await.unwrap();
    let limited = groups::list(&f.pool, f.offering_id).await.unwrap();
    assert_eq!(limited.len(), 500);
    for id in &ordered_ids[..7] {
        assert!(
            limited.iter().any(|group| group.id == *id),
            "every anchored academic group must precede filler before LIMIT"
        );
    }
}

#[tokio::test]
async fn bulk_group_setup_rejects_closed_year_or_term_without_changing_saved_data() {
    let f = fixture("bulk_setup_lifecycle").await;
    let selected_teacher = teacher(&f.pool, "ครูทดสอบสถานะ").await;
    for (year_status, term_status) in [
        ("closed", "planning"),
        ("archived", "planning"),
        ("planning", "closed"),
        ("planning", "cancelled"),
    ] {
        sqlx::query("UPDATE academic_years SET status=$2 WHERE id=$1")
            .bind(f.context.year_id)
            .bind(year_status)
            .execute(&f.pool)
            .await
            .unwrap();
        sqlx::query("UPDATE academic_terms SET status=$2 WHERE id=$1")
            .bind(f.context.term_id)
            .bind(term_status)
            .execute(&f.pool)
            .await
            .unwrap();
        let input = request(&f, selected_teacher).await;
        let before = facts(&f).await;
        assert!(matches!(
            bulk_group_setup::apply(
                &crate::adapters::assessment_current::ASSESSMENT_CURRENT,
                &f.pool,
                f.context.teacher_id,
                f.offering_id,
                input.clone()
            )
            .await,
            Err(AppError::Conflict(_))
        ));
        assert!(matches!(
            bulk_group_setup::preview(&f.pool, f.offering_id, input).await,
            Err(AppError::Conflict(_))
        ));
        assert_eq!(facts(&f).await, before);
    }
}

#[tokio::test]
async fn bulk_group_setup_shared_tracking_policy_refuses_self_registration() {
    let f = fixture("bulk_setup_self_tracking").await;
    let (activity_version_id,scheduling_mode):(Uuid,ActivitySchedulingMode)=sqlx::query_as("SELECT v.id,v.scheduling_mode FROM activity_versions v JOIN academic_terms t ON t.id=$1 WHERE v.status='published' AND v.effective_from<=(now() AT TIME ZONE 'Asia/Bangkok')::date ORDER BY v.id LIMIT 1").bind(f.context.term_id).fetch_one(&f.pool).await.unwrap();
    let offering = offerings::create(
        &f.pool,
        f.context.teacher_id,
        CreateLearningOfferingRequest::Activity(CreateActivityOfferingRequest {
            weekly_period_target: 1,
            create_homeroom_groups: false,
            academic_term_id: f.context.term_id,
            activity_version_id,
            curriculum_activity_requirement_id: None,
            targets: vec![OfferingTargetInput {
                target_kind: OfferingTargetKind::Homeroom,
                homeroom_id: Some(f.context.homeroom_id),
                grade_level_id: f.context.grade_level_id,
                study_program_id: f.context.study_program_id,
            }],
            registration_type: ActivityRegistrationType::SelfRegistration,
            scheduling_mode,
            capacity: Some(40),
            attendance_requirement: ActivityAttendanceRequirement {
                minimum_percent: Some("80.00".into()),
                required_sessions: None,
            },
            pass_criteria: ActivityPassCriteria {
                require_attendance: true,
                require_teacher_confirmation: true,
                outcomes: vec!["pass".into(), "fail".into()],
            },
        }),
    )
    .await
    .unwrap();
    let group = groups::create(
        &f.pool,
        f.context.teacher_id,
        offering.id,
        CreateLearningGroupRequest {
            registration_open: false,
            name: "Self registration".into(),
            description: None,
            capacity: Some(40),
            preferred_room_ids: vec![],
        },
    )
    .await
    .unwrap();
    let group = groups::replace_homerooms(
        &f.pool,
        f.context.teacher_id,
        group.id,
        ReplaceLearningGroupHomeroomsRequest {
            row_version: group.row_version,
            homeroom_ids: vec![f.context.homeroom_id],
        },
    )
    .await
    .unwrap();
    let today = sqlx::query_scalar("SELECT (now() AT TIME ZONE 'Asia/Bangkok')::date")
        .fetch_one(&f.pool)
        .await
        .unwrap();
    let result = school_academic_delivery::services::roster_tracking::update(
        &f.pool,
        f.context.teacher_id,
        group.id,
        UpdateRosterTrackingRequest {
            mode: RosterTrackingMode::Homeroom,
            effective_from: Some(today),
            row_version: group.row_version,
        },
    )
    .await;
    assert!(
        matches!(result,Err(AppError::ValidationError(reason)) if reason.contains("สมัครด้วยตนเอง"))
    );
    assert_eq!(
        school_academic_delivery::services::roster_tracking::get(&f.pool, group.id)
            .await
            .unwrap()
            .mode,
        RosterTrackingMode::Manual
    );
    assert_eq!(
        groups::get(&f.pool, group.id).await.unwrap().row_version,
        group.row_version
    );
}
