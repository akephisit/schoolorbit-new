use super::*;

#[tokio::test]
async fn cutover_removes_omitted_group_facts_and_preserves_retained_and_groupless_facts() {
    let pool = predecessor("current_ops_group_cutover").await;
    let (offering,kept,term,year,student,subject,user):(Uuid,Uuid,Uuid,Uuid,Uuid,Uuid,Uuid)=sqlx::query_as("SELECT o.id,g.id,o.academic_term_id,o.academic_year_id,m.student_academic_year_id,d.subject_id,m.student_id FROM learning_offerings o JOIN learning_groups g ON g.learning_offering_id=o.id JOIN learning_group_students m ON m.learning_group_id=g.id JOIN course_offering_details d ON d.learning_offering_id=o.id JOIN academic_terms t ON t.id=o.academic_term_id WHERE t.status='active' LIMIT 1").fetch_one(&pool).await.unwrap();
    let omitted: Uuid = sqlx::query_scalar(
        "SELECT id FROM learning_groups WHERE learning_offering_id=$1 AND id<>$2 LIMIT 1",
    )
    .bind(offering)
    .bind(kept)
    .fetch_one(&pool)
    .await
    .unwrap();
    let other_student = Uuid::new_v4();
    let other_user = Uuid::new_v4();
    sqlx::query("INSERT INTO users SELECT (jsonb_populate_record(NULL::users,to_jsonb(u)||jsonb_build_object('id',$2::uuid,'national_id',NULL,'national_id_hash',NULL,'email',NULL,'username',$3::text))).* FROM users u WHERE id=$1").bind(user).bind(other_user).bind(format!("cutover-{other_user}")).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO student_academic_years SELECT (jsonb_populate_record(NULL::student_academic_years,to_jsonb(s)||jsonb_build_object('id',$2::uuid,'student_id',$3::uuid))).* FROM student_academic_years s WHERE id=$1").bind(student).bind(other_student).bind(other_user).execute(&pool).await.unwrap();
    let actor: Uuid = sqlx::query_scalar(
        "SELECT id FROM users WHERE user_type='staff' AND status='active' ORDER BY id LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let lock = Uuid::new_v4();
    sqlx::query("INSERT INTO academic_course_result_locks(id,subject_id,academic_term_id,academic_year_id,policy_version_id,policy_snapshot,roster_checksum,source_checksum,source_snapshot,locked_by) VALUES($1,$2,$3,$4,'06000000-0000-0000-0000-000000000001','{}',repeat('0',64),repeat('0',64),$5,$6)").bind(lock).bind(subject).bind(term).bind(year).bind(json!({"groups":[{"learningGroupId":kept,"learningOfferingId":offering},{"learningGroupId":omitted,"learningOfferingId":offering}]})).bind(actor).execute(&pool).await.unwrap();
    let kept_session = Uuid::new_v4();
    let omitted_session = Uuid::new_v4();
    let groupless = Uuid::new_v4();
    let kept_result = Uuid::new_v4();
    let omitted_result = Uuid::new_v4();
    let kept_score = Uuid::new_v4();
    let omitted_score = Uuid::new_v4();
    for (group, learner, learner_user, session, result, score) in [
        (kept, student, user, kept_session, kept_result, kept_score),
        (
            omitted,
            other_student,
            other_user,
            omitted_session,
            omitted_result,
            omitted_score,
        ),
    ] {
        sqlx::query("INSERT INTO attendance_sessions(id,academic_term_id,date,kind,source_key,title,teacher_ids,learning_group_id,offering_id,start_time,end_time,roster) VALUES($1,$2,'2025-08-25','lesson',$3,'Group cutover',ARRAY[]::uuid[],$4,$5,'08:00','09:00',jsonb_build_array(jsonb_build_object('studentId',$6::uuid,'studentAcademicYearId',$7::uuid)))").bind(session).bind(term).bind(session.to_string()).bind(group).bind(offering).bind(learner_user).bind(learner).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO attendance_records(session_id,student_id,student_academic_year_id,result,origin) VALUES($1,$2,$3,'present','teacher')").bind(session).bind(learner_user).bind(learner).execute(&pool).await.unwrap();
        let item = Uuid::new_v4();
        sqlx::query("INSERT INTO learning_group_score_items(id,learning_group_id,learning_offering_id,course_assessment_plan_id,assessment_phase_id,academic_term_id,academic_year_id,name,max_score) SELECT $1,$2,$3,p.id,phase.id,$4,$5,'Saved cutover score',100 FROM course_assessment_plans p JOIN course_assessment_phases phase ON phase.plan_id=p.id WHERE p.learning_offering_id=$3 ORDER BY phase.id LIMIT 1").bind(item).bind(group).bind(offering).bind(term).bind(year).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO learning_group_student_scores(id,learning_group_id,learning_offering_id,academic_term_id,academic_year_id,score_item_id,student_academic_year_id,score) VALUES($1,$2,$3,$4,$5,$6,$7,80)").bind(score).bind(group).bind(offering).bind(term).bind(year).bind(item).bind(learner).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO academic_course_results(id,learning_group_id,learning_offering_id,academic_term_id,academic_year_id,subject_id,course_result_lock_id,student_academic_year_id,calculated_score,calculated_grade,outcome,numeric_grade,selection_source) VALUES($1,$2,$3,$4,$5,$6,$7,$8,80,4,'numeric',4,'derived')").bind(result).bind(group).bind(offering).bind(term).bind(year).bind(subject).bind(lock).bind(learner).execute(&pool).await.unwrap();
    }
    sqlx::query("INSERT INTO attendance_sessions SELECT (jsonb_populate_record(NULL::attendance_sessions,to_jsonb(s)||jsonb_build_object('id',$2::uuid,'source_key',$2::text,'learning_group_id',NULL))).* FROM attendance_sessions s WHERE id=$1").bind(kept_session).bind(groupless).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO attendance_records(session_id,student_id,student_academic_year_id,result,origin) VALUES($1,$2,$3,'present','teacher')").bind(groupless).bind(user).bind(student).execute(&pool).await.unwrap();
    sqlx::raw_sql("ALTER TABLE academic_delivery_versions DISABLE TRIGGER academic_delivery_versions_immutable").execute(&pool).await.unwrap();
    sqlx::query("UPDATE academic_delivery_versions SET snapshot=jsonb_set(snapshot,'{offerings}',(SELECT jsonb_agg(CASE WHEN (o->>'id')::uuid=$2 THEN jsonb_set(o,'{groups}',(SELECT jsonb_agg(g) FROM jsonb_array_elements(o->'groups') g WHERE (g->>'id')::uuid<>$3)) ELSE o END) FROM jsonb_array_elements(snapshot->'offerings') o)) WHERE academic_term_id=$1").bind(term).bind(offering).bind(omitted).execute(&pool).await.unwrap();
    sqlx::raw_sql("ALTER TABLE academic_delivery_versions ENABLE TRIGGER academic_delivery_versions_immutable").execute(&pool).await.unwrap();
    let ids = vec![kept_session, groupless];
    let before:Value=sqlx::query_scalar("SELECT jsonb_build_object('sessions',(SELECT jsonb_agg(to_jsonb(s) ORDER BY id) FROM attendance_sessions s WHERE id=ANY($1)),'records',(SELECT jsonb_agg(to_jsonb(r) ORDER BY session_id,student_id) FROM attendance_records r WHERE session_id=ANY($1)),'score',(SELECT to_jsonb(s) FROM learning_group_student_scores s WHERE id=$2),'result',(SELECT to_jsonb(r) FROM academic_course_results r WHERE id=$3))").bind(&ids).bind(kept_score).bind(kept_result).fetch_one(&pool).await.unwrap();
    canonical(&pool).await;
    let after:Value=sqlx::query_scalar("SELECT jsonb_build_object('sessions',(SELECT jsonb_agg(to_jsonb(s) ORDER BY id) FROM attendance_sessions s WHERE id=ANY($1)),'records',(SELECT jsonb_agg(to_jsonb(r) ORDER BY session_id,student_id) FROM attendance_records r WHERE session_id=ANY($1)),'score',(SELECT to_jsonb(s) FROM learning_group_student_scores s WHERE id=$2),'result',(SELECT to_jsonb(r) FROM academic_course_results r WHERE id=$3))").bind(&ids).bind(kept_score).bind(kept_result).fetch_one(&pool).await.unwrap();
    assert_eq!(before, after);
    let gone:bool=sqlx::query_scalar("SELECT NOT EXISTS(SELECT 1 FROM learning_groups WHERE id=$1) AND NOT EXISTS(SELECT 1 FROM attendance_sessions WHERE id=$2) AND NOT EXISTS(SELECT 1 FROM attendance_records WHERE session_id=$2) AND NOT EXISTS(SELECT 1 FROM learning_group_student_scores WHERE id=$3) AND NOT EXISTS(SELECT 1 FROM learning_group_score_items WHERE learning_group_id=$1) AND NOT EXISTS(SELECT 1 FROM academic_course_results WHERE id=$4) AND EXISTS(SELECT 1 FROM learning_offerings WHERE id=$5)").bind(omitted).bind(omitted_session).bind(omitted_score).bind(omitted_result).bind(offering).fetch_one(&pool).await.unwrap();
    assert!(gone);
}
