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
