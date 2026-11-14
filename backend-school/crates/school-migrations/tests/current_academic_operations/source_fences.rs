use super::*;
use school_behavior::{
    models::{BehaviorAttendanceDay, BehaviorAttendanceFact, BehaviorSource},
    services::integration,
};

#[tokio::test]
async fn deleted_lesson_consumes_only_owned_notices_and_reconciles_behavior_without_resurrection() {
    let pool = predecessor("current_ops_derived_fences").await;
    let (offering,group,term,student,student_year,grade):(Uuid,Uuid,Uuid,Uuid,Uuid,Uuid)=sqlx::query_as("SELECT o.id,g.id,o.academic_term_id,m.student_id,m.student_academic_year_id,y.grade_level_id FROM learning_offerings o JOIN learning_groups g ON g.learning_offering_id=o.id JOIN learning_group_students m ON m.learning_group_id=g.id JOIN student_academic_years y ON y.id=m.student_academic_year_id WHERE NOT EXISTS(SELECT 1 FROM academic_delivery_versions v CROSS JOIN LATERAL jsonb_array_elements(v.snapshot->'offerings') graph WHERE v.id=academic_delivery_applicable_publication(o.academic_term_id,(now() AT TIME ZONE 'Asia/Bangkok')::date) AND (graph->>'id')::uuid=o.id) LIMIT 1")
        .fetch_one(&pool).await.unwrap();
    let actor: Uuid = sqlx::query_scalar(
        "SELECT id FROM users WHERE user_type='staff' AND status='active' ORDER BY id LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let date: chrono::NaiveDate =
        sqlx::query_scalar("SELECT (now() AT TIME ZONE 'Asia/Bangkok')::date")
            .fetch_one(&pool)
            .await
            .unwrap();
    let generation: Uuid =
        sqlx::query_scalar("SELECT generation FROM attendance_module_state WHERE singleton")
            .fetch_one(&pool)
            .await
            .unwrap();
    let lesson = Uuid::new_v4();
    let arrival = Uuid::new_v4();
    let special = Uuid::new_v4();
    let template = Uuid::new_v4();
    let round = Uuid::new_v4();
    sqlx::query("INSERT INTO attendance_special_templates(id,academic_term_id,definition,created_by) VALUES($1,$2,'{}',$3)").bind(template).bind(term).bind(actor).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO attendance_special_rounds(id,template_id,date) VALUES($1,$2,$3)")
        .bind(round)
        .bind(template)
        .bind(date)
        .execute(&pool)
        .await
        .unwrap();
    for (id, kind) in [
        (lesson, "lesson"),
        (arrival, "arrival"),
        (special, "special"),
    ] {
        sqlx::query("INSERT INTO attendance_sessions(id,academic_term_id,date,kind,source_key,title,teacher_ids,learning_group_id,offering_id,special_round_id,start_time,end_time,count_override,roster) VALUES($1,$2,$3,$4,$5,'Source fence fixture',ARRAY[]::uuid[],$6,$7,$8,'08:00','09:00',true,jsonb_build_array(jsonb_build_object('studentId',$9::uuid,'studentAcademicYearId',$10::uuid,'classNumber',NULL)))")
            .bind(id).bind(term).bind(date).bind(kind).bind(id.to_string()).bind((kind=="lesson").then_some(group)).bind((kind=="lesson").then_some(offering)).bind((kind=="special").then_some(round)).bind(student).bind(student_year).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO attendance_records(session_id,student_id,student_academic_year_id,special_round_id,result,origin) VALUES($1,$2,$3,$4,'present','teacher')")
            .bind(id).bind(student).bind(student_year).bind((kind=="special").then_some(round)).execute(&pool).await.unwrap();
    }
    let mut selected_pending = Vec::new();
    let mut unrelated = Vec::new();
    let mut consumed_id = None;
    for (key, selected, consumed) in [
        (format!("session:{lesson}:{student}"), true, false),
        (format!("correction:{lesson}:{student}:2"), true, false),
        (format!("correction:{lesson}:{student}:1"), true, true),
        (format!("arrival:{term}:{date}:{student}"), false, false),
        (format!("session:{special}:{student}"), false, false),
        (format!("digest:{term}:{date}:08:00:00"), false, false),
    ] {
        let ledger = Uuid::new_v4();
        let notification = Uuid::new_v4();
        let receipt = Uuid::new_v4();
        sqlx::query("INSERT INTO attendance_notifications(id,academic_term_id,recipient_id,student_id,dedup_key,notification_id) VALUES($1,$2,$3,$3,$4,$5)")
            .bind(ledger).bind(term).bind(student).bind(key).bind(notification).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO notifications(id,user_id,title,message,type,link) VALUES($1,$2,'Saved source','Saved source message','info','/student/attendance')")
            .bind(notification).bind(student).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO notification_receipts(id,notification_id,recipient_id,source_kind,source_id,idempotency_key,payload_hash,inbox_accepted_at,presentation_consumed_at,presentation_outcome) VALUES($1,$2,$3,'attendance',$4,$5,sha256(convert_to(jsonb_build_array('Saved source','Saved source message','info','/student/attendance')::text,'UTF8')),now(),CASE WHEN $6 THEN now() ELSE NULL END,CASE WHEN $6 THEN 'attempted' ELSE NULL END)")
            .bind(receipt).bind(notification).bind(student).bind(ledger).bind(receipt.to_string()).bind(consumed).execute(&pool).await.unwrap();
        if !consumed {
            sqlx::query("INSERT INTO notification_presentations(receipt_id,expires_at) VALUES($1,now()+interval '15 minutes')").bind(receipt).execute(&pool).await.unwrap();
        }
        if selected && !consumed {
            selected_pending.push(receipt);
        } else if consumed {
            consumed_id = Some(receipt);
        } else {
            unrelated.push(receipt);
        }
    }
    let unrelated_before: Value = sqlx::query_scalar(
        "SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM notification_receipts r WHERE id=ANY($1)",
    )
    .bind(&unrelated)
    .fetch_one(&pool)
    .await
    .unwrap();
    let consumed_before: Value =
        sqlx::query_scalar("SELECT to_jsonb(r) FROM notification_receipts r WHERE id=$1")
            .bind(consumed_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let inbox_before: Value =
        sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(n) ORDER BY id) FROM notifications n")
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query("UPDATE behavior_settings SET enabled=true,count_absent_lessons=true,count_special=true WHERE singleton").execute(&pool).await.unwrap();
    for (kind, points) in [("lesson", 2), ("arrival", 3), ("special", 4)] {
        sqlx::query("INSERT INTO behavior_attendance_rules(kind,status,effective_from,action,points,created_by) VALUES($1,'present',$2,'merit',$3,$4)")
            .bind(kind).bind(date).bind(points).bind(actor).execute(&pool).await.unwrap();
    }
    let revision: i64 = sqlx::query_scalar("SELECT nextval('attendance_score_revision_seq')")
        .fetch_one(&pool)
        .await
        .unwrap();
    let fact = |id, kind| BehaviorAttendanceFact {
        source_key: if kind == BehaviorSource::Arrival {
            "arrival".into()
        } else {
            Uuid::to_string(&id)
        },
        source_session_id: if kind == BehaviorSource::Arrival {
            None
        } else {
            Some(id)
        },
        kind,
        student_id: student,
        status: "present".into(),
        counted: true,
        grade_level_id: grade,
        homeroom_id: None,
        overrides: None,
    };
    let mut tx = pool.begin().await.unwrap();
    integration::replace_attendance_day(
        &mut tx,
        BehaviorAttendanceDay {
            generation,
            academic_term_id: term,
            date,
            revision,
            facts: vec![
                fact(lesson, BehaviorSource::Lesson),
                fact(arrival, BehaviorSource::Arrival),
                fact(special, BehaviorSource::Special),
            ],
        },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    sqlx::query("INSERT INTO behavior_daily_totals(student_id,date,academic_term_id,grade_level_id,side,source,cycle_id,points,details_purged) SELECT $1,$2,$3,$4,'merit','manual',id,7,true FROM behavior_cycles WHERE side='merit' AND state='current'")
        .bind(student).bind(date).bind(term).bind(grade).execute(&pool).await.unwrap();
    let manual_before:Value=sqlx::query_scalar("SELECT to_jsonb(t) FROM behavior_daily_totals t WHERE student_id=$1 AND academic_term_id=$2 AND source='manual'").bind(student).bind(term).fetch_one(&pool).await.unwrap();
    let effects_before:Value=sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(e) ORDER BY e.id) FROM behavior_entries e JOIN behavior_daily_totals t ON t.id=e.total_id WHERE t.academic_term_id=$1 AND t.source IN('arrival','special')").bind(term).fetch_one(&pool).await.unwrap();
    let malformed = Uuid::new_v4();
    sqlx::query("INSERT INTO attendance_notifications(id,academic_term_id,recipient_id,student_id,dedup_key,notification_id) VALUES($1,$2,$3,$3,$4,$5)")
        .bind(malformed).bind(term).bind(student).bind(format!("session:{lesson}:{}",Uuid::new_v4())).bind(Uuid::new_v4()).execute(&pool).await.unwrap();
    let refused = school_migrations::run_tenant_migrations_detailed(&pool)
        .await
        .unwrap_err();
    assert_eq!(
        refused.diagnostic.code,
        "ACADEMIC_CURRENT_ATTENDANCE_NOTICE_UNMAPPABLE"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM attendance_sessions WHERE id=$1")
            .bind(lesson)
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    sqlx::query("DELETE FROM attendance_notifications WHERE id=$1")
        .bind(malformed)
        .execute(&pool)
        .await
        .unwrap();
    canonical(&pool).await;
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM notification_receipts r JOIN notification_presentations p ON p.receipt_id=r.id WHERE r.id=ANY($1) AND r.presentation_consumed_at IS NULL").bind(&selected_pending).fetch_one(&pool).await.unwrap(),0);
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM notification_receipts WHERE id=ANY($1) AND presentation_consumed_at IS NOT NULL AND presentation_outcome='skipped'").bind(&selected_pending).fetch_one(&pool).await.unwrap(),2);
    assert_eq!(unrelated_before,sqlx::query_scalar::<_,Value>("SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM notification_receipts r WHERE id=ANY($1)").bind(&unrelated).fetch_one(&pool).await.unwrap());
    assert_eq!(
        consumed_before,
        sqlx::query_scalar::<_, Value>(
            "SELECT to_jsonb(r) FROM notification_receipts r WHERE id=$1"
        )
        .bind(consumed_id)
        .fetch_one(&pool)
        .await
        .unwrap()
    );
    assert_eq!(
        inbox_before,
        sqlx::query_scalar::<_, Value>(
            "SELECT jsonb_agg(to_jsonb(n) ORDER BY id) FROM notifications n"
        )
        .fetch_one(&pool)
        .await
        .unwrap()
    );
    let (queued_revision,reset):(i64,bool)=sqlx::query_as("SELECT row_version,reset FROM attendance_score_changes WHERE generation=$1 AND academic_term_id=$2 AND date=$3").bind(generation).bind(term).bind(date).fetch_one(&pool).await.unwrap();
    assert!(!reset);
    assert!(queued_revision > revision);
    let remaining:Vec<(Uuid,String)>=sqlx::query_as("SELECT s.id,s.kind FROM attendance_sessions s JOIN attendance_records r ON r.session_id=s.id WHERE s.academic_term_id=$1 AND s.date=$2 AND r.student_id=$3 ORDER BY s.id")
        .bind(term).bind(date).bind(student).fetch_all(&pool).await.unwrap();
    assert_eq!(remaining.len(), 2);
    let facts = remaining
        .into_iter()
        .map(|(id, kind)| {
            fact(
                id,
                if kind == "arrival" {
                    BehaviorSource::Arrival
                } else {
                    BehaviorSource::Special
                },
            )
        })
        .collect();
    let mut tx = pool.begin().await.unwrap();
    integration::replace_attendance_day(
        &mut tx,
        BehaviorAttendanceDay {
            generation,
            academic_term_id: term,
            date,
            revision: queued_revision,
            facts,
        },
    )
    .await
    .unwrap();
    sqlx::query("DELETE FROM attendance_score_changes WHERE generation=$1 AND academic_term_id=$2 AND date=$3 AND row_version=$4").bind(generation).bind(term).bind(date).bind(queued_revision).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    let mut replay = pool.begin().await.unwrap();
    assert_eq!(
        integration::replace_attendance_day(
            &mut replay,
            BehaviorAttendanceDay {
                generation,
                academic_term_id: term,
                date,
                revision,
                facts: vec![fact(lesson, BehaviorSource::Lesson)]
            }
        )
        .await
        .unwrap(),
        school_behavior::models::BehaviorDeliveryOutcome::Superseded
    );
    replay.commit().await.unwrap();
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT points FROM behavior_daily_totals WHERE academic_term_id=$1 AND source='lesson'").bind(term).fetch_one(&pool).await.unwrap(),0);
    assert!(sqlx::query_scalar::<_, bool>(
        "SELECT cancelled_at IS NOT NULL FROM behavior_entries WHERE source_session_id=$1"
    )
    .bind(lesson)
    .fetch_one(&pool)
    .await
    .unwrap());
    assert_eq!(manual_before,sqlx::query_scalar::<_,Value>("SELECT to_jsonb(t) FROM behavior_daily_totals t WHERE student_id=$1 AND academic_term_id=$2 AND source='manual'").bind(student).bind(term).fetch_one(&pool).await.unwrap());
    assert_eq!(effects_before,sqlx::query_scalar::<_,Value>("SELECT jsonb_agg(to_jsonb(e) ORDER BY e.id) FROM behavior_entries e JOIN behavior_daily_totals t ON t.id=e.total_id WHERE t.academic_term_id=$1 AND t.source IN('arrival','special')").bind(term).fetch_one(&pool).await.unwrap());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM attendance_sessions WHERE id=$1")
            .bind(lesson)
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn archived_lesson_effect_without_offering_attribution_refuses_instead_of_clearing_other_sources(
) {
    let pool = predecessor("current_ops_archived_behavior").await;
    let (offering,term,student,grade):(Uuid,Uuid,Uuid,Uuid)=sqlx::query_as("SELECT o.id,o.academic_term_id,m.student_id,y.grade_level_id FROM learning_offerings o JOIN learning_groups g ON g.learning_offering_id=o.id JOIN learning_group_students m ON m.learning_group_id=g.id JOIN student_academic_years y ON y.id=m.student_academic_year_id WHERE NOT EXISTS(SELECT 1 FROM academic_delivery_versions v CROSS JOIN LATERAL jsonb_array_elements(v.snapshot->'offerings') graph WHERE v.id=academic_delivery_applicable_publication(o.academic_term_id,(now() AT TIME ZONE 'Asia/Bangkok')::date) AND (graph->>'id')::uuid=o.id) LIMIT 1")
        .fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO attendance_term_summaries(academic_term_id,student_id,category,scope_key,scope_label,present,late,absent,skipped,sick_leave,personal_leave,activity,unchecked,expected) VALUES($1,$2,'lesson',$3,'Archived offering',1,0,0,0,0,0,0,0,1)")
        .bind(term).bind(student).bind(offering.to_string()).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO behavior_daily_totals(student_id,date,academic_term_id,grade_level_id,side,source,cycle_id,points,details_purged) SELECT $1,(now() AT TIME ZONE 'Asia/Bangkok')::date,$2,$3,'merit','lesson',id,2,true FROM behavior_cycles WHERE side='merit' AND state='current'")
        .bind(student).bind(term).bind(grade).execute(&pool).await.unwrap();
    let before: Value = sqlx::query_scalar(
        "SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM behavior_daily_totals t",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let error = school_migrations::run_tenant_migrations_detailed(&pool)
        .await
        .unwrap_err();
    assert_eq!(
        error.diagnostic.code,
        "ACADEMIC_CURRENT_BEHAVIOR_ARCHIVE_UNMAPPABLE"
    );
    assert_eq!(
        before,
        sqlx::query_scalar::<_, Value>(
            "SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM behavior_daily_totals t"
        )
        .fetch_one(&pool)
        .await
        .unwrap()
    );
    assert!(sqlx::query_scalar::<_,bool>("SELECT to_regclass('academic_delivery_versions') IS NOT NULL AND to_regclass('academic_timetables') IS NULL").fetch_one(&pool).await.unwrap());
}

#[tokio::test]
async fn protected_historical_lesson_effect_refuses_before_deleting_its_actual_source() {
    let pool = predecessor("current_ops_closed_behavior").await;
    let (offering,group,term,student,grade):(Uuid,Uuid,Uuid,Uuid,Uuid)=sqlx::query_as("SELECT o.id,g.id,o.academic_term_id,m.student_id,y.grade_level_id FROM learning_offerings o JOIN learning_groups g ON g.learning_offering_id=o.id JOIN learning_group_students m ON m.learning_group_id=g.id JOIN student_academic_years y ON y.id=m.student_academic_year_id WHERE NOT EXISTS(SELECT 1 FROM academic_delivery_versions v CROSS JOIN LATERAL jsonb_array_elements(v.snapshot->'offerings') graph WHERE v.id=academic_delivery_applicable_publication(o.academic_term_id,(now() AT TIME ZONE 'Asia/Bangkok')::date) AND (graph->>'id')::uuid=o.id) LIMIT 1")
        .fetch_one(&pool).await.unwrap();
    let lesson = Uuid::new_v4();
    let cycle = Uuid::new_v4();
    sqlx::query("INSERT INTO attendance_sessions(id,academic_term_id,date,kind,source_key,title,teacher_ids,learning_group_id,offering_id,start_time,end_time) VALUES($1,$2,(now() AT TIME ZONE 'Asia/Bangkok')::date-30,'lesson',$3,'Protected lesson',ARRAY[]::uuid[],$4,$5,'08:00','09:00')")
        .bind(lesson).bind(term).bind(lesson.to_string()).bind(group).bind(offering).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO behavior_cycles(id,side,starts_on,starts_at,ends_at,ends_on,state,label) VALUES($1,'merit',(now() AT TIME ZONE 'Asia/Bangkok')::date-60,now()-interval '60 days',now()-interval '1 day',(now() AT TIME ZONE 'Asia/Bangkok')::date-1,'history','Protected fixture')")
        .bind(cycle).execute(&pool).await.unwrap();
    let total:Uuid=sqlx::query_scalar("INSERT INTO behavior_daily_totals(student_id,date,academic_term_id,grade_level_id,side,source,cycle_id,points) VALUES($1,(now() AT TIME ZONE 'Asia/Bangkok')::date-30,$2,$3,'merit','lesson',$4,2) RETURNING id")
        .bind(student).bind(term).bind(grade).bind(cycle).fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO behavior_entries(total_id,source_key,source_session_id,source_generation,source_revision,source_status,raw_points,item_name,points) VALUES($1,$2,$3,(SELECT generation FROM attendance_module_state WHERE singleton),1,'present',2,'Protected contribution',2)")
        .bind(total).bind(lesson.to_string()).bind(lesson).execute(&pool).await.unwrap();
    let error = school_migrations::run_tenant_migrations_detailed(&pool)
        .await
        .unwrap_err();
    assert_eq!(
        error.diagnostic.code,
        "ACADEMIC_CURRENT_BEHAVIOR_EFFECT_PROTECTED"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT points FROM behavior_daily_totals WHERE id=$1")
            .bind(total)
            .fetch_one(&pool)
            .await
            .unwrap(),
        2
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM attendance_sessions WHERE id=$1")
            .bind(lesson)
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn details_only_behavior_cleanup_with_live_lesson_refuses_atomically() {
    use school_behavior::models::{
        BehaviorCleanupMode, BehaviorCleanupScope, BehaviorSide, ExecuteBehaviorCleanup,
        PreviewBehaviorCleanup,
    };
    use school_behavior::services::cleanup;
    let pool = predecessor("current_ops_purged_live_behavior").await;
    let (offering,group,term,student,student_year,grade):(Uuid,Uuid,Uuid,Uuid,Uuid,Uuid)=sqlx::query_as("SELECT o.id,g.id,o.academic_term_id,m.student_id,m.student_academic_year_id,y.grade_level_id FROM learning_offerings o JOIN learning_groups g ON g.learning_offering_id=o.id JOIN learning_group_students m ON m.learning_group_id=g.id JOIN student_academic_years y ON y.id=m.student_academic_year_id WHERE NOT EXISTS(SELECT 1 FROM academic_delivery_versions v CROSS JOIN LATERAL jsonb_array_elements(v.snapshot->'offerings') graph WHERE v.id=academic_delivery_applicable_publication(o.academic_term_id,(now() AT TIME ZONE 'Asia/Bangkok')::date) AND (graph->>'id')::uuid=o.id) LIMIT 1").fetch_one(&pool).await.unwrap();
    let actor: Uuid = sqlx::query_scalar(
        "SELECT id FROM users WHERE user_type='staff' AND status='active' ORDER BY id LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let date: chrono::NaiveDate =
        sqlx::query_scalar("SELECT (now() AT TIME ZONE 'Asia/Bangkok')::date")
            .fetch_one(&pool)
            .await
            .unwrap();
    let lesson = Uuid::new_v4();
    sqlx::query("INSERT INTO attendance_sessions(id,academic_term_id,date,kind,source_key,title,teacher_ids,learning_group_id,offering_id,start_time,end_time,roster) VALUES($1,$2,$3,'lesson',$4,'Purged details live source',ARRAY[]::uuid[],$5,$6,'08:00','09:00',jsonb_build_array(jsonb_build_object('studentId',$7::uuid,'studentAcademicYearId',$8::uuid)))").bind(lesson).bind(term).bind(date).bind(lesson.to_string()).bind(group).bind(offering).bind(student).bind(student_year).execute(&pool).await.unwrap();
    let total:Uuid=sqlx::query_scalar("INSERT INTO behavior_daily_totals(student_id,date,academic_term_id,grade_level_id,side,source,cycle_id,points) SELECT $1,$2,$3,$4,'merit','lesson',id,2 FROM behavior_cycles WHERE side='merit' AND state='current' RETURNING id").bind(student).bind(date).bind(term).bind(grade).fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO behavior_entries(total_id,source_key,source_session_id,source_generation,source_revision,source_status,raw_points,item_name,points) VALUES($1,$2,$3,(SELECT generation FROM attendance_module_state WHERE singleton),1,'present',2,'Saved lesson effect',2)").bind(total).bind(lesson.to_string()).bind(lesson).execute(&pool).await.unwrap();
    let scope = BehaviorCleanupScope {
        start: Some(date),
        end: Some(date),
        academic_year_id: None,
        academic_term_id: Some(term),
        sides: vec![BehaviorSide::Merit],
        sources: vec![BehaviorSource::Lesson],
    };
    let preview = cleanup::preview(
        &pool,
        actor,
        PreviewBehaviorCleanup {
            scope: scope.clone(),
            mode: BehaviorCleanupMode::DetailsOnly,
        },
    )
    .await
    .unwrap();
    let outcome = cleanup::execute(
        &pool,
        actor,
        ExecuteBehaviorCleanup {
            request_id: Uuid::new_v4(),
            token: preview.token,
            revision: preview.revision,
            scope,
            mode: BehaviorCleanupMode::DetailsOnly,
            reason: "Sanitized migration regression details-only cleanup".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(outcome.entries_deleted, 1);
    assert!(sqlx::query_scalar::<_,bool>("SELECT points=2 AND details_purged AND NOT EXISTS(SELECT 1 FROM behavior_entries WHERE total_id=$1) AND EXISTS(SELECT 1 FROM behavior_suppressions WHERE academic_term_id=$2 AND source='lesson' AND side='merit') AND NOT EXISTS(SELECT 1 FROM attendance_term_summaries WHERE academic_term_id=$2) FROM behavior_daily_totals WHERE id=$1").bind(total).bind(term).fetch_one(&pool).await.unwrap());
    let before:Value=sqlx::query_scalar("SELECT jsonb_build_object('session',(SELECT to_jsonb(s) FROM attendance_sessions s WHERE id=$1),'total',(SELECT to_jsonb(t) FROM behavior_daily_totals t WHERE id=$2),'suppressions',(SELECT jsonb_agg(to_jsonb(s) ORDER BY id) FROM behavior_suppressions s))").bind(lesson).bind(total).fetch_one(&pool).await.unwrap();
    let error = school_migrations::run_tenant_migrations_detailed(&pool)
        .await
        .unwrap_err();
    assert_eq!(
        error.diagnostic.code,
        "ACADEMIC_CURRENT_BEHAVIOR_EFFECT_PROTECTED"
    );
    let after:Value=sqlx::query_scalar("SELECT jsonb_build_object('session',(SELECT to_jsonb(s) FROM attendance_sessions s WHERE id=$1),'total',(SELECT to_jsonb(t) FROM behavior_daily_totals t WHERE id=$2),'suppressions',(SELECT jsonb_agg(to_jsonb(s) ORDER BY id) FROM behavior_suppressions s))").bind(lesson).bind(total).fetch_one(&pool).await.unwrap();
    assert_eq!(before, after);
    assert!(sqlx::query_scalar::<_,bool>("SELECT to_regclass('academic_delivery_versions') IS NOT NULL AND to_regclass('academic_timetables') IS NULL").fetch_one(&pool).await.unwrap());
}
