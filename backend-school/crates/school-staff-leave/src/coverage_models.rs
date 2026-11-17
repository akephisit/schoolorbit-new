use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
pub use school_academic_timetable::services::dated_teaching::DatedTeachingLesson;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
macro_rules! dto {($name:ident {$($fields:tt)*})=>{#[derive(Clone,Debug,Serialize,Deserialize)] #[cfg_attr(feature="api-docs",derive(utoipa::ToSchema))] #[serde(rename_all="camelCase",deny_unknown_fields)] pub struct $name {$($fields)*}};}
macro_rules! coverage_enum {($name:ident {$($variant:ident),+})=>{#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)] #[cfg_attr(feature="api-docs",derive(utoipa::ToSchema))] #[serde(rename_all="snake_case")] pub enum $name {$($variant),+}};}
coverage_enum!(LeaveCoverageState {
    Unplanned,
    Incomplete,
    AwaitingConfirmation,
    Confirmed,
    Emergency,
    LegacyUnverified
});
coverage_enum!(LeaveCoverageMode {
    Substitution,
    Reciprocal
});
coverage_enum!(LeaveCoverageConsent {
    Pending,
    Confirmed,
    Declined
});
coverage_enum!(LeaveCoverageTimetableState {
    Ready,
    Missing,
    Legacy
});
dto!(LeaveCoverageContext {pub academic_year_id:Uuid,pub academic_year_name:String,pub academic_term_id:Uuid,pub academic_term_name:String});
dto!(LeaveCoverageOptions {pub default_academic_year_id:Option<Uuid>,pub default_academic_term_id:Option<Uuid>,pub contexts:Vec<LeaveCoverageContext>});
dto!(PreviewLeaveCoverageRequest {pub period:super::models::LeavePeriodInput});
dto!(LeaveCoverageLabel {pub id:Uuid,pub name:String});
dto!(LeaveCoverageAssignment {pub id:Uuid,pub replacement_teacher_id:Uuid,pub replacement_teacher_name:String,pub mode:LeaveCoverageMode,pub taught_offering_id:Option<Uuid>,pub taught_offering_name:Option<String>,pub taught_learning_group_id:Option<Uuid>,pub taught_activity:Option<String>,pub return_lesson_key:Option<String>,pub return_date:Option<NaiveDate>,pub consent:LeaveCoverageConsent,pub confirmed_at:Option<DateTime<Utc>>,#[serde(default,skip_serializing_if="Option::is_none")] pub confirmation_id:Option<Uuid>,#[serde(default,skip_serializing_if="Option::is_none")] pub confirmation_plan_revision:Option<i64>,pub return_lesson:Option<DatedTeachingLesson>});
dto!(LeaveCoverageLesson {#[serde(flatten)] pub lesson:DatedTeachingLesson,pub assignment:Option<LeaveCoverageAssignment>});
dto!(LeaveCoverageDay {pub date:NaiveDate,pub lessons:Vec<LeaveCoverageLesson>});
dto!(LeaveCoveragePlan {pub revision:i64,pub plan_hash:String,pub state:LeaveCoverageState,pub source_digest:Option<String>,pub academic_year_id:Option<Uuid>,pub academic_term_id:Option<Uuid>,pub timetable_state:LeaveCoverageTimetableState,pub days:Vec<LeaveCoverageDay>,pub can_manage:bool,pub blocking_reason:Option<String>,pub emergency_reason:Option<String>});
macro_rules! stored_dto {($name:ident {$($fields:tt)*})=>{#[derive(Clone,Debug,Serialize,Deserialize)] #[serde(rename_all="camelCase",deny_unknown_fields)] pub(crate) struct $name {$($fields)*}};}
stored_dto!(StoredLeaveCoveragePlan {pub revision:i64,pub plan_hash:String,pub state:LeaveCoverageState,pub source_digest:Option<String>,pub academic_year_id:Option<Uuid>,pub academic_term_id:Option<Uuid>,pub half_day_boundary:Option<NaiveTime>,pub timetable_state:LeaveCoverageTimetableState,pub days:Vec<LeaveCoverageDay>,pub can_manage:bool,pub blocking_reason:Option<String>,pub emergency_reason:Option<String>});
impl From<StoredLeaveCoveragePlan> for LeaveCoveragePlan {
    fn from(plan: StoredLeaveCoveragePlan) -> Self {
        Self {
            revision: plan.revision,
            plan_hash: plan.plan_hash,
            state: plan.state,
            source_digest: plan.source_digest,
            academic_year_id: plan.academic_year_id,
            academic_term_id: plan.academic_term_id,
            timetable_state: plan.timetable_state,
            days: plan.days,
            can_manage: plan.can_manage,
            blocking_reason: plan.blocking_reason,
            emergency_reason: plan.emergency_reason,
        }
    }
}

dto!(LeaveCoverageCandidatesRequest {pub expected_revision:i64,pub source_digest:String,pub lesson_keys:Vec<String>,pub search:Option<String>,pub subject_group_id:Option<Uuid>,pub grade_level_id:Option<Uuid>,#[serde(default)] pub same_subject_group_only:bool,#[serde(default)] pub same_student_group_only:bool,#[serde(default)] pub same_grade_only:bool});
dto!(LeaveCoverageCandidate {pub teacher_id:Uuid,pub display_name:String,pub subject_groups:Vec<LeaveCoverageLabel>,pub grade_levels:Vec<LeaveCoverageLabel>,pub is_same_subject_group:bool,pub taught_offerings:Vec<LeaveCoverageLabel>});
dto!(LeaveCoverageCandidates {pub candidates:Vec<LeaveCoverageCandidate>,pub subject_groups:Vec<LeaveCoverageLabel>,pub grade_levels:Vec<LeaveCoverageLabel>,pub has_more:bool});
dto!(LeaveCoverageReturnLessonsRequest {pub expected_revision:i64,pub source_digest:String,pub lesson_key:String,pub replacement_teacher_id:Uuid,pub date:NaiveDate});
dto!(LeaveCoverageReturnLessons {pub lessons:Vec<DatedTeachingLesson>});
dto!(LeaveCoverageSelection {pub lesson_key:String,pub replacement_teacher_id:Uuid,pub mode:LeaveCoverageMode,pub taught_offering_id:Option<Uuid>,pub taught_activity:Option<String>,pub return_lesson_key:Option<String>});
dto!(SaveLeaveCoverageRequest {pub request_id:Uuid,pub expected_revision:i64,pub source_digest:String,pub assignments:Vec<LeaveCoverageSelection>});
dto!(ConfirmLeaveCoverageRequest {pub request_id:Uuid,pub expected_revision:i64,pub assignment_ids:Vec<Uuid>,pub decision:LeaveCoverageConsent});
dto!(SubmitLeaveRequest {pub request_id:Uuid,pub expected_revision:i64,#[serde(default)] pub emergency_reason:Option<String>});
dto!(LeaveCoverageTask {pub request_id:Uuid,pub request_revision:i64,pub plan_revision:i64,pub plan_hash:String,pub owner_name:String,pub assignments:Vec<LeaveCoverageLesson>});
dto!(LeaveCoverageTaskPage {pub items:Vec<LeaveCoverageTask>,pub has_more:bool});
#[derive(Clone, Debug, Default, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::IntoParams))]
#[cfg_attr(feature="api-docs",into_params(parameter_in=Query))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LeaveCoverageTaskQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

impl From<super::models::LeaveMutationRequest> for SubmitLeaveRequest {
    fn from(v: super::models::LeaveMutationRequest) -> Self {
        Self {
            request_id: v.request_id,
            expected_revision: v.expected_revision,
            emergency_reason: None,
        }
    }
}
