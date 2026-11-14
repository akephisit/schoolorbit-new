//! Destructive requests use the production router, central session admission and real owners.
use super::*;
use crate::integration_tests::auth::session_http_tests::protected_router::{
    ProtectedFixture, request,
};
use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
};
use school_auth::session_crypto::RawSessionToken;
use serde_json::{Value, json};
use tower::ServiceExt;

struct Composed {
    fixture: ProtectedFixture,
    app: axum::Router,
    collaboration:
        std::sync::Arc<school_academic_timetable::collaboration::TimetableCollaborationRuntime>,
    actor: Uuid,
    auth_session: Uuid,
    token: RawSessionToken,
    csrf: String,
    group: Uuid,
    sibling: Uuid,
    offering: Uuid,
    year: Uuid,
    term: Uuid,
    session: Uuid,
    survivor: Uuid,
    pending: Uuid,
    consumed: Uuid,
    evidence_file: Uuid,
    unrelated_file: Uuid,
}
impl Composed {
    fn request(&self, method: Method, path: &str, body: Option<&Value>) -> Request<Body> {
        let mut req = request(
            method,
            path,
            Some("https://tenant-a.schoolorbit.test"),
            Some(&self.token),
            Some(&self.csrf),
        );
        if let Some(body) = body {
            req.headers_mut().insert(
                "content-type",
                axum::http::HeaderValue::from_static("application/json"),
            );
            *req.body_mut() = Body::from(body.to_string());
        }
        req
    }
    fn path(&self) -> String {
        format!("/api/academic/learning-groups/{}", self.group)
    }
    async fn preview(&self) -> Value {
        let path = format!(
            "{}/deletion-preview?academicYearId={}&academicTermId={}",
            self.path(),
            self.year,
            self.term
        );
        let response = self
            .app
            .clone()
            .oneshot(self.request(Method::GET, &path, None))
            .await
            .unwrap();
        let status = response.status();
        let body = json_response(response).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body["data"].clone()
    }
    fn command(&self, preview: &Value) -> Value {
        json!({"academicYearId":self.year,"academicTermId":self.term,"rowVersion":preview["rowVersion"],"sourceDigest":preview["sourceDigest"]})
    }
    async fn facts(&self) -> Value {
        sqlx::query_scalar("SELECT jsonb_build_object(
            'groups',(SELECT jsonb_agg(to_jsonb(g) ORDER BY id) FROM learning_groups g),
            'teachers',(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM learning_group_teachers t),
            'members',(SELECT jsonb_agg(to_jsonb(m) ORDER BY id) FROM learning_group_students m),
            'blocks',(SELECT jsonb_agg(to_jsonb(b) ORDER BY id) FROM academic_timetable_blocks b),
            'placements',(SELECT jsonb_agg(to_jsonb(b) ORDER BY id) FROM academic_timetable_block_groups b),
            'exams',(SELECT jsonb_agg(to_jsonb(i) ORDER BY id) FROM academic_exam_schedule_items i),
            'scores',(SELECT jsonb_agg(to_jsonb(s) ORDER BY id) FROM learning_group_student_scores s),
            'results',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM academic_course_results r),
            'corrections',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM academic_result_corrections r),
            'termResults',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM academic_term_aggregate_revisions r),
            'yearResults',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM academic_annual_result_revisions r),
            'sessions',(SELECT jsonb_agg(to_jsonb(s) ORDER BY id) FROM attendance_sessions s),
            'records',(SELECT jsonb_agg(to_jsonb(r) ORDER BY session_id,student_id) FROM attendance_records r),
            'points',(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM behavior_daily_totals t),
            'entries',(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM behavior_entries t),
            'notices',(SELECT jsonb_agg(to_jsonb(n) ORDER BY id) FROM notification_receipts n),
            'evaluators',(SELECT jsonb_agg(to_jsonb(e) ORDER BY id) FROM supervision_evaluators e),
            'inbox',(SELECT jsonb_agg(to_jsonb(n) ORDER BY id) FROM notifications n),
            'files',(SELECT jsonb_agg(to_jsonb(f) ORDER BY id) FROM files f),
            'versions',(SELECT jsonb_agg(to_jsonb(v) ORDER BY id) FROM file_versions v),
            'fileOperations',(SELECT jsonb_agg(to_jsonb(o) ORDER BY id) FROM file_operations o),
            'observations',(SELECT jsonb_agg(to_jsonb(o) ORDER BY id) FROM supervision_observations o))")
            .fetch_one(&self.fixture.tenant_a_pool).await.unwrap()
    }
}
async fn json_response(response: axum::response::Response) -> Value {
    serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), 4_194_304)
            .await
            .unwrap(),
    )
    .unwrap()
}
async fn composed(name: &str) -> Composed {
    composed_with_points(name, 4).await
}
async fn composed_with_points(name: &str, points: i32) -> Composed {
    let pool = prepare_concurrent_delivery_runtime_fixture(name).await;
    let (group,offering,year,term,student,student_year,grade):(Uuid,Uuid,Uuid,Uuid,Uuid,Uuid,Uuid)=sqlx::query_as("SELECT g.id,g.learning_offering_id,g.academic_year_id,g.academic_term_id,m.student_id,m.student_academic_year_id,y.grade_level_id FROM learning_groups g JOIN academic_terms t ON t.id=g.academic_term_id JOIN course_offering_details d ON d.learning_offering_id=g.learning_offering_id JOIN learning_group_students m ON m.learning_group_id=g.id JOIN student_academic_years y ON y.id=m.student_academic_year_id WHERE t.status='active' AND EXISTS(SELECT 1 FROM academic_timetable_block_groups b WHERE b.learning_group_id=g.id) AND EXISTS(SELECT 1 FROM academic_exam_schedule_items i WHERE i.learning_group_id=g.id) ORDER BY g.id,m.id LIMIT 1")
        .fetch_one(&pool).await.unwrap();
    let fixture = ProtectedFixture::with_prepared_tenant_pool(name, pool).await;
    let actor = fixture
        .insert_user(&fixture.tenant_a_pool, &format!("{name}-manager"), "active")
        .await;
    let role:Uuid=sqlx::query_scalar("INSERT INTO roles(code,name,user_type) VALUES($1,'Exact deletion manager','staff') RETURNING id").bind(format!("DELETE-{actor}")).fetch_one(&fixture.tenant_a_pool).await.unwrap();
    sqlx::query("INSERT INTO role_permissions(role_id,permission_id) SELECT $1,id FROM permissions WHERE code=$2").bind(role).bind(school_permissions::registry::codes::LEARNING_OFFERING_MANAGE_SCHOOL).execute(&fixture.tenant_a_pool).await.unwrap();
    sqlx::query("INSERT INTO user_roles(user_id,role_id) VALUES($1,$2)")
        .bind(actor)
        .bind(role)
        .execute(&fixture.tenant_a_pool)
        .await
        .unwrap();
    let (session_id, token) = fixture
        .insert_session(&fixture.tenant_a_pool, actor, 81, false)
        .await;
    let csrf = fixture.csrf(session_id);
    let sibling = Uuid::new_v4();
    sqlx::query("INSERT INTO learning_groups SELECT (jsonb_populate_record(NULL::learning_groups,to_jsonb(g)||jsonb_build_object('id',$2::uuid,'code',$2::text,'name','Retained sibling'))).* FROM learning_groups g WHERE id=$1").bind(group).bind(sibling).execute(&fixture.tenant_a_pool).await.unwrap();
    sqlx::query("INSERT INTO learning_group_students SELECT (jsonb_populate_record(NULL::learning_group_students,to_jsonb(m)||jsonb_build_object('id',uuidv7(),'learning_group_id',$2::uuid))).* FROM learning_group_students m WHERE learning_group_id=$1").bind(group).bind(sibling).execute(&fixture.tenant_a_pool).await.unwrap();
    let date = runtime_operation_date();
    let session = Uuid::new_v4();
    let survivor = Uuid::new_v4();
    for (id, selected) in [(session, true), (survivor, false)] {
        sqlx::query("INSERT INTO attendance_sessions(id,academic_term_id,date,kind,source_key,title,teacher_ids,learning_group_id,offering_id,start_time,end_time,count_override,roster) VALUES($1,$2,$3,'lesson',$4,'Saved exact lesson',ARRAY[]::uuid[],$5,$6,'08:00','09:00',true,jsonb_build_array(jsonb_build_object('studentId',$7::uuid,'studentAcademicYearId',$8::uuid,'classNumber',NULL)))")
            .bind(id).bind(term).bind(date).bind(id.to_string()).bind(if selected {group}else{sibling}).bind(offering).bind(student).bind(student_year).execute(&fixture.tenant_a_pool).await.unwrap();
        sqlx::query("INSERT INTO attendance_records(session_id,student_id,student_academic_year_id,result,origin) VALUES($1,$2,$3,'absent','teacher')").bind(id).bind(student).bind(student_year).execute(&fixture.tenant_a_pool).await.unwrap();
    }
    let generation: Uuid =
        sqlx::query_scalar("SELECT generation FROM attendance_module_state WHERE singleton")
            .fetch_one(&fixture.tenant_a_pool)
            .await
            .unwrap();
    sqlx::query("UPDATE behavior_settings SET enabled=true,count_absent_lessons=true,daily_deduction_cap=5 WHERE singleton").execute(&fixture.tenant_a_pool).await.unwrap();
    sqlx::query("INSERT INTO behavior_attendance_rules(kind,status,effective_from,action,points,created_by) VALUES('lesson','absent',$1,$3,$4,$2)").bind(date).bind(actor).bind(if points==0{"none"}else{"deduction"}).bind(points).execute(&fixture.tenant_a_pool).await.unwrap();
    let revision: i64 = sqlx::query_scalar("SELECT nextval('attendance_score_revision_seq')")
        .fetch_one(&fixture.tenant_a_pool)
        .await
        .unwrap();
    let mut tx = fixture.tenant_a_pool.begin().await.unwrap();
    school_behavior::services::integration::replace_attendance_day(
        &mut tx,
        school_behavior::models::BehaviorAttendanceDay {
            generation,
            academic_term_id: term,
            date,
            revision,
            facts: [session, survivor]
                .into_iter()
                .map(|id| school_behavior::models::BehaviorAttendanceFact {
                    source_key: id.to_string(),
                    source_session_id: Some(id),
                    kind: school_behavior::models::BehaviorSource::Lesson,
                    student_id: student,
                    status: "absent".into(),
                    counted: true,
                    grade_level_id: grade,
                    homeroom_id: None,
                    overrides: None,
                })
                .collect(),
        },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let pending = Uuid::new_v4();
    let consumed = Uuid::new_v4();
    for (receipt, is_consumed) in [(pending, false), (consumed, true)] {
        let ledger = receipt;
        let notification = Uuid::new_v4();
        let key = if is_consumed {
            format!("correction:{session}:{student}:1")
        } else {
            format!("session:{session}:{student}")
        };
        sqlx::query("INSERT INTO attendance_notifications(id,academic_term_id,recipient_id,student_id,dedup_key,notification_id) VALUES($1,$2,$3,$3,$4,$5)").bind(ledger).bind(term).bind(student).bind(key).bind(notification).execute(&fixture.tenant_a_pool).await.unwrap();
        sqlx::query("INSERT INTO notifications(id,user_id,title,message,type,link) VALUES($1,$2,'Saved lesson','Saved lesson','info','/student/attendance')").bind(notification).bind(student).execute(&fixture.tenant_a_pool).await.unwrap();
        sqlx::query("INSERT INTO notification_receipts(id,notification_id,recipient_id,source_kind,source_id,idempotency_key,payload_hash,inbox_accepted_at,presentation_consumed_at,presentation_outcome) VALUES($1,$2,$3,'attendance',$4,$5,sha256(convert_to(jsonb_build_array('Saved lesson','Saved lesson','info','/student/attendance')::text,'UTF8')),now(),CASE WHEN $6 THEN now() ELSE NULL END,CASE WHEN $6 THEN 'attempted' ELSE NULL END)").bind(receipt).bind(notification).bind(student).bind(ledger).bind(receipt.to_string()).bind(is_consumed).execute(&fixture.tenant_a_pool).await.unwrap();
        if !is_consumed {
            sqlx::query("INSERT INTO notification_presentations(receipt_id,expires_at) VALUES($1,now()+interval '15 minutes')").bind(receipt).execute(&fixture.tenant_a_pool).await.unwrap();
        }
    }
    let (plan,phase):(Uuid,Uuid)=sqlx::query_as("SELECT p.id,phase.id FROM course_assessment_plans p JOIN course_assessment_phases phase ON phase.plan_id=p.id WHERE p.learning_offering_id=$1 ORDER BY phase.phase_code,phase.id LIMIT 1").bind(offering).fetch_one(&fixture.tenant_a_pool).await.unwrap();
    let item = Uuid::new_v4();
    sqlx::query("INSERT INTO learning_group_score_items(id,learning_group_id,learning_offering_id,course_assessment_plan_id,assessment_phase_id,academic_term_id,academic_year_id,name,max_score) VALUES($1,$2,$3,$4,$5,$6,$7,'Exact saved score',10)").bind(item).bind(group).bind(offering).bind(plan).bind(phase).bind(term).bind(year).execute(&fixture.tenant_a_pool).await.unwrap();
    sqlx::query("INSERT INTO learning_group_student_scores(learning_group_id,learning_offering_id,academic_term_id,academic_year_id,score_item_id,student_academic_year_id,score) VALUES($1,$2,$3,$4,$5,$6,7.25)").bind(group).bind(offering).bind(term).bind(year).bind(item).bind(student_year).execute(&fixture.tenant_a_pool).await.unwrap();
    // Frozen Results are independent facts, removed only through their exact owning primitive.
    let subject: Uuid = sqlx::query_scalar(
        "SELECT subject_id FROM course_offering_details WHERE learning_offering_id=$1",
    )
    .bind(offering)
    .fetch_one(&fixture.tenant_a_pool)
    .await
    .unwrap();
    let lock = Uuid::new_v4();
    let result = Uuid::new_v4();
    sqlx::query("INSERT INTO academic_course_result_locks(id,subject_id,academic_term_id,academic_year_id,policy_version_id,policy_snapshot,roster_checksum,source_checksum,source_snapshot,locked_by) VALUES($1,$2,$3,$4,'06000000-0000-0000-0000-000000000001','{}',repeat('0',64),repeat('0',64),$5,$6)")
        .bind(lock).bind(subject).bind(term).bind(year).bind(json!({"groups":[{"learningGroupId":group,"learningOfferingId":offering},{"learningGroupId":sibling,"learningOfferingId":offering}]})).bind(actor).execute(&fixture.tenant_a_pool).await.unwrap();
    sqlx::query("INSERT INTO academic_course_results(id,learning_group_id,learning_offering_id,academic_term_id,academic_year_id,subject_id,course_result_lock_id,student_academic_year_id,calculated_score,calculated_grade,outcome,numeric_grade,selection_source) VALUES($1,$2,$3,$4,$5,$6,$7,$8,80,4,'numeric',4,'derived')")
        .bind(result).bind(group).bind(offering).bind(term).bind(year).bind(subject).bind(lock).bind(student_year).execute(&fixture.tenant_a_pool).await.unwrap();
    sqlx::query("INSERT INTO academic_result_corrections(course_result_id,old_course_outcome,new_course_outcome,old_numeric_grade,new_numeric_grade,expected_effective_version,corrected_by) VALUES($1,'numeric','numeric',4,2,1,$2)").bind(result).bind(actor).execute(&fixture.tenant_a_pool).await.unwrap();
    let policy = Uuid::new_v4();
    let aggregate = Uuid::new_v4();
    let annual = Uuid::new_v4();
    sqlx::query("INSERT INTO academic_aggregate_policy_versions(id,name,passing_grade,minimum_learner_level,allow_reviewed_holds,approved_by) VALUES($1,'Exact current deletion',1,1,true,$2)").bind(policy).bind(actor).execute(&fixture.tenant_a_pool).await.unwrap();
    let snapshot = json!({"policy":{"id":policy,"name":"Exact current deletion","passingGrade":"1.00","minimumLearnerLevel":1,"allowReviewedHolds":true,"approvedBy":actor,"approvedAt":"2025-01-01T00:00:00Z"},
        "results":{"academicYearId":year,"academicTermId":term,"studentAcademicYearId":student_year,"passingGrade":"1.00","courses":[{"subjectId":subject,"learningOfferingId":offering,"resultId":result,"effectiveVersion":2,"credits":"1.00","outcome":"numeric","numericGrade":"2.00"}],"totals":{},"activities":[],"activityTotals":{},"sourceChecksum":"0".repeat(64)},
        "learnerEvaluations":{"studentAcademicYearId":student_year,"policyVersionId":"06000000-0000-0000-0000-000000000002","domains":[]},"blockers":[],"holdFindings":[],"canLock":true,"sourceChecksum":"0".repeat(64)});
    sqlx::query("INSERT INTO academic_term_aggregate_revisions(id,student_academic_year_id,academic_year_id,academic_term_id,revision,policy_id,source_checksum,request_id,request_checksum,snapshot,official_gpa,locked_by) VALUES($1,$2,$3,$4,1,$5,repeat('0',64),$6,repeat('0',64),$7,2,$8)")
        .bind(aggregate).bind(student_year).bind(year).bind(term).bind(policy).bind(Uuid::new_v4()).bind(&snapshot).bind(actor).execute(&fixture.tenant_a_pool).await.unwrap();
    let annual_snapshot = json!({"academicYearId":year,"studentAcademicYearId":student_year,"terms":[{"academicTermId":term,"termName":"Current fixture","sequence":1,"isCurrent":true,"revision":{"id":aggregate,"revision":1,"snapshot":snapshot,"officialGpa":"2.00","holdReason":null,"lockedBy":actor,"lockedAt":"2025-01-01T00:00:00Z","isCurrent":true}}],"totals":{},"canLock":true,"needsHold":false,"sourceChecksum":"0".repeat(64)});
    sqlx::query("INSERT INTO academic_annual_result_revisions(id,student_academic_year_id,academic_year_id,revision,source_checksum,request_id,request_checksum,snapshot,official_gpa,locked_by) VALUES($1,$2,$3,1,repeat('0',64),$4,repeat('0',64),$5,2,$6)")
        .bind(annual).bind(student_year).bind(year).bind(Uuid::new_v4()).bind(annual_snapshot).bind(actor).execute(&fixture.tenant_a_pool).await.unwrap();
    sqlx::query("INSERT INTO academic_annual_result_term_sources(annual_revision_id,academic_year_id,academic_term_id,student_academic_year_id,term_aggregate_revision_id) VALUES($1,$2,$3,$4,$5)").bind(annual).bind(year).bind(term).bind(student_year).bind(aggregate).execute(&fixture.tenant_a_pool).await.unwrap();
    // Saved Supervision evaluation/context survives source detachment.
    assert_eq!(sqlx::query("UPDATE supervision_observations SET learning_group_id=$1,timetable_block_group_id=(SELECT id FROM academic_timetable_block_groups WHERE learning_group_id=$1 ORDER BY id LIMIT 1),lesson_snapshot=$2 WHERE academic_term_id=$3")
        .bind(group).bind(json!({"savedSubject":"Actual saved lesson","savedPeriod":"08:00-09:00"})).bind(term).execute(&fixture.tenant_a_pool).await.unwrap().rows_affected(),1);
    sqlx::query("INSERT INTO supervision_evaluators(observation_id,evaluator_user_id,status,submitted_at) SELECT id,observed_user_id,'submitted',now() FROM supervision_observations WHERE academic_term_id=$1 ON CONFLICT(observation_id,evaluator_user_id) DO UPDATE SET status='submitted',submitted_at=EXCLUDED.submitted_at").bind(term).execute(&fixture.tenant_a_pool).await.unwrap();
    // A genuine manual score is never part of Attendance-derived reconciliation.
    let manual_item:Uuid=sqlx::query_scalar("INSERT INTO behavior_items(name,side) VALUES('Independent manual merit','merit') RETURNING id").fetch_one(&fixture.tenant_a_pool).await.unwrap();
    let manual_version:Uuid=sqlx::query_scalar("INSERT INTO behavior_item_versions(item_id,effective_from,points,created_by) VALUES($1,$2,7,$3) RETURNING id").bind(manual_item).bind(date).bind(actor).fetch_one(&fixture.tenant_a_pool).await.unwrap();
    let batch:Uuid=sqlx::query_scalar("INSERT INTO behavior_batches(academic_term_id,item_version_id,occurred_at,date,created_by) VALUES($1,$2,now(),$3,$4) RETURNING id").bind(term).bind(manual_version).bind(date).bind(actor).fetch_one(&fixture.tenant_a_pool).await.unwrap();
    let total:Uuid=sqlx::query_scalar("INSERT INTO behavior_daily_totals(student_id,date,academic_term_id,grade_level_id,side,source,cycle_id,points) SELECT $1,$2,$3,$4,'merit','manual',id,7 FROM behavior_cycles WHERE side='merit' AND state='current' RETURNING id").bind(student).bind(date).bind(term).bind(grade).fetch_one(&fixture.tenant_a_pool).await.unwrap();
    sqlx::query("INSERT INTO behavior_entries(total_id,batch_id,item_name,points,created_by) VALUES($1,$2,'Independent manual merit',7,$3)").bind(total).bind(batch).bind(actor).execute(&fixture.tenant_a_pool).await.unwrap();
    let evidence_file = Uuid::new_v4();
    let unrelated_file = Uuid::new_v4();
    let file_version = Uuid::new_v4();
    let device = Uuid::new_v4();
    for id in [evidence_file, unrelated_file] {
        sqlx::query("INSERT INTO files(id,owner_user_id,display_filename,created_by,purpose_code,visibility,lifecycle_status) VALUES($1,$2,'synthetic-evidence.jpg',$3,'attendance_evidence','private','ready')").bind(id).bind(student).bind(actor).execute(&fixture.tenant_a_pool).await.unwrap();
    }
    sqlx::query("INSERT INTO file_versions(id,file_id,version_number,provider_code,storage_class,storage_status,object_key,detected_mime_type,canonical_extension,byte_size,checksum,scan_status,created_by) VALUES($1,$2,1,'r2','private','stored',$3,'image/jpeg','jpg',1024,repeat('0',64),'clean',$4)").bind(file_version).bind(evidence_file).bind(format!("tenants/{}/attendance/evidence/{evidence_file}/v1/original.jpg",fixture.tenant_a_id)).bind(actor).execute(&fixture.tenant_a_pool).await.unwrap();
    sqlx::query("UPDATE files SET current_version_id=$2 WHERE id=$1")
        .bind(evidence_file)
        .bind(file_version)
        .execute(&fixture.tenant_a_pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO attendance_devices(id,name,paired_by,operator_id) VALUES($1,'Exact fixture device',$2,$2)").bind(device).bind(actor).execute(&fixture.tenant_a_pool).await.unwrap();
    sqlx::query("INSERT INTO attendance_scan_events(id,session_id,student_id,device_id,evidence_file_id,captured_at) VALUES($1,$2,$3,$4,$5,now())").bind(Uuid::new_v4()).bind(session).bind(student).bind(device).bind(evidence_file).execute(&fixture.tenant_a_pool).await.unwrap();
    let mut binding = fixture.tenant_a_pool.begin().await.unwrap();
    sqlx::query("SELECT set_config('schoolorbit.storage_binding_attestation',repeat('b',64),true)")
        .execute(&mut *binding)
        .await
        .unwrap();
    sqlx::query("UPDATE file_storage_cleanup_watches SET namespace_fingerprint=repeat('a',64),namespace_bound_at=now(),binding_evidence_digest=repeat('b',64) WHERE target_kind='version' AND target_id=$1").bind(file_version).execute(&mut *binding).await.unwrap();
    binding.commit().await.unwrap();
    let state = fixture.application_state();
    let collaboration = state.websocket_manager.collaboration();
    let app = crate::app::build_app(state);
    Composed {
        fixture,
        app,
        collaboration,
        actor,
        auth_session: session_id,
        token,
        csrf,
        group,
        sibling,
        offering,
        year,
        term,
        session,
        survivor,
        pending,
        consumed,
        evidence_file,
        unrelated_file,
    }
}

#[tokio::test]
async fn current_composed_http_group_deletion_removes_exact_children_and_preserves_other_facts() {
    let f = composed("delivery_composed_exact").await;
    let before = f.facts().await;
    let preview = f.preview().await;
    for key in [
        "learningGroups",
        "timetableBlocks",
        "examScheduleItems",
        "assessmentScores",
        "attendanceSessions",
        "derivedBehaviorEvents",
        "officialResults",
        "resultCorrections",
        "termAggregates",
        "annualAggregates",
        "supervisionLinks",
        "evidenceFiles",
    ] {
        assert!(
            preview["impact"][key].as_i64().unwrap() > 0,
            "actual fixture must exercise {key}: {preview}"
        );
    }
    assert_eq!(before, f.facts().await, "preview is read only");
    let response = f
        .app
        .clone()
        .oneshot(f.request(Method::DELETE, &f.path(), Some(&f.command(&preview))))
        .await
        .unwrap();
    let status = response.status();
    let body = json_response(response).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["deleted"], true);
    let after = f.facts().await;
    for key in ["groups", "teachers", "members", "placements"] {
        let keep = |facts: &Value| {
            facts[key]
                .as_array()
                .map(Vec::as_slice)
                .unwrap_or(&[])
                .iter()
                .filter(|row| {
                    if key == "groups" {
                        row["id"] != f.group.to_string()
                    } else {
                        row["learning_group_id"] != f.group.to_string()
                    }
                })
                .cloned()
                .collect::<Vec<_>>()
        };
        assert_eq!(
            keep(&before),
            keep(&after),
            "every retained {key} fact stays exact"
        );
    }
    assert_eq!(before["evaluators"], after["evaluators"]);
    assert_eq!(before["inbox"], after["inbox"]);
    let file = |facts: &Value, id: Uuid| {
        facts["files"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == id.to_string())
            .unwrap()
            .clone()
    };
    assert_eq!(
        file(&before, f.unrelated_file),
        file(&after, f.unrelated_file)
    );
    assert_eq!(
        file(&after, f.evidence_file)["lifecycle_status"],
        "delete_requested"
    );
    let mut expected_versions = before["versions"].clone();
    for version in expected_versions.as_array_mut().unwrap() {
        if version["file_id"] == f.evidence_file.to_string() {
            version["storage_status"] = json!("delete_requested");
        }
    }
    assert_eq!(
        expected_versions, after["versions"],
        "only durable deletion state changes; physical object cleanup remains queued"
    );
    assert!(after["fileOperations"].as_array().unwrap().iter().any(
        |operation| operation["file_id"] == f.evidence_file.to_string()
            && operation["operation_type"] == "delete_object"
            && operation["status"] == "pending"
    ));
    assert!(after["results"].is_null());
    assert!(after["corrections"].is_null());
    let term = after["termResults"].as_array().unwrap();
    assert_eq!(term.len(), 1);
    assert!(term[0]["official_gpa"].is_null());
    let annual = after["yearResults"].as_array().unwrap();
    assert_eq!(annual.len(), 1);
    assert!(annual[0]["official_gpa"].is_null());
    assert!(
        term[0]["snapshot"]["results"]["courses"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        annual[0]["snapshot"]["terms"][0]["revision"]["snapshot"]["results"]["courses"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    for (old, new) in before["observations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(after["observations"].as_array().unwrap())
    {
        assert_eq!(old["id"], new["id"]);
        assert_eq!(old["status"], new["status"]);
        assert_eq!(old["lesson_snapshot"], new["lesson_snapshot"]);
        assert!(new["learning_group_id"].is_null());
        assert!(new["timetable_block_group_id"].is_null());
    }
    let manual = |facts: &Value| {
        facts["entries"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e["item_name"] == "Independent manual merit")
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(manual(&before), manual(&after));

    assert!(
        after["groups"]
            .as_array()
            .unwrap()
            .iter()
            .any(|g| g["id"] == f.sibling.to_string())
    );
    assert!(
        !after["groups"]
            .as_array()
            .unwrap()
            .iter()
            .any(|g| g["id"] == f.group.to_string())
    );
    assert!(
        sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM learning_offerings WHERE id=$1)"
        )
        .bind(f.offering)
        .fetch_one(&f.fixture.tenant_a_pool)
        .await
        .unwrap()
    );
    let retained = |facts: &Value, key: &str, id: Uuid| {
        facts[key]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == id.to_string())
            .unwrap()
            .clone()
    };
    assert_eq!(
        retained(&before, "sessions", f.survivor),
        retained(&after, "sessions", f.survivor)
    );
    assert_eq!(
        retained(&before, "notices", f.consumed),
        retained(&after, "notices", f.consumed)
    );
    assert!(
        !after["sessions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["id"] == f.session.to_string())
    );
    let pending = retained(&after, "notices", f.pending);
    assert_eq!(pending["presentation_outcome"], "skipped");
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT COALESCE(sum(points),0)::bigint FROM behavior_daily_totals WHERE academic_term_id=$1 AND source='lesson' AND side='deduction'").bind(f.term).fetch_one(&f.fixture.tenant_a_pool).await.unwrap(),4,"canonical cap replay releases allowance to retained actual source");
}

#[tokio::test]
async fn current_composed_http_late_fault_rolls_back_all_owners_and_stale_evidence_refuses() {
    let f = composed("delivery_composed_rollback").await;
    let preview = f.preview().await;
    let before = f.facts().await;
    sqlx::raw_sql("CREATE FUNCTION fail_exact_current_group_delete() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM pg_advisory_xact_lock(9714058); RAISE EXCEPTION 'EXACT_DELETE_LATE_FAULT'; END $$;CREATE TRIGGER fail_exact_current_group_delete BEFORE DELETE ON learning_groups FOR EACH ROW EXECUTE FUNCTION fail_exact_current_group_delete()")
        .execute(&f.fixture.tenant_a_pool).await.unwrap();
    let mut fault_gate = f.fixture.tenant_a_pool.acquire().await.unwrap();
    sqlx::query("SELECT pg_advisory_lock(9714058)")
        .execute(&mut *fault_gate)
        .await
        .unwrap();
    let app = f.app.clone();
    let req = f.request(Method::DELETE, &f.path(), Some(&f.command(&preview)));
    let request = tokio::spawn(async move { app.oneshot(req).await.unwrap() });
    wait_for_delete_gate(&f.fixture.tenant_a_pool).await;
    assert!(
        sqlx::query_scalar::<_, bool>("SELECT pg_advisory_unlock(9714058)")
            .fetch_one(&mut *fault_gate)
            .await
            .unwrap()
    );
    drop(fault_gate);
    let response = request.await.unwrap();
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        before,
        f.facts().await,
        "late source-delete fault rolls back every real owner"
    );
    sqlx::query("DROP TRIGGER fail_exact_current_group_delete ON learning_groups")
        .execute(&f.fixture.tenant_a_pool)
        .await
        .unwrap();
    sqlx::query("UPDATE learning_group_student_scores SET score=8,row_version=row_version+1 WHERE learning_group_id=$1").bind(f.group).execute(&f.fixture.tenant_a_pool).await.unwrap();
    let before = f.facts().await;
    let response = f
        .app
        .clone()
        .oneshot(f.request(Method::DELETE, &f.path(), Some(&f.command(&preview))))
        .await
        .unwrap();
    let status = response.status();
    let body = json_response(response).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(before, f.facts().await);
    let mut mismatched = f.command(&f.preview().await);
    mismatched["academicYearId"] = json!(Uuid::new_v4());
    let response = f
        .app
        .clone()
        .oneshot(f.request(Method::DELETE, &f.path(), Some(&mismatched)))
        .await
        .unwrap();
    assert!(!response.status().is_success());
    assert_eq!(before, f.facts().await);
}

async fn wait_for_delete_gate(pool: &PgPool) {
    for _ in 0..300 {
        let waiting:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname=current_database() AND wait_event_type='Lock' AND query LIKE '%DELETE FROM learning_groups WHERE id=ANY%')").fetch_one(pool).await.unwrap();
        if waiting {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    panic!("accepted real deletion did not reach the late source gate");
}
#[tokio::test]
async fn current_composed_http_abort_keeps_accepted_delete_and_serializes_queued_save() {
    let f = composed("delivery_composed_abort").await;
    let completion =
        school_auth_http::tenant::tenant_context_by_subdomain(&f.fixture.runtime, "tenant-a")
            .await
            .unwrap();
    let preview = f.preview().await;
    let offering = offerings::get(&completion.pool, f.offering).await.unwrap();
    let gate_key = 9_714_057_i64;
    sqlx::raw_sql("CREATE FUNCTION gate_current_group_delete() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM pg_advisory_xact_lock(9714057); RETURN OLD; END $$;CREATE TRIGGER gate_current_group_delete BEFORE DELETE ON learning_groups FOR EACH ROW EXECUTE FUNCTION gate_current_group_delete()")
        .execute(&f.fixture.tenant_a_pool).await.unwrap();
    let mut gate = f.fixture.tenant_a_pool.acquire().await.unwrap();
    sqlx::query("SELECT pg_advisory_lock($1)")
        .bind(gate_key)
        .execute(&mut *gate)
        .await
        .unwrap();
    let app = f.app.clone();
    let req = f.request(Method::DELETE, &f.path(), Some(&f.command(&preview)));
    let http = tokio::spawn(async move { app.oneshot(req).await.unwrap() });
    wait_for_delete_gate(&f.fixture.tenant_a_pool).await;
    http.abort();
    assert!(http.await.unwrap_err().is_cancelled());
    assert_eq!(
        f.collaboration.accepted_write_count(),
        1,
        "accepted admission remains owned while the detached command waits before commit"
    );
    let app = f.app.clone();
    let req = f.request(
        Method::PATCH,
        &format!(
            "/api/academic/offerings/{}/weekly-period-target",
            f.offering
        ),
        Some(&json!({"rowVersion":offering.row_version,"weeklyPeriodTarget":7})),
    );
    let mut queued = tokio::spawn(async move { app.oneshot(req).await.unwrap() });
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(100), &mut queued)
            .await
            .is_err(),
        "queued save must wait for the accepted deletion term lock"
    );
    let still_present: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM learning_groups WHERE id=$1)")
            .bind(f.group)
            .fetch_one(&f.fixture.tenant_a_pool)
            .await
            .unwrap();
    assert!(
        still_present,
        "uncommitted deletion is invisible while accepted task holds gate"
    );
    assert!(
        sqlx::query_scalar::<_, bool>("SELECT pg_advisory_unlock($1)")
            .bind(gate_key)
            .fetch_one(&mut *gate)
            .await
            .unwrap()
    );
    drop(gate);
    let response = tokio::time::timeout(std::time::Duration::from_secs(10), queued)
        .await
        .unwrap()
        .unwrap();
    let status = response.status();
    let body = json_response(response).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        !sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM learning_groups WHERE id=$1)")
            .bind(f.group)
            .fetch_one(&f.fixture.tenant_a_pool)
            .await
            .unwrap()
    );
    assert_eq!(
        offerings::get(&completion.pool, f.offering)
            .await
            .unwrap()
            .weekly_period_target,
        7
    );
    assert!(
        sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM learning_groups WHERE id=$1)")
            .bind(f.sibling)
            .fetch_one(&f.fixture.tenant_a_pool)
            .await
            .unwrap()
    );
}

async fn wait_for_sql_lock(pool: &PgPool, fragment: &str) {
    for _ in 0..300 {
        let waiting:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname=current_database() AND wait_event_type='Lock' AND query LIKE '%'||$1||'%')").bind(fragment).fetch_one(pool).await.unwrap();
        if waiting {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    panic!("expected real transaction to wait at {fragment}");
}
#[tokio::test]
async fn current_composed_http_cleanup_lock_precedes_confirmed_digest_acceptance() {
    for explicit_term in [false, true] {
        cleanup_race(explicit_term).await;
    }
}
async fn cleanup_race(explicit_term: bool) {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("school_http=error")
        .with_test_writer()
        .try_init();
    use school_behavior::{models::*, services::cleanup};
    let f = composed_with_points(
        &format!("delivery_composed_cleanup_race_{explicit_term}"),
        0,
    )
    .await;
    let completion =
        school_auth_http::tenant::tenant_context_by_subdomain(&f.fixture.runtime, "tenant-a")
            .await
            .unwrap();
    let preview = f.preview().await;
    assert_eq!(preview["impact"]["derivedBehaviorEvents"], 0);
    let date = runtime_operation_date();
    let scope = BehaviorCleanupScope {
        start: Some(date),
        end: Some(date),
        academic_year_id: None,
        academic_term_id: explicit_term.then_some(f.term),
        sides: vec![BehaviorSide::Deduction],
        sources: vec![BehaviorSource::Lesson],
    };
    let prepared = cleanup::preview(
        &completion.pool,
        f.actor,
        PreviewBehaviorCleanup {
            scope: scope.clone(),
            mode: BehaviorCleanupMode::DeleteEffects,
        },
    )
    .await
    .unwrap();
    sqlx::raw_sql("CREATE FUNCTION gate_exact_behavior_cleanup() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM pg_advisory_xact_lock(9714059); RETURN NEW; END $$;CREATE TRIGGER gate_exact_behavior_cleanup BEFORE INSERT ON behavior_suppressions FOR EACH ROW EXECUTE FUNCTION gate_exact_behavior_cleanup()")
  .execute(&f.fixture.tenant_a_pool).await.unwrap();
    let mut gate = f.fixture.tenant_a_pool.acquire().await.unwrap();
    sqlx::query("SELECT pg_advisory_lock(9714059)")
        .execute(&mut *gate)
        .await
        .unwrap();
    let pool = completion.pool.clone();
    let actor = f.actor;
    let cleanup = tokio::spawn(async move {
        cleanup::execute(
            &pool,
            actor,
            ExecuteBehaviorCleanup {
                request_id: Uuid::new_v4(),
                token: prepared.token,
                revision: prepared.revision,
                scope,
                mode: BehaviorCleanupMode::DeleteEffects,
                reason: "Exact zero-effect cleanup race".into(),
            },
        )
        .await
    });
    wait_for_sql_lock(
        &f.fixture.tenant_a_pool,
        "INSERT INTO behavior_suppressions",
    )
    .await;
    let app = f.app.clone();
    let req = f.request(Method::DELETE, &f.path(), Some(&f.command(&preview)));
    let deletion = tokio::spawn(async move { app.oneshot(req).await.unwrap() });
    wait_for_sql_lock(
        &f.fixture.tenant_a_pool,
        if explicit_term {
            "FROM academic_terms WHERE id=$1 AND academic_year_id=$2 FOR UPDATE"
        } else {
            "pg_advisory_xact_lock(17020000)"
        },
    )
    .await;
    assert!(
        sqlx::query_scalar::<_, bool>("SELECT pg_advisory_unlock(9714059)")
            .fetch_one(&mut *gate)
            .await
            .unwrap()
    );
    drop(gate);
    assert_eq!(cleanup.await.unwrap().unwrap().status, "completed");
    let response = deletion.await.unwrap();
    let status = response.status();
    let body = json_response(response).await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "cleanup changes still-safe source evidence before digest acceptance: {body}"
    );
    assert!(
        sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM learning_groups WHERE id=$1)")
            .bind(f.group)
            .fetch_one(&f.fixture.tenant_a_pool)
            .await
            .unwrap()
    );
    let current = f.preview().await;
    assert_ne!(preview["sourceDigest"], current["sourceDigest"]);
    assert_eq!(current["impact"]["derivedBehaviorEvents"], 0);
    let response = f
        .app
        .clone()
        .oneshot(f.request(Method::DELETE, &f.path(), Some(&f.command(&current))))
        .await
        .unwrap();
    let status = response.status();
    let body = json_response(response).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "a fresh confirmation can delete the same safe zero effects: {body}"
    );
}

#[tokio::test]
async fn current_composed_http_queued_delete_revalidates_permission_generation_and_session() {
    for revoke_session in [false, true] {
        let f = composed(&format!("delivery_queued_auth_{revoke_session}")).await;
        let preview = f.preview().await;
        let before = f.facts().await;
        sqlx::raw_sql("CREATE FUNCTION gate_then_fail_current_delete() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM pg_advisory_xact_lock(9714060); RAISE EXCEPTION 'EXACT_FIRST_DELETE_ROLLBACK'; END $$;CREATE TRIGGER gate_then_fail_current_delete BEFORE DELETE ON learning_groups FOR EACH ROW EXECUTE FUNCTION gate_then_fail_current_delete()")
   .execute(&f.fixture.tenant_a_pool).await.unwrap();
        let mut gate = f.fixture.tenant_a_pool.acquire().await.unwrap();
        sqlx::query("SELECT pg_advisory_lock(9714060)")
            .execute(&mut *gate)
            .await
            .unwrap();
        let app = f.app.clone();
        let req = f.request(Method::DELETE, &f.path(), Some(&f.command(&preview)));
        let first = tokio::spawn(async move { app.oneshot(req).await.unwrap() });
        wait_for_delete_gate(&f.fixture.tenant_a_pool).await;
        let app = f.app.clone();
        let req = f.request(Method::DELETE, &f.path(), Some(&f.command(&preview)));
        let queued = tokio::spawn(async move { app.oneshot(req).await.unwrap() });
        for _ in 0..300 {
            if f.collaboration.accepted_write_count() == 2 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        assert_eq!(
            f.collaboration.accepted_write_count(),
            2,
            "second real request captured authorization before waiting for aggregate admission"
        );
        if revoke_session {
            let response = f
                .app
                .clone()
                .oneshot(f.request(
                    Method::DELETE,
                    &format!("/api/auth/sessions/{}", f.auth_session),
                    None,
                ))
                .await
                .unwrap();
            let status = response.status();
            let body = json_response(response).await;
            assert_eq!(status, StatusCode::OK, "{body}");
        } else {
            sqlx::query("DELETE FROM role_permissions WHERE role_id IN(SELECT role_id FROM user_roles WHERE user_id=$1)").bind(f.actor).execute(&f.fixture.tenant_a_pool).await.unwrap();
            f.fixture
                .runtime
                .permission_cache
                .invalidate_user("tenant-a", f.actor);
        }
        assert!(
            sqlx::query_scalar::<_, bool>("SELECT pg_advisory_unlock(9714060)")
                .fetch_one(&mut *gate)
                .await
                .unwrap()
        );
        drop(gate);
        assert_eq!(
            first.await.unwrap().status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
        let response = queued.await.unwrap();
        let status = response.status();
        let body = json_response(response).await;
        assert_eq!(
            status,
            if revoke_session {
                StatusCode::UNAUTHORIZED
            } else {
                StatusCode::FORBIDDEN
            },
            "queued captured identity must revalidate after gates: {body}"
        );
        assert_eq!(
            before,
            f.facts().await,
            "denial and prior fault preserve every owner fact"
        );
    }
}
