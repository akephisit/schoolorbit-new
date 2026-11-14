use super::*;
use school_behavior::{
    models::{BehaviorAttendanceDay, BehaviorAttendanceFact, BehaviorSource},
    services::integration,
};

#[tokio::test]
async fn selected_behavior_sources_cannot_attest_a_different_saved_learner_or_context() {
    for corruption in [
        "entry_date",
        "entry_key",
        "entry_student",
        "entry_term",
        "entry_kind",
        "entry_generation",
        "entry_session",
        "snapshot_date",
        "snapshot_student",
        "snapshot_term",
        "snapshot_kind",
        "snapshot_generation",
    ] {
        let pool = predecessor(&format!("current_ops_source_{corruption}")).await;
        let (offering,group,term,student,student_year,grade):(Uuid,Uuid,Uuid,Uuid,Uuid,Uuid)=sqlx::query_as("SELECT o.id,g.id,o.academic_term_id,m.student_id,m.student_academic_year_id,y.grade_level_id FROM learning_offerings o JOIN learning_groups g ON g.learning_offering_id=o.id JOIN learning_group_students m ON m.learning_group_id=g.id JOIN student_academic_years y ON y.id=m.student_academic_year_id WHERE NOT EXISTS(SELECT 1 FROM academic_delivery_versions v CROSS JOIN LATERAL jsonb_array_elements(v.snapshot->'offerings') graph WHERE v.id=academic_delivery_applicable_publication(o.academic_term_id,(now() AT TIME ZONE 'Asia/Bangkok')::date) AND (graph->>'id')::uuid=o.id) LIMIT 1").fetch_one(&pool).await.unwrap();
        let other_student = Uuid::new_v4();
        sqlx::query("INSERT INTO users(id,username,password_hash,first_name,last_name,user_type,status) VALUES($1,$2,'synthetic','Other saved','Learner','student','active')").bind(other_student).bind(format!("other-{other_student}")).execute(&pool).await.unwrap();
        let other_term: Uuid =
            sqlx::query_scalar("SELECT id FROM academic_terms WHERE id<>$1 ORDER BY id LIMIT 1")
                .bind(term)
                .fetch_one(&pool)
                .await
                .unwrap();
        let actor: Uuid = sqlx::query_scalar(
            "SELECT id FROM users WHERE user_type='staff' AND status='active' LIMIT 1",
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
        sqlx::query("INSERT INTO attendance_sessions(id,academic_term_id,date,kind,source_key,title,teacher_ids,learning_group_id,offering_id,start_time,end_time,count_override,roster) VALUES($1,$2,$3,'lesson',$1::text,'Independent saved source',ARRAY[]::uuid[],$4,$5,'08:00','09:00',true,jsonb_build_array(jsonb_build_object('studentId',$6::uuid,'studentAcademicYearId',$7::uuid,'classNumber',NULL)))").bind(lesson).bind(term).bind(date).bind(group).bind(offering).bind(student).bind(student_year).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO attendance_records(session_id,student_id,student_academic_year_id,result,origin) VALUES($1,$2,$3,'present','teacher')").bind(lesson).bind(student).bind(student_year).execute(&pool).await.unwrap();
        sqlx::query("UPDATE behavior_settings SET enabled=true WHERE singleton")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO behavior_attendance_rules(kind,status,effective_from,action,points,created_by) VALUES('lesson','present',$1,'merit',2,$2)").bind(date).bind(actor).execute(&pool).await.unwrap();
        let generation: Uuid =
            sqlx::query_scalar("SELECT generation FROM attendance_module_state WHERE singleton")
                .fetch_one(&pool)
                .await
                .unwrap();
        let mut tx = pool.begin().await.unwrap();
        integration::replace_attendance_day(
            &mut tx,
            BehaviorAttendanceDay {
                generation,
                academic_term_id: term,
                date,
                revision: 1,
                facts: vec![BehaviorAttendanceFact {
                    source_key: lesson.to_string(),
                    source_session_id: Some(lesson),
                    kind: BehaviorSource::Lesson,
                    student_id: student,
                    status: "present".into(),
                    counted: true,
                    grade_level_id: grade,
                    homeroom_id: None,
                    overrides: None,
                }],
            },
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
        match corruption {
            "entry_date" => {
                sqlx::query("UPDATE behavior_daily_totals SET date=date-1 WHERE source='lesson' AND student_id=$1").bind(student).execute(&pool).await.unwrap();
            }
            "entry_student" => {
                sqlx::query("UPDATE behavior_daily_totals SET student_id=$2 WHERE source='lesson' AND student_id=$1").bind(student).bind(other_student).execute(&pool).await.unwrap();
            }
            "entry_term" => {
                sqlx::query("UPDATE behavior_daily_totals SET academic_term_id=$2 WHERE source='lesson' AND student_id=$1").bind(student).bind(other_term).execute(&pool).await.unwrap();
            }
            "entry_kind" => {
                sqlx::query("UPDATE behavior_daily_totals SET source='manual' WHERE source='lesson' AND student_id=$1").bind(student).execute(&pool).await.unwrap();
            }
            "entry_key" => {
                sqlx::query("UPDATE behavior_entries SET source_key=$2 WHERE source_session_id=$1")
                    .bind(lesson)
                    .bind(Uuid::new_v4().to_string())
                    .execute(&pool)
                    .await
                    .unwrap();
            }
            "entry_generation" => {
                sqlx::query(
                    "UPDATE behavior_entries SET source_generation=$2 WHERE source_session_id=$1",
                )
                .bind(lesson)
                .bind(Uuid::new_v4())
                .execute(&pool)
                .await
                .unwrap();
            }
            "entry_session" => {
                sqlx::query("UPDATE behavior_entries SET source_session_id=$2 WHERE source_key=$1")
                    .bind(lesson.to_string())
                    .bind(Uuid::new_v4())
                    .execute(&pool)
                    .await
                    .unwrap();
            }
            "snapshot_date" => {
                sqlx::query(
                    "UPDATE behavior_source_effect_snapshots SET date=date-1 WHERE source_key=$1",
                )
                .bind(lesson.to_string())
                .execute(&pool)
                .await
                .unwrap();
            }
            "snapshot_student" => {
                sqlx::query(
                    "UPDATE behavior_source_effect_snapshots SET student_id=$2 WHERE source_key=$1",
                )
                .bind(lesson.to_string())
                .bind(other_student)
                .execute(&pool)
                .await
                .unwrap();
            }
            "snapshot_term" => {
                sqlx::query("UPDATE behavior_source_effect_snapshots SET academic_term_id=$2 WHERE source_key=$1").bind(lesson.to_string()).bind(other_term).execute(&pool).await.unwrap();
            }
            "snapshot_kind" => {
                sqlx::query("UPDATE behavior_source_effect_snapshots SET kind='special' WHERE source_key=$1").bind(lesson.to_string()).execute(&pool).await.unwrap();
            }
            "snapshot_generation" => {
                sqlx::query(
                    "UPDATE behavior_source_effect_snapshots SET generation=$2 WHERE source_key=$1",
                )
                .bind(lesson.to_string())
                .bind(Uuid::new_v4())
                .execute(&pool)
                .await
                .unwrap();
            }
            _ => unreachable!(),
        }
        let before:Value=sqlx::query_scalar("SELECT jsonb_build_object('sessions',(SELECT jsonb_agg(to_jsonb(s) ORDER BY id) FROM attendance_sessions s),'records',(SELECT jsonb_agg(to_jsonb(r) ORDER BY session_id,student_id) FROM attendance_records r),'totals',(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM behavior_daily_totals t),'entries',(SELECT jsonb_agg(to_jsonb(e) ORDER BY id) FROM behavior_entries e),'snapshots',(SELECT jsonb_agg(to_jsonb(e) ORDER BY source_key,student_id,date) FROM behavior_source_effect_snapshots e))").fetch_one(&pool).await.unwrap();
        let error = school_migrations::run_tenant_migrations_detailed(&pool)
            .await
            .unwrap_err();
        assert_eq!(
            error.diagnostic.code, "ACADEMIC_CURRENT_BEHAVIOR_EFFECT_PROTECTED",
            "{corruption}"
        );
        let after:Value=sqlx::query_scalar("SELECT jsonb_build_object('sessions',(SELECT jsonb_agg(to_jsonb(s) ORDER BY id) FROM attendance_sessions s),'records',(SELECT jsonb_agg(to_jsonb(r) ORDER BY session_id,student_id) FROM attendance_records r),'totals',(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM behavior_daily_totals t),'entries',(SELECT jsonb_agg(to_jsonb(e) ORDER BY id) FROM behavior_entries e),'snapshots',(SELECT jsonb_agg(to_jsonb(e) ORDER BY source_key,student_id,date) FROM behavior_source_effect_snapshots e))").fetch_one(&pool).await.unwrap();
        assert_eq!(
            before, after,
            "{corruption}: source and points must survive atomic refusal"
        );
    }
}
