use std::collections::HashSet;

use sqlx::types::Json;
use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

use crate::error::AppError;
use crate::middleware::permission::ActorContext;
use crate::modules::question_bank::models::{
    QuestionBankListQuery, QuestionChoice, QuestionChoiceRow, QuestionDetail, QuestionRow,
    QuestionScopeRow, QuestionSummary, RichContent, RichContentBlock, UpsertQuestionChoiceRequest,
    UpsertQuestionRequest,
};
use crate::permissions::registry::codes;
use crate::policies::resource_access_policy::{self, UserResourceListAccess};

const VALID_QUESTION_TYPES: &[&str] =
    &["single_choice", "multiple_choice", "short_answer", "essay"];
const VALID_DIFFICULTIES: &[&str] = &["easy", "medium", "hard"];
const VALID_STATUSES: &[&str] = &["draft", "ready", "archived"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestionBankListAccess {
    pub read_school: bool,
    pub read_assigned_user_id: Option<Uuid>,
    pub read_subject_group_ids: Vec<Uuid>,
    pub manage_school: bool,
    pub manage_assigned_user_id: Option<Uuid>,
    pub manage_subject_group_ids: Vec<Uuid>,
}

impl QuestionBankListAccess {
    fn scoped(
        read_assigned_user_id: Option<Uuid>,
        read_subject_group_ids: Vec<Uuid>,
        manage_assigned_user_id: Option<Uuid>,
        manage_subject_group_ids: Vec<Uuid>,
    ) -> Self {
        Self {
            read_school: false,
            read_assigned_user_id,
            read_subject_group_ids,
            manage_school: false,
            manage_assigned_user_id,
            manage_subject_group_ids,
        }
    }
}

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

pub async fn resolve_question_bank_list_access(
    pool: &PgPool,
    actor: &ActorContext,
) -> Result<QuestionBankListAccess, AppError> {
    let can_manage_assigned = can_manage_assigned(actor);
    let manage_assigned_user_id = can_manage_assigned.then_some(actor.user_id);
    let manage_subject_group_ids = if can_manage_subject_group(actor) {
        actor_subject_group_ids(pool, actor.user_id).await?
    } else {
        Vec::new()
    };

    if can_read_school(actor) {
        return Ok(QuestionBankListAccess {
            read_school: true,
            read_assigned_user_id: None,
            read_subject_group_ids: Vec::new(),
            manage_school: can_manage_school(actor),
            manage_assigned_user_id,
            manage_subject_group_ids,
        });
    }

    actor.require_any_permission(&[
        codes::ACADEMIC_QUESTION_BANK_READ_ASSIGNED,
        codes::ACADEMIC_QUESTION_BANK_READ_ORGANIZATION_UNIT,
        codes::ACADEMIC_QUESTION_BANK_MANAGE_ASSIGNED,
        codes::ACADEMIC_QUESTION_BANK_MANAGE_ORGANIZATION_UNIT,
    ])?;

    let read_assigned_user_id = can_read_assigned(actor).then_some(actor.user_id);
    let read_subject_group_ids = if can_read_subject_group(actor) {
        actor_subject_group_ids(pool, actor.user_id).await?
    } else {
        Vec::new()
    };

    Ok(QuestionBankListAccess::scoped(
        read_assigned_user_id,
        read_subject_group_ids,
        manage_assigned_user_id,
        manage_subject_group_ids,
    ))
}

pub async fn list_questions(
    pool: &PgPool,
    query: &QuestionBankListQuery,
    access: &QuestionBankListAccess,
) -> Result<Vec<QuestionSummary>, AppError> {
    let mut builder = QueryBuilder::<Postgres>::new(
        r#"
SELECT
    q.id,
    q.subject_id,
    q.grade_level_id,
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
    gl.level_type AS grade_level_type,
    gl.year AS grade_level_year,
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
LEFT JOIN grade_levels gl ON gl.id = q.grade_level_id
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
    builder.push(" ORDER BY q.updated_at DESC, q.created_at DESC, q.id DESC");

    let rows = builder
        .build_query_as::<QuestionRow>()
        .fetch_all(pool)
        .await
        .map_err(|error| {
            tracing::error!("Failed to list question bank questions: {}", error);
            AppError::InternalServerError("ไม่สามารถดึงรายการข้อสอบได้".to_string())
        })?;

    Ok(rows.into_iter().map(QuestionSummary::from).collect())
}

pub async fn get_question(
    pool: &PgPool,
    actor: &ActorContext,
    question_id: Uuid,
) -> Result<QuestionDetail, AppError> {
    let scope = fetch_question_scope(pool, question_id).await?;
    require_question_read_access(pool, actor, &scope).await?;
    let access = question_access_for_actor(pool, actor).await?;
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
    require_create_access(pool, actor, payload.subject_id).await?;

    let mut tx = pool.begin().await.map_err(|error| {
        tracing::error!("Failed to start question create transaction: {}", error);
        AppError::InternalServerError("ไม่สามารถเริ่มบันทึกข้อสอบได้".to_string())
    })?;

    let question_id = sqlx::query_scalar::<_, Uuid>(
        r#"
INSERT INTO academic_question_bank_questions (
    subject_id,
    grade_level_id,
    owner_user_id,
    question_type,
    difficulty,
    points,
    stem_content,
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
    .bind(payload.grade_level_id)
    .bind(actor_id)
    .bind(&payload.question_type)
    .bind(&payload.difficulty)
    .bind(payload.points)
    .bind(Json(payload.stem_content.clone()))
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
    tx.commit().await.map_err(|error| {
        tracing::error!("Failed to commit question create transaction: {}", error);
        AppError::InternalServerError("บันทึกข้อสอบไม่สำเร็จ".to_string())
    })?;

    let access = question_access_for_actor(pool, actor).await?;
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
    require_question_manage_access(pool, actor, &scope).await?;
    require_create_access(pool, actor, payload.subject_id).await?;

    let mut tx = pool.begin().await.map_err(|error| {
        tracing::error!("Failed to start question update transaction: {}", error);
        AppError::InternalServerError("ไม่สามารถเริ่มแก้ไขข้อสอบได้".to_string())
    })?;

    sqlx::query(
        r#"
UPDATE academic_question_bank_questions
SET
    subject_id = $2,
    grade_level_id = $3,
    question_type = $4,
    difficulty = $5,
    points = $6,
    stem_content = $7,
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
    .bind(payload.grade_level_id)
    .bind(&payload.question_type)
    .bind(&payload.difficulty)
    .bind(payload.points)
    .bind(Json(payload.stem_content.clone()))
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
    tx.commit().await.map_err(|error| {
        tracing::error!("Failed to commit question update transaction: {}", error);
        AppError::InternalServerError("แก้ไขข้อสอบไม่สำเร็จ".to_string())
    })?;

    let access = question_access_for_actor(pool, actor).await?;
    fetch_question_detail(pool, question_id, &access).await
}

pub async fn delete_question(
    pool: &PgPool,
    actor: &ActorContext,
    question_id: Uuid,
) -> Result<(), AppError> {
    let scope = fetch_question_scope(pool, question_id).await?;
    require_question_manage_access(pool, actor, &scope).await?;

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
    if !rich_content_has_body(&payload.stem_content) {
        return Err(AppError::ValidationError("ต้องระบุโจทย์".to_string()));
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
    content.blocks.iter().any(|block| match block {
        RichContentBlock::Paragraph { text } => !text.trim().is_empty(),
        RichContentBlock::Math { latex, .. } => !latex.trim().is_empty(),
        RichContentBlock::Image { .. } => true,
    })
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

async fn fetch_question_detail(
    pool: &PgPool,
    question_id: Uuid,
    access: &QuestionBankListAccess,
) -> Result<QuestionDetail, AppError> {
    let mut builder = QueryBuilder::<Postgres>::new(
        r#"
SELECT
    q.id,
    q.subject_id,
    q.grade_level_id,
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
    gl.level_type AS grade_level_type,
    gl.year AS grade_level_year,
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
LEFT JOIN grade_levels gl ON gl.id = q.grade_level_id
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

    Ok(QuestionDetail {
        question: QuestionSummary::from(question),
        choices: choices.into_iter().map(QuestionChoice::from).collect(),
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

async fn require_create_access(
    pool: &PgPool,
    actor: &ActorContext,
    subject_id: Option<Uuid>,
) -> Result<(), AppError> {
    if can_manage_school(actor) || can_manage_assigned(actor) {
        return Ok(());
    }

    actor.require_permission(codes::ACADEMIC_QUESTION_BANK_MANAGE_ORGANIZATION_UNIT)?;
    let Some(subject_id) = subject_id else {
        return Err(AppError::Forbidden(
            "ต้องเลือกรายวิชาสำหรับสิทธิ์ระดับกลุ่มสาระ".to_string(),
        ));
    };
    if subject_is_accessible_by_actor_unit(pool, subject_id, actor.user_id).await? {
        return Ok(());
    }
    Err(AppError::Forbidden(
        "ไม่มีสิทธิ์จัดการข้อสอบของรายวิชานี้".to_string(),
    ))
}

async fn require_question_read_access(
    pool: &PgPool,
    actor: &ActorContext,
    scope: &QuestionScopeRow,
) -> Result<(), AppError> {
    if can_read_school(actor) {
        return Ok(());
    }
    if can_read_assigned(actor)
        && (scope.owner_user_id == actor.user_id
            || subject_is_assigned_to_actor(pool, scope.subject_id, actor.user_id).await?)
    {
        return Ok(());
    }
    if can_read_subject_group(actor)
        && subject_group_is_accessible(pool, scope.subject_group_id, actor.user_id).await?
    {
        return Ok(());
    }
    Err(AppError::Forbidden("ไม่มีสิทธิ์ดูข้อสอบนี้".to_string()))
}

async fn require_question_manage_access(
    pool: &PgPool,
    actor: &ActorContext,
    scope: &QuestionScopeRow,
) -> Result<(), AppError> {
    if can_manage_school(actor) {
        return Ok(());
    }
    if can_manage_assigned(actor) && scope.owner_user_id == actor.user_id {
        return Ok(());
    }
    if can_manage_subject_group(actor)
        && subject_group_is_accessible(pool, scope.subject_group_id, actor.user_id).await?
    {
        return Ok(());
    }
    Err(AppError::Forbidden("ไม่มีสิทธิ์จัดการข้อสอบนี้".to_string()))
}

async fn question_access_for_actor(
    pool: &PgPool,
    actor: &ActorContext,
) -> Result<QuestionBankListAccess, AppError> {
    resolve_question_bank_list_access(pool, actor).await
}

fn push_list_filters(
    builder: &mut QueryBuilder<'_, Postgres>,
    query: &QuestionBankListQuery,
    access: &QuestionBankListAccess,
) {
    if let Some(subject_id) = query.subject_id {
        builder.push(" AND q.subject_id = ");
        builder.push_bind(subject_id);
    }
    if let Some(grade_level_id) = query.grade_level_id {
        builder.push(" AND q.grade_level_id = ");
        builder.push_bind(grade_level_id);
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
        builder.push(" AND (q.stem_content::text ILIKE ");
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

fn push_read_expression(builder: &mut QueryBuilder<'_, Postgres>, access: &QuestionBankListAccess) {
    let mut has_predicate = false;
    if let Some(actor_id) = access.read_assigned_user_id {
        builder.push("(");
        builder.push("q.owner_user_id = ");
        builder.push_bind(actor_id);
        builder.push(" OR EXISTS (SELECT 1 FROM classroom_courses cc WHERE cc.subject_id = q.subject_id AND cc.primary_instructor_id = ");
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

fn push_manage_expression(
    builder: &mut QueryBuilder<'_, Postgres>,
    access: &QuestionBankListAccess,
) {
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

fn valid_filter_value<'a>(value: Option<&'a str>, valid_values: &[&str]) -> Option<&'a str> {
    value
        .map(str::trim)
        .filter(|value| valid_values.contains(value))
}

fn can_read_assigned(actor: &ActorContext) -> bool {
    actor.has_any_permission(&[
        codes::ACADEMIC_QUESTION_BANK_READ_ASSIGNED,
        codes::ACADEMIC_QUESTION_BANK_MANAGE_ASSIGNED,
    ])
}

fn can_read_subject_group(actor: &ActorContext) -> bool {
    actor.has_any_permission(&[
        codes::ACADEMIC_QUESTION_BANK_READ_ORGANIZATION_UNIT,
        codes::ACADEMIC_QUESTION_BANK_MANAGE_ORGANIZATION_UNIT,
    ])
}

fn can_read_school(actor: &ActorContext) -> bool {
    actor.has_any_permission(&[
        codes::ACADEMIC_QUESTION_BANK_READ_SCHOOL,
        codes::ACADEMIC_QUESTION_BANK_MANAGE_SCHOOL,
    ])
}

fn can_manage_assigned(actor: &ActorContext) -> bool {
    actor.has_permission(codes::ACADEMIC_QUESTION_BANK_MANAGE_ASSIGNED)
}

fn can_manage_subject_group(actor: &ActorContext) -> bool {
    actor.has_permission(codes::ACADEMIC_QUESTION_BANK_MANAGE_ORGANIZATION_UNIT)
}

fn can_manage_school(actor: &ActorContext) -> bool {
    actor.has_permission(codes::ACADEMIC_QUESTION_BANK_MANAGE_SCHOOL)
}

async fn actor_subject_group_ids(pool: &PgPool, actor_id: Uuid) -> Result<Vec<Uuid>, AppError> {
    let Some(organization_unit_ids) = resource_access_policy::accessible_organization_unit_ids(
        pool,
        UserResourceListAccess::OrganizationUnit(actor_id),
    )
    .await?
    else {
        return Ok(Vec::new());
    };

    if organization_unit_ids.is_empty() {
        return Ok(Vec::new());
    }

    sqlx::query_scalar(
        r#"
SELECT DISTINCT subject_group_id
FROM organization_units
WHERE id = ANY($1)
  AND subject_group_id IS NOT NULL
  AND is_active = true
"#,
    )
    .bind(&organization_unit_ids)
    .fetch_all(pool)
    .await
    .map_err(|error| {
        tracing::error!(
            "Failed to fetch question bank subject group access: {}",
            error
        );
        AppError::InternalServerError("ไม่สามารถตรวจสอบกลุ่มสาระได้".to_string())
    })
}

async fn subject_is_accessible_by_actor_unit(
    pool: &PgPool,
    subject_id: Uuid,
    actor_id: Uuid,
) -> Result<bool, AppError> {
    let subject_group_id = subject_group_id_for_subject(pool, subject_id).await?;
    subject_group_is_accessible(pool, subject_group_id, actor_id).await
}

async fn subject_group_is_accessible(
    pool: &PgPool,
    subject_group_id: Option<Uuid>,
    actor_id: Uuid,
) -> Result<bool, AppError> {
    let Some(subject_group_id) = subject_group_id else {
        return Ok(false);
    };
    let subject_group_ids = actor_subject_group_ids(pool, actor_id).await?;
    Ok(subject_group_ids.contains(&subject_group_id))
}

async fn subject_group_id_for_subject(
    pool: &PgPool,
    subject_id: Uuid,
) -> Result<Option<Uuid>, AppError> {
    sqlx::query_scalar("SELECT group_id FROM subjects WHERE id = $1")
        .bind(subject_id)
        .fetch_optional(pool)
        .await
        .map_err(|error| {
            tracing::error!("Failed to fetch subject group: {}", error);
            AppError::InternalServerError("ไม่สามารถตรวจสอบรายวิชาได้".to_string())
        })
        .map(|value| value.flatten())
}

async fn subject_is_assigned_to_actor(
    pool: &PgPool,
    subject_id: Option<Uuid>,
    actor_id: Uuid,
) -> Result<bool, AppError> {
    let Some(subject_id) = subject_id else {
        return Ok(false);
    };
    sqlx::query_scalar(
        r#"
SELECT EXISTS(
    SELECT 1
    FROM classroom_courses cc
    WHERE cc.subject_id = $1
      AND cc.primary_instructor_id = $2
)
"#,
    )
    .bind(subject_id)
    .bind(actor_id)
    .fetch_one(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to check assigned question subject: {}", error);
        AppError::InternalServerError("ไม่สามารถตรวจสอบรายวิชาที่รับผิดชอบได้".to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_content(text: &str) -> RichContent {
        RichContent {
            blocks: vec![RichContentBlock::Paragraph {
                text: text.to_string(),
            }],
        }
    }

    fn math_content(latex: &str) -> RichContent {
        RichContent {
            blocks: vec![RichContentBlock::Math {
                latex: latex.to_string(),
                display: true,
            }],
        }
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
            subject_id: None,
            grade_level_id: None,
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

    #[test]
    fn rich_content_accepts_math_as_body() {
        assert!(rich_content_has_body(&math_content("\\\\frac{x}{2}")));
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
}
