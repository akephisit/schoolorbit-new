use super::*;

struct Legacy {
    pool: PgPool,
    blocks: Vec<Uuid>,
    groups: Vec<Uuid>,
    kept_group: Uuid,
    sync: Uuid,
    sessions: Vec<Uuid>,
    observation: Uuid,
    legacy_observation: Uuid,
    policy: Value,
    facts: Value,
    audits: Vec<Uuid>,
    retained_audits: Value,
}
async fn legacy(name: &str, linked_inactive: bool) -> Legacy {
    let pool = predecessor(name).await;
    let (block,group,delivery_group,offering,term,year,observation):(Uuid,Uuid,Uuid,Uuid,Uuid,Uuid,Uuid)=sqlx::query_as("SELECT b.id,g.id,g.learning_group_id,g.learning_offering_id,b.academic_term_id,b.academic_year_id,o.id FROM academic_timetable_blocks b JOIN academic_timetable_block_groups g ON g.block_id=b.id JOIN supervision_observations o ON o.timetable_block_group_id=g.id WHERE b.is_active AND g.is_active AND b.timetable_version_id=academic_timetable_applicable_publication(b.academic_term_id,(now() AT TIME ZONE 'Asia/Bangkok')::date) LIMIT 1").fetch_one(&pool).await.unwrap();
    // Construct saved predecessor content as it existed before immutable publication.
    // No current guard is disabled, and every legacy guard is restored before cutover.
    for table in [
        "academic_timetable_blocks",
        "academic_timetable_block_groups",
        "academic_timetable_block_group_instructors",
        "academic_timetable_block_homerooms",
        "academic_timetable_block_teachers",
        "academic_timetable_block_group_sync",
    ] {
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!("ALTER TABLE {table} DISABLE TRIGGER {table}_version_immutable; ALTER TABLE {table} DISABLE TRIGGER {table}_content_revision"))).execute(&pool).await.unwrap();
    }
    let inactive_block = Uuid::new_v4();
    let host = Uuid::new_v4();
    let inactive_group = Uuid::new_v4();
    let child = Uuid::new_v4();
    for (id, active) in [(inactive_block, false), (host, true)] {
        sqlx::query("INSERT INTO academic_timetable_blocks SELECT (jsonb_populate_record(NULL::academic_timetable_blocks,to_jsonb(b)||jsonb_build_object('id',$2::uuid,'is_active',$3::bool,'series_id',$4::uuid))).* FROM academic_timetable_blocks b WHERE id=$1").bind(block).bind(id).bind(active).bind(Uuid::new_v4()).execute(&pool).await.unwrap();
    }
    for (id, parent, active) in [(inactive_group, host, false), (child, inactive_block, true)] {
        sqlx::query("INSERT INTO academic_timetable_block_groups SELECT (jsonb_populate_record(NULL::academic_timetable_block_groups,to_jsonb(g)||jsonb_build_object('id',$2::uuid,'block_id',$3::uuid,'is_active',$4::bool))).* FROM academic_timetable_block_groups g WHERE id=$1").bind(group).bind(id).bind(parent).bind(active).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO academic_timetable_block_group_instructors SELECT (jsonb_populate_record(NULL::academic_timetable_block_group_instructors,to_jsonb(i)||jsonb_build_object('id',gen_random_uuid(),'block_group_id',$2::uuid))).* FROM academic_timetable_block_group_instructors i WHERE block_group_id=$1").bind(group).bind(id).execute(&pool).await.unwrap();
    }
    let sync = Uuid::new_v4();
    sqlx::query("INSERT INTO academic_timetable_block_group_sync(id,block_id,learning_group_id,learning_offering_id,academic_term_id,academic_year_id,status,linked_block_group_id) VALUES($1,$2,$3,$4,$5,$6,$7,$8)").bind(sync).bind(host).bind(delivery_group).bind(offering).bind(term).bind(year).bind(if linked_inactive {"LINKED"}else{"EXCLUDED"}).bind(linked_inactive.then_some(inactive_group)).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO academic_timetable_block_group_sync(block_id,learning_group_id,learning_offering_id,academic_term_id,academic_year_id,status,linked_block_group_id) VALUES($1,$2,$3,$4,$5,'LINKED',$6)").bind(inactive_block).bind(delivery_group).bind(offering).bind(term).bind(year).bind(child).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO academic_timetable_block_homerooms(block_id,homeroom_id,academic_term_id,academic_year_id,target_kind,is_active) SELECT $1,homeroom_ids[1],academic_term_id,academic_year_id,'reservation',false FROM academic_timetable_block_groups WHERE id=$2").bind(host).bind(group).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO academic_timetable_block_teachers(block_id,teacher_id,academic_term_id,academic_year_id,is_active) SELECT $1,teacher_id,academic_term_id,academic_year_id,false FROM learning_group_teachers WHERE learning_group_id=$2 LIMIT 1").bind(host).bind(delivery_group).execute(&pool).await.unwrap();
    for table in [
        "academic_timetable_blocks",
        "academic_timetable_block_groups",
        "academic_timetable_block_group_instructors",
        "academic_timetable_block_homerooms",
        "academic_timetable_block_teachers",
        "academic_timetable_block_group_sync",
    ] {
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!("ALTER TABLE {table} ENABLE TRIGGER {table}_version_immutable; ALTER TABLE {table} ENABLE TRIGGER {table}_content_revision"))).execute(&pool).await.unwrap();
    }
    sqlx::query("UPDATE supervision_observations SET timetable_block_group_id=$2,lesson_snapshot=$3 WHERE id=$1").bind(observation).bind(child).bind(json!({"savedSubject":"Original lesson","savedPeriod":"08:00-09:00"})).execute(&pool).await.unwrap();
    let legacy_observation = Uuid::new_v4();
    sqlx::query("INSERT INTO supervision_observations SELECT (jsonb_populate_record(NULL::supervision_observations,to_jsonb(o)||jsonb_build_object('id',$2::uuid,'timetable_block_group_id',$3::uuid,'lesson_snapshot','{}'::jsonb))).* FROM supervision_observations o WHERE id=$1").bind(observation).bind(legacy_observation).bind(inactive_group).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO supervision_evaluators(observation_id,evaluator_user_id,status,submitted_at) SELECT id,observed_user_id,'submitted',now() FROM supervision_observations WHERE id=$1 ON CONFLICT(observation_id,evaluator_user_id) DO UPDATE SET status='submitted',submitted_at=EXCLUDED.submitted_at").bind(observation).execute(&pool).await.unwrap();
    let (student,student_year):(Uuid,Uuid)=sqlx::query_as("SELECT student_id,student_academic_year_id FROM learning_group_students WHERE learning_group_id=$1 LIMIT 1").bind(delivery_group).fetch_one(&pool).await.unwrap();
    let sessions = vec![Uuid::new_v4(), Uuid::new_v4()];
    for (session, placed) in [(sessions[0], child), (sessions[1], group)] {
        sqlx::query("INSERT INTO attendance_sessions(id,academic_term_id,date,kind,source_key,title,teacher_ids,learning_group_id,offering_id,start_time,end_time,roster) VALUES($1,$2,'2025-08-25','lesson',$3,'Saved fact',ARRAY[]::uuid[],$4,$5,'08:00','09:00',jsonb_build_array(jsonb_build_object('studentId',$6::uuid,'studentAcademicYearId',$7::uuid)))").bind(session).bind(term).bind(format!("legacy-saved:{placed}")).bind(delivery_group).bind(offering).bind(student).bind(student_year).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO attendance_records(session_id,student_id,student_academic_year_id,result,origin) VALUES($1,$2,$3,'present','teacher')").bind(session).bind(student).bind(student_year).execute(&pool).await.unwrap();
    }
    sqlx::query("UPDATE academic_opening_policy SET require_published_offerings=true,require_published_timetable=false").execute(&pool).await.unwrap();
    let policy:Value=sqlx::query_scalar("SELECT to_jsonb(p)-'require_published_offerings'-'require_published_timetable'||jsonb_build_object('require_offerings',require_published_offerings,'require_timetable',require_published_timetable) FROM academic_opening_policy p").fetch_one(&pool).await.unwrap();
    let mut audits = Vec::new();
    let mut retained_ids = Vec::new();
    for (entity, event, remove) in [
        ("learning_offering", "unknown.old.operation", true),
        ("learning_group", "unknown.old.operation", true),
        ("learning_group_student", "unknown.old.operation", true),
        (
            "academic_timetable_block_group",
            "academic_teacher_handoff.applied",
            true,
        ),
        ("academic_delivery_version", "old.version", true),
        ("subject_version", "academic_teacher_handoff.applied", false),
        ("curriculum_revision", "curriculum.published", false),
        ("academic_term", "academic_term.activated", false),
        ("academic_course_result", "result.locked", false),
    ] {
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO academic_audit_events(id,entity_type,event_code,entity_id,academic_year_id,academic_term_id,payload) VALUES($1,$2,$3,$4,$5,$6,$7)").bind(id).bind(entity).bind(event).bind(offering).bind(year).bind(term).bind(json!({"fixture":"Preserved independent history"})).execute(&pool).await.unwrap();
        if remove {
            audits.push(id)
        } else {
            retained_ids.push(id)
        }
    }
    let retained_audits: Value = sqlx::query_scalar(
        "SELECT jsonb_agg(to_jsonb(a) ORDER BY id) FROM academic_audit_events a WHERE id=ANY($1)",
    )
    .bind(&retained_ids)
    .fetch_one(&pool)
    .await
    .unwrap();
    let facts:Value=sqlx::query_scalar("SELECT jsonb_build_object('sessions',(SELECT jsonb_agg(to_jsonb(s) ORDER BY id) FROM attendance_sessions s WHERE id=ANY($1)),'records',(SELECT jsonb_agg(to_jsonb(r) ORDER BY session_id,student_id) FROM attendance_records r WHERE session_id=ANY($1)),'memberships',(SELECT jsonb_agg(to_jsonb(m) ORDER BY id) FROM learning_group_students m WHERE learning_group_id=$2),'observation',(SELECT to_jsonb(o)-'timetable_block_group_id'-'updated_at' FROM supervision_observations o WHERE id=$3),'evaluators',(SELECT jsonb_agg(to_jsonb(e) ORDER BY id) FROM supervision_evaluators e WHERE observation_id=$3))").bind(&sessions).bind(delivery_group).bind(observation).fetch_one(&pool).await.unwrap();
    Legacy {
        pool,
        blocks: vec![inactive_block, host],
        groups: vec![inactive_group, child],
        kept_group: delivery_group,
        sync,
        sessions,
        observation,
        legacy_observation,
        policy,
        facts,
        audits,
        retained_audits,
    }
}

#[tokio::test]
async fn removes_inactive_placements_without_erasing_saved_facts_or_independent_history() {
    let f = legacy("current_ops_removed_placements", false).await;
    canonical(&f.pool).await;
    let actual:Value=sqlx::query_scalar("SELECT jsonb_build_object('sessions',(SELECT jsonb_agg(to_jsonb(s) ORDER BY id) FROM attendance_sessions s WHERE id=ANY($1)),'records',(SELECT jsonb_agg(to_jsonb(r) ORDER BY session_id,student_id) FROM attendance_records r WHERE session_id=ANY($1)),'memberships',(SELECT jsonb_agg(to_jsonb(m) ORDER BY id) FROM learning_group_students m WHERE learning_group_id=$2),'observation',(SELECT to_jsonb(o)-'timetable_block_group_id'-'updated_at' FROM supervision_observations o WHERE id=$3),'evaluators',(SELECT jsonb_agg(to_jsonb(e) ORDER BY id) FROM supervision_evaluators e WHERE observation_id=$3))").bind(&f.sessions).bind(f.kept_group).bind(f.observation).fetch_one(&f.pool).await.unwrap();
    assert_eq!(actual, f.facts);
    let clean:bool=sqlx::query_scalar("SELECT NOT EXISTS(SELECT 1 FROM academic_timetable_blocks WHERE NOT is_active) AND NOT EXISTS(SELECT 1 FROM academic_timetable_block_groups WHERE NOT is_active) AND NOT EXISTS(SELECT 1 FROM academic_timetable_block_homerooms WHERE NOT is_active) AND NOT EXISTS(SELECT 1 FROM academic_timetable_block_teachers WHERE NOT is_active) AND NOT EXISTS(SELECT 1 FROM academic_timetable_blocks WHERE id=$1) AND EXISTS(SELECT 1 FROM academic_timetable_blocks WHERE id=$2) AND NOT EXISTS(SELECT 1 FROM academic_timetable_block_groups WHERE id=ANY($3)) AND NOT EXISTS(SELECT 1 FROM academic_timetable_block_group_instructors WHERE block_group_id=ANY($3)) AND EXISTS(SELECT 1 FROM academic_timetable_block_group_sync WHERE id=$4 AND status='EXCLUDED' AND linked_block_group_id IS NULL)").bind(f.blocks[0]).bind(f.blocks[1]).bind(&f.groups).bind(f.sync).fetch_one(&f.pool).await.unwrap();
    assert!(clean);
    let detached:bool=sqlx::query_scalar("SELECT count(*)=2 FROM supervision_observations WHERE id=ANY($1) AND timetable_block_group_id IS NULL AND learning_group_id=$2 AND lesson_snapshot<>'{}'::jsonb").bind(vec![f.observation,f.legacy_observation]).bind(f.kept_group).fetch_one(&f.pool).await.unwrap();
    assert!(detached);
    assert_eq!(
        sqlx::query_scalar::<_, Value>("SELECT to_jsonb(p) FROM academic_opening_policy p")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        f.policy
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM academic_audit_events WHERE id=ANY($1)")
            .bind(&f.audits)
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        0
    );
    let retained:Value=sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(a) ORDER BY id) FROM academic_audit_events a WHERE id IN(SELECT (value->>'id')::uuid FROM jsonb_array_elements($1))").bind(&f.retained_audits).fetch_one(&f.pool).await.unwrap();
    assert_eq!(retained, f.retained_audits);
    let before = current_facts(&f.pool).await;
    canonical(&f.pool).await;
    assert_eq!(before, current_facts(&f.pool).await);
}

#[tokio::test]
async fn linked_sync_to_removed_placement_refuses_atomically() {
    let f = legacy("current_ops_inactive_link_refusal", true).await;
    let error = school_migrations::run_tenant_migrations_detailed(&f.pool)
        .await
        .unwrap_err();
    assert_eq!(
        error.diagnostic.code,
        "ACADEMIC_CURRENT_INACTIVE_SYNC_UNMAPPABLE"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT max(version) FROM _sqlx_migrations WHERE success")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        156
    );
    assert!(sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM academic_timetable_block_groups WHERE id=$1 AND NOT is_active)").bind(f.groups[0]).fetch_one(&f.pool).await.unwrap());
}

#[tokio::test]
async fn unplaced_import_copies_are_removed_while_placed_exam_and_paper_sources_remain() {
    let pool = predecessor("current_ops_unplaced_exam").await;
    let exclusive_evidence:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_constraint WHERE conrelid='attendance_scan_events'::regclass AND contype='u' AND pg_get_constraintdef(oid)='UNIQUE (evidence_file_id)')").fetch_one(&pool).await.unwrap();
    assert!(
        exclusive_evidence,
        "the real155 predecessor prohibits duplicate evidence-file scan references"
    );
    let item:Uuid=sqlx::query_scalar("SELECT item.id FROM academic_exam_schedule_items item WHERE EXISTS(SELECT 1 FROM academic_exam_sessions s WHERE s.exam_schedule_item_id=item.id) LIMIT 1").fetch_one(&pool).await.unwrap();
    let round = Uuid::new_v4();
    let unplaced = Uuid::new_v4();
    sqlx::query("INSERT INTO academic_exam_rounds SELECT (jsonb_populate_record(NULL::academic_exam_rounds,to_jsonb(r)||jsonb_build_object('id',$2::uuid,'name','Unplaced current projection fixture'))).* FROM academic_exam_rounds r JOIN academic_exam_schedule_items item ON item.exam_round_id=r.id WHERE item.id=$1").bind(item).bind(round).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO academic_exam_schedule_items SELECT (jsonb_populate_record(NULL::academic_exam_schedule_items,to_jsonb(i)||jsonb_build_object('id',$2::uuid,'exam_round_id',$3::uuid))).* FROM academic_exam_schedule_items i WHERE id=$1").bind(item).bind(unplaced).bind(round).execute(&pool).await.unwrap();
    let before:Value=sqlx::query_scalar("SELECT jsonb_build_object('item',(SELECT to_jsonb(i) FROM academic_exam_schedule_items i WHERE id=$1),'session',(SELECT jsonb_agg(to_jsonb(s) ORDER BY id) FROM academic_exam_sessions s WHERE exam_schedule_item_id=$1),'phases',(SELECT jsonb_agg(to_jsonb(p) ORDER BY p.id) FROM course_assessment_phases p JOIN academic_exam_schedule_items i ON i.course_assessment_plan_id=p.plan_id WHERE i.id=$1),'seats',(SELECT jsonb_agg(to_jsonb(seat) ORDER BY seat.id) FROM academic_exam_seat_assignments seat))").bind(item).fetch_one(&pool).await.unwrap();
    canonical(&pool).await;
    let after:Value=sqlx::query_scalar("SELECT jsonb_build_object('item',(SELECT to_jsonb(i) FROM academic_exam_schedule_items i WHERE id=$1),'session',(SELECT jsonb_agg(to_jsonb(s) ORDER BY id) FROM academic_exam_sessions s WHERE exam_schedule_item_id=$1),'phases',(SELECT jsonb_agg(to_jsonb(p) ORDER BY p.id) FROM course_assessment_phases p JOIN academic_exam_schedule_items i ON i.course_assessment_plan_id=p.plan_id WHERE i.id=$1),'seats',(SELECT jsonb_agg(to_jsonb(seat) ORDER BY seat.id) FROM academic_exam_seat_assignments seat))").bind(item).fetch_one(&pool).await.unwrap();
    assert_eq!(
        before, after,
        "placed identities, actual seats and paper receipt phase sources stay unchanged"
    );
    assert!(sqlx::query_scalar::<_,bool>("SELECT NOT EXISTS(SELECT 1 FROM academic_exam_schedule_items WHERE id=$1) AND EXISTS(SELECT 1 FROM academic_exam_rounds WHERE id=$2) AND NOT EXISTS(SELECT 1 FROM academic_exam_schedule_items i WHERE NOT EXISTS(SELECT 1 FROM academic_exam_sessions s WHERE s.exam_schedule_item_id=i.id))").bind(unplaced).bind(round).fetch_one(&pool).await.unwrap());
}
