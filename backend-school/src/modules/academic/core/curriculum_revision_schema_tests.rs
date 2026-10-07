use super::*;
use sqlx::types::Json;

async fn predecessor(name: &str) -> sqlx::PgPool {
    let pool = create_named_test_pool(name).await;
    apply_migrations_through(&pool, 40).await.unwrap();
    seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
        .await
        .unwrap();
    crate::modules::academic::cutover_test_support::apply_phase_b_runtime_migrations(&pool)
        .await
        .unwrap();
    apply_migrations_through(&pool, 90).await.unwrap();
    pool
}

async fn seed_hierarchy(pool: &sqlx::PgPool) -> Vec<Uuid> {
    let year: Uuid = sqlx::query_scalar("SELECT id FROM academic_years ORDER BY year LIMIT 1")
        .fetch_one(pool)
        .await
        .unwrap();
    let mut programs = Vec::new();
    for (code, name, junior) in [
        ("J-SCI-MATH", "วิทยาศาสตร์-คณิตศาสตร์ ม.ต้น", true),
        ("J-ART-LANG", "ศิลป์-ภาษา ม.ต้น", true),
        ("J-CAR-TECH", "การงานอาชีพ-เทคโนโลยี ม.ต้น", true),
        ("S-SCI-MATH", "วิทยาศาสตร์-คณิตศาสตร์ ม.ปลาย", false),
        ("S-ART-LANG", "ศิลป์-ภาษา ม.ปลาย", false),
    ] {
        let grades: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM grade_levels WHERE level_type='secondary' AND year BETWEEN $1 AND $2 ORDER BY year")
            .bind(if junior {1} else {4}).bind(if junior {3} else {6}).fetch_all(pool).await.unwrap();
        assert_eq!(grades.len(), 3);
        let curriculum: Uuid = sqlx::query_scalar("INSERT INTO curricula(code,identity_key,name_th,grade_level_ids) VALUES($1,lower($1),$2,$3) RETURNING id")
            .bind(code).bind(name).bind(Json(grades)).fetch_one(pool).await.unwrap();
        let version: Uuid = sqlx::query_scalar("INSERT INTO curriculum_versions(curriculum_id,version_name,start_academic_year_id,end_academic_year_id,status,migration_provenance) VALUES($1,'2569',$2,$2,'draft','{\"migration\":41}') RETURNING id")
            .bind(curriculum).bind(year).fetch_one(pool).await.unwrap();
        let program = Uuid::new_v4();
        sqlx::query("INSERT INTO study_programs(id,curriculum_version_id,code,name_th,is_default,status) VALUES($1,$2,'DEFAULT','แผนมาตรฐาน',true,'draft')")
            .bind(program).bind(version).execute(pool).await.unwrap();
        let slot: Uuid = sqlx::query_scalar("INSERT INTO curriculum_term_slots(curriculum_version_id,sequence,term_type,type_occurrence,name) VALUES($1,1,'regular',1,'ภาคเรียนที่ 1') RETURNING id")
            .bind(version).fetch_one(pool).await.unwrap();
        if code == "J-ART-LANG" {
            sqlx::query("INSERT INTO curriculum_course_requirements SELECT (jsonb_populate_record(NULL::curriculum_course_requirements,to_jsonb(requirement)||jsonb_build_object('id',gen_random_uuid(),'curriculum_version_id',$1::uuid,'study_program_id',$2::uuid,'term_slot_id',$3::uuid))).* FROM curriculum_course_requirements requirement ORDER BY requirement.id LIMIT 1")
                .bind(version).bind(program).bind(slot).execute(pool).await.unwrap();
        }
        sqlx::query("UPDATE study_programs SET status='published' WHERE id=$1")
            .bind(program)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query(
            "UPDATE curriculum_versions SET status='published',published_at=now() WHERE id=$1",
        )
        .bind(version)
        .execute(pool)
        .await
        .unwrap();
        programs.push(program);
    }
    programs
}

#[tokio::test]
async fn migration_091_preserves_edition_year_and_published_immutability() {
    let pool = predecessor("curriculum_091_revision").await;
    let programs = seed_hierarchy(&pool).await;
    apply_migrations_through(&pool, 91).await.unwrap();
    assert!(!column_exists(&pool, "curriculum_versions", "start_academic_year_id").await);
    assert!(!column_exists(&pool, "curriculum_versions", "end_academic_year_id").await);
    let editions: Vec<(i32, bool)> = sqlx::query_as("SELECT v.revision_year,v.migration_provenance ? 'revisionSelection' FROM curriculum_versions v JOIN study_programs p ON p.curriculum_version_id=v.id WHERE p.id=ANY($1)")
        .bind(&programs).fetch_all(&pool).await.unwrap();
    assert_eq!(editions, vec![(2569, true); 5]);
    assert!(sqlx::query("UPDATE curriculum_versions SET revision_year=2572 WHERE id=(SELECT curriculum_version_id FROM study_programs WHERE id=$1)")
        .bind(programs[0]).execute(&pool).await.is_err());
}

#[tokio::test]
async fn migration_092_merges_levels_without_replacing_program_or_requirement_identities() {
    let pool = predecessor("curriculum_092_preservation").await;
    let programs = seed_hierarchy(&pool).await;
    let before: Value = sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(r)-'curriculum_version_id'-'term_slot_id'-'updated_at'-'row_version' ORDER BY id) FROM curriculum_course_requirements r")
        .fetch_one(&pool).await.unwrap();
    apply_migrations_through(&pool, 92).await.unwrap();
    let after: Value = sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(r)-'curriculum_version_id'-'term_slot_id'-'updated_at'-'row_version' ORDER BY id) FROM curriculum_course_requirements r")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(before, after);
    let rows: Vec<(Uuid, String, String)> = sqlx::query_as("SELECT p.id,p.name_th,c.code FROM study_programs p JOIN curriculum_versions v ON v.id=p.curriculum_version_id JOIN curricula c ON c.id=v.curriculum_id WHERE p.id=ANY($1)")
        .bind(&programs).fetch_all(&pool).await.unwrap();
    assert_eq!(rows.len(), 5);
    assert_eq!(rows.iter().filter(|row| row.2 == "J-SECONDARY").count(), 3);
    assert_eq!(rows.iter().filter(|row| row.2 == "S-SECONDARY").count(), 2);
    assert!(rows.iter().all(|row| row.1 != "แผนมาตรฐาน"));
    let version_count: i64 = sqlx::query_scalar(
        "SELECT count(DISTINCT curriculum_version_id) FROM study_programs WHERE id=ANY($1)",
    )
    .bind(&programs)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(version_count, 2);
    apply_migrations_through(&pool, 92).await.unwrap();
    let protected: bool = sqlx::query_scalar(
        "SELECT bool_and(tgenabled='O') FROM pg_trigger WHERE tgname LIKE '%published%immutable'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(protected);
}

#[tokio::test]
async fn migration_092_refuses_ambiguous_mapping_atomically() {
    let pool = predecessor("curriculum_092_refusal").await;
    let programs = seed_hierarchy(&pool).await;
    apply_migrations_through(&pool, 91).await.unwrap();
    sqlx::query("UPDATE curricula SET name_th='หลักสูตรอื่นที่ใช้รหัสเหมือนกัน' WHERE code='J-ART-LANG'")
        .execute(&pool)
        .await
        .unwrap();
    assert!(apply_migrations_through(&pool, 92).await.is_err());
    let count: i64 = sqlx::query_scalar(
        "SELECT count(DISTINCT curriculum_version_id) FROM study_programs WHERE id=ANY($1)",
    )
    .bind(&programs)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 5);
    let renamed: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM curricula WHERE code IN ('J-SECONDARY','S-SECONDARY')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(renamed, 0);
}

#[tokio::test]
async fn migration_092_refuses_partial_roots_and_duplicate_grade_coverage() {
    for (name, mutation) in [
        ("curriculum_092_partial", "UPDATE curricula SET code='OTHER-CAR-TECH' WHERE code='J-CAR-TECH'"),
        ("curriculum_092_duplicate_grade", "UPDATE curricula SET grade_level_ids=jsonb_build_array(grade_level_ids->0,grade_level_ids->0,grade_level_ids->1) WHERE code='J-ART-LANG'"),
    ] {
        let pool = predecessor(name).await;
        let programs = seed_hierarchy(&pool).await;
        apply_migrations_through(&pool, 91).await.unwrap();
        sqlx::query(sqlx::AssertSqlSafe(mutation)).execute(&pool).await.unwrap();
        assert!(apply_migrations_through(&pool, 92).await.is_err());
        let retained: i64 = sqlx::query_scalar("SELECT count(DISTINCT curriculum_version_id) FROM study_programs WHERE id=ANY($1)")
            .bind(&programs).fetch_one(&pool).await.unwrap();
        assert_eq!(retained, 5);
        let protected: bool = sqlx::query_scalar("SELECT bool_and(tgenabled='O') FROM pg_trigger WHERE tgname LIKE '%published%immutable'")
            .fetch_one(&pool).await.unwrap();
        assert!(protected);
    }
}

#[tokio::test]
async fn migration_093_preserves_levels_programs_requirements_and_live_references() {
    let pool = predecessor("curriculum_093_preservation").await;
    let program_ids = seed_hierarchy(&pool).await;
    apply_migrations_through(&pool, 92).await.unwrap();
    let level_ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT DISTINCT curriculum_version_id FROM study_programs WHERE id=ANY($1)",
    )
    .bind(&program_ids)
    .fetch_all(&pool)
    .await
    .unwrap();
    let before: Value = sqlx::query_scalar("SELECT jsonb_build_object('courses',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM curriculum_course_requirements r),'activities',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM curriculum_activity_requirements r),'rooms',(SELECT jsonb_agg(jsonb_build_array(id,study_program_id) ORDER BY id) FROM homerooms),'students',(SELECT jsonb_agg(jsonb_build_array(id,study_program_id) ORDER BY id) FROM student_academic_years),'targets',(SELECT jsonb_agg(jsonb_build_array(id,study_program_id) ORDER BY id) FROM learning_offering_targets))").fetch_one(&pool).await.unwrap();
    apply_migrations_through(&pool, 94).await.unwrap();
    apply_migrations_through(&pool, 94).await.unwrap();
    let after: Value = sqlx::query_scalar("SELECT jsonb_build_object('courses',(SELECT jsonb_agg((to_jsonb(r)-'curriculum_level_id')||jsonb_build_object('curriculum_version_id',r.curriculum_level_id) ORDER BY id) FROM curriculum_course_requirements r),'activities',(SELECT jsonb_agg((to_jsonb(r)-'curriculum_level_id')||jsonb_build_object('curriculum_version_id',r.curriculum_level_id) ORDER BY id) FROM curriculum_activity_requirements r),'rooms',(SELECT jsonb_agg(jsonb_build_array(id,study_program_id) ORDER BY id) FROM homerooms),'students',(SELECT jsonb_agg(jsonb_build_array(id,study_program_id) ORDER BY id) FROM student_academic_years),'targets',(SELECT jsonb_agg(jsonb_build_array(id,study_program_id) ORDER BY id) FROM learning_offering_targets))").fetch_one(&pool).await.unwrap();
    assert_eq!(before, after);
    let rows: Vec<(Uuid, Uuid, Uuid)> = sqlx::query_as("SELECT p.id,l.id,l.edition_id FROM study_programs p JOIN curriculum_levels l ON l.id=p.curriculum_level_id WHERE p.id=ANY($1)").bind(&program_ids).fetch_all(&pool).await.unwrap();
    assert_eq!(rows.len(), 5);
    assert!(rows.iter().all(|row| level_ids.contains(&row.1)));
    assert_eq!(
        rows.iter()
            .map(|row| row.2)
            .collect::<std::collections::HashSet<_>>()
            .len(),
        1
    );
    assert!(!table_exists(&pool, "curricula").await);
    assert!(!table_exists(&pool, "curriculum_versions").await);
    assert!(!column_exists(&pool, "study_programs", "owning_organization_unit_id").await);
    let audit_count:i64=sqlx::query_scalar("SELECT count(*) FROM academic_audit_events WHERE event_code='curriculum.edition_hierarchy.reconciled' AND payload->>'passed'='true'").fetch_one(&pool).await.unwrap();
    assert!(audit_count > 0);
    assert!(
        sqlx::query("UPDATE curriculum_levels SET name_th='changed' WHERE id=$1")
            .bind(level_ids[0])
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("UPDATE study_programs SET name_th='changed' WHERE id=$1")
            .bind(program_ids[0])
            .execute(&pool)
            .await
            .is_err()
    );
    let draft_edition: Uuid = sqlx::query_scalar(
        "INSERT INTO curriculum_editions(name,revision_year) VALUES('ฉบับทดสอบ',2570) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        sqlx::query("UPDATE curriculum_levels SET edition_id=$1 WHERE id=$2")
            .bind(draft_edition)
            .bind(level_ids[0])
            .execute(&pool)
            .await
            .is_err(),
        "moving a frozen level out of its published edition must be rejected"
    );
    assert!(sqlx::query("INSERT INTO curriculum_levels(edition_id,code,name_th,grade_level_ids,is_active) VALUES($1,'TEST','test','[]',true)").bind(rows[0].2).execute(&pool).await.is_err(),"adding a level into a published edition must be rejected");
}

#[tokio::test]
async fn migration_093_retains_unknown_year_and_versionless_drafts_without_guessing() {
    let pool = predecessor("curriculum_093_unknown").await;
    apply_migrations_through(&pool, 92).await.unwrap();
    let roots:Vec<Uuid>=sqlx::query_scalar("INSERT INTO curricula(code,identity_key,name_th,grade_level_ids) SELECT 'UNKNOWN-'||n,'unknown-'||n,'ระดับไม่ระบุปี '||n,'[]' FROM generate_series(1,3) n RETURNING id").fetch_all(&pool).await.unwrap();
    for root in &roots[..2] {
        sqlx::query("INSERT INTO curriculum_versions(curriculum_id,version_name,status,revision_year) VALUES($1,'ร่างไม่ทราบปี','draft',NULL)").bind(root).execute(&pool).await.unwrap();
    }
    apply_migrations_through(&pool, 94).await.unwrap();
    let rows:Vec<(Uuid,Option<i32>,String)>=sqlx::query_as("SELECT e.id,e.revision_year,e.status FROM curriculum_levels l JOIN curriculum_editions e ON e.id=l.edition_id WHERE l.code LIKE 'UNKNOWN-%'").fetch_all(&pool).await.unwrap();
    assert_eq!(rows.len(), 3);
    assert!(rows.iter().all(|r| r.1.is_none() && r.2 == "draft"));
    assert_eq!(
        rows.iter()
            .map(|r| r.0)
            .collect::<std::collections::HashSet<_>>()
            .len(),
        3
    );
    apply_migrations_through(&pool, 96).await.unwrap();
    for row in rows {
        assert!(school_academic_core::services::curriculum::publish(
            &pool,
            row.0,
            school_academic_core::models::PublishCurriculumRequest {
                row_version: 1,
                draft_id: school_academic_core::services::curriculum::get(&pool, row.0)
                    .await
                    .unwrap()
                    .draft_id
                    .unwrap(),
                change_note: "ไม่ทราบปีปรับปรุง".into()
            },
            Uuid::nil()
        )
        .await
        .is_err());
    }
}

#[tokio::test]
async fn migration_093_refuses_ambiguous_duplicate_level_membership_atomically() {
    let pool = predecessor("curriculum_093_ambiguous").await;
    apply_migrations_through(&pool, 92).await.unwrap();
    for n in 1..=2 {
        let root:Uuid=sqlx::query_scalar("INSERT INTO curricula(code,identity_key,name_th,grade_level_ids) VALUES($1,$1,'ระดับชื่อเดียวกัน','[]') RETURNING id").bind(format!("DUPLICATE-{n}")).fetch_one(&pool).await.unwrap();
        sqlx::query("INSERT INTO curriculum_versions(curriculum_id,version_name,revision_year,status) VALUES($1,'ฉบับเดียวกัน',2570,'draft')").bind(root).execute(&pool).await.unwrap();
    }
    let error = apply_migrations_through(&pool, 93).await.unwrap_err();
    assert!(error
        .to_string()
        .contains("CURRICULUM_093_AMBIGUOUS_EDITION_LEVEL"));
    assert!(table_exists(&pool, "curricula").await);
    assert!(!table_exists(&pool, "curriculum_editions").await);
}

#[tokio::test]
async fn migration_094_preserves_granted_actions_and_delegation_expiry_and_revocation() {
    let pool = predecessor("curriculum_094_grants").await;
    apply_migrations_through(&pool, 93).await.unwrap();
    let read_permission: Uuid = sqlx::query_scalar(
        "SELECT id FROM permissions WHERE code='academic_curriculum.read.organization_unit'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let role: Uuid = sqlx::query_scalar("SELECT id FROM roles ORDER BY id LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO role_permissions(role_id,permission_id) VALUES($1,$2) ON CONFLICT DO NOTHING",
    )
    .bind(role)
    .bind(read_permission)
    .execute(&pool)
    .await
    .unwrap();
    let unit: Uuid = sqlx::query_scalar("SELECT id FROM organization_units ORDER BY id LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let users: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM users WHERE user_type='staff' ORDER BY id LIMIT 2")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(users.len(), 2);
    sqlx::query("INSERT INTO organization_permission_grants(organization_unit_id,permission_id,position_code,created_by) VALUES($1,$2,'head',$3) ON CONFLICT DO NOTHING").bind(unit).bind(read_permission).bind(users[0]).execute(&pool).await.unwrap();
    let delegation_id = Uuid::new_v4();
    sqlx::query("INSERT INTO organization_permission_delegations(id,from_user_id,to_user_id,permission_id,organization_unit_id,reason,started_at,expires_at,revoked_at) VALUES($1,$2,$3,$4,$5,'expired revoked history','2020-01-01','2021-01-01','2020-12-31')").bind(delegation_id).bind(users[0]).bind(users[1]).bind(read_permission).bind(unit).execute(&pool).await.unwrap();
    let before: Value = sqlx::query_scalar(
        "SELECT to_jsonb(d)-'permission_id' FROM organization_permission_delegations d WHERE id=$1",
    )
    .bind(delegation_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    apply_migrations_through(&pool, 94).await.unwrap();
    let current_read: Uuid = sqlx::query_scalar(
        "SELECT id FROM permissions WHERE code='academic_curriculum.read.school' AND is_active",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM role_permissions WHERE role_id=$1 AND permission_id=$2)"
    )
    .bind(role)
    .bind(current_read)
    .fetch_one(&pool)
    .await
    .unwrap());
    assert!(sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM organization_permission_grants WHERE organization_unit_id=$1 AND permission_id=$2 AND position_code='head' AND created_by=$3)").bind(unit).bind(current_read).bind(users[0]).fetch_one(&pool).await.unwrap());
    let after:Value=sqlx::query_scalar("SELECT to_jsonb(d)-'permission_id' FROM organization_permission_delegations d WHERE id=$1 AND permission_id=$2").bind(delegation_id).bind(current_read).fetch_one(&pool).await.unwrap();
    assert_eq!(before, after);
    let retired:i64=sqlx::query_scalar("SELECT count(*) FROM permissions WHERE module='academic_curriculum' AND scope IN ('organization_unit','organization_tree') AND is_active").fetch_one(&pool).await.unwrap();
    assert_eq!(retired, 0);
}
