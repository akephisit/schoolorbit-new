//! Calendar planning for the ordinary teaching track; never a qualification decision.
use crate::career::{StaffCareerCurrent, StaffCareerFact, StaffPersonnelType};
use crate::personnel::StaffAcademicRank;
use chrono::{Months, NaiveDate};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RankMilestoneStatus {
    Future,
    DueSoon,
    TimeReachedPendingReview,
    Incomplete,
    Unsupported,
    NoNextRank,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RankMilestoneReason {
    MissingPersonnelType,
    MissingPosition,
    MissingRank,
    MissingPersonnelTypeDate,
    MissingPositionDate,
    MissingRankDate,
    FutureEffectiveDate,
    UnsupportedPersonnelType,
    UnsupportedPosition,
    NotApplicableRank,
    HighestRank,
    NoReviewedCriteria,
    CalendarOverflow,
}
impl RankMilestoneReason {
    pub const fn label(self) -> &'static str {
        match self {
            Self::MissingPersonnelType => "ยังไม่ระบุประเภทบุคลากร",
            Self::MissingPosition => "ยังไม่ระบุตำแหน่ง",
            Self::MissingRank => "ยังไม่ระบุวิทยฐานะ",
            Self::MissingPersonnelTypeDate => "ยังไม่ระบุวันที่มีผลของประเภทข้าราชการ",
            Self::MissingPositionDate => "ยังไม่ระบุวันที่มีผลของตำแหน่งครู",
            Self::MissingRankDate => "ยังไม่ระบุวันที่มีผลของวิทยฐานะปัจจุบัน",
            Self::FutureEffectiveDate => "มีวันที่มีผลหลังวันที่ประเมิน กรุณาตรวจสอบประวัติ",
            Self::UnsupportedPersonnelType => "รองรับการคำนวณเฉพาะข้าราชการสายงานการสอน",
            Self::UnsupportedPosition => "ตำแหน่งนี้ยังไม่อยู่ในเกณฑ์คำนวณสายงานการสอน",
            Self::NotApplicableRank => "ระบุว่าวิทยฐานะไม่ใช้กับตำแหน่งนี้",
            Self::HighestRank => "เป็นวิทยฐานะสูงสุดในสายงานนี้แล้ว",
            Self::NoReviewedCriteria => "ยังไม่มีฉบับเกณฑ์ที่ตรวจสอบสำหรับวันที่นี้",
            Self::CalendarOverflow => "วันที่อยู่นอกช่วงที่คำนวณได้ กรุณาตรวจสอบประวัติ",
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RankCriteriaSource {
    pub title: String,
    pub url: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RankCriteriaVersion {
    pub version: String,
    pub effective_from: String,
    pub reviewed_on: String,
    pub ordinary_years: u32,
    pub conditional_reduced_years: u32,
    pub sources: Vec<RankCriteriaSource>,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RankMilestone {
    pub as_of: NaiveDate,
    pub status: RankMilestoneStatus,
    pub reasons: Vec<RankMilestoneReason>,
    pub reason_labels: Vec<String>,
    #[schema(required = true)]
    pub next_rank: Option<StaffAcademicRank>,
    #[schema(required = true)]
    pub recorded_start_date: Option<NaiveDate>,
    #[schema(required = true)]
    pub ordinary_date: Option<NaiveDate>,
    /// Comparison only: reduction evidence and all other qualifications are unverified.
    #[schema(required = true)]
    pub conditional_reduced_date: Option<NaiveDate>,
    #[schema(required = true)]
    pub days_until_ordinary_date: Option<i64>,
    #[schema(required = true)]
    pub criteria: Option<RankCriteriaVersion>,
}

// Immutable reviewed policy version. No frontend or reference-data management owns this policy.
fn criteria(as_of: NaiveDate) -> Option<RankCriteriaVersion> {
    if as_of.to_string().as_str() < "2026-10-03" {
        return None;
    }
    Some(RankCriteriaVersion {
        version: "teacher-ordinary-2026-10-03".into(),
        effective_from: "2026-10-03".into(),
        reviewed_on: "2026-10-03".into(),
        ordinary_years: 4,
        conditional_reduced_years: 3,
        sources: [
            (
                "1932/2567 · หลักเกณฑ์ตำแหน่งครูและที่แก้ไขเพิ่มเติม",
                "https://otepc.go.th/th/content_page/item/5169-2024-11-22-11-47-11.html",
            ),
            (
                "1222/2568 · การแก้ไขเพิ่มเติมหลักเกณฑ์",
                "https://otepc.go.th/en/content_page/item/5622-1222-2568-9-12-2564-18-2567.html",
            ),
            (
                "587/2569 · เงื่อนไขปริญญาโทสำหรับลดระยะเวลา",
                "https://otepc.go.th/en/content_page/item/6043-587-2569.html",
            ),
        ]
        .into_iter()
        .map(|(title, url)| RankCriteriaSource {
            title: title.into(),
            url: url.into(),
        })
        .collect(),
    })
}

#[derive(Debug, Clone)]
pub(crate) struct RankMilestoneFacts {
    pub personnel_type: Option<StaffPersonnelType>,
    pub personnel_type_date: Option<NaiveDate>,
    pub position_code: Option<String>,
    pub position_date: Option<NaiveDate>,
    pub academic_rank: Option<StaffAcademicRank>,
    pub rank_date: Option<NaiveDate>,
}
impl From<&StaffCareerCurrent> for RankMilestoneFacts {
    fn from(current: &StaffCareerCurrent) -> Self {
        Self {
            personnel_type: current
                .personnel_type
                .as_ref()
                .and_then(|entry| match entry.fact {
                    StaffCareerFact::PersonnelType { value } => value,
                    _ => None,
                }),
            personnel_type_date: current
                .personnel_type
                .as_ref()
                .and_then(|entry| entry.effective_date),
            position_code: current
                .job_position
                .as_ref()
                .and_then(|entry| entry.job_position.as_ref())
                .map(|position| position.code.clone()),
            position_date: current
                .job_position
                .as_ref()
                .and_then(|entry| entry.effective_date),
            academic_rank: current
                .academic_rank
                .as_ref()
                .and_then(|entry| match entry.fact {
                    StaffCareerFact::AcademicRank { value } => value,
                    _ => None,
                }),
            rank_date: current
                .academic_rank
                .as_ref()
                .and_then(|entry| entry.effective_date),
        }
    }
}
pub fn calculate_rank_milestone(current: &StaffCareerCurrent, as_of: NaiveDate) -> RankMilestone {
    calculate_rank_milestone_facts(&RankMilestoneFacts::from(current), as_of)
}
pub(crate) fn calculate_rank_milestone_facts(
    facts: &RankMilestoneFacts,
    as_of: NaiveDate,
) -> RankMilestone {
    use RankMilestoneReason as Reason;
    use RankMilestoneStatus as Status;
    let mut result = RankMilestone {
        as_of,
        status: Status::Incomplete,
        reasons: vec![],
        reason_labels: vec![],
        next_rank: None,
        recorded_start_date: None,
        ordinary_date: None,
        conditional_reduced_date: None,
        days_until_ordinary_date: None,
        criteria: criteria(as_of),
    };
    let personnel_type = facts.personnel_type;
    let position = facts.position_code.as_deref();
    let rank = facts.academic_rank;
    if personnel_type.is_none() {
        result.reasons.push(Reason::MissingPersonnelType);
    }
    if position.is_none() {
        result.reasons.push(Reason::MissingPosition);
    }
    if rank.is_none() {
        result.reasons.push(Reason::MissingRank);
    }
    if !result.reasons.is_empty() {
        return finish(result);
    }
    if personnel_type != Some(StaffPersonnelType::CivilServant) {
        return stop(
            result,
            Status::Unsupported,
            Reason::UnsupportedPersonnelType,
        );
    }
    if !position.is_some_and(|position| position == "teacher") {
        return stop(result, Status::Unsupported, Reason::UnsupportedPosition);
    }
    result.next_rank = match rank {
        Some(StaffAcademicRank::None) => Some(StaffAcademicRank::Proficient),
        Some(StaffAcademicRank::Proficient) => Some(StaffAcademicRank::SeniorProficient),
        Some(StaffAcademicRank::SeniorProficient) => Some(StaffAcademicRank::Expert),
        Some(StaffAcademicRank::Expert) => Some(StaffAcademicRank::SeniorExpert),
        Some(StaffAcademicRank::SeniorExpert) => {
            return stop(result, Status::NoNextRank, Reason::HighestRank)
        }
        _ => return stop(result, Status::Unsupported, Reason::NotApplicableRank),
    };
    let Some(rule) = &result.criteria else {
        return stop(result, Status::Unsupported, Reason::NoReviewedCriteria);
    };
    let years = (rule.ordinary_years, rule.conditional_reduced_years);
    let mut dates = vec![];
    for (entry, reason) in [
        (facts.personnel_type_date, Reason::MissingPersonnelTypeDate),
        (facts.position_date, Reason::MissingPositionDate),
    ]
    .into_iter()
    .chain(
        (rank != Some(StaffAcademicRank::None))
            .then_some((facts.rank_date, Reason::MissingRankDate)),
    ) {
        match entry {
            Some(date) if date <= as_of => dates.push(date),
            Some(_) => result.reasons.push(Reason::FutureEffectiveDate),
            None => result.reasons.push(reason),
        }
    }
    if !result.reasons.is_empty() {
        return finish(result);
    }
    let Some(start) = dates.into_iter().max() else {
        return finish(result);
    };
    let ordinary = start.checked_add_months(Months::new(years.0 * 12));
    let reduced = start.checked_add_months(Months::new(years.1 * 12));
    let (Some(ordinary), Some(reduced)) = (ordinary, reduced) else {
        return stop(result, Status::Incomplete, Reason::CalendarOverflow);
    };
    let days = ordinary.signed_duration_since(as_of).num_days();
    result.recorded_start_date = Some(start);
    result.ordinary_date = Some(ordinary);
    result.conditional_reduced_date = Some(reduced);
    result.days_until_ordinary_date = Some(days);
    result.status = if days <= 0 {
        Status::TimeReachedPendingReview
    } else if days <= 90 {
        Status::DueSoon
    } else {
        Status::Future
    };
    finish(result)
}
fn stop(
    mut result: RankMilestone,
    status: RankMilestoneStatus,
    reason: RankMilestoneReason,
) -> RankMilestone {
    result.status = status;
    result.reasons.push(reason);
    finish(result)
}
fn finish(mut result: RankMilestone) -> RankMilestone {
    result.reasons.sort_by_key(|reason| *reason as u8);
    result.reasons.dedup();
    result.reason_labels = result
        .reasons
        .iter()
        .map(|reason| reason.label().into())
        .collect();
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::career::{StaffCareerEntry, StaffCareerSource};
    use crate::personnel::StaffJobPositionSummary;
    use chrono::Utc;
    use uuid::Uuid;
    fn date(s: &str) -> NaiveDate {
        s.parse().unwrap()
    }
    fn current(start: &str, rank: StaffAcademicRank) -> StaffCareerCurrent {
        let position_id = Uuid::new_v4();
        let entry = |fact| StaffCareerEntry {
            id: Uuid::new_v4(),
            staff_id: Uuid::nil(),
            fact,
            job_position: None,
            effective_date: Some(date(start)),
            order_date: None,
            order_number: None,
            note: None,
            source: StaffCareerSource::StaffEntry,
            revision: 1,
            is_current: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let mut position = entry(StaffCareerFact::JobPosition {
            value: Some(position_id),
        });
        position.job_position = Some(StaffJobPositionSummary {
            id: position_id,
            code: "teacher".into(),
            name: "ครู".into(),
            is_active: true,
            is_selectable: true,
        });
        StaffCareerCurrent {
            personnel_type: Some(entry(StaffCareerFact::PersonnelType {
                value: Some(StaffPersonnelType::CivilServant),
            })),
            job_position: Some(position),
            academic_rank: Some(entry(StaffCareerFact::AcademicRank { value: Some(rank) })),
        }
    }
    #[test]
    fn ordinary_calendar_and_conditional_comparison_never_decide_eligibility() {
        let c = current("2024-02-29", StaffAcademicRank::Proficient);
        let r = calculate_rank_milestone(&c, date("2026-10-03"));
        assert_eq!(r.ordinary_date, Some(date("2028-02-29")));
        assert_eq!(r.conditional_reduced_date, Some(date("2027-02-28")));
        assert_eq!(r.status, RankMilestoneStatus::Future);
        assert_eq!(r.next_rank, Some(StaffAcademicRank::SeniorProficient));
        assert_eq!(r.criteria.unwrap().version, "teacher-ordinary-2026-10-03");
    }
    #[test]
    fn day_90_and_today_are_inclusive_but_day_91_is_future() {
        let c = current("2023-01-01", StaffAcademicRank::None);
        for (as_of, status) in [
            ("2026-10-02", RankMilestoneStatus::Unsupported),
            ("2026-10-03", RankMilestoneStatus::DueSoon),
            ("2026-10-04", RankMilestoneStatus::DueSoon),
            ("2027-01-01", RankMilestoneStatus::TimeReachedPendingReview),
            ("2027-01-02", RankMilestoneStatus::TimeReachedPendingReview),
        ] {
            assert_eq!(calculate_rank_milestone(&c, date(as_of)).status, status);
        }
        let c = current("2023-01-02", StaffAcademicRank::None);
        assert_eq!(
            calculate_rank_milestone(&c, date("2026-10-03")).status,
            RankMilestoneStatus::Future
        );
    }
    #[test]
    fn uses_latest_concurrent_effective_date_not_order_or_created_time() {
        let mut c = current("2020-01-01", StaffAcademicRank::Expert);
        c.job_position.as_mut().unwrap().effective_date = Some(date("2024-06-01"));
        c.academic_rank.as_mut().unwrap().order_date = Some(date("2025-01-01"));
        let r = calculate_rank_milestone(&c, date("2026-10-03"));
        assert_eq!(r.recorded_start_date, Some(date("2024-06-01")));
        assert_eq!(r.ordinary_date, Some(date("2028-06-01")));
    }
    #[test]
    fn incomplete_future_and_out_of_track_facts_produce_no_guessed_date() {
        let today = date("2026-10-03");
        let r = calculate_rank_milestone(&StaffCareerCurrent::default(), today);
        assert_eq!(r.reasons.len(), 3);
        let mut c = current("2020-01-01", StaffAcademicRank::Proficient);
        c.academic_rank.as_mut().unwrap().effective_date = None;
        let r = calculate_rank_milestone(&c, today);
        assert_eq!(r.reasons, vec![RankMilestoneReason::MissingRankDate]);
        assert!(r.ordinary_date.is_none());
        c.academic_rank.as_mut().unwrap().effective_date = Some(date("2027-01-01"));
        assert_eq!(
            calculate_rank_milestone(&c, today).reasons,
            vec![RankMilestoneReason::FutureEffectiveDate]
        );
        c.personnel_type.as_mut().unwrap().fact = StaffCareerFact::PersonnelType {
            value: Some(StaffPersonnelType::ContractEmployee),
        };
        assert_eq!(
            calculate_rank_milestone(&c, today).status,
            RankMilestoneStatus::Unsupported
        );
        c = current("2020-01-01", StaffAcademicRank::None);
        c.job_position
            .as_mut()
            .unwrap()
            .job_position
            .as_mut()
            .unwrap()
            .code = "teacher_assistant".into();
        assert_eq!(
            calculate_rank_milestone(&c, today).reasons,
            vec![RankMilestoneReason::UnsupportedPosition]
        );
    }
    #[test]
    fn all_targets_highest_rank_and_explicit_not_applicable_are_distinct() {
        for (rank, next) in [
            (StaffAcademicRank::None, StaffAcademicRank::Proficient),
            (
                StaffAcademicRank::Proficient,
                StaffAcademicRank::SeniorProficient,
            ),
            (
                StaffAcademicRank::SeniorProficient,
                StaffAcademicRank::Expert,
            ),
            (StaffAcademicRank::Expert, StaffAcademicRank::SeniorExpert),
        ] {
            let mut c = current("2020-01-01", rank);
            if rank == StaffAcademicRank::None {
                c.academic_rank.as_mut().unwrap().effective_date = None;
            }
            assert_eq!(
                calculate_rank_milestone(&c, date("2026-10-03")).next_rank,
                Some(next)
            );
        }
        assert_eq!(
            calculate_rank_milestone(
                &current("2020-01-01", StaffAcademicRank::SeniorExpert),
                date("2026-10-03")
            )
            .status,
            RankMilestoneStatus::NoNextRank
        );
        assert_eq!(
            calculate_rank_milestone(
                &current("2020-01-01", StaffAcademicRank::NotApplicable),
                date("2026-10-03")
            )
            .status,
            RankMilestoneStatus::Unsupported
        );
    }
}
