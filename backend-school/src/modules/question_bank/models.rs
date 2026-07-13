use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use uuid::Uuid;

pub const RICH_CONTENT_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RichContent {
    pub schema_version: u16,
    pub document: RichDocument,
}

impl RichContent {
    pub fn image_file_ids(&self) -> impl Iterator<Item = Uuid> + '_ {
        self.document
            .blocks()
            .iter()
            .filter_map(|block| match block {
                RichBlockNode::Image { attrs } => Some(attrs.file_id),
                _ => None,
            })
    }

    pub fn has_body(&self) -> bool {
        self.document.blocks().iter().any(RichBlockNode::has_body)
    }

    pub fn search_text(&self) -> String {
        let mut parts = Vec::new();
        for block in self.document.blocks() {
            match block {
                RichBlockNode::Paragraph { content } => {
                    parts.extend(content.iter().filter_map(RichInlineNode::search_text));
                }
                RichBlockNode::MathBlock { attrs } => {
                    if !attrs.latex.trim().is_empty() {
                        parts.push(attrs.latex.trim());
                    }
                }
                RichBlockNode::Image { attrs } => {
                    if let Some(alt_text) = attrs.alt_text.as_deref().map(str::trim) {
                        if !alt_text.is_empty() {
                            parts.push(alt_text);
                        }
                    }
                    if let Some(caption) = attrs.caption.as_deref().map(str::trim) {
                        if !caption.is_empty() {
                            parts.push(caption);
                        }
                    }
                }
            }
        }
        parts.join(" ")
    }

    pub fn validate_shape(&self) -> Result<(), &'static str> {
        if self.schema_version != RICH_CONTENT_SCHEMA_VERSION {
            return Err("ไม่รองรับเวอร์ชันเอกสารข้อสอบนี้");
        }
        let blocks = self.document.blocks();
        if blocks.len() > 500 {
            return Err("เนื้อหาข้อสอบมีจำนวนส่วนมากเกินไป");
        }
        for block in blocks {
            block.validate_shape()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum RichDocument {
    Doc {
        #[serde(default)]
        content: Vec<RichBlockNode>,
    },
}

impl RichDocument {
    pub fn blocks(&self) -> &[RichBlockNode] {
        match self {
            Self::Doc { content } => content,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum RichBlockNode {
    Paragraph {
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        content: Vec<RichInlineNode>,
    },
    MathBlock {
        attrs: MathNodeAttributes,
    },
    Image {
        attrs: ImageNodeAttributes,
    },
}

impl RichBlockNode {
    fn has_body(&self) -> bool {
        match self {
            Self::Paragraph { content } => content.iter().any(RichInlineNode::has_body),
            Self::MathBlock { attrs } => !attrs.latex.trim().is_empty(),
            Self::Image { .. } => true,
        }
    }

    fn validate_shape(&self) -> Result<(), &'static str> {
        match self {
            Self::Paragraph { content } => {
                if content.len() > 2_000 {
                    return Err("ย่อหน้ามีจำนวนส่วนมากเกินไป");
                }
                for node in content {
                    node.validate_shape()?;
                }
            }
            Self::MathBlock { attrs } => attrs.validate_shape()?,
            Self::Image { attrs } => attrs.validate_shape()?,
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum RichInlineNode {
    Text {
        text: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        marks: Vec<RichTextMark>,
    },
    InlineMath {
        attrs: MathNodeAttributes,
    },
    #[serde(rename = "hardBreak")]
    HardBreak,
}

impl RichInlineNode {
    fn has_body(&self) -> bool {
        match self {
            Self::Text { text, .. } => !text.trim().is_empty(),
            Self::InlineMath { attrs } => !attrs.latex.trim().is_empty(),
            Self::HardBreak => false,
        }
    }

    fn search_text(&self) -> Option<&str> {
        match self {
            Self::Text { text, .. } if !text.trim().is_empty() => Some(text.trim()),
            Self::InlineMath { attrs } if !attrs.latex.trim().is_empty() => {
                Some(attrs.latex.trim())
            }
            _ => None,
        }
    }

    fn validate_shape(&self) -> Result<(), &'static str> {
        match self {
            Self::Text { text, marks } => {
                if text.chars().count() > 100_000 {
                    return Err("ข้อความยาวเกินขอบเขตที่รองรับ");
                }
                if marks.len() > 4 {
                    return Err("รูปแบบข้อความมีจำนวนมากเกินไป");
                }
            }
            Self::InlineMath { attrs } => attrs.validate_shape()?,
            Self::HardBreak => {}
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum RichTextMark {
    Bold,
    Italic,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MathNodeAttributes {
    pub latex: String,
}

impl MathNodeAttributes {
    fn validate_shape(&self) -> Result<(), &'static str> {
        if self.latex.chars().count() > 20_000 {
            return Err("สมการยาวเกินขอบเขตที่รองรับ");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImageNodeAttributes {
    pub file_id: Uuid,
    pub alt_text: Option<String>,
    pub caption: Option<String>,
    pub alignment: ImageAlignment,
    pub width_percent: u8,
}

impl ImageNodeAttributes {
    fn validate_shape(&self) -> Result<(), &'static str> {
        if !(10..=100).contains(&self.width_percent) {
            return Err("ความกว้างรูปต้องอยู่ระหว่าง 10 ถึง 100 เปอร์เซ็นต์");
        }
        if self
            .alt_text
            .as_ref()
            .is_some_and(|value| value.chars().count() > 1_000)
        {
            return Err("คำอธิบายรูปยาวเกินขอบเขตที่รองรับ");
        }
        if self
            .caption
            .as_ref()
            .is_some_and(|value| value.chars().count() > 2_000)
        {
            return Err("คำบรรยายรูปยาวเกินขอบเขตที่รองรับ");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ImageAlignment {
    Left,
    Center,
    Right,
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
