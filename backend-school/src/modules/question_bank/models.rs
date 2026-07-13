use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RichContent {
    pub blocks: Vec<RichContentBlock>,
}

impl RichContent {
    pub fn image_file_ids(&self) -> impl Iterator<Item = Uuid> + '_ {
        self.blocks.iter().filter_map(|block| match block {
            RichContentBlock::Image { file_id, .. } => Some(*file_id),
            _ => None,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RichContentBlock {
    Paragraph {
        text: String,
    },
    Math {
        latex: String,
        display: bool,
    },
    Image {
        #[serde(rename = "fileId")]
        file_id: Uuid,
        #[serde(rename = "altText")]
        alt_text: Option<String>,
        caption: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestionBankListQuery {
    pub subject_id: Option<Uuid>,
    pub question_type: Option<String>,
    pub difficulty: Option<String>,
    pub status: Option<String>,
    pub tag: Option<String>,
    pub search: Option<String>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertQuestionRequest {
    pub subject_id: Uuid,
    pub question_type: String,
    pub difficulty: String,
    pub points: f64,
    pub stem_content: RichContent,
    pub explanation_content: Option<RichContent>,
    pub rubric_content: Option<RichContent>,
    pub tags: Vec<String>,
    pub status: String,
    pub choices: Vec<UpsertQuestionChoiceRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertQuestionChoiceRequest {
    pub id: Option<Uuid>,
    pub label: String,
    pub content: RichContent,
    pub is_correct: bool,
    pub sort_order: i32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestionChoice {
    pub id: Uuid,
    pub question_id: Uuid,
    pub label: String,
    pub content: RichContent,
    pub is_correct: bool,
    pub sort_order: i32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestionSummary {
    pub id: Uuid,
    // Legacy rows created before migration 024 may remain unassigned until repaired.
    pub subject_id: Option<Uuid>,
    pub owner_user_id: Uuid,
    pub question_type: String,
    pub difficulty: String,
    pub points: f64,
    pub stem_content: RichContent,
    pub explanation_content: Option<RichContent>,
    pub rubric_content: Option<RichContent>,
    pub tags: Vec<String>,
    pub status: String,
    pub subject_code: Option<String>,
    pub subject_name_th: Option<String>,
    pub subject_name_en: Option<String>,
    pub subject_group_id: Option<Uuid>,
    pub subject_group_name: Option<String>,
    pub choice_count: i64,
    pub correct_choice_count: i64,
    pub can_manage: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestionDetail {
    #[serde(flatten)]
    pub question: QuestionSummary,
    pub choices: Vec<QuestionChoice>,
    pub files: Vec<QuestionFile>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestionFile {
    pub id: Uuid,
    pub url: String,
    pub thumbnail_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestionBankSummary {
    pub total: i64,
    pub choice: i64,
    pub written: i64,
    pub ready: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestionBankPage {
    pub items: Vec<QuestionSummary>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
    pub total_pages: i64,
    pub summary: QuestionBankSummary,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct QuestionBankSubjectOption {
    pub id: Uuid,
    pub code: String,
    pub name_th: String,
    pub name_en: Option<String>,
    pub subject_group_id: Option<Uuid>,
    pub subject_group_name: Option<String>,
    pub can_create: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestionBankOptions {
    pub subjects: Vec<QuestionBankSubjectOption>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct QuestionRow {
    pub id: Uuid,
    pub subject_id: Option<Uuid>,
    pub owner_user_id: Uuid,
    pub question_type: String,
    pub difficulty: String,
    pub points: f64,
    pub stem_content: Json<RichContent>,
    pub explanation_content: Option<Json<RichContent>>,
    pub rubric_content: Option<Json<RichContent>>,
    pub tags: Vec<String>,
    pub status: String,
    pub subject_code: Option<String>,
    pub subject_name_th: Option<String>,
    pub subject_name_en: Option<String>,
    pub subject_group_id: Option<Uuid>,
    pub subject_group_name: Option<String>,
    pub choice_count: i64,
    pub correct_choice_count: i64,
    pub can_manage: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct QuestionChoiceRow {
    pub id: Uuid,
    pub question_id: Uuid,
    pub label: String,
    pub content: Json<RichContent>,
    pub is_correct: bool,
    pub sort_order: i32,
}

impl From<QuestionRow> for QuestionSummary {
    fn from(row: QuestionRow) -> Self {
        Self {
            id: row.id,
            subject_id: row.subject_id,
            owner_user_id: row.owner_user_id,
            question_type: row.question_type,
            difficulty: row.difficulty,
            points: row.points,
            stem_content: row.stem_content.0,
            explanation_content: row.explanation_content.map(|content| content.0),
            rubric_content: row.rubric_content.map(|content| content.0),
            tags: row.tags,
            status: row.status,
            subject_code: row.subject_code,
            subject_name_th: row.subject_name_th,
            subject_name_en: row.subject_name_en,
            subject_group_id: row.subject_group_id,
            subject_group_name: row.subject_group_name,
            choice_count: row.choice_count,
            correct_choice_count: row.correct_choice_count,
            can_manage: row.can_manage,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

impl From<QuestionChoiceRow> for QuestionChoice {
    fn from(row: QuestionChoiceRow) -> Self {
        Self {
            id: row.id,
            question_id: row.question_id,
            label: row.label,
            content: row.content.0,
            is_correct: row.is_correct,
            sort_order: row.sort_order,
        }
    }
}

#[derive(Debug, sqlx::FromRow)]
pub struct QuestionScopeRow {
    pub owner_user_id: Uuid,
    pub subject_id: Option<Uuid>,
    pub subject_group_id: Option<Uuid>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct QuestionBankSummaryRow {
    pub total: i64,
    pub choice: i64,
    pub written: i64,
    pub ready: i64,
}
