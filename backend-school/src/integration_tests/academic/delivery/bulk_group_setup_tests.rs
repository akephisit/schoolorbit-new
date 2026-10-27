use super::*;
use school_academic_delivery::{models::*, services::{bulk_group_setup, roster_tracking, versions}};

struct BulkFixture {
    pool: PgPool,
    context: RuntimeContext,
    offering_id: Uuid,
    version_id: Uuid,
    group_ids: Vec<Uuid>,
}

async fn fixture(name: &str) -> BulkFixture {
    let pool = prepare_concurrent_delivery_runtime_fixture(name).await;
    let context = planning_runtime_context(&pool).await;
    let today: NaiveDate = sqlx::query_scalar("SELECT (now() AT TIME ZONE 'Asia/Bangkok')::date").fetch_one(&pool).await.unwrap();
    sqlx::query("UPDATE academic_years SET end_date=GREATEST(end_date,$2+180) WHERE id=$1").bind(context.year_id).bind(today).execute(&pool).await.unwrap();
    sqlx::query("UPDATE academic_terms SET planned_end_date=GREATEST(planned_end_date,$2+60) WHERE id=$1").bind(context.term_id).bind(today).execute(&pool).await.unwrap();
    let opening = create_runtime_change_set(&pool, context.teacher_id, context.term_id, 0, name).await;
    let CreateLearningOfferingRequest::Course(course) = course_request(&context) else { unreachable!() };
    let opening = change_sets::upsert_change_item(&pool, context.teacher_id, opening.id,
        UpsertAcademicTermChangeItemRequest::AddCourse { change_set_row_version: opening.row_version, offering: course }).await.unwrap();
    let offering_id = opening.items.iter().find_map(|item| match item {
        AcademicTermChangeItem::AddOffering { learning_offering_id, .. } => Some(*learning_offering_id), _ => None,
    }).unwrap();
    let mut group_ids: Vec<_> = groups::list(&pool, offering_id).await.unwrap().into_iter().map(|group| group.id).collect();
    for name in ["Bulk second", "Bulk unselected"] {
        let group = groups::create(&pool, context.teacher_id, offering_id, CreateLearningGroupRequest { name: name.into(), description: None, capacity: Some(40), preferred_room_ids: vec![] }).await.unwrap();
        let group = groups::replace_homerooms(&pool, context.teacher_id, group.id, ReplaceLearningGroupHomeroomsRequest { row_version: group.row_version, homeroom_ids: vec![context.homeroom_id] }).await.unwrap();
        group_ids.push(group.id);
    }
    assert!(group_ids.len() >= 3);
    BulkFixture { pool, context, offering_id, version_id: opening.target_delivery_version_id, group_ids }
}

async fn teacher(pool: &PgPool, label: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO users(id,email,username,password_hash,first_name,last_name,user_type,status) VALUES($1,$2,$3,'fixture-not-a-login',$4,'ทดสอบ','staff','active')")
        .bind(id).bind(format!("{id}@bulk.example.invalid")).bind(id.to_string()).bind(label).execute(pool).await.unwrap();
    id
}

async fn request(f: &BulkFixture, teacher_id: Uuid) -> BulkGroupSetupRequest {
    let version = versions::get_version(&f.pool, f.version_id).await.unwrap();
    let mut selected = Vec::new();
    for id in &f.group_ids[..2] {
        let group = groups::get(&f.pool, *id).await.unwrap();
        selected.push(BulkGroupSetupTarget { learning_group_id: *id, row_version: group.row_version });
    }
    BulkGroupSetupRequest { delivery_version_id: f.version_id, delivery_version_row_version: version.row_version, groups: selected,
        primary_teacher: Some(BulkGroupSetupPrimaryTeacher { teacher_id, replace_existing_primary: false }), roster_tracking: None }
}

async fn facts(f: &BulkFixture) -> String {
    sqlx::query_scalar("SELECT jsonb_build_object('groups',(SELECT jsonb_agg(to_jsonb(g) ORDER BY id) FROM learning_groups g WHERE learning_offering_id=$1),'teachers',(SELECT jsonb_agg(to_jsonb(t) ORDER BY t.id) FROM learning_group_teachers t JOIN learning_groups g ON g.id=t.learning_group_id WHERE g.learning_offering_id=$1),'tracking',(SELECT jsonb_agg(to_jsonb(t) ORDER BY t.learning_group_id) FROM learning_group_roster_tracking t JOIN learning_groups g ON g.id=t.learning_group_id WHERE g.learning_offering_id=$1),'versions',(SELECT jsonb_agg(to_jsonb(v) ORDER BY id) FROM academic_delivery_versions v WHERE academic_term_id=$2))::text")
        .bind(f.offering_id).bind(f.context.term_id).fetch_one(&f.pool).await.unwrap()
}

#[tokio::test]
async fn bulk_group_setup_preserves_exceptions_and_other_roles_and_never_builds_rosters() {
    let f = fixture("bulk_setup_preserve").await;
    let secondary = teacher(&f.pool, "ร่วมสอน").await;
    let assistant = teacher(&f.pool, "ผู้ช่วย").await;
    let selected_teacher = teacher(&f.pool, "ครูหลักชุดใหม่").await;
    let first = groups::get(&f.pool, f.group_ids[0]).await.unwrap();
    let first = groups::replace_teachers(&f.pool, f.context.teacher_id, first.id, ReplaceLearningGroupTeachersRequest {
        row_version: first.row_version, teachers: vec![
            TeacherAssignmentInput { teacher_id: f.context.teacher_id, role: LearningTeacherRole::Primary },
            TeacherAssignmentInput { teacher_id: secondary, role: LearningTeacherRole::Secondary },
            TeacherAssignmentInput { teacher_id: assistant, role: LearningTeacherRole::Assistant },
        ],
    }).await.unwrap();
    let preserved: Vec<_> = first.teacher_assignments.iter().filter(|a| a.role != LearningTeacherRole::Primary).map(|a| (a.id,a.teacher_id,a.role,a.row_version)).collect();
    let untouched = serde_json::to_value(groups::get(&f.pool, f.group_ids[2]).await.unwrap()).unwrap();
    let memberships: i64 = sqlx::query_scalar("SELECT count(*) FROM learning_group_students WHERE learning_group_id=ANY($1)").bind(&f.group_ids).fetch_one(&f.pool).await.unwrap();
    let mut input = request(&f, selected_teacher).await;
    let today = sqlx::query_scalar("SELECT (now() AT TIME ZONE 'Asia/Bangkok')::date").fetch_one(&f.pool).await.unwrap();
    input.roster_tracking = Some(BulkGroupSetupTrackingInput { mode: RosterTrackingMode::Homeroom, effective_from: Some(today) });
    let before = facts(&f).await;
    let preview = bulk_group_setup::preview(&f.pool, f.offering_id, input.clone()).await.unwrap();
    assert_eq!(facts(&f).await, before, "preview must be entirely read-only");
    assert_eq!(preview.changed_count, 2);
    assert_eq!(preview.blocked_count, 0);
    assert_eq!(preview.targets.iter().find(|t| t.group.id == first.id).unwrap().primary_teacher_action, BulkGroupSetupTeacherAction::PreserveExisting);
    let result = bulk_group_setup::apply(&f.pool, f.context.teacher_id, f.offering_id, input.clone()).await.unwrap();
    assert!(result.delivery_version_row_version > input.delivery_version_row_version);
    for group in &result.groups {
        assert_eq!(group.row_version, input.groups.iter().find(|t| t.learning_group_id == group.id).unwrap().row_version + 1);
        assert_eq!(group.status, LearningOfferingStatus::Draft);
        assert_eq!(group.roster_status, RosterStatus::Draft);
    }
    let first_after = result.groups.iter().find(|g| g.id == first.id).unwrap();
    assert!(first_after.teacher_assignments.iter().any(|a| a.teacher_id == f.context.teacher_id && a.role == LearningTeacherRole::Primary));
    assert_eq!(first_after.teacher_assignments.iter().filter(|a| a.role != LearningTeacherRole::Primary).map(|a| (a.id,a.teacher_id,a.role,a.row_version)).collect::<Vec<_>>(), preserved);
    assert!(result.groups.iter().find(|g| g.id == f.group_ids[1]).unwrap().teacher_assignments.iter().any(|a| a.teacher_id == selected_teacher && a.role == LearningTeacherRole::Primary));
    assert_eq!(serde_json::to_value(groups::get(&f.pool,f.group_ids[2]).await.unwrap()).unwrap(), untouched);
    assert_eq!(sqlx::query_scalar::<i64>("SELECT count(*) FROM learning_group_students WHERE learning_group_id=ANY($1)").bind(&f.group_ids).fetch_one(&f.pool).await.unwrap(), memberships);
    assert!(result.roster_tracking.iter().all(|config| config.tracking.mode == RosterTrackingMode::Homeroom));
    let mut replace = request(&f, selected_teacher).await;
    replace.primary_teacher.as_mut().unwrap().replace_existing_primary = true;
    let replacement = bulk_group_setup::apply(&f.pool, f.context.teacher_id, f.offering_id, replace).await.unwrap();
    let first_after = replacement.groups.iter().find(|g| g.id == first.id).unwrap();
    assert_eq!(first_after.teacher_assignments.iter().filter(|a| a.role != LearningTeacherRole::Primary).map(|a| (a.id,a.teacher_id,a.role,a.row_version)).collect::<Vec<_>>(), preserved);
    assert!(first_after.teacher_assignments.iter().any(|a| a.teacher_id == selected_teacher && a.role == LearningTeacherRole::Primary));
    let no_op = request(&f, selected_teacher).await;
    assert!(matches!(bulk_group_setup::apply(&f.pool,f.context.teacher_id,f.offering_id,no_op).await,Err(AppError::Conflict(_))));
}

#[tokio::test]
async fn bulk_group_setup_rejects_stale_scope_and_any_blocked_target_atomically() {
    let f = fixture("bulk_setup_guards").await;
    let selected_teacher = teacher(&f.pool, "ครูใหม่").await;
    let fresh = request(&f, selected_teacher).await;
    for mut invalid in [fresh.clone(),fresh.clone(),fresh.clone(),fresh.clone(),fresh.clone()] .into_iter().enumerate() {
        match invalid.0 {
            0 => invalid.1.delivery_version_row_version += 1,
            1 => invalid.1.groups[1].row_version += 1,
            2 => invalid.1.groups[1].learning_group_id = Uuid::new_v4(),
            3 => invalid.1.groups.push(invalid.1.groups[0].clone()),
            _ => invalid.1.groups.clear(),
        }
        let before = facts(&f).await;
        assert!(bulk_group_setup::apply(&f.pool,f.context.teacher_id,f.offering_id,invalid.1).await.is_err());
        assert_eq!(facts(&f).await,before);
    }
    let first = groups::get(&f.pool,f.group_ids[0]).await.unwrap();
    groups::replace_teachers(&f.pool,f.context.teacher_id,first.id,ReplaceLearningGroupTeachersRequest { row_version:first.row_version,teachers:vec![TeacherAssignmentInput {teacher_id:selected_teacher,role:LearningTeacherRole::Secondary}] }).await.unwrap();
    let input = request(&f,selected_teacher).await;
    let before = facts(&f).await;
    let preview = bulk_group_setup::preview(&f.pool,f.offering_id,input.clone()).await.unwrap();
    assert_eq!(preview.blocked_count,1);
    assert_eq!(preview.changed_count,1);
    assert!(matches!(bulk_group_setup::apply(&f.pool,f.context.teacher_id,f.offering_id,input).await,Err(AppError::Conflict(_))));
    assert_eq!(facts(&f).await,before);
    let mut bad_tracking = request(&f,f.context.teacher_id).await;
    bad_tracking.roster_tracking=Some(BulkGroupSetupTrackingInput {mode:RosterTrackingMode::Homeroom,effective_from:Some(NaiveDate::from_ymd_opt(2000,1,1).unwrap())});
    assert!(bulk_group_setup::apply(&f.pool,f.context.teacher_id,f.offering_id,bad_tracking).await.is_err());
    assert_eq!(facts(&f).await,before);
    sqlx::query("UPDATE users SET status='inactive' WHERE id=$1").bind(selected_teacher).execute(&f.pool).await.unwrap();
    assert!(matches!(bulk_group_setup::apply(&f.pool,f.context.teacher_id,f.offering_id,request(&f,selected_teacher).await).await,Err(AppError::ValidationError(_))));
}

#[tokio::test]
async fn bulk_group_setup_concurrent_writers_save_exactly_one_atomic_batch() {
    let f = fixture("bulk_setup_concurrent").await;
    let first = teacher(&f.pool,"ครูหนึ่ง").await;
    let second = teacher(&f.pool,"ครูสอง").await;
    let first_input = request(&f,first).await;
    let second_input = request(&f,second).await;
    let (a,b) = tokio::join!(bulk_group_setup::apply(&f.pool,f.context.teacher_id,f.offering_id,first_input),bulk_group_setup::apply(&f.pool,f.context.teacher_id,f.offering_id,second_input));
    assert_eq!(usize::from(a.is_ok())+usize::from(b.is_ok()),1);
    assert!(matches!(if a.is_err() {a} else {b},Err(AppError::Conflict(_))));
    let teachers: Vec<Uuid> = sqlx::query_scalar("SELECT teacher_id FROM learning_group_teachers WHERE learning_group_id=ANY($1) AND role='primary' ORDER BY learning_group_id").bind(&f.group_ids[..2]).fetch_all(&f.pool).await.unwrap();
    assert_eq!(teachers.len(),2);
    assert_eq!(teachers[0],teachers[1]);
}
