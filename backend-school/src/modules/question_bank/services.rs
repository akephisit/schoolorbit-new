use std::collections::HashSet;

use sqlx::types::Json;
use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

use crate::error::AppError;
use crate::middleware::permission::ActorContext;
use crate::modules::question_bank::models::{
    QuestionBankListQuery, QuestionBankOptions, QuestionBankPage, QuestionBankSubjectOption,
    QuestionBankSummary, QuestionBankSummaryRow, QuestionChoice, QuestionChoiceRow, QuestionDetail,
    QuestionFile, QuestionRow, QuestionScopeRow, QuestionSummary, RichContent,
    UpsertQuestionChoiceRequest, UpsertQuestionRequest,
};
use crate::policies::question_bank_access_policy::{self, QuestionBankAccess};
use crate::utils::file_url::FileUrlBuilder;

const VALID_QUESTION_TYPES: &[&str] =
    &["single_choice", "multiple_choice", "short_answer", "essay"];
const VALID_DIFFICULTIES: &[&str] = &["easy", "medium", "hard"];
const VALID_STATUSES: &[&str] = &["draft", "ready", "archived"];
const DEFAULT_PAGE_SIZE: i64 = 20;
const MAX_PAGE_SIZE: i64 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QuestionKind {
    SingleChoice,
    MultipleChoice,
    ShortAnswer,
    Essay,
}

impl QuestionKind {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "single_choice" => Some(Self::SingleChoice),
            "multiple_choice" => Some(Self::MultipleChoice),
            "short_answer" => Some(Self::ShortAnswer),
            "essay" => Some(Self::Essay),
            _ => None,
        }
    }

    fn requires_choices(self) -> bool {
        matches!(self, Self::SingleChoice | Self::MultipleChoice)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PageParams {
    page: i64,
    page_size: i64,
    offset: i64,
}

#[derive(Debug, sqlx::FromRow)]
struct PayloadFileRow {
    id: Uuid,
    user_id: Option<Uuid>,
    mime_type: String,
    file_type: String,
    is_temporary: bool,
}

pub async fn list_questions(
    pool: &PgPool,
    query: &QuestionBankListQuery,
    access: &QuestionBankAccess,
) -> Result<QuestionBankPage, AppError> {
    let page_params = normalize_page_params(query.page, query.page_size);
    let summary = fetch_question_summary(pool, query, access).await?;

    let mut builder = QueryBuilder::<Postgres>::new(
        r#"
SELECT
    q.id,
    q.subject_id,
    q.owner_user_id,
    q.question_type,
    q.difficulty,
    q.points,
    q.stem_content,
    q.explanation_content,
    q.rubric_content,
    q.tags,
    q.status,
    s.code AS subject_code,
    s.name_th AS subject_name_th,
    s.name_en AS subject_name_en,
    s.group_id AS subject_group_id,
    sg.name_th AS subject_group_name,
    COALESCE(choice_stats.choice_count, 0)::BIGINT AS choice_count,
    COALESCE(choice_stats.correct_choice_count, 0)::BIGINT AS correct_choice_count,
"#,
    );
    push_manage_expression(&mut builder, access);
    builder.push(
        r#" AS can_manage,
    q.created_at,
    q.updated_at
FROM academic_question_bank_questions q
LEFT JOIN subjects s ON s.id = q.subject_id
LEFT JOIN subject_groups sg ON sg.id = s.group_id
LEFT JOIN LATERAL (
    SELECT
        COUNT(*) AS choice_count,
        COUNT(*) FILTER (WHERE c.is_correct = true) AS correct_choice_count
    FROM academic_question_bank_choices c
    WHERE c.question_id = q.id
) choice_stats ON true
WHERE q.deleted_at IS NULL
"#,
    );
    push_list_filters(&mut builder, query, access);
    builder.push(" ORDER BY q.updated_at DESC, q.created_at DESC, q.id DESC LIMIT ");
    builder.push_bind(page_params.page_size);
    builder.push(" OFFSET ");
    builder.push_bind(page_params.offset);

    let rows = builder
        .build_query_as::<QuestionRow>()
        .fetch_all(pool)
        .await
        .map_err(|error| {
            tracing::error!("Failed to list question bank questions: {}", error);
            AppError::InternalServerError("ไม่สามารถดึงรายการข้อสอบได้".to_string())
        })?;

    let total_pages = ((summary.total + page_params.page_size - 1) / page_params.page_size).max(1);
    Ok(QuestionBankPage {
        items: rows.into_iter().map(QuestionSummary::from).collect(),
        total: summary.total,
        page: page_params.page,
        page_size: page_params.page_size,
        total_pages,
        summary,
    })
}

pub async fn list_options(
    pool: &PgPool,
    access: &QuestionBankAccess,
) -> Result<QuestionBankOptions, AppError> {
    let mut builder = QueryBuilder::<Postgres>::new(
        r#"
SELECT
    s.id,
    s.code,
    s.name_th,
    s.name_en,
    s.group_id AS subject_group_id,
    sg.name_th AS subject_group_name,
"#,
    );
    push_subject_manage_expression(&mut builder, access);
    builder.push(
        r#" AS can_create
FROM subjects s
LEFT JOIN subject_groups sg ON sg.id = s.group_id
WHERE (
"#,
    );
    push_subject_read_expression(&mut builder, access);
    builder.push(
        r#"
)
ORDER BY s.code ASC, s.name_th ASC, s.start_academic_year_id DESC, s.id ASC
"#,
    );

    let subjects = builder
        .build_query_as::<QuestionBankSubjectOption>()
        .fetch_all(pool)
        .await
        .map_err(|error| {
            tracing::error!("Failed to list question bank subject options: {}", error);
            AppError::InternalServerError("ไม่สามารถดึงรายวิชาสำหรับคลังข้อสอบได้".to_string())
        })?;

    Ok(QuestionBankOptions { subjects })
}

pub async fn get_question(
    pool: &PgPool,
    actor: &ActorContext,
    question_id: Uuid,
) -> Result<QuestionDetail, AppError> {
    let scope = fetch_question_scope(pool, question_id).await?;
    question_bank_access_policy::require_question_read_access(pool, actor, &scope).await?;
    let access = question_bank_access_policy::resolve_access(pool, actor).await?;
    fetch_question_detail(pool, question_id, &access).await
}

pub async fn create_question(
    pool: &PgPool,
    actor: &ActorContext,
    actor_id: Uuid,
    mut payload: UpsertQuestionRequest,
) -> Result<QuestionDetail, AppError> {
    normalize_payload(&mut payload);
    validate_question_payload(&payload)?;
    question_bank_access_policy::require_subject_create_access(pool, actor, payload.subject_id)
        .await?;

    let image_file_ids = collect_payload_image_file_ids(&payload);
    let temporary_image_file_ids =
        validate_payload_files(pool, actor_id, &image_file_ids, &HashSet::new()).await?;
    let stem_search_text = payload.stem_content.search_text();

    let mut tx = pool.begin().await.map_err(|error| {
        tracing::error!("Failed to start question create transaction: {}", error);
        AppError::InternalServerError("ไม่สามารถเริ่มบันทึกข้อสอบได้".to_string())
    })?;

    let question_id = sqlx::query_scalar::<_, Uuid>(
        r#"
INSERT INTO academic_question_bank_questions (
    subject_id,
    owner_user_id,
    question_type,
    difficulty,
    points,
    stem_content,
    search_text,
    explanation_content,
    rubric_content,
    tags,
    status,
    created_by,
    updated_by
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $12)
RETURNING id
"#,
    )
    .bind(payload.subject_id)
    .bind(actor_id)
    .bind(&payload.question_type)
    .bind(&payload.difficulty)
    .bind(payload.points)
    .bind(Json(payload.stem_content.clone()))
    .bind(stem_search_text)
    .bind(payload.explanation_content.clone().map(Json))
    .bind(payload.rubric_content.clone().map(Json))
    .bind(&payload.tags)
    .bind(&payload.status)
    .bind(actor_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|error| {
        tracing::error!("Failed to create question bank question: {}", error);
        AppError::InternalServerError("บันทึกข้อสอบไม่สำเร็จ".to_string())
    })?;

    insert_choices(&mut tx, question_id, &payload.choices).await?;
    finalize_temporary_files(&mut tx, actor_id, &temporary_image_file_ids).await?;
    tx.commit().await.map_err(|error| {
        tracing::error!("Failed to commit question create transaction: {}", error);
        AppError::InternalServerError("บันทึกข้อสอบไม่สำเร็จ".to_string())
    })?;

    let access = question_bank_access_policy::resolve_access(pool, actor).await?;
    fetch_question_detail(pool, question_id, &access).await
}

pub async fn update_question(
    pool: &PgPool,
    actor: &ActorContext,
    question_id: Uuid,
    actor_id: Uuid,
    mut payload: UpsertQuestionRequest,
) -> Result<QuestionDetail, AppError> {
    normalize_payload(&mut payload);
    validate_question_payload(&payload)?;
    let scope = fetch_question_scope(pool, question_id).await?;
    question_bank_access_policy::require_question_manage_access(pool, actor, &scope).await?;
    if scope.subject_id != Some(payload.subject_id) {
        question_bank_access_policy::require_subject_create_access(pool, actor, payload.subject_id)
            .await?;
    }

    let existing_file_ids = fetch_question_file_ids(pool, question_id).await?;
    let image_file_ids = collect_payload_image_file_ids(&payload);
    let temporary_image_file_ids =
        validate_payload_files(pool, actor_id, &image_file_ids, &existing_file_ids).await?;
    let stem_search_text = payload.stem_content.search_text();

    let mut tx = pool.begin().await.map_err(|error| {
        tracing::error!("Failed to start question update transaction: {}", error);
        AppError::InternalServerError("ไม่สามารถเริ่มแก้ไขข้อสอบได้".to_string())
    })?;

    sqlx::query(
        r#"
UPDATE academic_question_bank_questions
SET
    subject_id = $2,
    grade_level_id = NULL,
    question_type = $3,
    difficulty = $4,
    points = $5,
    stem_content = $6,
    search_text = $7,
    explanation_content = $8,
    rubric_content = $9,
    tags = $10,
    status = $11,
    updated_by = $12,
    updated_at = NOW()
WHERE id = $1
  AND deleted_at IS NULL
"#,
    )
    .bind(question_id)
    .bind(payload.subject_id)
    .bind(&payload.question_type)
    .bind(&payload.difficulty)
    .bind(payload.points)
    .bind(Json(payload.stem_content.clone()))
    .bind(stem_search_text)
    .bind(payload.explanation_content.clone().map(Json))
    .bind(payload.rubric_content.clone().map(Json))
    .bind(&payload.tags)
    .bind(&payload.status)
    .bind(actor_id)
    .execute(&mut *tx)
    .await
    .map_err(|error| {
        tracing::error!("Failed to update question bank question: {}", error);
        AppError::InternalServerError("แก้ไขข้อสอบไม่สำเร็จ".to_string())
    })?;

    sqlx::query("DELETE FROM academic_question_bank_choices WHERE question_id = $1")
        .bind(question_id)
        .execute(&mut *tx)
        .await
        .map_err(|error| {
            tracing::error!("Failed to replace question choices: {}", error);
            AppError::InternalServerError("แก้ไขตัวเลือกไม่สำเร็จ".to_string())
        })?;
    insert_choices(&mut tx, question_id, &payload.choices).await?;
    finalize_temporary_files(&mut tx, actor_id, &temporary_image_file_ids).await?;
    tx.commit().await.map_err(|error| {
        tracing::error!("Failed to commit question update transaction: {}", error);
        AppError::InternalServerError("แก้ไขข้อสอบไม่สำเร็จ".to_string())
    })?;

    let access = question_bank_access_policy::resolve_access(pool, actor).await?;
    fetch_question_detail(pool, question_id, &access).await
}

pub async fn delete_question(
    pool: &PgPool,
    actor: &ActorContext,
    question_id: Uuid,
) -> Result<(), AppError> {
    let scope = fetch_question_scope(pool, question_id).await?;
    question_bank_access_policy::require_question_manage_access(pool, actor, &scope).await?;

    sqlx::query(
        r#"
UPDATE academic_question_bank_questions
SET deleted_at = NOW(), updated_at = NOW()
WHERE id = $1
  AND deleted_at IS NULL
"#,
    )
    .bind(question_id)
    .execute(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to delete question bank question: {}", error);
        AppError::InternalServerError("ลบข้อสอบไม่สำเร็จ".to_string())
    })?;

    Ok(())
}

fn normalize_page_params(page: Option<i64>, page_size: Option<i64>) -> PageParams {
    let page = page.unwrap_or(1).max(1);
    let page_size = page_size
        .unwrap_or(DEFAULT_PAGE_SIZE)
        .clamp(1, MAX_PAGE_SIZE);
    PageParams {
        page,
        page_size,
        offset: (page - 1) * page_size,
    }
}

fn normalize_payload(payload: &mut UpsertQuestionRequest) {
    payload.question_type = payload.question_type.trim().to_string();
    payload.difficulty = payload.difficulty.trim().to_string();
    payload.status = payload.status.trim().to_string();
    payload.tags = normalize_tags(&payload.tags);
    for choice in &mut payload.choices {
        choice.label = choice.label.trim().to_string();
    }
}

pub fn normalize_tags(tags: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut normalized = Vec::new();
    for tag in tags {
        let tag = tag.trim().to_lowercase();
        if !tag.is_empty() && seen.insert(tag.clone()) {
            normalized.push(tag);
        }
    }
    normalized
}

pub fn validate_question_payload(payload: &UpsertQuestionRequest) -> Result<(), AppError> {
    let kind = QuestionKind::parse(&payload.question_type)
        .ok_or_else(|| AppError::ValidationError("ประเภทข้อสอบไม่ถูกต้อง".to_string()))?;
    if !VALID_DIFFICULTIES.contains(&payload.difficulty.as_str()) {
        return Err(AppError::ValidationError("ระดับความยากไม่ถูกต้อง".to_string()));
    }
    if !VALID_STATUSES.contains(&payload.status.as_str()) {
        return Err(AppError::ValidationError("สถานะข้อสอบไม่ถูกต้อง".to_string()));
    }
    if !payload.points.is_finite() || payload.points < 0.0 || payload.points >= 10000.0 {
        return Err(AppError::ValidationError(
            "คะแนนต้องเป็นตัวเลขไม่ติดลบและไม่เกินขอบเขตที่รองรับ".to_string(),
        ));
    }
    validate_rich_content(&payload.stem_content)?;
    if !rich_content_has_body(&payload.stem_content) {
        return Err(AppError::ValidationError("ต้องระบุโจทย์".to_string()));
    }
    if let Some(content) = &payload.explanation_content {
        validate_rich_content(content)?;
    }
    if let Some(content) = &payload.rubric_content {
        validate_rich_content(content)?;
    }

    if kind.requires_choices() {
        validate_choice_payload(kind, &payload.choices)?;
    } else if !payload.choices.is_empty() {
        return Err(AppError::ValidationError(
            "ข้อสอบแบบเขียนตอบไม่รองรับตัวเลือก".to_string(),
        ));
    }

    Ok(())
}

fn validate_choice_payload(
    kind: QuestionKind,
    choices: &[UpsertQuestionChoiceRequest],
) -> Result<(), AppError> {
    if choices.len() < 2 {
        return Err(AppError::ValidationError(
            "ข้อสอบแบบตัวเลือกต้องมีอย่างน้อย 2 ตัวเลือก".to_string(),
        ));
    }

    let mut labels = HashSet::new();
    let mut correct_count = 0usize;
    for choice in choices {
        if choice.label.trim().is_empty() {
            return Err(AppError::ValidationError("ต้องระบุป้ายกำกับตัวเลือก".to_string()));
        }
        if !labels.insert(choice.label.trim().to_lowercase()) {
            return Err(AppError::ValidationError("ตัวเลือกซ้ำ".to_string()));
        }
        if !rich_content_has_body(&choice.content) {
            return Err(AppError::ValidationError("ต้องระบุเนื้อหาตัวเลือก".to_string()));
        }
        validate_rich_content(&choice.content)?;
        if choice.is_correct {
            correct_count += 1;
        }
    }

    match kind {
        QuestionKind::SingleChoice if correct_count != 1 => Err(AppError::ValidationError(
            "ข้อสอบแบบเลือกตอบต้องมีเฉลยถูก 1 ตัวเลือก".to_string(),
        )),
        QuestionKind::MultipleChoice if correct_count == 0 => Err(AppError::ValidationError(
            "ข้อสอบแบบหลายคำตอบต้องมีเฉลยอย่างน้อย 1 ตัวเลือก".to_string(),
        )),
        _ => Ok(()),
    }
}

fn rich_content_has_body(content: &RichContent) -> bool {
    content.has_body()
}

fn validate_rich_content(content: &RichContent) -> Result<(), AppError> {
    content
        .validate_shape()
        .map_err(|message| AppError::ValidationError(message.to_string()))
}

fn collect_payload_image_file_ids(payload: &UpsertQuestionRequest) -> HashSet<Uuid> {
    let mut ids: HashSet<Uuid> = payload.stem_content.image_file_ids().collect();
    if let Some(content) = &payload.explanation_content {
        ids.extend(content.image_file_ids());
    }
    if let Some(content) = &payload.rubric_content {
        ids.extend(content.image_file_ids());
    }
    for choice in &payload.choices {
        ids.extend(choice.content.image_file_ids());
    }
    ids
}

async fn validate_payload_files(
    pool: &PgPool,
    actor_id: Uuid,
    file_ids: &HashSet<Uuid>,
    existing_file_ids: &HashSet<Uuid>,
) -> Result<HashSet<Uuid>, AppError> {
    if file_ids.is_empty() {
        return Ok(HashSet::new());
    }
    let requested: Vec<Uuid> = file_ids.iter().copied().collect();
    let rows = sqlx::query_as::<_, PayloadFileRow>(
        r#"
SELECT id, user_id, mime_type, file_type, is_temporary
FROM files
WHERE id = ANY($1)
  AND deleted_at IS NULL
  AND (is_temporary = false OR expires_at IS NULL OR expires_at > NOW())
"#,
    )
    .bind(&requested)
    .fetch_all(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to validate question bank files: {}", error);
        AppError::InternalServerError("ไม่สามารถตรวจสอบรูปข้อสอบได้".to_string())
    })?;

    if rows.len() != requested.len() {
        return Err(AppError::ValidationError(
            "พบรูปข้อสอบที่ไม่มีอยู่หรือถูกลบแล้ว".to_string(),
        ));
    }
    let mut temporary_file_ids = HashSet::new();
    for row in rows {
        if !existing_file_ids.contains(&row.id) && row.user_id != Some(actor_id) {
            return Err(AppError::Forbidden("ไม่มีสิทธิ์ใช้ไฟล์รูปนี้ในข้อสอบ".to_string()));
        }
        if !row.mime_type.starts_with("image/") || row.file_type != "course_material" {
            return Err(AppError::ValidationError(
                "ไฟล์ประกอบข้อสอบต้องเป็นรูปภาพ".to_string(),
            ));
        }
        if row.is_temporary {
            if row.user_id != Some(actor_id) {
                return Err(AppError::Forbidden(
                    "ไม่สามารถยืนยันไฟล์ชั่วคราวของผู้ใช้อื่นได้".to_string(),
                ));
            }
            temporary_file_ids.insert(row.id);
        }
    }
    Ok(temporary_file_ids)
}

async fn finalize_temporary_files(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    actor_id: Uuid,
    file_ids: &HashSet<Uuid>,
) -> Result<(), AppError> {
    if file_ids.is_empty() {
        return Ok(());
    }
    let file_ids: Vec<Uuid> = file_ids.iter().copied().collect();
    let result = sqlx::query(
        r#"
UPDATE files
SET is_temporary = false,
    expires_at = NULL,
    updated_at = NOW()
WHERE id = ANY($1)
  AND user_id = $2
  AND is_temporary = true
  AND deleted_at IS NULL
"#,
    )
    .bind(&file_ids)
    .bind(actor_id)
    .execute(&mut **tx)
    .await
    .map_err(|error| {
        tracing::error!("Failed to finalize question bank files: {}", error);
        AppError::InternalServerError("ไม่สามารถยืนยันรูปข้อสอบได้".to_string())
    })?;
    if result.rows_affected() != file_ids.len() as u64 {
        return Err(AppError::ValidationError(
            "รูปข้อสอบบางไฟล์หมดอายุแล้ว กรุณาเลือกไฟล์ใหม่".to_string(),
        ));
    }
    Ok(())
}

async fn insert_choices(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    question_id: Uuid,
    choices: &[UpsertQuestionChoiceRequest],
) -> Result<(), AppError> {
    if choices.is_empty() {
        return Ok(());
    }

    let mut builder = QueryBuilder::<Postgres>::new(
        r#"
INSERT INTO academic_question_bank_choices (
    question_id,
    label,
    content,
    is_correct,
    sort_order
)
"#,
    );
    builder.push_values(choices, |mut row, choice| {
        row.push_bind(question_id)
            .push_bind(&choice.label)
            .push_bind(Json(choice.content.clone()))
            .push_bind(choice.is_correct)
            .push_bind(choice.sort_order);
    });
    builder.build().execute(&mut **tx).await.map_err(|error| {
        tracing::error!("Failed to insert question choices: {}", error);
        AppError::InternalServerError("บันทึกตัวเลือกไม่สำเร็จ".to_string())
    })?;
    Ok(())
}

async fn fetch_question_summary(
    pool: &PgPool,
    query: &QuestionBankListQuery,
    access: &QuestionBankAccess,
) -> Result<QuestionBankSummary, AppError> {
    let mut builder = QueryBuilder::<Postgres>::new(
        r#"
SELECT
    COUNT(*)::BIGINT AS total,
    COUNT(*) FILTER (WHERE q.question_type IN ('single_choice', 'multiple_choice'))::BIGINT AS choice,
    COUNT(*) FILTER (WHERE q.question_type IN ('short_answer', 'essay'))::BIGINT AS written,
    COUNT(*) FILTER (WHERE q.status = 'ready')::BIGINT AS ready
FROM academic_question_bank_questions q
LEFT JOIN subjects s ON s.id = q.subject_id
WHERE q.deleted_at IS NULL
"#,
    );
    push_list_filters(&mut builder, query, access);

    let row = builder
        .build_query_as::<QuestionBankSummaryRow>()
        .fetch_one(pool)
        .await
        .map_err(|error| {
            tracing::error!("Failed to summarize question bank questions: {}", error);
            AppError::InternalServerError("ไม่สามารถสรุปคลังข้อสอบได้".to_string())
        })?;
    Ok(QuestionBankSummary {
        total: row.total,
        choice: row.choice,
        written: row.written,
        ready: row.ready,
    })
}

async fn fetch_question_detail(
    pool: &PgPool,
    question_id: Uuid,
    access: &QuestionBankAccess,
) -> Result<QuestionDetail, AppError> {
    let mut builder = QueryBuilder::<Postgres>::new(
        r#"
SELECT
    q.id,
    q.subject_id,
    q.owner_user_id,
    q.question_type,
    q.difficulty,
    q.points,
    q.stem_content,
    q.explanation_content,
    q.rubric_content,
    q.tags,
    q.status,
    s.code AS subject_code,
    s.name_th AS subject_name_th,
    s.name_en AS subject_name_en,
    s.group_id AS subject_group_id,
    sg.name_th AS subject_group_name,
    COALESCE(choice_stats.choice_count, 0)::BIGINT AS choice_count,
    COALESCE(choice_stats.correct_choice_count, 0)::BIGINT AS correct_choice_count,
"#,
    );
    push_manage_expression(&mut builder, access);
    builder.push(
        r#" AS can_manage,
    q.created_at,
    q.updated_at
FROM academic_question_bank_questions q
LEFT JOIN subjects s ON s.id = q.subject_id
LEFT JOIN subject_groups sg ON sg.id = s.group_id
LEFT JOIN LATERAL (
    SELECT
        COUNT(*) AS choice_count,
        COUNT(*) FILTER (WHERE c.is_correct = true) AS correct_choice_count
    FROM academic_question_bank_choices c
    WHERE c.question_id = q.id
) choice_stats ON true
WHERE q.id = "#,
    );
    builder.push_bind(question_id);
    builder.push(" AND q.deleted_at IS NULL");

    let question = builder
        .build_query_as::<QuestionRow>()
        .fetch_optional(pool)
        .await
        .map_err(|error| {
            tracing::error!("Failed to fetch question detail: {}", error);
            AppError::InternalServerError("ไม่สามารถดึงข้อสอบได้".to_string())
        })?
        .ok_or_else(|| AppError::NotFound("ไม่พบข้อสอบ".to_string()))?;

    let choices = sqlx::query_as::<_, QuestionChoiceRow>(
        r#"
SELECT id, question_id, label, content, is_correct, sort_order
FROM academic_question_bank_choices
WHERE question_id = $1
ORDER BY sort_order ASC, label ASC, id ASC
"#,
    )
    .bind(question_id)
    .fetch_all(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to fetch question choices: {}", error);
        AppError::InternalServerError("ไม่สามารถดึงตัวเลือกได้".to_string())
    })?;

    let question = QuestionSummary::from(question);
    let choices: Vec<QuestionChoice> = choices.into_iter().map(QuestionChoice::from).collect();
    let file_ids = collect_question_file_ids(&question, &choices);
    let files = fetch_question_files(pool, &file_ids).await?;

    Ok(QuestionDetail {
        question,
        choices,
        files,
    })
}

async fn fetch_question_scope(
    pool: &PgPool,
    question_id: Uuid,
) -> Result<QuestionScopeRow, AppError> {
    sqlx::query_as::<_, QuestionScopeRow>(
        r#"
SELECT
    q.owner_user_id,
    q.subject_id,
    s.group_id AS subject_group_id
FROM academic_question_bank_questions q
LEFT JOIN subjects s ON s.id = q.subject_id
WHERE q.id = $1
  AND q.deleted_at IS NULL
"#,
    )
    .bind(question_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to fetch question scope: {}", error);
        AppError::InternalServerError("ไม่สามารถตรวจสอบสิทธิ์ข้อสอบได้".to_string())
    })?
    .ok_or_else(|| AppError::NotFound("ไม่พบข้อสอบ".to_string()))
}

async fn fetch_question_file_ids(
    pool: &PgPool,
    question_id: Uuid,
) -> Result<HashSet<Uuid>, AppError> {
    let question = sqlx::query_as::<
        _,
        (
            Json<RichContent>,
            Option<Json<RichContent>>,
            Option<Json<RichContent>>,
        ),
    >(
        r#"
SELECT stem_content, explanation_content, rubric_content
FROM academic_question_bank_questions
WHERE id = $1 AND deleted_at IS NULL
"#,
    )
    .bind(question_id)
    .fetch_one(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to fetch existing question files: {}", error);
        AppError::InternalServerError("ไม่สามารถตรวจสอบรูปเดิมของข้อสอบได้".to_string())
    })?;
    let choices = sqlx::query_scalar::<_, Json<RichContent>>(
        "SELECT content FROM academic_question_bank_choices WHERE question_id = $1",
    )
    .bind(question_id)
    .fetch_all(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to fetch existing choice files: {}", error);
        AppError::InternalServerError("ไม่สามารถตรวจสอบรูปเดิมของตัวเลือกได้".to_string())
    })?;

    let mut ids: HashSet<Uuid> = question.0.image_file_ids().collect();
    if let Some(content) = question.1 {
        ids.extend(content.0.image_file_ids());
    }
    if let Some(content) = question.2 {
        ids.extend(content.0.image_file_ids());
    }
    for content in choices {
        ids.extend(content.0.image_file_ids());
    }
    Ok(ids)
}

fn collect_question_file_ids(
    question: &QuestionSummary,
    choices: &[QuestionChoice],
) -> HashSet<Uuid> {
    let mut ids: HashSet<Uuid> = question.stem_content.image_file_ids().collect();
    if let Some(content) = &question.explanation_content {
        ids.extend(content.image_file_ids());
    }
    if let Some(content) = &question.rubric_content {
        ids.extend(content.image_file_ids());
    }
    for choice in choices {
        ids.extend(choice.content.image_file_ids());
    }
    ids
}

async fn fetch_question_files(
    pool: &PgPool,
    file_ids: &HashSet<Uuid>,
) -> Result<Vec<QuestionFile>, AppError> {
    if file_ids.is_empty() {
        return Ok(Vec::new());
    }
    let file_ids: Vec<Uuid> = file_ids.iter().copied().collect();
    let rows = sqlx::query_as::<_, (Uuid, String, Option<String>)>(
        r#"
SELECT id, storage_path, thumbnail_path
FROM files
WHERE id = ANY($1)
  AND deleted_at IS NULL
"#,
    )
    .bind(&file_ids)
    .fetch_all(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to fetch question file URLs: {}", error);
        AppError::InternalServerError("ไม่สามารถดึงรูปประกอบข้อสอบได้".to_string())
    })?;
    let url_builder = FileUrlBuilder::new().map_err(|error| {
        tracing::error!("Failed to build question file URLs: {}", error);
        AppError::InternalServerError("ไม่สามารถสร้าง URL รูปข้อสอบได้".to_string())
    })?;
    Ok(rows
        .into_iter()
        .map(|(id, storage_path, thumbnail_path)| QuestionFile {
            id,
            url: url_builder.build_url(&storage_path),
            thumbnail_url: thumbnail_path.map(|path| url_builder.build_url(&path)),
        })
        .collect())
}

fn push_list_filters(
    builder: &mut QueryBuilder<'_, Postgres>,
    query: &QuestionBankListQuery,
    access: &QuestionBankAccess,
) {
    if let Some(subject_id) = query.subject_id {
        builder.push(" AND q.subject_id = ");
        builder.push_bind(subject_id);
    }
    if let Some(question_type) =
        valid_filter_value(query.question_type.as_deref(), VALID_QUESTION_TYPES)
    {
        builder.push(" AND q.question_type = ");
        builder.push_bind(question_type.to_string());
    }
    if let Some(difficulty) = valid_filter_value(query.difficulty.as_deref(), VALID_DIFFICULTIES) {
        builder.push(" AND q.difficulty = ");
        builder.push_bind(difficulty.to_string());
    }
    if let Some(status) = valid_filter_value(query.status.as_deref(), VALID_STATUSES) {
        builder.push(" AND q.status = ");
        builder.push_bind(status.to_string());
    }
    if let Some(tag) = query
        .tag
        .as_ref()
        .map(|tag| tag.trim().to_lowercase())
        .filter(|tag| !tag.is_empty())
    {
        builder.push(" AND ");
        builder.push_bind(tag);
        builder.push(" = ANY(q.tags)");
    }
    if let Some(search) = query
        .search
        .as_ref()
        .map(|search| search.trim())
        .filter(|search| !search.is_empty())
    {
        let pattern = format!("%{}%", search);
        builder.push(" AND (q.search_text ILIKE ");
        builder.push_bind(pattern.clone());
        builder.push(" OR s.code ILIKE ");
        builder.push_bind(pattern.clone());
        builder.push(" OR s.name_th ILIKE ");
        builder.push_bind(pattern.clone());
        builder.push(" OR s.name_en ILIKE ");
        builder.push_bind(pattern);
        builder.push(")");
    }

    if !access.read_school {
        builder.push(" AND (");
        push_read_expression(builder, access);
        builder.push(")");
    }
}

fn push_read_expression(builder: &mut QueryBuilder<'_, Postgres>, access: &QuestionBankAccess) {
    let mut has_predicate = false;
    if let Some(actor_id) = access.read_assigned_user_id {
        builder.push("(q.owner_user_id = ");
        builder.push_bind(actor_id);
        builder.push(" OR EXISTS (SELECT 1 FROM classroom_courses cc JOIN classroom_course_instructors cci ON cci.classroom_course_id = cc.id WHERE cc.subject_id = q.subject_id AND cci.instructor_id = ");
        builder.push_bind(actor_id);
        builder.push("))");
        has_predicate = true;
    }
    if !access.read_subject_group_ids.is_empty() {
        if has_predicate {
            builder.push(" OR ");
        }
        builder.push("s.group_id = ANY(");
        builder.push_bind(access.read_subject_group_ids.clone());
        builder.push(")");
        has_predicate = true;
    }
    if !has_predicate {
        builder.push("FALSE");
    }
}

fn push_manage_expression(builder: &mut QueryBuilder<'_, Postgres>, access: &QuestionBankAccess) {
    if access.manage_school {
        builder.push("TRUE");
        return;
    }

    let mut has_predicate = false;
    builder.push("(");
    if let Some(actor_id) = access.manage_assigned_user_id {
        builder.push("q.owner_user_id = ");
        builder.push_bind(actor_id);
        has_predicate = true;
    }
    if !access.manage_subject_group_ids.is_empty() {
        if has_predicate {
            builder.push(" OR ");
        }
        builder.push("s.group_id = ANY(");
        builder.push_bind(access.manage_subject_group_ids.clone());
        builder.push(")");
        has_predicate = true;
    }
    if !has_predicate {
        builder.push("FALSE");
    }
    builder.push(")");
}

fn push_subject_read_expression(
    builder: &mut QueryBuilder<'_, Postgres>,
    access: &QuestionBankAccess,
) {
    if access.read_school {
        builder.push("TRUE");
        return;
    }

    let mut has_predicate = false;
    if let Some(actor_id) = access.read_assigned_user_id {
        builder.push("(EXISTS (SELECT 1 FROM academic_question_bank_questions q WHERE q.subject_id = s.id AND q.owner_user_id = ");
        builder.push_bind(actor_id);
        builder.push(" AND q.deleted_at IS NULL) OR EXISTS (SELECT 1 FROM classroom_courses cc JOIN classroom_course_instructors cci ON cci.classroom_course_id = cc.id WHERE cc.subject_id = s.id AND cci.instructor_id = ");
        builder.push_bind(actor_id);
        builder.push("))");
        has_predicate = true;
    }
    if !access.read_subject_group_ids.is_empty() {
        if has_predicate {
            builder.push(" OR ");
        }
        builder.push("s.group_id = ANY(");
        builder.push_bind(access.read_subject_group_ids.clone());
        builder.push(")");
        has_predicate = true;
    }
    if !has_predicate {
        builder.push("FALSE");
    }
}

fn push_subject_manage_expression(
    builder: &mut QueryBuilder<'_, Postgres>,
    access: &QuestionBankAccess,
) {
    if access.manage_school {
        builder.push("TRUE");
        return;
    }

    let mut has_predicate = false;
    builder.push("(");
    if let Some(actor_id) = access.manage_assigned_user_id {
        builder.push("EXISTS (SELECT 1 FROM classroom_courses cc JOIN classroom_course_instructors cci ON cci.classroom_course_id = cc.id WHERE cc.subject_id = s.id AND cci.instructor_id = ");
        builder.push_bind(actor_id);
        builder.push(")");
        has_predicate = true;
    }
    if !access.manage_subject_group_ids.is_empty() {
        if has_predicate {
            builder.push(" OR ");
        }
        builder.push("s.group_id = ANY(");
        builder.push_bind(access.manage_subject_group_ids.clone());
        builder.push(")");
        has_predicate = true;
    }
    if !has_predicate {
        builder.push("FALSE");
    }
    builder.push(")");
}

fn valid_filter_value<'a>(value: Option<&'a str>, valid_values: &[&str]) -> Option<&'a str> {
    value
        .map(str::trim)
        .filter(|value| valid_values.contains(value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::question_bank::models::{
        ImageAlignment, ImageNodeAttributes, MathNodeAttributes, RichBlockNode, RichDocument,
        RichInlineNode, RICH_CONTENT_SCHEMA_VERSION,
    };

    fn text_content(text: &str) -> RichContent {
        RichContent {
            schema_version: RICH_CONTENT_SCHEMA_VERSION,
            document: RichDocument::Doc {
                content: vec![RichBlockNode::Paragraph {
                    content: vec![RichInlineNode::Text {
                        text: text.to_string(),
                        marks: vec![],
                    }],
                }],
            },
        }
    }

    fn math_content(latex: &str) -> RichContent {
        RichContent {
            schema_version: RICH_CONTENT_SCHEMA_VERSION,
            document: RichDocument::Doc {
                content: vec![RichBlockNode::Paragraph {
                    content: vec![RichInlineNode::InlineMath {
                        attrs: MathNodeAttributes {
                            latex: latex.to_string(),
                        },
                    }],
                }],
            },
        }
    }

    fn image_block(file_id: Uuid) -> RichBlockNode {
        RichBlockNode::Image {
            attrs: ImageNodeAttributes {
                file_id,
                alt_text: None,
                caption: None,
                alignment: ImageAlignment::Center,
                width_percent: 60,
            },
        }
    }

    fn push_block(content: &mut RichContent, block: RichBlockNode) {
        let RichDocument::Doc { content } = &mut content.document;
        content.push(block);
    }

    fn valid_choice(label: &str, correct: bool) -> UpsertQuestionChoiceRequest {
        UpsertQuestionChoiceRequest {
            id: None,
            label: label.to_string(),
            content: text_content(label),
            is_correct: correct,
            sort_order: 0,
        }
    }

    fn base_payload(question_type: &str) -> UpsertQuestionRequest {
        UpsertQuestionRequest {
            subject_id: Uuid::new_v4(),
            question_type: question_type.to_string(),
            difficulty: "medium".to_string(),
            points: 1.0,
            stem_content: text_content("โจทย์"),
            explanation_content: None,
            rubric_content: None,
            tags: vec![],
            status: "draft".to_string(),
            choices: vec![],
        }
    }

    fn assigned_access(actor_id: Uuid) -> QuestionBankAccess {
        QuestionBankAccess {
            read_school: false,
            read_assigned_user_id: Some(actor_id),
            read_subject_group_ids: Vec::new(),
            manage_school: false,
            manage_assigned_user_id: Some(actor_id),
            manage_subject_group_ids: Vec::new(),
        }
    }

    #[test]
    fn rich_content_accepts_math_as_body() {
        assert!(rich_content_has_body(&math_content("\\\\frac{x}{2}")));
    }

    #[test]
    fn rich_content_search_text_combines_text_math_and_image_metadata() {
        let mut content = text_content("จากสมการ");
        let RichDocument::Doc { content: blocks } = &mut content.document;
        let RichBlockNode::Paragraph { content: inline } = &mut blocks[0] else {
            panic!("expected paragraph")
        };
        inline.push(RichInlineNode::InlineMath {
            attrs: MathNodeAttributes {
                latex: "x=1-2x".to_string(),
            },
        });
        inline.push(RichInlineNode::Text {
            text: " x มีค่าเท่าใด".to_string(),
            marks: vec![],
        });
        assert_eq!(content.search_text(), "จากสมการ x=1-2x x มีค่าเท่าใด");
    }

    #[test]
    fn rich_content_deserializes_the_versioned_editor_contract() {
        let content: RichContent = serde_json::from_value(serde_json::json!({
            "schemaVersion": 1,
            "document": {
                "type": "doc",
                "content": [{
                    "type": "paragraph",
                    "content": [
                        { "type": "text", "text": "จากสมการ " },
                        { "type": "inline_math", "attrs": { "latex": "x=1-2x" } },
                        { "type": "text", "text": " x มีค่าเท่าใด", "marks": [{ "type": "bold" }] }
                    ]
                }]
            }
        }))
        .expect("versioned rich content should deserialize");

        assert!(content.validate_shape().is_ok());
        assert_eq!(content.search_text(), "จากสมการ x=1-2x x มีค่าเท่าใด");
    }

    #[test]
    fn rich_content_rejects_editor_only_image_attributes() {
        let result = serde_json::from_value::<RichContent>(serde_json::json!({
            "schemaVersion": 1,
            "document": {
                "type": "doc",
                "content": [{
                    "type": "image",
                    "attrs": {
                        "fileId": Uuid::new_v4(),
                        "pendingId": "browser-only",
                        "previewUrl": "blob:browser-only",
                        "altText": null,
                        "caption": null,
                        "alignment": "center",
                        "widthPercent": 60
                    }
                }]
            }
        }));

        assert!(result.is_err());
    }

    #[test]
    fn normalize_tags_trims_lowercases_and_dedupes() {
        let tags = vec![
            " Algebra ".to_string(),
            "algebra".to_string(),
            "".to_string(),
            "Graph".to_string(),
        ];
        assert_eq!(normalize_tags(&tags), vec!["algebra", "graph"]);
    }

    #[test]
    fn single_choice_requires_exactly_one_correct_choice() {
        let mut payload = base_payload("single_choice");
        payload.choices = vec![valid_choice("A", true), valid_choice("B", true)];
        assert!(validate_question_payload(&payload).is_err());

        payload.choices = vec![valid_choice("A", true), valid_choice("B", false)];
        assert!(validate_question_payload(&payload).is_ok());
    }

    #[test]
    fn written_question_rejects_choices() {
        let mut payload = base_payload("essay");
        payload.choices = vec![valid_choice("A", true), valid_choice("B", false)];
        assert!(validate_question_payload(&payload).is_err());
    }

    #[test]
    fn empty_stem_is_rejected() {
        let mut payload = base_payload("short_answer");
        payload.stem_content = text_content("   ");
        assert!(validate_question_payload(&payload).is_err());
    }

    #[test]
    fn page_params_are_bounded_and_positive() {
        assert_eq!(
            normalize_page_params(Some(-4), Some(500)),
            PageParams {
                page: 1,
                page_size: 100,
                offset: 0,
            }
        );
    }

    #[test]
    fn assigned_scope_uses_team_teaching_junction() {
        let mut builder = QueryBuilder::<Postgres>::new("");
        push_read_expression(&mut builder, &assigned_access(Uuid::new_v4()));
        assert!(builder.sql().contains("classroom_course_instructors"));
        assert!(!builder.sql().contains("primary_instructor_id"));
    }

    #[test]
    fn payload_collects_images_from_stem_and_choices() {
        let stem_file_id = Uuid::new_v4();
        let choice_file_id = Uuid::new_v4();
        let mut payload = base_payload("single_choice");
        push_block(&mut payload.stem_content, image_block(stem_file_id));
        let mut choice = valid_choice("A", true);
        push_block(&mut choice.content, image_block(choice_file_id));
        payload.choices = vec![choice, valid_choice("B", false)];

        let ids = collect_payload_image_file_ids(&payload);
        assert!(ids.contains(&stem_file_id));
        assert!(ids.contains(&choice_file_id));
    }
}
