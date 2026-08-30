use chrono::NaiveDate;
use uuid::Uuid;

use super::{timetable_template_service, timetable_version_service};
use crate::error::AppError;
use crate::modules::academic::cutover_test_support::{
    apply_migrations_through, apply_phase_b_runtime_migrations, seed_academic_cutover_fixture,
    CutoverFixture,
};
use crate::modules::academic::models::timetable::{
    ApplyTemplateRequest, ClearTimetableRequest, FromCurrentRequest,
};
use crate::modules::academic::models::timetable_version::CloneTimetableVersionRequest;
use crate::test_helpers::create_named_test_pool;

#[tokio::test]
async fn template_source_apply_and_clear_are_version_scoped() {
    let pool = create_named_test_pool("timetable_template_version_scope").await;
    apply_migrations_through(&pool, 40).await.unwrap();
    seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
        .await
        .unwrap();
    apply_phase_b_runtime_migrations(&pool).await.unwrap();
    apply_migrations_through(&pool, 54).await.unwrap();

    let actor_id = Uuid::parse_str("50000000-0000-0000-0000-000000000002").unwrap();
    let (source_id, source_row_version, term_id, term_start): (Uuid, i64, Uuid, NaiveDate) =
        sqlx::query_as(
            r#"SELECT version.id, version.row_version, term.id, term.start_date
               FROM academic_timetable_versions version
               JOIN academic_terms term ON term.id = version.academic_term_id
               WHERE version.status = 'published' AND term.status = 'active'
               ORDER BY version.effective_from, version.id LIMIT 1"#,
        )
        .fetch_one(&pool)
        .await
        .unwrap();
    let draft = timetable_version_service::clone_draft(
        &pool,
        actor_id,
        source_id,
        CloneTimetableVersionRequest {
            effective_from: term_start.succ_opt().unwrap(),
            source_row_version,
        },
    )
    .await
    .unwrap();

    let template = timetable_template_service::from_current(
        &pool,
        actor_id,
        FromCurrentRequest {
            timetable_version_id: source_id,
            academic_term_id: term_id,
            name: "แม่แบบจากรุ่นเผยแพร่".to_string(),
            description: None,
            entry_types: None,
        },
    )
    .await
    .unwrap();
    assert!(!template.entries.is_empty());

    let published_before: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM academic_timetable_entries WHERE timetable_version_id = $1 AND is_active",
    )
    .bind(source_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let cleared = timetable_template_service::clear_timetable(
        &pool,
        actor_id,
        ClearTimetableRequest {
            timetable_version_id: draft.id,
            academic_term_id: term_id,
            entry_types: None,
        },
    )
    .await
    .unwrap();
    assert!(!cleared.is_empty());
    let published_after: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM academic_timetable_entries WHERE timetable_version_id = $1 AND is_active",
    )
    .bind(source_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(published_after, published_before);

    let applied = timetable_template_service::apply_template(
        &pool,
        actor_id,
        template.template.id,
        ApplyTemplateRequest {
            timetable_version_id: draft.id,
            academic_term_id: term_id,
        },
    )
    .await
    .unwrap();
    assert_eq!(applied.applied, template.entries.len());
    for (template_entry, applied_entry_id) in template.entries.iter().zip(&applied.entry_ids) {
        let actual: Vec<(Uuid, String)> = sqlx::query_as(
            r#"SELECT instructor_id, role::text
               FROM timetable_entry_instructors
               WHERE entry_id = $1
               ORDER BY CASE role WHEN 'primary' THEN 1 ELSE 2 END,
                        created_at,
                        instructor_id"#,
        )
        .bind(applied_entry_id)
        .fetch_all(&pool)
        .await
        .unwrap();
        let expected = template_entry
            .instructor_ids
            .iter()
            .enumerate()
            .map(|(index, instructor_id)| {
                (
                    *instructor_id,
                    if index == 0 { "primary" } else { "secondary" }.to_string(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
    }
    let exact_group_instructor_count: i64 = sqlx::query_scalar(
        r#"SELECT count(*)
           FROM timetable_entry_instructors instructor
           JOIN academic_timetable_entries entry ON entry.id = instructor.entry_id
           WHERE entry.id = ANY($1) AND entry.learning_group_id IS NOT NULL"#,
    )
    .bind(&applied.entry_ids)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(exact_group_instructor_count > 0);
    let wrong_version_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM academic_timetable_entries WHERE id = ANY($1) AND timetable_version_id <> $2",
    )
    .bind(&applied.entry_ids)
    .bind(draft.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(wrong_version_count, 0);

    timetable_template_service::clear_timetable(
        &pool,
        actor_id,
        ClearTimetableRequest {
            timetable_version_id: draft.id,
            academic_term_id: term_id,
            entry_types: None,
        },
    )
    .await
    .unwrap();
    let group_template_entry = template
        .entries
        .iter()
        .find(|entry| entry.resource_kind != "structural" && !entry.instructor_ids.is_empty())
        .expect("source template must contain one taught group entry");
    let ineligible_teacher_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO users (
               id, email, username, password_hash, first_name, last_name,
               user_type, status
           ) VALUES ($1, $2, $3, 'fixture-not-a-login', 'ครูนอกแม่แบบ', 'ทดสอบ', 'staff', 'active')"#,
    )
    .bind(ineligible_teacher_id)
    .bind(format!("{ineligible_teacher_id}@example.invalid"))
    .bind(format!("teacher-{ineligible_teacher_id}"))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE timetable_template_entries SET instructor_ids = $2 WHERE id = $1")
        .bind(group_template_entry.id)
        .bind(sqlx::types::Json(vec![ineligible_teacher_id]))
        .execute(&pool)
        .await
        .unwrap();
    let applied_with_ineligible_teacher = timetable_template_service::apply_template(
        &pool,
        actor_id,
        template.template.id,
        ApplyTemplateRequest {
            timetable_version_id: draft.id,
            academic_term_id: term_id,
        },
    )
    .await
    .expect("an ineligible template teacher must not abort the whole template");
    let group_entries_without_instructors: i64 = sqlx::query_scalar(
        r#"SELECT count(*)
           FROM academic_timetable_entries entry
           WHERE entry.id = ANY($1)
             AND entry.learning_group_id IS NOT NULL
             AND NOT EXISTS (
                 SELECT 1 FROM timetable_entry_instructors instructor
                 WHERE instructor.entry_id = entry.id
             )"#,
    )
    .bind(&applied_with_ineligible_teacher.entry_ids)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(group_entries_without_instructors, 1);

    let published_apply = timetable_template_service::apply_template(
        &pool,
        actor_id,
        template.template.id,
        ApplyTemplateRequest {
            timetable_version_id: source_id,
            academic_term_id: term_id,
        },
    )
    .await;
    assert!(matches!(published_apply, Err(AppError::Conflict(_))));
}
