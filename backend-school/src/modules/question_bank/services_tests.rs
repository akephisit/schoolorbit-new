use super::models::{
    ImageAlignment, ImageNodeAttributes, QuestionBankListQuery, RichBlockNode, RichContent,
    RichDocument, RichInlineNode, RichTextMark, UpsertQuestionChoiceRequest, UpsertQuestionRequest,
    RICH_CONTENT_SCHEMA_VERSION,
};
use super::services;
use crate::error::AppError;
use crate::middleware::permission::ActorContext;
use crate::modules::academic::cutover_test_support::{
    apply_migrations_through, apply_phase_b_runtime_migrations,
    record_passing_phase_a_reconciliation_marker, seed_academic_cutover_fixture, CutoverFixture,
};
use crate::permissions::registry::codes;
use crate::policies::question_bank_access_policy;
use crate::test_helpers::{create_named_test_pool, create_test_user};
use sqlx::PgPool;
use uuid::Uuid;

async fn migrated_pool(name: &str) -> PgPool {
    let pool = create_named_test_pool(name).await;
    apply_migrations_through(&pool, 40)
        .await
        .expect("legacy question-bank fixture migrations should run");
    seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
        .await
        .expect("academic cutover fixture should seed");
    apply_phase_b_runtime_migrations(&pool)
        .await
        .expect("canonical question-bank fixture migrations should run");
    apply_migrations_through(&pool, 59)
        .await
        .expect("current question-bank fixture migrations should run");
    pool
}

async fn test_user(pool: &PgPool, role: &str) -> Uuid {
    let unique = Uuid::new_v4();
    create_test_user(
        pool,
        &format!("question-bank-{role}-{unique}@example.test"),
        "test-password",
    )
    .await
    .expect("question-bank fixture user should insert")
}

fn actor(user_id: Uuid, permissions: &[&str]) -> ActorContext {
    ActorContext {
        user_id,
        permissions: permissions
            .iter()
            .map(|permission| permission.to_string())
            .collect(),
    }
}

fn rich_content(text: &str, image_file_id: Option<Uuid>) -> RichContent {
    let mut content = vec![RichBlockNode::Paragraph {
        content: vec![RichInlineNode::Text {
            text: text.to_string(),
            marks: vec![RichTextMark::Bold],
        }],
    }];
    if let Some(file_id) = image_file_id {
        content.push(RichBlockNode::Image {
            attrs: ImageNodeAttributes {
                file_id,
                alt_text: Some(format!("ภาพประกอบ {text}")),
                caption: None,
                alignment: ImageAlignment::Center,
                width_percent: 60,
            },
        });
    }
    RichContent {
        schema_version: RICH_CONTENT_SCHEMA_VERSION,
        document: RichDocument::Doc { content },
    }
}

async fn insert_ready_question_image(pool: &PgPool, owner_user_id: Uuid, label: &str) -> Uuid {
    let file_id = Uuid::new_v4();
    let version_id = Uuid::new_v4();
    sqlx::query(
        r#"
INSERT INTO files (
    id,
    owner_user_id,
    display_filename,
    purpose_code,
    visibility,
    lifecycle_status,
    retention_class,
    created_by
)
VALUES ($1, $2, $3, 'question_bank_image', 'private', 'pending', 'standard', $2)
"#,
    )
    .bind(file_id)
    .bind(owner_user_id)
    .bind(format!("{label}.png"))
    .execute(pool)
    .await
    .expect("question image metadata should insert");

    sqlx::query(
        r#"
INSERT INTO file_versions (
    id,
    file_id,
    version_number,
    provider_code,
    storage_class,
    storage_status,
    object_key,
    detected_mime_type,
    canonical_extension,
    byte_size,
    checksum,
    scan_status,
    created_by
)
VALUES ($1, $2, 1, 'test', 'private', 'stored', $3, 'image/png', 'png', 64, $4, 'clean', $5)
"#,
    )
    .bind(version_id)
    .bind(file_id)
    .bind(format!("question-bank/{file_id}.png"))
    .bind("a".repeat(64))
    .bind(owner_user_id)
    .execute(pool)
    .await
    .expect("question image version should insert");

    sqlx::query(
        "UPDATE files SET current_version_id = $2, lifecycle_status = 'ready' WHERE id = $1",
    )
    .bind(file_id)
    .bind(version_id)
    .execute(pool)
    .await
    .expect("question image should become ready");
    file_id
}

fn question_payload(
    subject_id: Uuid,
    label: &str,
    image_file_id: Option<Uuid>,
    with_choices: bool,
) -> UpsertQuestionRequest {
    UpsertQuestionRequest {
        subject_id,
        question_type: if with_choices {
            "single_choice".to_string()
        } else {
            "short_answer".to_string()
        },
        difficulty: "medium".to_string(),
        points: 2.0,
        stem_content: rich_content(label, image_file_id),
        explanation_content: Some(rich_content(&format!("เฉลย {label}"), None)),
        rubric_content: None,
        tags: vec!["export".to_string()],
        status: "ready".to_string(),
        choices: if with_choices {
            vec![
                UpsertQuestionChoiceRequest {
                    id: None,
                    label: "ก".to_string(),
                    content: rich_content("คำตอบ ก", None),
                    is_correct: true,
                    sort_order: 10,
                },
                UpsertQuestionChoiceRequest {
                    id: None,
                    label: "ข".to_string(),
                    content: rich_content("คำตอบ ข", None),
                    is_correct: false,
                    sort_order: 20,
                },
            ]
        } else {
            Vec::new()
        },
    }
}

fn assert_bad_request(error: AppError) {
    assert_eq!(error.status_code(), axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn export_data_is_ordered_bounded_and_fail_closed() {
    let pool = migrated_pool("question_bank_export_data").await;
    let normalized_fixture_content = sqlx::query_scalar::<_, serde_json::Value>(
        "SELECT stem_content FROM academic_question_bank_questions WHERE id = '93000000-0000-0000-0000-000000000001'",
    )
    .fetch_one(&pool)
    .await
    .expect("legacy empty question content should survive as canonical content");
    assert_eq!(normalized_fixture_content["schemaVersion"], 1);
    assert_eq!(normalized_fixture_content["document"]["type"], "doc");
    assert_eq!(
        normalized_fixture_content["document"]["content"],
        serde_json::json!([])
    );
    sqlx::query(
        "UPDATE academic_question_bank_questions SET stem_content = '{}'::jsonb WHERE id = '93000000-0000-0000-0000-000000000001'",
    )
    .execute(&pool)
    .await
    .expect_err("hardened rich-content constraint should reject missing canonical keys");

    let first_owner_id = test_user(&pool, "first-owner").await;
    let second_owner_id = test_user(&pool, "second-owner").await;
    let assigned_reader_id = test_user(&pool, "assigned-reader").await;
    let school_reader_id = test_user(&pool, "school-reader").await;
    let unit_reader_id = test_user(&pool, "unit-reader").await;
    let subject_id =
        sqlx::query_scalar::<_, Uuid>("SELECT id FROM subjects ORDER BY code, id LIMIT 1")
            .fetch_one(&pool)
            .await
            .expect("question-bank fixture should have a canonical subject");
    let child_subject_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM subjects WHERE id <> $1 ORDER BY code, id LIMIT 1",
    )
    .bind(subject_id)
    .fetch_one(&pool)
    .await
    .expect("question-bank fixture should have a second canonical subject");
    let root_unit_id = Uuid::parse_str("c5e06a47-ebf6-40f6-bbf9-59c509e842f2")
        .expect("fixture organization unit ID should parse");
    let child_unit_id = Uuid::new_v4();
    sqlx::query(
        r#"
INSERT INTO organization_units (id, code, name, parent_unit_id, category, unit_type)
VALUES ($1, $2, 'หน่วยงานลูกคลังข้อสอบ', $3, 'academic', 'unit')
"#,
    )
    .bind(child_unit_id)
    .bind(format!("QUESTION-BANK-{}", Uuid::new_v4()))
    .bind(root_unit_id)
    .execute(&pool)
    .await
    .expect("question-bank child organization unit should insert");
    sqlx::query(
        r#"
INSERT INTO organization_members (
    id, user_id, organization_unit_id, position_code, started_at
)
VALUES ($1, $2, $3, 'head', '2020-01-01')
"#,
    )
    .bind(Uuid::new_v4())
    .bind(unit_reader_id)
    .bind(root_unit_id)
    .execute(&pool)
    .await
    .expect("question-bank unit reader membership should insert");
    sqlx::query(
        r#"
INSERT INTO organization_permission_grants (
    organization_unit_id, permission_id, created_by, position_code
)
SELECT $1, permission.id, $2, 'head'
FROM permissions permission
WHERE permission.code = $3
"#,
    )
    .bind(root_unit_id)
    .bind(unit_reader_id)
    .bind(codes::ACADEMIC_QUESTION_BANK_READ_ORGANIZATION_UNIT)
    .execute(&pool)
    .await
    .expect("question-bank exact-unit grant should insert");
    sqlx::query(
        r#"
UPDATE subjects
SET owning_organization_unit_id = CASE
    WHEN id = $1 THEN $3
    WHEN id = $2 THEN $4
    ELSE owning_organization_unit_id
END
WHERE id = ANY($5)
"#,
    )
    .bind(subject_id)
    .bind(child_subject_id)
    .bind(root_unit_id)
    .bind(child_unit_id)
    .bind(vec![subject_id, child_subject_id])
    .execute(&pool)
    .await
    .expect("question-bank subjects should receive canonical organization owners");
    let first_file_id = insert_ready_question_image(&pool, first_owner_id, "first").await;
    let second_file_id = insert_ready_question_image(&pool, second_owner_id, "second").await;

    let first_manager = actor(
        first_owner_id,
        &[codes::ACADEMIC_QUESTION_BANK_MANAGE_SCHOOL],
    );
    let second_manager = actor(
        second_owner_id,
        &[codes::ACADEMIC_QUESTION_BANK_MANAGE_SCHOOL],
    );
    let assigned_manager = actor(
        assigned_reader_id,
        &[codes::ACADEMIC_QUESTION_BANK_MANAGE_SCHOOL],
    );
    let first = services::create_question(
        &pool,
        &first_manager,
        first_owner_id,
        question_payload(subject_id, "ข้อแรก", Some(first_file_id), true),
    )
    .await
    .expect("first question should create");
    let second = services::create_question(
        &pool,
        &second_manager,
        second_owner_id,
        question_payload(subject_id, "ข้อสอง", Some(second_file_id), false),
    )
    .await
    .expect("second question should create");
    let assigned = services::create_question(
        &pool,
        &assigned_manager,
        assigned_reader_id,
        question_payload(subject_id, "ข้อของผู้รับผิดชอบ", None, false),
    )
    .await
    .expect("assigned question should create");
    let child = services::create_question(
        &pool,
        &first_manager,
        first_owner_id,
        question_payload(child_subject_id, "ข้อของหน่วยงานลูก", None, false),
    )
    .await
    .expect("child-unit question should create");

    let school_reader = actor(
        school_reader_id,
        &[codes::ACADEMIC_QUESTION_BANK_READ_SCHOOL],
    );
    let school_access = question_bank_access_policy::resolve_access(&pool, &school_reader)
        .await
        .expect("school question-bank access should resolve");
    let options = services::list_options(&pool, &school_access)
        .await
        .expect("canonical subject options should remain available");
    let subject_option = options
        .subjects
        .iter()
        .find(|subject| subject.id == subject_id)
        .expect("created question subject should be listed from its published version");
    assert!(!subject_option.name_th.is_empty());

    let page = services::list_questions(
        &pool,
        &QuestionBankListQuery {
            subject_id: Some(subject_id),
            question_type: None,
            difficulty: None,
            status: None,
            tag: None,
            search: None,
            page: Some(1),
            page_size: Some(100),
        },
        &school_access,
    )
    .await
    .expect("canonical question list should remain available");
    let created_question_ids = [first.question.id, second.question.id, assigned.question.id]
        .into_iter()
        .collect::<std::collections::HashSet<_>>();
    assert!(page.total >= created_question_ids.len() as i64);
    assert!(created_question_ids.iter().all(|question_id| page
        .items
        .iter()
        .any(|question| question.id == *question_id && question.points == 2.0)));
    assert!(page.items.iter().all(
        |question| question.subject_name_th.as_deref() == Some(subject_option.name_th.as_str())
    ));

    let exported = services::export_question_data(
        &pool,
        &school_reader,
        &[second.question.id, first.question.id],
    )
    .await
    .expect("authorized export should return every selected question");
    assert_eq!(
        exported
            .iter()
            .map(|detail| detail.question.id)
            .collect::<Vec<_>>(),
        vec![second.question.id, first.question.id]
    );
    assert!(exported[0].choices.is_empty());
    assert_eq!(exported[1].choices.len(), 2);
    assert_eq!(exported[1].choices[0].label, "ก");
    assert_eq!(exported[1].choices[1].label, "ข");
    assert_eq!(exported[0].files.len(), 1);
    assert_eq!(exported[0].files[0].id, second_file_id);
    assert_eq!(exported[1].files.len(), 1);
    assert_eq!(exported[1].files[0].id, first_file_id);

    let existing_detail = services::get_question(&pool, &school_reader, first.question.id)
        .await
        .expect("single-question detail should remain available");
    assert_eq!(existing_detail.question.id, first.question.id);
    assert_eq!(existing_detail.choices.len(), 2);
    assert_eq!(existing_detail.files.len(), 1);
    assert_eq!(existing_detail.files[0].id, first_file_id);

    let unit_reader = actor(
        unit_reader_id,
        &[codes::ACADEMIC_QUESTION_BANK_READ_ORGANIZATION_UNIT],
    );
    let unit_export = services::export_question_data(&pool, &unit_reader, &[first.question.id])
        .await
        .expect("exact-unit reader should export a question owned by the granted unit");
    assert_eq!(unit_export[0].question.id, first.question.id);
    let child_unit_error = services::export_question_data(
        &pool,
        &unit_reader,
        &[first.question.id, child.question.id],
    )
    .await
    .expect_err("exact-unit access must not expand to a child organization unit");
    assert_eq!(
        child_unit_error.status_code(),
        axum::http::StatusCode::NOT_FOUND
    );

    assert_bad_request(
        services::export_question_data(&pool, &school_reader, &[])
            .await
            .expect_err("empty export should fail validation"),
    );
    assert_bad_request(
        services::export_question_data(
            &pool,
            &school_reader,
            &[first.question.id, first.question.id],
        )
        .await
        .expect_err("duplicate IDs should fail validation"),
    );
    let over_limit_ids = (0..201).map(|_| Uuid::new_v4()).collect::<Vec<_>>();
    assert_bad_request(
        services::export_question_data(&pool, &school_reader, &over_limit_ids)
            .await
            .expect_err("more than 200 IDs should fail validation"),
    );

    let missing_error =
        services::export_question_data(&pool, &school_reader, &[first.question.id, Uuid::new_v4()])
            .await
            .expect_err("a missing ID should fail the entire export");
    let assigned_reader = actor(
        assigned_reader_id,
        &[codes::ACADEMIC_QUESTION_BANK_READ_ASSIGNED],
    );
    let unauthorized_error = services::export_question_data(
        &pool,
        &assigned_reader,
        &[assigned.question.id, first.question.id],
    )
    .await
    .expect_err("an unauthorized ID should fail instead of returning a partial export");
    assert_eq!(
        missing_error.status_code(),
        axum::http::StatusCode::NOT_FOUND
    );
    assert_eq!(
        unauthorized_error.status_code(),
        missing_error.status_code()
    );
    assert_eq!(
        unauthorized_error.public_message(),
        missing_error.public_message()
    );
}

#[tokio::test]
async fn rich_content_hardening_rejects_nonempty_legacy_documents() {
    let pool = create_named_test_pool("question_bank_legacy_content").await;
    apply_migrations_through(&pool, 40)
        .await
        .expect("legacy question-bank fixture migrations should run");
    seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
        .await
        .expect("academic cutover fixture should seed");
    apply_migrations_through(&pool, 44)
        .await
        .expect("phase A runtime migrations should run");
    record_passing_phase_a_reconciliation_marker(&pool)
        .await
        .expect("phase A reconciliation marker should record");
    apply_migrations_through(&pool, 45)
        .await
        .expect("phase B cleanup should run");

    sqlx::query(
        r#"
UPDATE academic_question_bank_questions
SET stem_content = '{"blocks":[{"text":"legacy content"}]}'::jsonb
WHERE id = '93000000-0000-0000-0000-000000000001'
"#,
    )
    .execute(&pool)
    .await
    .expect("the migration 025 NULL loophole should be reproduced before hardening");

    let error = apply_migrations_through(&pool, 46)
        .await
        .expect_err("nonempty legacy content must stop the hardening migration");
    assert!(error
        .to_string()
        .contains("QUESTION_BANK_046_UNSUPPORTED_LEGACY_RICH_CONTENT"));
}
