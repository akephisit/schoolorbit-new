use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

macro_rules! string_enum {
    ($name:ident { $($variant:ident),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, ToSchema)]
        #[serde(rename_all = "snake_case")]
        pub enum $name {
            $($variant),+
        }
    };
}

string_enum!(SchoolFontStyle { Normal, Italic });
string_enum!(SchoolFontUploadStatus {
    Ready,
    DuplicateSelection,
    DuplicateExisting,
    UnsupportedVariable,
    UnsupportedWeight,
    MissingFamily,
    InvalidDisplayName,
    Unavailable,
});

impl SchoolFontStyle {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Italic => "italic",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "normal" => Some(Self::Normal),
            "italic" => Some(Self::Italic),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SchoolFontSummary {
    pub id: Uuid,
    pub display_name: String,
    pub font_family: String,
    pub font_weight: u16,
    pub font_style: SchoolFontStyle,
    pub reference_count: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SchoolFontListResponse {
    pub items: Vec<SchoolFontSummary>,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InspectSchoolFontUploadsRequest {
    pub file_ids: Vec<Uuid>,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AttachSchoolFontBatchRequest {
    pub file_ids: Vec<Uuid>,
    #[serde(default)]
    pub rights_confirmed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SchoolFontUploadInspectionFile {
    pub file_id: Uuid,
    pub display_filename: String,
    #[schema(required = true)]
    pub font_family: Option<String>,
    #[schema(required = true)]
    pub font_weight: Option<u16>,
    #[schema(required = true)]
    pub font_style: Option<SchoolFontStyle>,
    pub status: SchoolFontUploadStatus,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SchoolFontUploadInspection {
    pub files: Vec<SchoolFontUploadInspectionFile>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SchoolFontDeleteConflict {
    pub reference_count: i64,
}
