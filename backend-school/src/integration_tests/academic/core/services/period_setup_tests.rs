use super::*;
use school_academic_core::models::{
    BellScheduleEditBlockReason, CloneBellScheduleRequest, Homeroom, UpdateBellScheduleRequest,
    UpdateHomeroomCapacityRequest,
};
use school_academic_core::services::{activation_context, lifecycle_guard, opening_diagnostics};
use school_errors::AppError;

#[tokio::test]
async fn period_setup_period_snapshots_keep_the_accepted_cas_base_across_later_and_concurrent_edits(
) {
    let pool = prepare_current_core_fixture("period_setup_period_snapshot").await;
    let actor = fixture_actor(&pool).await;
    let schedule = bell_schedules::create(
        &pool,
        actor,
        CreateBellScheduleRequest {
            academic_year_id: CURRENT_YEAR_ID,
            name: "Paired periods".into(),
            owning_organization_unit_id: None,
        },
    )
    .await
    .unwrap();
    let input = |version, name: &str| ReplaceBellSchedulePeriodsRequest {
        row_version: version,
        periods: vec![BellSchedulePeriodInput {
            name: Some(name.into()),
            start_time: NaiveTime::from_hms_opt(8, 0, 0).unwrap(),
            end_time: NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
            order_index: 1,
            applicable_days: vec!["MON".into()],
            is_active: true,
        }],
    };
    let initial = bell_schedules::list_periods(&pool, schedule.id)
        .await
        .unwrap();
    assert_eq!(initial.row_version, schedule.row_version);
    assert!(initial.periods.is_empty());
    let accepted = bell_schedules::replace_periods(
        &pool,
        actor,
        schedule.id,
        input(initial.row_version, "Staff A periods"),
    )
    .await
    .unwrap()
    .snapshot;
    assert_eq!(accepted.bell_schedule_id, schedule.id);
    assert_eq!(accepted.row_version, initial.row_version + 1);
    let metadata = bell_schedules::update(
        &pool,
        actor,
        schedule.id,
        UpdateBellScheduleRequest {
            name: "Staff B metadata".into(),
            is_default: false,
            owning_organization_unit_id: None,
            row_version: accepted.row_version,
        },
    )
    .await
    .unwrap();
    assert_eq!(metadata.row_version, accepted.row_version + 1);
    assert!(matches!(
        bell_schedules::replace_periods(
            &pool,
            actor,
            schedule.id,
            input(accepted.row_version, "Stale A periods")
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    let mut base = bell_schedules::list_periods(&pool, schedule.id)
        .await
        .unwrap();
    assert_eq!(base.row_version, metadata.row_version);
    assert_eq!(base.periods[0].id, accepted.periods[0].id);
    for index in 0..4 {
        let change = input(base.row_version, &format!("Concurrent period {index}"));
        let (reader, writer) = tokio::join!(
            bell_schedules::list_periods(&pool, schedule.id),
            bell_schedules::replace_periods(&pool, actor, schedule.id, change)
        );
        let reader = reader.unwrap();
        let writer = writer.unwrap().snapshot;
        assert!(reader.row_version == base.row_version || reader.row_version == writer.row_version);
        let expected = if reader.row_version == base.row_version {
            &base
        } else {
            &writer
        };
        assert_eq!(
            serde_json::to_value(&reader.periods).unwrap(),
            serde_json::to_value(&expected.periods).unwrap()
        );
        assert_eq!(writer.row_version, base.row_version + 1);
        base = writer;
    }
    let first = input(base.row_version, "First CAS writer");
    let second = input(base.row_version, "Second CAS writer");
    let (first, second) = tokio::join!(
        bell_schedules::replace_periods(&pool, actor, schedule.id, first),
        bell_schedules::replace_periods(&pool, actor, schedule.id, second)
    );
    let winner = match (first, second) {
        (Ok(outcome), Err(AppError::Conflict(_))) | (Err(AppError::Conflict(_)), Ok(outcome)) => {
            outcome.snapshot
        }
        other => panic!("expected exactly one CAS winner: {other:?}"),
    };
    let latest = bell_schedules::list_periods(&pool, schedule.id)
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(latest).unwrap(),
        serde_json::to_value(winner).unwrap()
    );
}

#[tokio::test]
async fn period_setup_enrollment_readiness_links_use_the_real_owner_and_permissions() {
    let pool = prepare_current_core_fixture("period_setup_enrollment_links").await;
    let actor = fixture_actor(&pool).await;
    let room = occupied_room(&pool).await;
    let schedule: Uuid = sqlx::query_scalar(
        "SELECT bell_schedule_id FROM academic_terms WHERE academic_year_id=$1 ORDER BY id LIMIT 1",
    )
    .bind(CURRENT_YEAR_ID)
    .fetch_one(&pool)
    .await
    .unwrap();
    let term = add_unopened_term(&pool, actor, schedule).await;
    let pupil: Uuid = sqlx::query_scalar("INSERT INTO users(password_hash,first_name,last_name,user_type,status) VALUES('synthetic-not-login','Planned','Pupil','student','active') RETURNING id").fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO student_academic_years(id,student_id,academic_year_id,grade_level_id,study_program_id,status) VALUES(uuidv7(),$1,$2,$3,$4,'planned')")
        .bind(pupil).bind(CURRENT_YEAR_ID).bind(room.grade_level_id).bind(room.study_program_id).execute(&pool).await.unwrap();
    sqlx::query("UPDATE grade_levels SET is_active=false WHERE id=$1")
        .bind(room.grade_level_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE academic_opening_policy SET require_homeroom_placements=true")
        .execute(&pool)
        .await
        .unwrap();
    for (permission, allowed) in [
        (codes::STUDENT_ACADEMIC_YEAR_READ_SCHOOL, true),
        (codes::STUDENT_ACADEMIC_YEAR_MANAGE_SCHOOL, true),
        (codes::ACADEMIC_YEAR_READ_SCHOOL, false),
    ] {
        let caller = ActorContext {
            user_id: actor,
            permissions: vec![
                codes::ACADEMIC_LIFECYCLE_READ_SCHOOL.into(),
                permission.into(),
            ],
        };
        let workspace = school_academic_lifecycle::services::get_activation_workspace(
            &pool,
            &caller,
            CURRENT_YEAR_ID,
            term.id,
        )
        .await
        .unwrap();
        for code in [
            "opening.student_reference",
            "opening.homeroom_placement_missing",
        ] {
            let finding = workspace
                .findings
                .iter()
                .find(|row| row.code == code)
                .expect("induced enrollment finding");
            assert_eq!(
                finding.resolution_url,
                allowed.then(|| format!(
                    "/staff/academic/student-years?academicYearId={CURRENT_YEAR_ID}"
                ))
            );
        }
    }
}

#[tokio::test]
async fn period_setup_http_capabilities_deny_without_writes_and_read_hidden_context() {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;
    let pool = prepare_current_core_fixture("period_setup_http_permissions").await;
    let actor = fixture_actor(&pool).await;
    let room = occupied_room(&pool).await;
    let schedule: Uuid = sqlx::query_scalar(
        "SELECT bell_schedule_id FROM academic_terms WHERE academic_year_id=$1 ORDER BY id LIMIT 1",
    )
    .bind(CURRENT_YEAR_ID)
    .fetch_one(&pool)
    .await
    .unwrap();
    let before: String = sqlx::query_scalar("SELECT jsonb_build_object('rooms',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM homerooms r),'schedules',(SELECT jsonb_agg(to_jsonb(s) ORDER BY id) FROM bell_schedules s),'audit',(SELECT count(*) FROM academic_audit_events))::text").fetch_one(&pool).await.unwrap();
    for (method, path, body) in [
        (
            "PATCH",
            format!("/api/academic/homerooms/{}/capacity", room.id),
            json!({"capacity":room.capacity+1,"rowVersion":room.row_version}),
        ),
        (
            "POST",
            format!("/api/academic/bell-schedules/{schedule}/clone"),
            json!({"academicYearId":CURRENT_YEAR_ID,"name":"HTTP denied","owningOrganizationUnitId":null,"rowVersion":1}),
        ),
        (
            "GET",
            format!("/api/academic/homerooms/year-context?academicYearId={FUTURE_YEAR_ID}"),
            serde_json::Value::Null,
        ),
        (
            "GET",
            format!("/api/academic/bell-schedules/{schedule}/periods"),
            serde_json::Value::Null,
        ),
        (
            "PUT",
            format!("/api/academic/bell-schedules/{schedule}/periods"),
            json!({"rowVersion":1,"periods":[]}),
        ),
    ] {
        let app = crate::integration_tests::academic::delivery::authenticated_academic_router(
            &pool,
            actor,
            &[],
            school_academic_http::core::routes(),
        );
        let response = app
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
    let after: String = sqlx::query_scalar("SELECT jsonb_build_object('rooms',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM homerooms r),'schedules',(SELECT jsonb_agg(to_jsonb(s) ORDER BY id) FROM bell_schedules s),'audit',(SELECT count(*) FROM academic_audit_events))::text").fetch_one(&pool).await.unwrap();
    assert_eq!(before, after);
    for permission in [codes::HOMEROOM_READ_SCHOOL, codes::HOMEROOM_MANAGE_SCHOOL] {
        let app = crate::integration_tests::academic::delivery::authenticated_academic_router(
            &pool,
            actor,
            &[permission],
            school_academic_http::core::routes(),
        );
        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/api/academic/homerooms/year-context?academicYearId={FUTURE_YEAR_ID}"
                    ))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), 100_000)
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["data"]["academicYearId"], FUTURE_YEAR_ID.to_string());
        assert_eq!(value["data"]["status"], "planning");
    }
}

#[tokio::test]
async fn period_setup_draft_period_references_preserve_rows_and_block_target_reassignment() {
    let pool = prepare_current_core_fixture("period_setup_draft_references").await;
    let actor = fixture_actor(&pool).await;
    let schedule = bell_schedules::create(
        &pool,
        actor,
        CreateBellScheduleRequest {
            academic_year_id: CURRENT_YEAR_ID,
            name: "Draft referenced schedule".into(),
            owning_organization_unit_id: None,
        },
    )
    .await
    .unwrap();
    let periods = bell_schedules::replace_periods(
        &pool,
        actor,
        schedule.id,
        ReplaceBellSchedulePeriodsRequest {
            row_version: schedule.row_version,
            periods: vec![BellSchedulePeriodInput {
                name: Some("Referenced period".into()),
                start_time: NaiveTime::from_hms_opt(8, 0, 0).unwrap(),
                end_time: NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
                order_index: 1,
                applicable_days: vec!["MON".into()],
                is_active: true,
            }],
        },
    )
    .await
    .unwrap()
    .snapshot
    .periods;
    let term = add_unopened_term(&pool, actor, schedule.id).await;
    // A valid accepted empty delivery graph with a real draft structural block.
    let delivery: Uuid = sqlx::query_scalar("INSERT INTO academic_delivery_versions(academic_term_id,academic_year_id,effective_from,status,snapshot,published_by,published_at,publication_idempotency_key,publication_request_hash,publication_no) VALUES($1,$2,CURRENT_DATE,'published','{\"offerings\":[]}', $3,now(),uuidv7(),repeat('a',64),1) RETURNING id")
        .bind(term.id).bind(CURRENT_YEAR_ID).bind(actor).fetch_one(&pool).await.unwrap();
    let version: Uuid = sqlx::query_scalar("INSERT INTO academic_timetable_versions(academic_term_id,academic_year_id,status,bell_schedule_id,delivery_version_id,created_by) VALUES($1,$2,'draft',$3,$4,$5) RETURNING id")
        .bind(term.id).bind(CURRENT_YEAR_ID).bind(schedule.id).bind(delivery).bind(actor).fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO academic_timetable_blocks(timetable_version_id,academic_term_id,academic_year_id,bell_schedule_id,bell_schedule_period_id,day_of_week,block_kind,structural_kind,title) VALUES($1,$2,$3,$4,$5,'MON','STRUCTURAL','BREAK','Real draft block')")
        .bind(version).bind(term.id).bind(CURRENT_YEAR_ID).bind(schedule.id).bind(periods[0].id).execute(&pool).await.unwrap();
    let guarded = bell_schedules::get(&pool, schedule.id).await.unwrap();
    assert_eq!(
        guarded.edit_blocked_reason,
        Some(BellScheduleEditBlockReason::TimetableReferences)
    );
    assert!(matches!(
        bell_schedules::replace_periods(
            &pool,
            actor,
            schedule.id,
            ReplaceBellSchedulePeriodsRequest {
                row_version: guarded.row_version,
                periods: vec![]
            }
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    assert_eq!(
        bell_schedules::list_periods(&pool, schedule.id)
            .await
            .unwrap()
            .periods[0]
            .id,
        periods[0].id
    );
    let clone = bell_schedules::clone_schedule(
        &pool,
        actor,
        schedule.id,
        CloneBellScheduleRequest {
            academic_year_id: CURRENT_YEAR_ID,
            name: "New future schedule".into(),
            owning_organization_unit_id: None,
            row_version: guarded.row_version,
        },
    )
    .await
    .unwrap();
    assert!(matches!(
        years_terms::update_term(
            &pool,
            actor,
            term.id,
            UpdateAcademicTermRequest {
                custom_name: Some(term.name),
                term_type: term.term_type,
                included_in_year_result: term.included_in_year_result,
                blocks_year_closure: term.blocks_year_closure,
                bell_schedule_id: clone.id,
                row_version: term.row_version
            }
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    let workspace = workspaces::setup_workspace(
        &pool,
        &ActorContext {
            user_id: actor,
            permissions: vec![codes::WILDCARD.into()],
        },
    )
    .await
    .unwrap();
    assert!(!workspace.bell_schedule_setup_term_ids.contains(&term.id));
    assert_eq!(sqlx::query_scalar::<_, Uuid>("SELECT bell_schedule_period_id FROM academic_timetable_blocks WHERE timetable_version_id=$1").bind(version).fetch_one(&pool).await.unwrap(), periods[0].id);
}

async fn occupied_room(pool: &PgPool) -> Homeroom {
    let id = sqlx::query_scalar(
        "SELECT homeroom_id FROM homeroom_placements WHERE academic_year_id=$1 AND status='current' ORDER BY id LIMIT 1",
    ).bind(CURRENT_YEAR_ID).fetch_one(pool).await.unwrap();
    student_years::get_homeroom(pool, id).await.unwrap()
}

#[tokio::test]
async fn period_setup_capacity_is_narrow_revisioned_audited_and_checks_actual_members() {
    let pool = prepare_current_core_fixture("period_setup_capacity").await;
    let actor = fixture_actor(&pool).await;
    let original = occupied_room(&pool).await;
    let hidden = student_years::homeroom_year_context(&pool, FUTURE_YEAR_ID)
        .await
        .unwrap();
    assert_eq!(hidden.academic_year_id, FUTURE_YEAR_ID);
    assert_eq!(hidden.status, AcademicYearStatus::Planning);
    assert!(
        !sqlx::query_scalar::<_, bool>("SELECT is_visible FROM academic_years WHERE id=$1")
            .bind(FUTURE_YEAR_ID)
            .fetch_one(&pool)
            .await
            .unwrap()
    );
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM homeroom_placements WHERE homeroom_id=$1 AND status IN ('planned','current')",
    ).bind(original.id).fetch_one(&pool).await.unwrap();
    assert!(count > 0);
    for invalid in [0, -1] {
        assert!(matches!(
            student_years::update_homeroom_capacity(
                &pool,
                actor,
                original.id,
                UpdateHomeroomCapacityRequest {
                    capacity: invalid,
                    row_version: original.row_version
                }
            )
            .await,
            Err(AppError::ValidationError(_))
        ));
    }
    if count > 1 {
        assert!(matches!(
            student_years::update_homeroom_capacity(
                &pool,
                actor,
                original.id,
                UpdateHomeroomCapacityRequest {
                    capacity: count as i32 - 1,
                    row_version: original.row_version
                }
            )
            .await,
            Err(AppError::Conflict(_))
        ));
    }
    let updated = student_years::update_homeroom_capacity(
        &pool,
        actor,
        original.id,
        UpdateHomeroomCapacityRequest {
            capacity: original.capacity + 3,
            row_version: original.row_version,
        },
    )
    .await
    .unwrap();
    assert_eq!(updated.capacity, original.capacity + 3);
    assert_eq!(updated.student_count, Some(count));
    assert_eq!(updated.row_version, original.row_version + 1);
    assert_eq!(
        (
            &updated.name,
            &updated.code,
            updated.grade_level_id,
            updated.study_program_id,
            &updated.room_number
        ),
        (
            &original.name,
            &original.code,
            original.grade_level_id,
            original.study_program_id,
            &original.room_number
        )
    );
    assert!(matches!(
        student_years::update_homeroom_capacity(
            &pool,
            actor,
            original.id,
            UpdateHomeroomCapacityRequest {
                capacity: updated.capacity + 1,
                row_version: original.row_version
            }
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    let audit: serde_json::Value = sqlx::query_scalar(
        "SELECT payload FROM academic_audit_events WHERE entity_id=$1 AND event_code='homeroom.capacity_updated'",
    ).bind(original.id).fetch_one(&pool).await.unwrap();
    assert_eq!(audit["before"]["capacity"], original.capacity);
    assert_eq!(audit["after"]["capacity"], updated.capacity);
    assert_eq!(audit["studentCount"], count);
    let mut current = updated;
    for (status, allowed) in [
        ("planning", true),
        ("ready", true),
        ("closing", false),
        ("closed", false),
        ("archived", false),
    ] {
        sqlx::query("UPDATE academic_years SET status=$1 WHERE id=$2")
            .bind(status)
            .bind(CURRENT_YEAR_ID)
            .execute(&pool)
            .await
            .unwrap();
        if status == "planning" && count > 1 {
            assert!(matches!(
                student_years::update_homeroom(
                    &pool,
                    original.id,
                    school_academic_core::models::UpdateHomeroomRequest {
                        custom_name: Some(current.name.clone()),
                        grade_level_id: current.grade_level_id,
                        room_number: current.room_number.clone().unwrap(),
                        study_program_id: current.study_program_id,
                        capacity: count as i32 - 1,
                        row_version: current.row_version,
                    }
                )
                .await,
                Err(AppError::Conflict(_))
            ));
        }
        let result = student_years::update_homeroom_capacity(
            &pool,
            actor,
            original.id,
            UpdateHomeroomCapacityRequest {
                capacity: current.capacity + 1,
                row_version: current.row_version,
            },
        )
        .await;
        if allowed {
            current = result.unwrap();
        } else {
            assert!(matches!(result, Err(AppError::Conflict(_))));
        }
    }
    assert_eq!(
        student_years::get_homeroom(&pool, current.id)
            .await
            .unwrap()
            .row_version,
        current.row_version
    );
    assert!(serde_json::from_value::<UpdateHomeroomCapacityRequest>(
        json!({"capacity": 45, "rowVersion": 1, "gradeLevelId": Uuid::new_v4()})
    )
    .is_err());
}

#[tokio::test]
async fn period_setup_capacity_concurrent_writes_have_one_winner() {
    let pool = prepare_current_core_fixture("period_setup_capacity_concurrent").await;
    let actor = fixture_actor(&pool).await;
    let room = occupied_room(&pool).await;
    let (first, second) = tokio::join!(
        student_years::update_homeroom_capacity(
            &pool,
            actor,
            room.id,
            UpdateHomeroomCapacityRequest {
                capacity: room.capacity + 1,
                row_version: room.row_version
            }
        ),
        student_years::update_homeroom_capacity(
            &pool,
            actor,
            room.id,
            UpdateHomeroomCapacityRequest {
                capacity: room.capacity + 2,
                row_version: room.row_version
            }
        ),
    );
    assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);
    let denied = if first.is_err() { first } else { second };
    assert!(matches!(denied, Err(AppError::Conflict(_))));
    assert_eq!(
        student_years::get_homeroom(&pool, room.id)
            .await
            .unwrap()
            .row_version,
        room.row_version + 1
    );
}

#[tokio::test]
async fn period_setup_capacity_waits_for_roster_writer_and_rechecks_member_floor() {
    let pool = prepare_current_core_fixture("period_setup_capacity_roster_lock").await;
    let actor = fixture_actor(&pool).await;
    let room = occupied_room(&pool).await;
    let before: i64 = sqlx::query_scalar("SELECT count(*) FROM homeroom_placements WHERE homeroom_id=$1 AND status IN ('planned','current')")
        .bind(room.id).fetch_one(&pool).await.unwrap();
    let mut tx = pool.begin().await.unwrap();
    lifecycle_guard::require_year_write_exclusive(&mut tx, room.academic_year_id)
        .await
        .unwrap();
    let student = Uuid::new_v4();
    let student_year = Uuid::new_v4();
    sqlx::query("INSERT INTO users(id,username,password_hash,first_name,last_name,user_type) VALUES($1,$2,'!','New','Member','student')")
        .bind(student).bind(student.to_string()).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO student_academic_years(id,student_id,academic_year_id,grade_level_id,study_program_id,status) VALUES($1,$2,$3,$4,$5,'active')")
        .bind(student_year).bind(student).bind(room.academic_year_id).bind(room.grade_level_id).bind(room.study_program_id).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO homeroom_placements(id,student_academic_year_id,academic_year_id,homeroom_id,start_date,status,enrollment_type) VALUES($1,$2,$3,$4,CURRENT_DATE,'current','normal')")
        .bind(Uuid::new_v4()).bind(student_year).bind(room.academic_year_id).bind(room.id).execute(&mut *tx).await.unwrap();
    let task_pool = pool.clone();
    let mut command = tokio::spawn(async move {
        student_years::update_homeroom_capacity(
            &task_pool,
            actor,
            room.id,
            UpdateHomeroomCapacityRequest {
                capacity: before as i32,
                row_version: room.row_version,
            },
        )
        .await
    });
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(100), &mut command)
            .await
            .is_err()
    );
    tx.commit().await.unwrap();
    assert!(matches!(command.await.unwrap(), Err(AppError::Conflict(_))));
}

async fn add_unopened_term(
    pool: &PgPool,
    actor: Uuid,
    schedule: Uuid,
) -> school_academic_core::models::AcademicTerm {
    years_terms::create_term(
        pool,
        actor,
        CreateAcademicTermRequest {
            academic_year_id: CURRENT_YEAR_ID,
            term_type: AcademicTermType::Regular,
            custom_name: Some("ภาคเรียนใหม่".into()),
            included_in_year_result: true,
            blocks_year_closure: true,
            bell_schedule_id: schedule,
        },
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn period_setup_shared_schedule_stays_locked_and_clone_is_atomic_for_unopened_term() {
    let pool = prepare_current_core_fixture("period_setup_bell_clone").await;
    let actor = fixture_actor(&pool).await;
    let source_id: Uuid = sqlx::query_scalar(
        "SELECT bell_schedule_id FROM academic_terms WHERE academic_year_id=$1 AND status='active'",
    )
    .bind(CURRENT_YEAR_ID)
    .fetch_one(&pool)
    .await
    .unwrap();
    let source = bell_schedules::get(&pool, source_id).await.unwrap();
    let term = add_unopened_term(&pool, actor, source.id).await;
    let workspace = workspaces::setup_workspace(
        &pool,
        &ActorContext {
            user_id: actor,
            permissions: vec![codes::WILDCARD.into()],
        },
    )
    .await
    .unwrap();
    assert!(
        workspace.bell_schedule_setup_term_ids.contains(&term.id),
        "another term's published timetable must not prevent assignment to this unopened term"
    );
    assert_eq!(
        source.edit_blocked_reason,
        Some(BellScheduleEditBlockReason::AlreadyUsed)
    );
    assert!(source.can_clone);
    let original_periods = bell_schedules::list_periods(&pool, source.id)
        .await
        .unwrap()
        .periods;
    assert!(!original_periods.is_empty());
    assert!(matches!(
        bell_schedules::replace_periods(
            &pool,
            actor,
            source.id,
            ReplaceBellSchedulePeriodsRequest {
                periods: vec![],
                row_version: source.row_version
            }
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    assert!(matches!(
        bell_schedules::update(
            &pool,
            actor,
            source.id,
            UpdateBellScheduleRequest {
                name: "แก้ตารางที่ใช้แล้ว".into(),
                is_default: source.is_default,
                owning_organization_unit_id: source.owning_organization_unit_id,
                row_version: source.row_version,
            }
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    let request = CloneBellScheduleRequest {
        academic_year_id: CURRENT_YEAR_ID,
        name: "ตารางภาคเรียนใหม่".into(),
        owning_organization_unit_id: None,
        row_version: source.row_version,
    };
    let clone = bell_schedules::clone_schedule(&pool, actor, source.id, request.clone())
        .await
        .unwrap();
    assert_ne!(clone.id, source.id);
    assert_eq!(clone.edit_blocked_reason, None);
    let copied = bell_schedules::list_periods(&pool, clone.id)
        .await
        .unwrap()
        .periods;
    assert_eq!(copied.len(), original_periods.len());
    for (old, new) in original_periods.iter().zip(&copied) {
        assert_ne!(old.id, new.id);
        assert_eq!(
            (
                old.start_time,
                old.end_time,
                old.order_index,
                &old.applicable_days,
                old.is_active
            ),
            (
                new.start_time,
                new.end_time,
                new.order_index,
                &new.applicable_days,
                new.is_active
            )
        );
    }
    let count_before: i64 = sqlx::query_scalar("SELECT count(*) FROM bell_schedules")
        .fetch_one(&pool)
        .await
        .unwrap();
    let mut stale = request;
    stale.row_version += 1;
    assert!(matches!(
        bell_schedules::clone_schedule(&pool, actor, source.id, stale).await,
        Err(AppError::Conflict(_))
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM bell_schedules")
            .fetch_one(&pool)
            .await
            .unwrap(),
        count_before
    );
    sqlx::query("UPDATE academic_terms SET status='ready' WHERE id=$1")
        .bind(term.id)
        .execute(&pool)
        .await
        .unwrap();
    let ready = years_terms::get_term(&pool, term.id).await.unwrap();
    let mut assignment = UpdateAcademicTermRequest {
        term_type: ready.term_type,
        custom_name: Some(ready.name.clone()),
        included_in_year_result: ready.included_in_year_result,
        blocks_year_closure: ready.blocks_year_closure,
        bell_schedule_id: clone.id,
        row_version: ready.row_version,
    };
    assignment.custom_name = Some("เปลี่ยนข้อมูลอื่นไม่ได้".into());
    assert!(matches!(
        years_terms::update_term(&pool, actor, term.id, assignment.clone()).await,
        Err(AppError::Conflict(_))
    ));
    assignment.custom_name = Some(ready.name);
    let assigned = years_terms::update_term(&pool, actor, term.id, assignment)
        .await
        .unwrap();
    assert_eq!(assigned.bell_schedule_id, clone.id);
    assert_eq!(assigned.status, AcademicTermStatus::Ready);
    assert_eq!(
        bell_schedules::list_periods(&pool, source.id)
            .await
            .unwrap()
            .periods
            .iter()
            .map(|row| row.id)
            .collect::<Vec<_>>(),
        original_periods
            .iter()
            .map(|row| row.id)
            .collect::<Vec<_>>()
    );
    sqlx::query(
        "UPDATE academic_terms SET status='closed' WHERE academic_year_id=$1 AND status='active'",
    )
    .bind(CURRENT_YEAR_ID)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE academic_terms SET status='active' WHERE id=$1")
        .bind(term.id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        bell_schedules::get(&pool, clone.id)
            .await
            .unwrap()
            .edit_blocked_reason,
        Some(BellScheduleEditBlockReason::AlreadyUsed)
    );
    assert!(matches!(
        bell_schedules::replace_periods(
            &pool,
            actor,
            clone.id,
            ReplaceBellSchedulePeriodsRequest {
                periods: vec![],
                row_version: clone.row_version
            }
        )
        .await,
        Err(AppError::Conflict(_))
    ));
}

#[tokio::test]
async fn period_setup_opening_diagnostics_preserve_raw_invalid_days_and_identify_named_period() {
    let pool = prepare_current_core_fixture("period_setup_opening_diagnostics").await;
    let actor = fixture_actor(&pool).await;
    let source: Uuid = sqlx::query_scalar(
        "SELECT bell_schedule_id FROM academic_terms WHERE academic_year_id=$1 AND status='active'",
    )
    .bind(CURRENT_YEAR_ID)
    .fetch_one(&pool)
    .await
    .unwrap();
    let term = add_unopened_term(&pool, actor, source).await;
    let clone = bell_schedules::clone_schedule(
        &pool,
        actor,
        source,
        CloneBellScheduleRequest {
            academic_year_id: CURRENT_YEAR_ID,
            name: "ตารางตรวจปัญหา".into(),
            owning_organization_unit_id: None,
            row_version: bell_schedules::get(&pool, source)
                .await
                .unwrap()
                .row_version,
        },
    )
    .await
    .unwrap();
    years_terms::update_term(
        &pool,
        actor,
        term.id,
        UpdateAcademicTermRequest {
            term_type: term.term_type,
            custom_name: Some(term.name),
            included_in_year_result: term.included_in_year_result,
            blocks_year_closure: term.blocks_year_closure,
            bell_schedule_id: clone.id,
            row_version: term.row_version,
        },
    )
    .await
    .unwrap();
    sqlx::query("UPDATE bell_schedule_periods SET name='คาบที่ต้องแก้',applicable_days='{MON,TUE}' WHERE bell_schedule_id=$1")
        .bind(clone.id).execute(&pool).await.unwrap();
    let room = occupied_room(&pool).await;
    let pupil: Uuid = sqlx::query_scalar("INSERT INTO users(password_hash,first_name,last_name,user_type,status) VALUES('synthetic-not-login','Capacity','Pupil','student','active') RETURNING id").fetch_one(&pool).await.unwrap();
    let annual: Uuid = sqlx::query_scalar("INSERT INTO student_academic_years(id,student_id,academic_year_id,grade_level_id,study_program_id,status) VALUES(uuidv7(),$1,$2,$3,$4,'active') RETURNING id")
        .bind(pupil).bind(CURRENT_YEAR_ID).bind(room.grade_level_id).bind(room.study_program_id).fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO homeroom_placements(id,student_academic_year_id,academic_year_id,homeroom_id,placement_scope,status,enrollment_type) VALUES(uuidv7(),$1,$2,$3,'academic_year','current','promotion')")
        .bind(annual).bind(CURRENT_YEAR_ID).bind(room.id).execute(&pool).await.unwrap();
    sqlx::query("UPDATE homerooms SET capacity=1 WHERE id=$1")
        .bind(room.id)
        .execute(&pool)
        .await
        .unwrap();
    let occupants: i64 = sqlx::query_scalar("SELECT count(DISTINCT student_academic_year_id) FROM homeroom_placements WHERE homeroom_id=$1 AND status IN ('planned','current')").bind(room.id).fetch_one(&pool).await.unwrap();
    let mut tx = pool.begin().await.unwrap();
    let state = activation_context::read_activation_state(&mut tx, CURRENT_YEAR_ID, term.id)
        .await
        .unwrap();
    assert!(
        state
            .issues
            .iter()
            .any(|issue| issue.code
                == school_academic_core::models::ActivationIssueCode::BellSchedule)
    );
    let issues = opening_diagnostics::explain(&state);
    assert!(issues.iter().any(|issue| issue.resource
        == opening_diagnostics::DiagnosticResource::Homeroom(room.id)
        && issue.message.contains(&room.name)
        && issue.message.contains(&format!("{occupants} คน"))
        && issue.message.contains("ความจุ 1 คน")));
    assert!(issues
        .iter()
        .any(|issue| issue.code == "opening.bell_period_days_invalid"
            && issue.message.contains("คาบที่ต้องแก้")
            && issue.message.contains("ตารางตรวจปัญหา")));
    tx.commit().await.unwrap();
    let raw: Vec<String> = sqlx::query_scalar(
        "SELECT applicable_days FROM bell_schedule_periods WHERE bell_schedule_id=$1",
    )
    .bind(clone.id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert!(raw.iter().all(|days| days == "{MON,TUE}"));
    let restricted = ActorContext {
        user_id: actor,
        permissions: vec![codes::ACADEMIC_LIFECYCLE_READ_SCHOOL.into()],
    };
    let workspace = school_academic_lifecycle::services::get_activation_workspace(
        &pool,
        &restricted,
        CURRENT_YEAR_ID,
        term.id,
    )
    .await
    .unwrap();
    assert!(!workspace.can_activate);
    assert!(workspace
        .findings
        .iter()
        .any(|issue| issue.code == "opening.bell_period_days_invalid"
            && issue.resolution_url.is_none()));
    let authorized = ActorContext {
        user_id: actor,
        permissions: vec![
            codes::ACADEMIC_LIFECYCLE_READ_SCHOOL.into(),
            codes::ACADEMIC_YEAR_READ_SCHOOL.into(),
            codes::ACADEMIC_TERM_MANAGE_SCHOOL.into(),
            codes::HOMEROOM_MANAGE_SCHOOL.into(),
        ],
    };
    let workspace = school_academic_lifecycle::services::get_activation_workspace(
        &pool,
        &authorized,
        CURRENT_YEAR_ID,
        term.id,
    )
    .await
    .unwrap();
    assert!(workspace.findings.iter().any(|issue| issue.code
        == "opening.bell_period_days_invalid"
        && issue.resolution_url.as_deref()
            == Some(
                format!(
                    "/staff/academic/core?academicYearId={CURRENT_YEAR_ID}&academicTermId={}",
                    term.id
                )
                .as_str()
            )));
}

#[tokio::test]
async fn period_setup_published_and_cancelled_used_schedules_cannot_be_reopened_for_edits() {
    let pool = prepare_current_core_fixture("period_setup_pinned_schedule").await;
    let actor = fixture_actor(&pool).await;
    let (term, schedule): (Uuid, Uuid) = sqlx::query_as(
        "SELECT academic_term_id,bell_schedule_id FROM academic_timetable_versions WHERE status='published' AND academic_year_id=$1 ORDER BY id LIMIT 1",
    ).bind(CURRENT_YEAR_ID).fetch_one(&pool).await.unwrap();
    add_unopened_term(&pool, actor, schedule).await;
    // Legacy publications retain their schedule even if the current term status
    // no longer identifies operational use.
    sqlx::query("UPDATE academic_terms SET status='cancelled' WHERE id=$1")
        .bind(term)
        .execute(&pool)
        .await
        .unwrap();
    let pinned = bell_schedules::get(&pool, schedule).await.unwrap();
    sqlx::query("UPDATE academic_terms SET status='planning' WHERE id=$1")
        .bind(term)
        .execute(&pool)
        .await
        .unwrap();
    let workspace = workspaces::setup_workspace(
        &pool,
        &ActorContext {
            user_id: actor,
            permissions: vec![codes::WILDCARD.into()],
        },
    )
    .await
    .unwrap();
    assert!(
        !workspace.bell_schedule_setup_term_ids.contains(&term),
        "the target term's timetable references prevent reassignment even after a status reset"
    );
    sqlx::query("UPDATE academic_terms SET status='cancelled' WHERE id=$1")
        .bind(term)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        pinned.edit_blocked_reason,
        Some(BellScheduleEditBlockReason::PinnedTimetable)
    );
    assert!(matches!(
        bell_schedules::replace_periods(
            &pool,
            actor,
            schedule,
            ReplaceBellSchedulePeriodsRequest {
                periods: vec![],
                row_version: pinned.row_version
            }
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    // Immutable acceptance records also protect a used schedule independently
    // of its current term pointer and independently of timetable publication.
    let clone = bell_schedules::clone_schedule(
        &pool,
        actor,
        schedule,
        CloneBellScheduleRequest {
            academic_year_id: CURRENT_YEAR_ID,
            name: "ตารางที่เคยเปิด".into(),
            owning_organization_unit_id: None,
            row_version: pinned.row_version,
        },
    )
    .await
    .unwrap();
    let new_term = add_unopened_term(&pool, actor, clone.id).await;
    sqlx::query("UPDATE academic_terms SET status='cancelled' WHERE id=$1")
        .bind(new_term.id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO academic_term_transition_receipts(request_id,academic_year_id,academic_term_id,actor_user_id,action,request_checksum,accepted_readiness,outcome,canonical_request) VALUES($1,$2,$3,$4,'activate',$5,'{}',$6,'{}')")
        .bind(Uuid::new_v4()).bind(CURRENT_YEAR_ID).bind(new_term.id).bind(actor).bind("0".repeat(64))
        .bind(json!({"context": {"bellScheduleId": clone.id}})).execute(&pool).await.unwrap();
    let used = bell_schedules::get(&pool, clone.id).await.unwrap();
    assert_eq!(
        used.edit_blocked_reason,
        Some(BellScheduleEditBlockReason::AlreadyUsed)
    );
    assert!(matches!(
        bell_schedules::replace_periods(
            &pool,
            actor,
            used.id,
            ReplaceBellSchedulePeriodsRequest {
                periods: vec![],
                row_version: used.row_version
            }
        )
        .await,
        Err(AppError::Conflict(_))
    ));
}
