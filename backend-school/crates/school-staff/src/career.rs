use crate::personnel::{StaffAcademicRank, StaffJobPositionSummary};
use chrono::{DateTime, NaiveDate, Utc};
use school_errors::AppError;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeSet;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

macro_rules! career_enum {
    ($name:ident { $($variant:ident => $code:literal),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ToSchema)]
        #[serde(rename_all = "snake_case")]
        pub enum $name { $(#[serde(rename = $code)] $variant),+ }
        impl $name {
            pub const fn as_str(self) -> &'static str { match self { $(Self::$variant => $code),+ } }
        }
        impl std::str::FromStr for $name {
            type Err = &'static str;
            fn from_str(value: &str) -> Result<Self, Self::Err> {
                match value { $($code => Ok(Self::$variant),)+ _ => Err("unknown career code") }
            }
        }
    };
}

career_enum!(StaffPersonnelType {
    CivilServant => "civil_servant",
    GovernmentEmployee => "government_employee",
    ContractEmployee => "contract_employee",
    PermanentEmployee => "permanent_employee",
    Other => "other",
});
career_enum!(StaffCareerKind {
    PersonnelType => "personnel_type",
    JobPosition => "job_position",
    AcademicRank => "academic_rank",
});
career_enum!(StaffCareerSource {
    ExistingRecord => "existing_record",
    StaffEntry => "staff_entry",
});

fn required_nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum StaffCareerFact {
    PersonnelType {
        #[serde(deserialize_with = "required_nullable")]
        #[schema(required = true)]
        value: Option<StaffPersonnelType>,
    },
    JobPosition {
        #[serde(deserialize_with = "required_nullable")]
        #[schema(required = true)]
        value: Option<Uuid>,
    },
    AcademicRank {
        #[serde(deserialize_with = "required_nullable")]
        #[schema(required = true)]
        value: Option<StaffAcademicRank>,
    },
}

impl StaffCareerFact {
    pub const fn kind(&self) -> StaffCareerKind {
        match self {
            Self::PersonnelType { .. } => StaffCareerKind::PersonnelType,
            Self::JobPosition { .. } => StaffCareerKind::JobPosition,
            Self::AcademicRank { .. } => StaffCareerKind::AcademicRank,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StaffCareerEntryInput {
    pub fact: StaffCareerFact,
    pub effective_date: Option<NaiveDate>,
    pub order_date: Option<NaiveDate>,
    pub order_number: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StaffCareerReference {
    pub id: Uuid,
    pub revision: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StaffCareerCurrent {
    #[schema(required = true)]
    pub personnel_type: Option<StaffCareerEntry>,
    #[schema(required = true)]
    pub job_position: Option<StaffCareerEntry>,
    #[schema(required = true)]
    pub academic_rank: Option<StaffCareerEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StaffCareerEntry {
    pub id: Uuid,
    pub staff_id: Uuid,
    pub fact: StaffCareerFact,
    #[schema(required = true)]
    pub job_position: Option<StaffJobPositionSummary>,
    #[schema(required = true)]
    pub effective_date: Option<NaiveDate>,
    #[schema(required = true)]
    pub order_date: Option<NaiveDate>,
    #[schema(required = true)]
    pub order_number: Option<String>,
    #[schema(required = true)]
    pub note: Option<String>,
    pub source: StaffCareerSource,
    pub revision: i64,
    pub is_current: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateStaffCareerRequest {
    pub entries: Vec<StaffCareerEntryInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StaffCareerCurrentChange {
    #[serde(deserialize_with = "required_nullable")]
    #[schema(required = true)]
    pub expected_current: Option<StaffCareerReference>,
    pub entry: StaffCareerEntryInput,
    pub correction_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateStaffCareerRequest {
    pub changes: Vec<StaffCareerCurrentChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateStaffCareerHistoryRequest {
    pub id: Uuid,
    pub entry: StaffCareerEntryInput,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CorrectStaffCareerHistoryRequest {
    pub expected_revision: i64,
    pub expected_is_current: bool,
    pub entry: StaffCareerEntryInput,
    pub reason: String,
}

#[derive(Debug, Default, Clone, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[into_params(parameter_in = Query)]
pub struct StaffCareerHistoryQuery {
    pub cursor: Option<Uuid>,
    pub page_size: Option<i64>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StaffCareerHistoryPage {
    pub rank_milestone: crate::rank_milestones::RankMilestone,
    pub items: Vec<StaffCareerEntry>,
    pub current: StaffCareerCurrent,
    #[schema(required = true)]
    pub next_cursor: Option<Uuid>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StaffCareerMutationAck {
    pub id: Uuid,
    pub revision: i64,
}

fn normalized_text(value: Option<&str>, limit: usize) -> Result<Option<String>, AppError> {
    let Some(value) = value else { return Ok(None) };
    if value.chars().any(char::is_control) || value.trim().chars().count() > limit {
        return Err(AppError::BadRequest(format!(
            "ข้อความต้องไม่เกิน {limit} ตัวอักษรและไม่มีอักขระควบคุม"
        )));
    }
    let value = value.trim();
    Ok((!value.is_empty()).then(|| value.to_owned()))
}

pub fn normalize_career_entry(
    input: &StaffCareerEntryInput,
    today: NaiveDate,
) -> Result<StaffCareerEntryInput, AppError> {
    if input.effective_date.is_some_and(|date| date > today)
        || input.order_date.is_some_and(|date| date > today)
    {
        return Err(AppError::BadRequest(
            "วันที่มีผลและวันที่คำสั่งต้องไม่เป็นวันในอนาคต".into(),
        ));
    }
    Ok(StaffCareerEntryInput {
        fact: input.fact.clone(),
        effective_date: input.effective_date,
        order_date: input.order_date,
        order_number: normalized_text(input.order_number.as_deref(), 100)?,
        note: normalized_text(input.note.as_deref(), 1000)?,
    })
}

pub fn normalize_correction_reason(input: &str) -> Result<String, AppError> {
    normalized_text(Some(input), 1000)?
        .ok_or_else(|| AppError::BadRequest("กรุณาระบุเหตุผลการแก้ไข".into()))
}

fn validate_kinds<'a>(
    entries: impl Iterator<Item = &'a StaffCareerEntryInput>,
    today: NaiveDate,
) -> Result<(), AppError> {
    let mut kinds = BTreeSet::new();
    for entry in entries {
        normalize_career_entry(entry, today)?;
        if !kinds.insert(entry.fact.kind()) {
            return Err(AppError::BadRequest("ข้อมูลแต่ละชนิดต้องไม่ซ้ำกัน".into()));
        }
    }
    Ok(())
}

pub fn validate_create_career(
    input: &CreateStaffCareerRequest,
    today: NaiveDate,
) -> Result<(), AppError> {
    validate_kinds(input.entries.iter(), today)
}

pub fn validate_update_career(
    input: &UpdateStaffCareerRequest,
    today: NaiveDate,
) -> Result<(), AppError> {
    validate_kinds(input.changes.iter().map(|change| &change.entry), today)?;
    if input.changes.iter().any(|change| {
        change
            .expected_current
            .as_ref()
            .is_some_and(|reference| reference.revision < 1)
    }) {
        return Err(AppError::BadRequest("รุ่นข้อมูลอ้างอิงไม่ถูกต้อง".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry() -> StaffCareerEntryInput {
        serde_json::from_value(serde_json::json!({
            "fact": { "kind": "academic_rank", "value": "proficient" },
            "effectiveDate": "2024-02-29", "orderDate": "2024-03-15",
            "orderNumber": "  คำสั่ง 12/2567  ", "note": "  ตามคำสั่ง  "
        }))
        .unwrap()
    }

    #[test]
    fn career_date_and_text_validation() {
        let today = NaiveDate::from_ymd_opt(2026, 10, 3).unwrap();
        let normalized = normalize_career_entry(&entry(), today).unwrap();
        assert_eq!(normalized.order_number.as_deref(), Some("คำสั่ง 12/2567"));
        assert_eq!(normalized.note.as_deref(), Some("ตามคำสั่ง"));
        for (number, note) in [
            ("ก".repeat(100), "😀".repeat(1000)),
            ("😀".repeat(100), "ก".repeat(1000)),
        ] {
            let mut input = entry();
            input.order_number = Some(number);
            input.note = Some(note);
            assert!(normalize_career_entry(&input, today).is_ok());
        }
        for value in ["ก".repeat(101), "😀".repeat(101), "คำสั่ง\n12".into()] {
            let mut input = entry();
            input.order_number = Some(value);
            assert!(normalize_career_entry(&input, today).is_err());
        }
        let mut input = entry();
        input.note = Some("ก".repeat(1001));
        assert!(normalize_career_entry(&input, today).is_err());
        input.note = Some("   ".into());
        input.order_number = Some("   ".into());
        let blank = normalize_career_entry(&input, today).unwrap();
        assert_eq!(blank.note, None);
        assert_eq!(blank.order_number, None);
        input.effective_date = Some(today.succ_opt().unwrap());
        assert!(normalize_career_entry(&input, today).is_err());
        input.effective_date = None;
        input.order_date = Some(today.succ_opt().unwrap());
        assert!(normalize_career_entry(&input, today).is_err());
        assert!(
            serde_json::from_value::<StaffCareerEntryInput>(serde_json::json!({
                "fact": {"kind":"academic_rank","value":"none"}, "effectiveDate":"2023-02-29"
            }))
            .is_err()
        );
    }

    #[test]
    fn career_fact_preserves_null_and_rank_codes() {
        let cases = [
            "null",
            "\"none\"",
            "\"not_applicable\"",
            "\"senior_expert\"",
        ];
        for value in cases {
            let json = format!("{{\"kind\":\"academic_rank\",\"value\":{value}}}");
            let fact: StaffCareerFact = serde_json::from_str(&json).unwrap();
            assert_eq!(
                serde_json::to_value(fact).unwrap(),
                serde_json::from_str::<serde_json::Value>(&json).unwrap()
            );
        }
        for invalid in [
            r#"{"kind":"academic_rank","value":"civil_servant"}"#,
            r#"{"kind":"academic_rank","value":"none","extra":true}"#,
            r#"{"kind":"job_position","value":"teacher"}"#,
            r#"{"kind":"personnel_type","value":"permanent"}"#,
            r#"{"kind":"academic_rank"}"#,
        ] {
            assert!(
                serde_json::from_str::<StaffCareerFact>(invalid).is_err(),
                "{invalid}"
            );
        }
    }

    #[test]
    fn career_rejects_duplicate_kinds_and_missing_expectation() {
        assert!(serde_json::from_str::<UpdateStaffCareerRequest>(
            r#"{"changes":[{"entry":{"fact":{"kind":"academic_rank","value":"none"}}}]}"#
        )
        .is_err());
        let input = serde_json::from_str::<UpdateStaffCareerRequest>(r#"{"changes":[{"expectedCurrent":null,"entry":{"fact":{"kind":"academic_rank","value":"none"}}}]}"#).unwrap();
        let today = NaiveDate::from_ymd_opt(2026, 10, 3).unwrap();
        assert!(validate_update_career(&input, today).is_ok());
        let duplicate = CreateStaffCareerRequest {
            entries: vec![entry(), entry()],
        };
        assert!(validate_create_career(&duplicate, today).is_err());
        let duplicate = UpdateStaffCareerRequest {
            changes: vec![input.changes[0].clone(), input.changes[0].clone()],
        };
        assert!(validate_update_career(&duplicate, today).is_err());
    }

    #[test]
    fn career_correction_reason_is_required_and_bounded() {
        assert!(normalize_correction_reason("  ").is_err());
        assert!(normalize_correction_reason("เหตุผล\tผิด").is_err());
        assert!(normalize_correction_reason(&"😀".repeat(1001)).is_err());
        assert_eq!(
            normalize_correction_reason("  แก้ตามคำสั่ง  ").unwrap(),
            "แก้ตามคำสั่ง"
        );
    }
}

#[cfg(test)]
mod schema_tests {
    use super::*;
    use utoipa::PartialSchema;
    #[test]
    fn career_schema_codes_match_serialized_values() {
        for (schema, values) in [
            (
                serde_json::to_value(StaffPersonnelType::schema()).unwrap(),
                vec![
                    "civil_servant",
                    "government_employee",
                    "contract_employee",
                    "permanent_employee",
                    "other",
                ],
            ),
            (
                serde_json::to_value(StaffCareerKind::schema()).unwrap(),
                vec!["personnel_type", "job_position", "academic_rank"],
            ),
            (
                serde_json::to_value(StaffCareerSource::schema()).unwrap(),
                vec!["existing_record", "staff_entry"],
            ),
        ] {
            assert_eq!(schema["enum"], serde_json::to_value(values).unwrap());
        }
    }
}
