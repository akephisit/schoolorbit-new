use std::collections::HashSet;

use uuid::Uuid;

use crate::error::AppError;
use crate::modules::supervision::models::{
    EvaluatorAssignmentInput, SupervisionCycleStatus, SupervisionEvaluatorStatus,
    SupervisionObservationStatus, SupervisionTargetType, SupervisionTemplateItemType,
    SupervisionTemplateStatus, SupervisionTemplateStepActionKind, SupervisionTemplateStepActorKind,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SupervisionTargetRule {
    pub target_type: SupervisionTargetType,
    pub target_id: Option<Uuid>,
    pub required_observations: i32,
    pub priority: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupervisionTargetMatch {
    pub staff_user_id: Uuid,
    pub subject_group_ids: Vec<Uuid>,
    pub organization_unit_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EvaluatorRatingInput {
    pub submitted: bool,
    pub rating_scores: Vec<Option<f64>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvaluatorSubmissionState {
    pub is_required: bool,
    pub submitted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct EvaluatorReplacementState {
    pub(super) evaluator_user_id: Uuid,
    pub(super) submitted: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SupervisionObservationListAccess {
    pub school: bool,
    pub own_user_id: Option<Uuid>,
    pub assigned_user_id: Option<Uuid>,
    pub organization_unit_ids: Vec<Uuid>,
}

impl SupervisionObservationListAccess {
    pub fn school() -> Self {
        Self {
            school: true,
            ..Self::default()
        }
    }

    pub fn is_empty(&self) -> bool {
        !self.school
            && self.own_user_id.is_none()
            && self.assigned_user_id.is_none()
            && self.organization_unit_ids.is_empty()
    }
}

pub fn resolve_supervision_target_rule<'a>(
    rules: &'a [SupervisionTargetRule],
    staff_match: &SupervisionTargetMatch,
) -> Option<&'a SupervisionTargetRule> {
    rules
        .iter()
        .filter(|rule| target_rule_matches(rule, staff_match))
        .min_by_key(|rule| (target_specificity_rank(rule.target_type), rule.priority))
}

pub fn teacher_can_edit_requested_observation(status: SupervisionObservationStatus) -> bool {
    status == SupervisionObservationStatus::Requested
}

pub fn manager_can_edit_observation(status: SupervisionObservationStatus) -> bool {
    matches!(
        status,
        SupervisionObservationStatus::Requested
            | SupervisionObservationStatus::Planned
            | SupervisionObservationStatus::Returned
    )
}

pub(super) fn normalize_evaluator_replacement(
    existing: &[EvaluatorReplacementState],
    requested: Vec<EvaluatorAssignmentInput>,
) -> Result<Vec<EvaluatorAssignmentInput>, AppError> {
    let mut normalized = Vec::new();
    let mut seen = HashSet::new();

    for evaluator in existing.iter().filter(|evaluator| evaluator.submitted) {
        if seen.insert(evaluator.evaluator_user_id) {
            normalized.push(EvaluatorAssignmentInput {
                evaluator_user_id: evaluator.evaluator_user_id,
                role_label: None,
                is_required: Some(true),
            });
        }
    }

    for evaluator in requested {
        if seen.insert(evaluator.evaluator_user_id) {
            normalized.push(evaluator);
        } else if !existing
            .iter()
            .any(|state| state.submitted && state.evaluator_user_id == evaluator.evaluator_user_id)
        {
            if let Some(existing_evaluator) = normalized
                .iter_mut()
                .find(|item| item.evaluator_user_id == evaluator.evaluator_user_id)
            {
                *existing_evaluator = evaluator;
            }
        }
    }

    if normalized.is_empty() {
        return Err(AppError::ValidationError(
            "ต้องมีผู้ประเมินอย่างน้อย 1 คน".to_string(),
        ));
    }

    if !has_required_evaluator(&normalized) {
        return Err(AppError::ValidationError(
            "ต้องมีผู้ประเมินหลักอย่างน้อย 1 คน".to_string(),
        ));
    }

    Ok(normalized)
}

pub(super) fn has_required_evaluator(evaluators: &[EvaluatorAssignmentInput]) -> bool {
    evaluators
        .iter()
        .any(|evaluator| evaluator.is_required.unwrap_or(true))
}

pub fn average_submitted_evaluator_rating(inputs: &[EvaluatorRatingInput]) -> Option<f64> {
    let evaluator_averages = inputs
        .iter()
        .filter(|input| input.submitted)
        .filter_map(|input| {
            let ratings: Vec<f64> = input.rating_scores.iter().flatten().copied().collect();
            if ratings.is_empty() {
                None
            } else {
                Some(ratings.iter().sum::<f64>() / ratings.len() as f64)
            }
        })
        .collect::<Vec<_>>();

    if evaluator_averages.is_empty() {
        return None;
    }

    Some(evaluator_averages.iter().sum::<f64>() / evaluator_averages.len() as f64)
}

pub fn can_view_observation_results(
    status: SupervisionObservationStatus,
    can_view_unreleased_results: bool,
) -> bool {
    can_view_unreleased_results
        || matches!(
            status,
            SupervisionObservationStatus::Published
                | SupervisionObservationStatus::Acknowledged
                | SupervisionObservationStatus::Completed
        )
}

pub fn all_required_evaluators_submitted(states: &[EvaluatorSubmissionState]) -> bool {
    states
        .iter()
        .filter(|state| state.is_required)
        .all(|state| state.submitted)
}

pub fn can_transition_observation_status(
    from: SupervisionObservationStatus,
    to: SupervisionObservationStatus,
) -> bool {
    use SupervisionObservationStatus::{
        Acknowledged, Approved, Cancelled, Completed, EvaluatorsSubmitted, InProgress, Planned,
        Published, Requested, Returned,
    };

    if from == to {
        return true;
    }

    matches!(
        (from, to),
        (Requested, Planned)
            | (Requested, Returned)
            | (Requested, Cancelled)
            | (Returned, Planned)
            | (Returned, Cancelled)
            | (Planned, InProgress)
            | (Planned, EvaluatorsSubmitted)
            | (Planned, Returned)
            | (Planned, Cancelled)
            | (InProgress, EvaluatorsSubmitted)
            | (InProgress, Returned)
            | (InProgress, Cancelled)
            | (EvaluatorsSubmitted, Approved)
            | (EvaluatorsSubmitted, Cancelled)
            | (Approved, Published)
            | (Approved, Cancelled)
            | (Published, Completed)
            | (Acknowledged, Completed)
    )
}

fn target_rule_matches(rule: &SupervisionTargetRule, staff_match: &SupervisionTargetMatch) -> bool {
    match rule.target_type {
        SupervisionTargetType::School => rule.target_id.is_none(),
        SupervisionTargetType::OrganizationUnit => rule
            .target_id
            .is_some_and(|id| staff_match.organization_unit_ids.contains(&id)),
        SupervisionTargetType::SubjectGroup => rule
            .target_id
            .is_some_and(|id| staff_match.subject_group_ids.contains(&id)),
        SupervisionTargetType::Staff => rule.target_id == Some(staff_match.staff_user_id),
    }
}

fn target_specificity_rank(target_type: SupervisionTargetType) -> i32 {
    match target_type {
        SupervisionTargetType::Staff => 0,
        SupervisionTargetType::SubjectGroup => 1,
        SupervisionTargetType::OrganizationUnit => 2,
        SupervisionTargetType::School => 3,
    }
}

pub(super) fn parse_cycle_status(code: &str) -> Result<SupervisionCycleStatus, AppError> {
    SupervisionCycleStatus::from_code(code)
        .ok_or_else(|| AppError::InternalServerError("สถานะรอบนิเทศในฐานข้อมูลไม่ถูกต้อง".to_string()))
}

pub(super) fn parse_template_status(code: &str) -> Result<SupervisionTemplateStatus, AppError> {
    SupervisionTemplateStatus::from_code(code).ok_or_else(|| {
        AppError::InternalServerError("สถานะแบบประเมินนิเทศในฐานข้อมูลไม่ถูกต้อง".to_string())
    })
}

pub(super) fn parse_target_type(code: &str) -> Result<SupervisionTargetType, AppError> {
    SupervisionTargetType::from_code(code).ok_or_else(|| {
        AppError::InternalServerError("ประเภทเป้าหมายนิเทศในฐานข้อมูลไม่ถูกต้อง".to_string())
    })
}

pub(super) fn parse_template_item_type(
    code: &str,
) -> Result<SupervisionTemplateItemType, AppError> {
    SupervisionTemplateItemType::from_code(code).ok_or_else(|| {
        AppError::InternalServerError("ประเภทหัวข้อแบบประเมินในฐานข้อมูลไม่ถูกต้อง".to_string())
    })
}

pub(super) fn parse_step_actor_kind(
    code: &str,
) -> Result<SupervisionTemplateStepActorKind, AppError> {
    SupervisionTemplateStepActorKind::from_code(code)
        .ok_or_else(|| AppError::InternalServerError("ประเภทผู้ดำเนินการขั้นตอนนิเทศไม่ถูกต้อง".to_string()))
}

pub(super) fn parse_step_action_kind(
    code: &str,
) -> Result<SupervisionTemplateStepActionKind, AppError> {
    SupervisionTemplateStepActionKind::from_code(code).ok_or_else(|| {
        AppError::InternalServerError("ประเภทการดำเนินการขั้นตอนนิเทศไม่ถูกต้อง".to_string())
    })
}

pub(super) fn parse_observation_status(
    code: &str,
) -> Result<SupervisionObservationStatus, AppError> {
    SupervisionObservationStatus::from_code(code)
        .ok_or_else(|| AppError::InternalServerError("สถานะรายการนิเทศในฐานข้อมูลไม่ถูกต้อง".to_string()))
}

pub(super) fn parse_optional_observation_status(
    code: Option<String>,
) -> Result<Option<SupervisionObservationStatus>, AppError> {
    code.map(|value| parse_observation_status(&value))
        .transpose()
}

pub(super) fn parse_evaluator_status(code: &str) -> Result<SupervisionEvaluatorStatus, AppError> {
    SupervisionEvaluatorStatus::from_code(code).ok_or_else(|| {
        AppError::InternalServerError("สถานะผู้ประเมินนิเทศในฐานข้อมูลไม่ถูกต้อง".to_string())
    })
}

pub fn evaluator_conflict_status_codes() -> &'static [&'static str] {
    &[
        "planned",
        "in_progress",
        "evaluators_submitted",
        "approved",
        "published",
        "completed",
    ]
}
