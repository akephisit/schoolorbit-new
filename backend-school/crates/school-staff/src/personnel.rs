use chrono::NaiveDate;
use serde::{Deserialize, Deserializer, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

macro_rules! personnel_enum {
    ($name:ident { $($variant:ident => ($code:literal, $label:literal)),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StaffJobPositionSummary {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub is_active: bool,
    pub is_selectable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateStaffInfoRequest {
    pub job_position_id: Option<Uuid>,
    pub academic_rank: Option<StaffAcademicRank>,
    pub education_level: Option<StaffEducationLevel>,
    pub major: Option<String>,
    pub university: Option<String>,
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

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateStaffInfoRequest {
    #[serde(
        default,
        deserialize_with = "deserialize_nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub job_position_id: Option<Option<Uuid>>,
    #[serde(
        default,
        deserialize_with = "deserialize_nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub academic_rank: Option<Option<StaffAcademicRank>>,
    #[serde(
        default,
        deserialize_with = "deserialize_nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub education_level: Option<Option<StaffEducationLevel>>,
    #[serde(
        default,
        deserialize_with = "deserialize_nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub major: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "deserialize_nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub university: Option<Option<String>>,
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
        self.job_position_id.is_none()
            && self.academic_rank.is_none()
            && self.education_level.is_none()
            && self.major.is_none()
            && self.university.is_none()
            && self.teaching_license_number.is_none()
            && self.teaching_license_expiry.is_none()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct StaffInfoResponse {
    #[schema(required = true)]
    pub job_position: Option<StaffJobPositionSummary>,
    #[schema(required = true)]
    pub academic_rank: Option<StaffAcademicRank>,
    #[schema(required = true)]
    pub education_level: Option<StaffEducationLevel>,
    #[schema(required = true)]
    pub major: Option<String>,
    #[schema(required = true)]
    pub university: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use utoipa::PartialSchema;

    #[test]
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
        let cleared: UpdateStaffInfoRequest =
            serde_json::from_str(r#"{"academic_rank":null}"#).unwrap();
        let explicit: UpdateStaffInfoRequest =
            serde_json::from_str(r#"{"academic_rank":"none"}"#).unwrap();
        assert!(omitted.is_empty());
        assert_eq!(omitted.academic_rank, None);
        assert_eq!(cleared.academic_rank, Some(None));
        assert_eq!(explicit.academic_rank, Some(Some(StaffAcademicRank::None)));
        assert_eq!(
            serde_json::to_string(&cleared).unwrap(),
            r#"{"academic_rank":null}"#
        );
        assert_eq!(StaffAcademicRank::None.label(), "ไม่มีวิทยฐานะ");
        assert_eq!(StaffEducationLevel::Bachelor.as_str(), "bachelor");
    }
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[into_params(parameter_in = Query)]
pub struct JobPositionListQuery {
    pub search: Option<String>,
    pub selectable_only: Option<bool>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct JobPositionPage {
    pub items: Vec<StaffJobPositionSummary>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema, sqlx::FromRow)]
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
#[derive(Debug, Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[into_params(parameter_in=Query)]
pub struct PersonnelOverviewQuery {
    pub status: Option<PersonnelStatusFilter>,
}
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PersonnelBucket {
    pub key: String,
    pub label: String,
    pub count: i64,
}
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PersonnelOverview {
    pub as_of: chrono::DateTime<chrono::Utc>,
    pub total: i64,
    pub active: i64,
    pub other_statuses: i64,
    pub filtered_total: i64,
    pub statuses: Vec<PersonnelBucket>,
    pub subject_groups: Vec<PersonnelBucket>,
    pub job_positions: Vec<PersonnelBucket>,
    pub academic_ranks: Vec<PersonnelBucket>,
    pub education_levels: Vec<PersonnelBucket>,
}
