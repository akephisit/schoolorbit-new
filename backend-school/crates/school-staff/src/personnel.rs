use crate::career::{CreateStaffCareerRequest, StaffCareerCurrent, UpdateStaffCareerRequest};
use chrono::NaiveDate;
use serde::{Deserialize, Deserializer, Serialize};
#[cfg(feature = "api-docs")]
use utoipa::ToSchema;
use uuid::Uuid;

macro_rules! personnel_enum {
    ($name:ident { $($variant:ident => ($code:literal, $label:literal)),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        #[cfg_attr(feature = "api-docs", derive(ToSchema))]
        #[serde(rename_all = "snake_case")]
        pub enum $name { $(#[serde(rename = $code)] $variant),+ }
        impl std::str::FromStr for $name {
            type Err = &'static str;
            fn from_str(value: &str) -> Result<Self, Self::Err> { match value { $($code => Ok(Self::$variant),)+ _ => Err("unknown personnel code") } }
        }
        impl $name {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];
            pub const fn as_str(self) -> &'static str { match self { $(Self::$variant => $code),+ } }
            pub const fn label(self) -> &'static str { match self { $(Self::$variant => $label),+ } }
        }
    };
}

personnel_enum!(StaffAcademicRank {
    None => ("none", "ไม่มีวิทยฐานะ"),
    NotApplicable => ("not_applicable", "ไม่ใช้กับตำแหน่งนี้"),
    Proficient => ("proficient", "ชำนาญการ"),
    SeniorProficient => ("senior_proficient", "ชำนาญการพิเศษ"),
    Expert => ("expert", "เชี่ยวชาญ"),
    SeniorExpert => ("senior_expert", "เชี่ยวชาญพิเศษ"),
});
personnel_enum!(StaffEducationLevel {
    Primary => ("primary", "ประถมศึกษา"),
    LowerSecondary => ("lower_secondary", "มัธยมศึกษาตอนต้น"),
    UpperSecondary => ("upper_secondary", "มัธยมศึกษาตอนปลาย"),
    VocationalCertificate => ("vocational_certificate", "ปวช."),
    HigherVocational => ("higher_vocational", "ปวส."),
    Diploma => ("diploma", "อนุปริญญา"),
    Bachelor => ("bachelor", "ปริญญาตรี"),
    Master => ("master", "ปริญญาโท"),
    Doctorate => ("doctorate", "ปริญญาเอก"),
    Other => ("other", "อื่น ๆ"),
});
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct StaffJobPositionSummary {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub is_active: bool,
    pub is_selectable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
pub struct StaffEducation {
    pub id: Uuid,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub education_level: Option<StaffEducationLevel>,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub major: Option<String>,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub university: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(deny_unknown_fields)]
pub struct StaffEducationInput {
    pub id: Option<Uuid>,
    pub education_level: Option<StaffEducationLevel>,
    pub major: Option<String>,
    pub university: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(deny_unknown_fields)]
pub struct ReplaceStaffEducation {
    pub expected_revision: i64,
    pub items: Vec<StaffEducationInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(deny_unknown_fields)]
pub struct CreateStaffInfoRequest {
    pub career: Option<CreateStaffCareerRequest>,
    #[serde(default)]
    pub educations: Vec<StaffEducationInput>,
    pub teaching_license_number: Option<String>,
    pub teaching_license_expiry: Option<NaiveDate>,
}

fn deserialize_nullable<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(deny_unknown_fields)]
pub struct UpdateStaffInfoRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub career: Option<UpdateStaffCareerRequest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub education: Option<ReplaceStaffEducation>,
    #[serde(
        default,
        deserialize_with = "deserialize_nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub teaching_license_number: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "deserialize_nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub teaching_license_expiry: Option<Option<NaiveDate>>,
}

impl UpdateStaffInfoRequest {
    pub fn is_empty(&self) -> bool {
        self.career
            .as_ref()
            .is_none_or(|career| career.changes.is_empty())
            && self.education.is_none()
            && self.teaching_license_number.is_none()
            && self.teaching_license_expiry.is_none()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
pub struct StaffInfoResponse {
    pub current_career: StaffCareerCurrent,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub job_position: Option<StaffJobPositionSummary>,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub academic_rank: Option<StaffAcademicRank>,
    pub educations: Vec<StaffEducation>,
    pub education_revision: i64,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub highest_education_level: Option<StaffEducationLevel>,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub teaching_license_number: Option<String>,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub teaching_license_expiry: Option<NaiveDate>,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "api-docs")]
    use utoipa::PartialSchema;

    #[test]
    #[cfg(feature = "api-docs")]
    fn personnel_schema_enum_matches_serialized_codes() {
        let schema = serde_json::to_value(StaffAcademicRank::schema()).unwrap();
        let expected = StaffAcademicRank::ALL
            .iter()
            .map(|v| v.as_str())
            .collect::<Vec<_>>();
        assert_eq!(schema["enum"], serde_json::to_value(expected).unwrap());
    }

    #[test]
    fn personnel_patch_serialization_preserves_explicit_null() {
        let omitted: UpdateStaffInfoRequest = serde_json::from_str("{}").unwrap();
        let cleared: UpdateStaffInfoRequest = serde_json::from_str(
            r#"{"teaching_license_number":null,"education":{"expected_revision":2,"items":[]}}"#,
        )
        .unwrap();
        assert!(omitted.is_empty());
        assert_eq!(cleared.teaching_license_number, Some(None));
        assert_eq!(cleared.education.as_ref().unwrap().items.len(), 0);
        assert_eq!(cleared.education.as_ref().unwrap().expected_revision, 2);
        assert_eq!(
            serde_json::to_string(&cleared).unwrap(),
            r#"{"education":{"expected_revision":2,"items":[]},"teaching_license_number":null}"#
        );
        assert!(serde_json::from_str::<UpdateStaffInfoRequest>(
            r#"{"education_level":"bachelor"}"#
        )
        .is_err());
        assert_eq!(StaffAcademicRank::None.label(), "ไม่มีวิทยฐานะ");
        assert_eq!(StaffEducationLevel::Bachelor.as_str(), "bachelor");
    }
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::IntoParams))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "api-docs", into_params(parameter_in = Query))]
pub struct JobPositionListQuery {
    pub search: Option<String>,
    pub selectable_only: Option<bool>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct JobPositionPage {
    pub items: Vec<StaffJobPositionSummary>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct StaffSubjectGroupSummary {
    pub id: Uuid,
    pub name: String,
}

personnel_enum!(PersonnelStatusFilter {
    All => ("all", "ทุกสถานะ"), Active => ("active", "ปฏิบัติงาน"),
    Inactive => ("inactive", "ไม่ใช้งาน"), Suspended => ("suspended", "ระงับการใช้งาน"),
    Resigned => ("resigned", "ลาออก"), Retired => ("retired", "เกษียณ"),
});
personnel_enum!(PersonnelDimension {
    Status => ("status", "สถานะ"), SubjectGroup => ("subject_group", "กลุ่มสาระ"),
    JobPosition => ("job_position", "ตำแหน่งงาน"), AcademicRank => ("academic_rank", "วิทยฐานะ"),
    EducationLevel => ("education_level", "วุฒิการศึกษาสูงสุด"),
});
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::IntoParams))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "api-docs", into_params(parameter_in=Query))]
pub struct PersonnelOverviewQuery {
    pub status: Option<PersonnelStatusFilter>,
}
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct PersonnelBucket {
    pub key: String,
    pub label: String,
    pub count: i64,
}
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct PersonnelOverview {
    pub as_of: chrono::DateTime<chrono::Utc>,
    /// Number of active staff in the authorized scope, independent of the detail status filter.
    pub total: i64,
    /// Number of staff matching the selected status in the authorized scope.
    pub filtered_total: i64,
    pub subject_groups: Vec<PersonnelBucket>,
    pub job_positions: Vec<PersonnelBucket>,
    pub academic_ranks: Vec<PersonnelBucket>,
    pub education_levels: Vec<PersonnelBucket>,
}
