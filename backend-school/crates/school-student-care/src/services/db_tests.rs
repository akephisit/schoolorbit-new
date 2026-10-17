use super::*;
use school_authorization::ActorContext;
use school_test_db::{create_named_test_pool_with_max_connections, run_test_migrations};

struct Fixture {
    pool: PgPool,
    admin: CareAccess,
    teacher: CareAccess,
    other: CareAccess,
    student: Uuid,
    second: Uuid,
    parent: Uuid,
    term: Uuid,
    date: NaiveDate,
}
async fn user(pool: &PgPool, name: &str, kind: &str) -> Uuid {
    sqlx::query_scalar("INSERT INTO users(username,email,password_hash,first_name,last_name,user_type,status,date_of_birth) VALUES($1,$2,'fixture-only',$1,'Synthetic',$3,'active','2012-01-01') RETURNING id").bind(name).bind(format!("{name}@example.invalid")).bind(kind).fetch_one(pool).await.unwrap()
}
fn access(id: Uuid, permissions: &[&str]) -> CareAccess {
    CareAccess::new(&ActorContext {
        user_id: id,
        permissions: permissions.iter().map(|s| s.to_string()).collect(),
    })
}
async fn fixture(name: &str) -> Fixture {
    let pool = create_named_test_pool_with_max_connections(name, 8).await;
    run_test_migrations(&pool).await;
    let admin = user(&pool, "care-admin", "staff").await;
    let teacher = user(&pool, "care-teacher", "staff").await;
    let other = user(&pool, "care-other", "staff").await;
    let student = user(&pool, "care-student", "student").await;
    let second = user(&pool, "care-second", "student").await;
    let parent = user(&pool, "care-parent", "parent").await;
    let role:Uuid=sqlx::query_scalar("INSERT INTO roles(code,name,user_type) VALUES('CARE_FIXTURE','Synthetic reviewer','staff') RETURNING id").fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO role_permissions(role_id,permission_id) SELECT $1,id FROM permissions WHERE module='student_care_sensitive'").bind(role).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO user_roles(user_id,role_id) SELECT unnest($1::uuid[]),$2")
        .bind(vec![admin, teacher, other])
        .bind(role)
        .execute(&pool)
        .await
        .unwrap();
    let date = NaiveDate::from_ymd_opt(2026, 10, 17).unwrap();
    let start = date - chrono::Duration::days(30);
    let end = date + chrono::Duration::days(300);
    let year:Uuid=sqlx::query_scalar("INSERT INTO academic_years(year,name,start_date,end_date,status) VALUES(2988,'Care fixture',$1,$2,'active') RETURNING id").bind(start).bind(end).fetch_one(&pool).await.unwrap();
    let bell:Uuid=sqlx::query_scalar("INSERT INTO bell_schedules(id,academic_year_id,code,name,is_default) VALUES(uuidv7(),$1,'CARE','Care fixture',true) RETURNING id").bind(year).fetch_one(&pool).await.unwrap();
    let term:Uuid=sqlx::query_scalar("INSERT INTO academic_terms(academic_year_id,sequence_no,code,name,term_type,start_date,planned_end_date,bell_schedule_id,status,included_in_year_result,blocks_year_closure) VALUES($1,1,'CARE','Care fixture','regular',$2,$3,$4,'active',true,true) RETURNING id").bind(year).bind(start).bind(end).bind(bell).fetch_one(&pool).await.unwrap();
    let grade:Uuid=sqlx::query_scalar("INSERT INTO grade_levels(level_type,year,is_active) VALUES('secondary',1,true) ON CONFLICT(level_type,year) DO UPDATE SET is_active=true RETURNING id").fetch_one(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO academic_year_grade_levels(academic_year_id,grade_level_id) VALUES($1,$2)",
    )
    .bind(year)
    .bind(grade)
    .execute(&pool)
    .await
    .unwrap();
    let edition:Uuid=sqlx::query_scalar("INSERT INTO curriculum_editions(id,name,revision_year) VALUES(uuidv7(),'Care fixture',2988) RETURNING id").fetch_one(&pool).await.unwrap();
    let level:Uuid=sqlx::query_scalar("INSERT INTO curriculum_levels(id,edition_id,code,name_th,grade_level_ids,is_active) VALUES(uuidv7(),$1,'CARE','Care fixture',$2,true) RETURNING id").bind(edition).bind(Json(vec![grade])).fetch_one(&pool).await.unwrap();
    let program:Uuid=sqlx::query_scalar("INSERT INTO study_programs(id,curriculum_level_id,code,name_th,status) VALUES(uuidv7(),$1,'CARE','Care fixture','draft') RETURNING id").bind(level).fetch_one(&pool).await.unwrap();
    let room:Uuid=sqlx::query_scalar("INSERT INTO homerooms(code,name,academic_year_id,grade_level_id,study_program_id,room_number,capacity,is_active) VALUES('CARE','Care fixture',$1,$2,$3,'1',50,true) RETURNING id").bind(year).bind(grade).bind(program).fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO homeroom_advisors(id,homeroom_id,user_id,role) VALUES(uuidv7(),$1,$2,'primary')").bind(room).bind(teacher).execute(&pool).await.unwrap();
    for (n, id) in [student, second].into_iter().enumerate() {
        sqlx::query("INSERT INTO student_info(user_id,student_id) VALUES($1,$2)")
            .bind(id)
            .bind(format!("CARE-{n}"))
            .execute(&pool)
            .await
            .unwrap();
        let sy:Uuid=sqlx::query_scalar("INSERT INTO student_academic_years(id,student_id,academic_year_id,grade_level_id,study_program_id,status) VALUES(uuidv7(),$1,$2,$3,$4,'active') RETURNING id").bind(id).bind(year).bind(grade).bind(program).fetch_one(&pool).await.unwrap();
        if n == 0 {
            sqlx::query("INSERT INTO homeroom_placements(id,student_academic_year_id,academic_year_id,homeroom_id,start_date,status,enrollment_type,class_number) VALUES(uuidv7(),$1,$2,$3,$4,'current','regular',1)").bind(sy).bind(year).bind(room).bind(start).execute(&pool).await.unwrap();
        }
    }
    sqlx::query("INSERT INTO student_parents(student_user_id,parent_user_id,relationship) VALUES($1,$2,'father')").bind(student).bind(parent).execute(&pool).await.unwrap();
    Fixture {
        pool,
        admin: access(
            admin,
            &[
                codes::STUDENT_CARE_READ_SCHOOL,
                codes::STUDENT_CARE_MANAGE_SCHOOL,
                codes::STUDENT_CARE_CONFIG_MANAGE_SCHOOL,
                codes::STUDENT_CARE_SENSITIVE_MANAGE_SCHOOL,
            ],
        ),
        teacher: access(
            teacher,
            &[
                codes::STUDENT_CARE_READ_ASSIGNED,
                codes::STUDENT_CARE_MANAGE_ASSIGNED,
                codes::STUDENT_CARE_SENSITIVE_MANAGE_ASSIGNED,
            ],
        ),
        other: access(
            other,
            &[
                codes::STUDENT_CARE_READ_ASSIGNED,
                codes::STUDENT_CARE_MANAGE_ASSIGNED,
                codes::STUDENT_CARE_SENSITIVE_MANAGE_ASSIGNED,
            ],
        ),
        student,
        second,
        parent,
        term,
        date,
    }
}
fn definition(mode: CareInstrumentMode, respondent: CareRespondent) -> CareInstrumentDefinition {
    CareInstrumentDefinition {
        name: "Synthetic only".into(),
        family: CareFamily::Other,
        variant: "Synthetic".into(),
        language: "en".into(),
        respondent,
        minimum_age: 10,
        maximum_age: 19,
        mode,
        instructions: String::new(),
        source: "Nonclinical test fixture".into(),
        license_note: "Synthetic content".into(),
        digital_rights_confirmed: true,
        questions: if mode == CareInstrumentMode::Digital {
            vec![CareQuestion {
                key: "q".into(),
                text: "Synthetic question".into(),
                scale_key: "base".into(),
                reverse: false,
                options: vec![
                    CareOption {
                        key: "a".into(),
                        label: "A".into(),
                        points: 0,
                        urgent: false,
                    },
                    CareOption {
                        key: "b".into(),
                        label: "B".into(),
                        points: 2,
                        urgent: true,
                    },
                ],
            }]
        } else {
            vec![]
        },
        scales: vec![CareScale {
            key: "base".into(),
            label: "Synthetic".into(),
            source_scale_keys: vec![],
            bands: vec![CareBand {
                key: "all".into(),
                label: "Fixture range".into(),
                minimum: 0,
                maximum: 2,
                urgent: false,
            }],
        }],
    }
}
async fn instrument_round(
    f: &Fixture,
    mode: CareInstrumentMode,
    respondent: CareRespondent,
) -> (CareInstrumentVersion, CareRound) {
    let v = create_instrument(
        &f.pool,
        f.admin,
        CreateCareInstrumentVersion {
            request_id: Uuid::now_v7(),
            instrument_id: None,
            expected_version: None,
            definition: definition(mode, respondent),
        },
    )
    .await
    .unwrap();
    let r = create_round(
        &f.pool,
        f.admin,
        CreateCareRound {
            request_id: Uuid::now_v7(),
            name: "Synthetic round".into(),
            purpose: "Fixture purpose".into(),
            instrument_version_ids: vec![v.id],
            academic_term_id: f.term,
            roster_date: f.date,
            due_date: f.date + chrono::Duration::days(10),
        },
    )
    .await
    .unwrap();
    (v, r)
}
async fn assignment(
    f: &Fixture,
    v: &CareInstrumentVersion,
    r: &CareRound,
    respondent: Uuid,
) -> CareAssignment {
    create_assignments(
        &f.pool,
        f.admin,
        CreateCareAssignments {
            request_id: Uuid::now_v7(),
            round_id: r.id,
            instrument_version_id: v.id,
            targets: vec![CareAssignmentTarget {
                student_id: f.student,
                respondent_id: respondent,
            }],
            reviewer_user_ids: vec![f.teacher.actor_id],
        },
    )
    .await
    .unwrap()
    .remove(0)
}
fn response(rev: i64) -> SaveCareResponse {
    SaveCareResponse {
        acknowledge_purpose: true,
        request_id: Uuid::now_v7(),
        row_version: rev,
        answers: vec![CareAnswer {
            question_key: "q".into(),
            option_key: "b".into(),
        }],
        manual_scores: vec![],
        manual_note: None,
    }
}
fn visit_payload(f: &Fixture, student: Uuid, visitors: Vec<Uuid>) -> SaveCareVisit {
    SaveCareVisit {
        request_id: Uuid::now_v7(),
        row_version: None,
        student_id: student,
        academic_term_id: f.term,
        planned_date: f.date,
        visitor_ids: visitors,
        status: CareVisitStatus::Planned,
        visited_date: None,
        contact_person: String::new(),
        factual_observations: String::new(),
        strengths: String::new(),
        needs: String::new(),
        agreed_actions: String::new(),
        next_follow_up: None,
    }
}
fn case_payload(f: &Fixture, source: Option<Uuid>) -> SaveCareCase {
    SaveCareCase {
        request_id: Uuid::now_v7(),
        row_version: None,
        student_id: f.student,
        title: "Synthetic support".into(),
        assigned_user_ids: vec![f.teacher.actor_id],
        next_follow_up: None,
        note: "Confidential synthetic note".into(),
        source_assignment_id: source,
        source_visit_id: None,
    }
}

#[tokio::test]
async fn care_submission_pins_version_replay_durable_alert_and_scoped_ack() {
    let f = fixture("care_submit").await;
    let (v, r) =
        instrument_round(&f, CareInstrumentMode::Digital, CareRespondent::SelfReport).await;
    let assignment = assignment(&f, &v, &r, f.student).await;
    let own = access(f.student, &[codes::STUDENT_CARE_READ_OWN]);
    let newer = create_instrument(
        &f.pool,
        f.admin,
        CreateCareInstrumentVersion {
            request_id: Uuid::now_v7(),
            instrument_id: Some(v.instrument_id),
            expected_version: Some(1),
            definition: definition(CareInstrumentMode::Digital, CareRespondent::SelfReport),
        },
    )
    .await
    .unwrap();
    assert_eq!(newer.version, 2);
    assert!(matches!(
        get_assignment(&f.pool, f.other, assignment.id).await,
        Err(AppError::NotFound(_))
    ));
    let mut draft = response(1);
    draft.answers.clear();
    let saved = save_response(&f.pool, own, assignment.id, draft, false)
        .await
        .unwrap();
    assert_eq!(saved.assignment.status, CareAssignmentStatus::InProgress);
    assert!(saved.results.is_empty());
    assert!(matches!(
        save_response(&f.pool, own, assignment.id, response(1), false).await,
        Err(AppError::Conflict(_))
    ));
    let payload = response(2);
    let first = save_response(&f.pool, own, assignment.id, payload.clone(), true)
        .await
        .unwrap();
    let replay = save_response(&f.pool, own, assignment.id, payload, true)
        .await
        .unwrap();
    assert_eq!(first.assignment.row_version, replay.assignment.row_version);
    assert_eq!(first.instrument.id, v.id);
    assert!(first.assignment.urgent);
    assert_eq!(first.results[0].score, 2);
    let routine_summary = assignments(
        &f.pool,
        access(f.admin.actor_id, &[codes::STUDENT_CARE_READ_SCHOOL]),
        CareListQuery::default(),
    )
    .await
    .unwrap();
    assert_eq!(routine_summary.len(), 1);
    assert!(!routine_summary[0].urgent);
    assert!(routine_summary[0].acknowledged_at.is_none());
    assert!(get_assignment(
        &f.pool,
        access(f.admin.actor_id, &[codes::STUDENT_CARE_READ_SCHOOL]),
        assignment.id
    )
    .await
    .is_err());
    let jobs: i64 =
        sqlx::query_scalar("SELECT count(*) FROM notification_dispatch_jobs WHERE recipient_id=$1")
            .bind(f.teacher.actor_id)
            .fetch_one(&f.pool)
            .await
            .unwrap();
    assert_eq!(jobs, 1);
    let routine = access(f.admin.actor_id, &[codes::STUDENT_CARE_READ_SCHOOL]);
    assert_eq!(
        overview(&f.pool, routine)
            .await
            .unwrap()
            .urgent_unacknowledged_count,
        0
    );
    assert_eq!(
        overview(&f.pool, f.teacher)
            .await
            .unwrap()
            .urgent_unacknowledged_count,
        1
    );
    let command = CareRevisionCommand {
        request_id: Uuid::now_v7(),
        row_version: first.assignment.row_version,
        reason: "Reviewed fixture".into(),
    };
    assert!(
        assignment_command(&f.pool, f.other, assignment.id, command.clone(), true)
            .await
            .is_err()
    );
    let ack = assignment_command(&f.pool, f.teacher, assignment.id, command.clone(), true)
        .await
        .unwrap();
    assert!(ack.acknowledged_at.is_some());
    assignment_command(&f.pool, f.teacher, assignment.id, command, true)
        .await
        .unwrap();
    let audit:Vec<String>=sqlx::query_scalar("SELECT private_detail_ciphertext FROM care_audit WHERE resource_id=$1 AND action='assignment_acknowledge'").bind(assignment.id).fetch_all(&f.pool).await.unwrap();
    assert_eq!(audit.len(), 1);
    assert!(!audit[0].contains("Reviewed fixture"));
    assert_eq!(
        decrypt::<CareCommandAuditDetail>(&audit[0]).unwrap().reason,
        "Reviewed fixture"
    );

    assert_eq!(
        overview(&f.pool, f.teacher)
            .await
            .unwrap()
            .urgent_unacknowledged_count,
        0
    );
    let stored: String =
        sqlx::query_scalar("SELECT response_ciphertext FROM care_assignments WHERE id=$1")
            .bind(assignment.id)
            .fetch_one(&f.pool)
            .await
            .unwrap();
    assert!(!stored.contains("questionKey"));
    assert!(
        sqlx::query("UPDATE care_instrument_versions SET version=3 WHERE id=$1")
            .bind(v.id)
            .execute(&f.pool)
            .await
            .is_err()
    );
}
#[tokio::test]
async fn care_bulk_scope_age_manual_and_guardian_revocation() {
    let f = fixture("care_denied").await;
    let (v, r) =
        instrument_round(&f, CareInstrumentMode::Digital, CareRespondent::SelfReport).await;
    let read_only = user(&f.pool, "read-only-reviewer", "staff").await;
    let role:Uuid=sqlx::query_scalar("INSERT INTO roles(code,name,user_type) VALUES('CARE_READ_ONLY','Synthetic read only','staff') RETURNING id").fetch_one(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO role_permissions(role_id,permission_id) SELECT $1,id FROM permissions WHERE code=$2").bind(role).bind(codes::STUDENT_CARE_SENSITIVE_READ_ASSIGNED).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO user_roles(user_id,role_id) VALUES($1,$2)")
        .bind(read_only)
        .bind(role)
        .execute(&f.pool)
        .await
        .unwrap();
    let mut selected_connection = f.pool.acquire().await.unwrap();
    assert!(sensitive_staff(&mut selected_connection, &[read_only])
        .await
        .is_err());
    assert!(
        sensitive_staff(&mut selected_connection, &[read_only, f.teacher.actor_id])
            .await
            .is_ok()
    );
    drop(selected_connection);
    let bulk = CreateCareAssignments {
        request_id: Uuid::now_v7(),
        round_id: r.id,
        instrument_version_id: v.id,
        targets: vec![
            CareAssignmentTarget {
                student_id: f.student,
                respondent_id: f.student,
            },
            CareAssignmentTarget {
                student_id: f.second,
                respondent_id: f.second,
            },
        ],
        reviewer_user_ids: vec![f.teacher.actor_id],
    };
    assert!(create_assignments(&f.pool, f.teacher, bulk).await.is_err());
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM care_assignments")
        .fetch_one(&f.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    assert!(save_visit(
        &f.pool,
        f.teacher,
        None,
        visit_payload(&f, f.second, vec![f.teacher.actor_id])
    )
    .await
    .is_err());
    assert!(save_case(&f.pool, f.teacher, None, case_payload(&f, None))
        .await
        .is_err());
    let (t, tr) = instrument_round(&f, CareInstrumentMode::Digital, CareRespondent::Teacher).await;
    let target = CreateCareAssignments {
        request_id: Uuid::now_v7(),
        round_id: tr.id,
        instrument_version_id: t.id,
        targets: vec![CareAssignmentTarget {
            student_id: f.student,
            respondent_id: f.other.actor_id,
        }],
        reviewer_user_ids: vec![f.teacher.actor_id],
    };
    assert!(create_assignments(&f.pool, f.teacher, target.clone())
        .await
        .is_err());
    let teacher_response = create_assignments(&f.pool, f.admin, target).await.unwrap();
    assert_eq!(teacher_response[0].respondent_id, f.other.actor_id);
    let (g, gr) = instrument_round(&f, CareInstrumentMode::Digital, CareRespondent::Guardian).await;
    let ga = assignment(&f, &g, &gr, f.parent).await;
    let parent = access(f.parent, &[codes::STUDENT_CARE_READ_OWN]);
    let p = response(1);
    save_response(&f.pool, parent, ga.id, p.clone(), false)
        .await
        .unwrap();
    sqlx::query("DELETE FROM student_parents WHERE parent_user_id=$1")
        .bind(f.parent)
        .execute(&f.pool)
        .await
        .unwrap();
    assert!(get_assignment(&f.pool, parent, ga.id).await.is_err());
    assert!(save_response(&f.pool, parent, ga.id, p, false)
        .await
        .is_err());
    assert!(assignments(&f.pool, parent, CareListQuery::default())
        .await
        .unwrap()
        .is_empty());
    let (m, mr) = instrument_round(
        &f,
        CareInstrumentMode::ManualResult,
        CareRespondent::SelfReport,
    )
    .await;
    let ma = assignment(&f, &m, &mr, f.student).await;
    let mut p = response(1);
    p.answers.clear();
    p.manual_scores = vec![CareManualScore {
        scale_key: "base".into(),
        score: 1,
    }];
    assert!(save_response(
        &f.pool,
        access(f.student, &[codes::STUDENT_CARE_READ_OWN]),
        ma.id,
        p.clone(),
        true
    )
    .await
    .is_err());
    assert!(save_response(&f.pool, f.teacher, ma.id, p.clone(), true)
        .await
        .is_err());
    save_response(&f.pool, f.teacher, ma.id, p.clone(), false)
        .await
        .unwrap();
    let reopened = get_assignment(&f.pool, f.teacher, ma.id).await.unwrap();
    assert_eq!(reopened.manual_scores[0].score, 1);
    p.row_version = 2;
    p.request_id = Uuid::now_v7();
    p.manual_note = Some("Paper scored by trained fixture staff".into());
    assert!(save_response(&f.pool, f.teacher, ma.id, p, true)
        .await
        .is_ok());
}
#[tokio::test]
async fn care_private_replay_revocation_case_transitions_and_concurrent_revision() {
    let f = fixture("care_replay").await;
    let (v, r) =
        instrument_round(&f, CareInstrumentMode::Digital, CareRespondent::SelfReport).await;
    let assignment = assignment(&f, &v, &r, f.student).await;
    let create = case_payload(&f, Some(assignment.id));
    let case = save_case(&f.pool, f.teacher, None, create.clone())
        .await
        .unwrap();
    assert!(save_case(
        &f.pool,
        f.other,
        None,
        case_payload(&f, Some(assignment.id))
    )
    .await
    .is_err());
    let change = SaveCareCase {
        request_id: Uuid::now_v7(),
        row_version: Some(1),
        assigned_user_ids: vec![f.other.actor_id],
        ..create.clone()
    };
    save_case(&f.pool, f.admin, Some(case.case.id), change)
        .await
        .unwrap();
    assert!(get_case(&f.pool, f.teacher, case.case.id).await.is_err());
    assert!(save_case(&f.pool, f.teacher, None, create).await.is_err());
    let visit_input = visit_payload(&f, f.student, vec![f.teacher.actor_id]);
    let visit = save_visit(&f.pool, f.teacher, None, visit_input.clone())
        .await
        .unwrap();
    let outsider = access(
        f.other.actor_id,
        &[
            codes::STUDENT_CARE_READ_SCHOOL,
            codes::STUDENT_CARE_SENSITIVE_MANAGE_ASSIGNED,
        ],
    );
    let mut escalation = case_payload(&f, None);
    escalation.source_visit_id = Some(visit.visit.id);
    escalation.assigned_user_ids = vec![f.other.actor_id];
    assert!(save_case(&f.pool, outsider, None, escalation)
        .await
        .is_err());
    let change = SaveCareVisit {
        request_id: Uuid::now_v7(),
        row_version: Some(1),
        visitor_ids: vec![f.other.actor_id],
        ..visit_input.clone()
    };
    save_visit(&f.pool, f.admin, Some(visit.visit.id), change)
        .await
        .unwrap();
    let stale = SaveCareVisit {
        request_id: Uuid::now_v7(),
        row_version: Some(1),
        visitor_ids: vec![f.other.actor_id],
        ..visit_input.clone()
    };
    assert!(matches!(
        save_visit(&f.pool, f.admin, Some(visit.visit.id), stale).await,
        Err(AppError::Conflict(_))
    ));
    assert!(save_visit(&f.pool, f.teacher, None, visit_input)
        .await
        .is_err());
    let event = AddCareCaseEvent {
        request_id: Uuid::now_v7(),
        row_version: 2,
        kind: CareEventKind::FollowUp,
        note: "Synthetic follow-up".into(),
        status: CareCaseStatus::Monitoring,
        next_follow_up: Some(
            Utc::now()
                .with_timezone(&chrono_tz::Asia::Bangkok)
                .date_naive()
                - chrono::Duration::days(1),
        ),
    };
    let other_event = AddCareCaseEvent {
        request_id: Uuid::now_v7(),
        ..event.clone()
    };
    let (left, right) = tokio::join!(
        add_case_event(&f.pool, f.other, case.case.id, event),
        add_case_event(&f.pool, f.other, case.case.id, other_event)
    );
    assert_eq!(usize::from(left.is_ok()) + usize::from(right.is_ok()), 1);
    assert!(matches!(
        left.as_ref().err().or(right.as_ref().err()),
        Some(AppError::Conflict(_))
    ));
    let today = Utc::now()
        .with_timezone(&chrono_tz::Asia::Bangkok)
        .date_naive();
    let due = CareListQuery {
        follow_up_before: Some(today),
        ..Default::default()
    };
    assert_eq!(cases(&f.pool, f.other, due.clone()).await.unwrap().len(), 1);
    assert!(cases(&f.pool, f.teacher, due).await.unwrap().is_empty());
    assert_eq!(
        overview(&f.pool, f.other)
            .await
            .unwrap()
            .follow_up_due_count,
        1
    );
    assert_eq!(
        overview(&f.pool, f.teacher)
            .await
            .unwrap()
            .follow_up_due_count,
        0
    );
    for n in 0..105 {
        let e = CareCaseEvent {
            id: Uuid::now_v7(),
            kind: CareEventKind::Note,
            note: format!("Synthetic history {n}"),
            created_by: f.other.actor_id,
            created_at: Utc::now(),
        };
        let mut c = f.pool.acquire().await.unwrap();
        store_case_event(&mut c, case.case.id, &e).await.unwrap();
    }
    let first = get_case(&f.pool, f.other, case.case.id).await.unwrap();
    assert_eq!(first.events.len(), 100);
    assert!(first.events[0].id > first.events[99].id);
    let older = list_case_events(
        &f.pool,
        f.other,
        case.case.id,
        CareListQuery {
            after_id: Some(first.events[99].id),
            limit: Some(100),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(older.len(), 8);
    assert!(
        list_case_events(&f.pool, f.teacher, case.case.id, CareListQuery::default())
            .await
            .is_err()
    );
    let closed = add_case_event(
        &f.pool,
        f.other,
        case.case.id,
        AddCareCaseEvent {
            request_id: Uuid::now_v7(),
            row_version: 3,
            kind: CareEventKind::Closure,
            note: "Closed fixture".into(),
            status: CareCaseStatus::Closed,
            next_follow_up: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(closed.case.status, CareCaseStatus::Closed);
    assert_eq!(closed.events[0].kind, CareEventKind::Closure);
    assert_eq!(
        overview(&f.pool, f.other)
            .await
            .unwrap()
            .follow_up_due_count,
        0
    );
}

#[tokio::test]
async fn care_submit_notification_failure_rolls_back_and_retries_once() {
    let f = fixture("care_rollback").await;
    let (v, r) =
        instrument_round(&f, CareInstrumentMode::Digital, CareRespondent::SelfReport).await;
    let assignment = assignment(&f, &v, &r, f.student).await;
    let own = access(f.student, &[codes::STUDENT_CARE_READ_OWN]);
    let p = response(1);
    sqlx::raw_sql("CREATE FUNCTION reject_care_notification() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'synthetic_failure'; END $$; CREATE TRIGGER reject_care_notification BEFORE INSERT ON notification_dispatch_jobs FOR EACH ROW EXECUTE FUNCTION reject_care_notification();").execute(&f.pool).await.unwrap();
    assert!(save_response(&f.pool, own, assignment.id, p.clone(), true)
        .await
        .is_err());
    let state = get_assignment(&f.pool, own, assignment.id).await.unwrap();
    assert_eq!(state.assignment.row_version, 1);
    assert_eq!(state.assignment.status, CareAssignmentStatus::Assigned);
    assert!(state.answers.is_empty());
    let receipt_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM care_mutation_receipts WHERE request_id=$1")
            .bind(p.request_id)
            .fetch_one(&f.pool)
            .await
            .unwrap();
    assert_eq!(receipt_count, 0);
    sqlx::query("DROP TRIGGER reject_care_notification ON notification_dispatch_jobs")
        .execute(&f.pool)
        .await
        .unwrap();
    save_response(&f.pool, own, assignment.id, p.clone(), true)
        .await
        .unwrap();
    save_response(&f.pool, own, assignment.id, p, true)
        .await
        .unwrap();
    let jobs: i64 = sqlx::query_scalar("SELECT count(*) FROM notification_dispatch_jobs")
        .fetch_one(&f.pool)
        .await
        .unwrap();
    assert_eq!(jobs, 1);
}

#[tokio::test]
async fn care_reviewer_handoff_fallback_and_no_active_receiver_still_persists() {
    let f = fixture("care_handoff").await;
    let (v, r) =
        instrument_round(&f, CareInstrumentMode::Digital, CareRespondent::SelfReport).await;
    let digital = assignment(&f, &v, &r, f.student).await;
    let (m, mr) = instrument_round(
        &f,
        CareInstrumentMode::ManualResult,
        CareRespondent::SelfReport,
    )
    .await;
    let manual = assignment(&f, &m, &mr, f.student).await;
    let mut draft = response(1);
    draft.answers.clear();
    draft.manual_scores = vec![CareManualScore {
        scale_key: "base".into(),
        score: 1,
    }];
    save_response(&f.pool, f.teacher, manual.id, draft.clone(), false)
        .await
        .unwrap();
    let extra_round = create_round(
        &f.pool,
        f.admin,
        CreateCareRound {
            request_id: Uuid::now_v7(),
            name: "Second synthetic round".into(),
            purpose: "Fixture".into(),
            instrument_version_ids: vec![v.id],
            academic_term_id: f.term,
            roster_date: f.date,
            due_date: f.date + chrono::Duration::days(5),
        },
    )
    .await
    .unwrap();
    let no_receiver = assignment(&f, &v, &extra_round, f.student).await;
    // The explicit reviewer remains active but loses management. Read-only
    // observers must not prevent notification of active school managers.
    sqlx::query("DELETE FROM user_roles WHERE user_id=$1")
        .bind(f.teacher.actor_id)
        .execute(&f.pool)
        .await
        .unwrap();
    let role:Uuid=sqlx::query_scalar("INSERT INTO roles(code,name,user_type) VALUES('CARE_OBSERVER','Synthetic observer','staff') RETURNING id").fetch_one(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO role_permissions(role_id,permission_id) SELECT $1,id FROM permissions WHERE code=$2").bind(role).bind(codes::STUDENT_CARE_SENSITIVE_READ_ASSIGNED).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO user_roles(user_id,role_id) VALUES($1,$2)")
        .bind(f.teacher.actor_id)
        .bind(role)
        .execute(&f.pool)
        .await
        .unwrap();
    let own = access(f.student, &[codes::STUDENT_CARE_READ_OWN]);
    let submitted = save_response(&f.pool, own, digital.id, response(1), true)
        .await
        .unwrap();
    let receivers:Vec<Uuid>=sqlx::query_scalar("SELECT recipient_id FROM notification_dispatch_jobs WHERE dedup_key LIKE $1 ORDER BY recipient_id").bind(format!("care:urgent:{}:%",digital.id)).fetch_all(&f.pool).await.unwrap();
    assert!(receivers.contains(&f.admin.actor_id));
    assert!(receivers.contains(&f.teacher.actor_id));
    let handoff = UpdateCareAssignmentReviewers {
        request_id: Uuid::now_v7(),
        row_version: submitted.assignment.row_version,
        reviewer_user_ids: vec![f.other.actor_id],
        reason: "Replace synthetic reviewer".into(),
    };
    let changed = update_assignment_reviewers(&f.pool, f.admin, digital.id, handoff.clone())
        .await
        .unwrap();
    assert!(changed.urgent);
    assert_eq!(changed.created_by, f.admin.actor_id);
    assert_eq!(changed.instrument_version_id, v.id);
    let routine = access(f.admin.actor_id, &[codes::STUDENT_CARE_MANAGE_SCHOOL]);
    let replay = update_assignment_reviewers(&f.pool, routine, digital.id, handoff.clone())
        .await
        .unwrap();
    assert!(!replay.urgent);
    assert!(replay.acknowledged_at.is_none());
    assert_eq!(replay.row_version, changed.row_version);
    assert!(update_assignment_reviewers(
        &f.pool,
        f.teacher,
        digital.id,
        UpdateCareAssignmentReviewers {
            request_id: Uuid::now_v7(),
            row_version: changed.row_version,
            ..handoff
        }
    )
    .await
    .is_err());
    assert!(get_assignment(&f.pool, f.teacher, digital.id)
        .await
        .is_err());
    let detail = get_assignment(&f.pool, f.other, digital.id).await.unwrap();
    assert_eq!(detail.answers[0].option_key, "b");
    assert_eq!(detail.results[0].score, 2);
    update_assignment_reviewers(
        &f.pool,
        f.admin,
        manual.id,
        UpdateCareAssignmentReviewers {
            request_id: Uuid::now_v7(),
            row_version: 2,
            reviewer_user_ids: vec![f.other.actor_id],
            reason: "Replace manual reviewer".into(),
        },
    )
    .await
    .unwrap();
    assert!(save_response(&f.pool, f.teacher, manual.id, draft, false)
        .await
        .is_err());
    let jobs: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM notification_dispatch_jobs WHERE dedup_key LIKE $1",
    )
    .bind(format!("care:reviewer-handoff:{}:%", digital.id))
    .fetch_one(&f.pool)
    .await
    .unwrap();
    assert_eq!(jobs, 1);
    // Historical receipt membership cannot authorize private fields after removal.
    let mixed = access(
        f.other.actor_id,
        &[
            codes::STUDENT_CARE_MANAGE_SCHOOL,
            codes::STUDENT_CARE_SENSITIVE_MANAGE_ASSIGNED,
        ],
    );
    let retained_request = UpdateCareAssignmentReviewers {
        request_id: Uuid::now_v7(),
        row_version: changed.row_version,
        reviewer_user_ids: vec![f.other.actor_id],
        reason: "Retain current reviewer".into(),
    };
    let retained =
        update_assignment_reviewers(&f.pool, mixed, digital.id, retained_request.clone())
            .await
            .unwrap();
    assert!(retained.urgent);
    update_assignment_reviewers(
        &f.pool,
        f.admin,
        digital.id,
        UpdateCareAssignmentReviewers {
            request_id: Uuid::now_v7(),
            row_version: retained.row_version,
            reviewer_user_ids: vec![f.admin.actor_id],
            reason: "Remove previous reviewer".into(),
        },
    )
    .await
    .unwrap();
    assert!(get_assignment(&f.pool, mixed, digital.id).await.is_err());
    let safe_replay = update_assignment_reviewers(&f.pool, mixed, digital.id, retained_request)
        .await
        .unwrap();
    assert!(!safe_replay.urgent);
    assert!(safe_replay.acknowledged_at.is_none());
    let mut saved_case_request = case_payload(&f, Some(digital.id));
    saved_case_request.assigned_user_ids = vec![f.other.actor_id];
    let saved_case = save_case(&f.pool, f.admin, None, saved_case_request.clone())
        .await
        .unwrap();
    sqlx::query("UPDATE users SET status='inactive' WHERE id=$1")
        .bind(f.other.actor_id)
        .execute(&f.pool)
        .await
        .unwrap();
    let replay = save_case(&f.pool, f.admin, None, saved_case_request)
        .await
        .unwrap();
    assert_eq!(replay.case.id, saved_case.case.id);
    sqlx::query("UPDATE users SET status='inactive' WHERE user_type='staff'")
        .execute(&f.pool)
        .await
        .unwrap();
    let last = save_response(&f.pool, own, no_receiver.id, response(1), true)
        .await
        .unwrap();
    assert!(last.assignment.urgent);
    assert_eq!(last.assignment.status, CareAssignmentStatus::Submitted);
    let jobs: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM notification_dispatch_jobs WHERE dedup_key LIKE $1",
    )
    .bind(format!("care:urgent:{}:%", no_receiver.id))
    .fetch_one(&f.pool)
    .await
    .unwrap();
    assert_eq!(jobs, 0);
}
