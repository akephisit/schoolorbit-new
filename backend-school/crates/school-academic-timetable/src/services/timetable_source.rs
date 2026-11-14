use std::collections::BTreeMap;

use school_academic_delivery::models::current::DeliveryGraph;
use school_academic_delivery::models::{
    ActivitySchedulingMode, LearningOfferingSnapshot, LearningTeacherRole,
};
#[cfg(test)]
use uuid::Uuid;

#[cfg(test)]
use crate::models::timetable_block::TimetableStructuralKind;
use crate::models::timetable_block::{TimetableBlock, TimetableBlockKind};
use crate::models::timetable_source::{TimetableSourceIssue, TimetableSourceIssueCode};

/// Reconciliation never edits placements or guesses a replacement resource.
pub fn source_issues(
    snapshot: &DeliveryGraph,
    blocks: &[TimetableBlock],
) -> Vec<TimetableSourceIssue> {
    let offerings = snapshot
        .offerings
        .iter()
        .map(|offering| (offering.id, offering))
        .collect::<BTreeMap<_, _>>();
    let mut issues = Vec::new();
    for block in blocks
        .iter()
        .filter(|block| block.is_active && block.block_kind != TimetableBlockKind::Structural)
    {
        let mut record = |code, learning_group_id, teacher_id| {
            issues.push(TimetableSourceIssue {
                block_id: block.id,
                learning_group_id,
                teacher_id,
                code,
            })
        };
        let Some(offering) = block.learning_offering_id.and_then(|id| offerings.get(&id)) else {
            record(TimetableSourceIssueCode::MissingOffering, None, None);
            continue;
        };
        let expected_mode = match &offering.catalog {
            LearningOfferingSnapshot::Course(_) => ActivitySchedulingMode::Independent,
            LearningOfferingSnapshot::Activity(catalog) => catalog.scheduling_mode,
        };
        if block.scheduling_mode != Some(expected_mode) {
            record(TimetableSourceIssueCode::SchedulingModeMismatch, None, None);
        }
        for placement in block.groups.iter().filter(|group| group.is_active) {
            if placement.learning_offering_id != offering.id {
                record(
                    TimetableSourceIssueCode::GroupOfferingMismatch,
                    Some(placement.learning_group_id),
                    None,
                );
                continue;
            }
            let Some(group) = offering
                .groups
                .iter()
                .find(|group| group.id == placement.learning_group_id)
            else {
                record(
                    TimetableSourceIssueCode::MissingGroup,
                    Some(placement.learning_group_id),
                    None,
                );
                continue;
            };
            let mut actual_coverage = placement.homeroom_ids.clone();
            let mut expected_coverage = group.homeroom_ids.clone();
            actual_coverage.sort_unstable();
            expected_coverage.sort_unstable();
            if actual_coverage != expected_coverage {
                record(
                    TimetableSourceIssueCode::HomeroomCoverageMismatch,
                    Some(group.id),
                    None,
                );
            }
            if placement.instructors.is_empty() {
                record(
                    TimetableSourceIssueCode::MissingInstructor,
                    Some(group.id),
                    None,
                );
            }
            for instructor in &placement.instructors {
                match group.teachers.iter().find(|teacher| {
                    teacher.teacher_id == instructor.teacher_id && teacher.is_active
                }) {
                    None => record(
                        TimetableSourceIssueCode::IneligibleInstructor,
                        Some(group.id),
                        Some(instructor.teacher_id),
                    ),
                    Some(teacher) if role_text(teacher.role) != instructor.role => record(
                        TimetableSourceIssueCode::InstructorRoleMismatch,
                        Some(group.id),
                        Some(instructor.teacher_id),
                    ),
                    Some(_) => {}
                }
            }
        }
    }
    issues
}

pub(crate) fn role_text(role: LearningTeacherRole) -> &'static str {
    match role {
        LearningTeacherRole::Primary => "primary",
        LearningTeacherRole::Secondary => "secondary",
        LearningTeacherRole::Assistant => "assistant",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveTime, Utc};

    fn structural_block(id: u128, day: &str) -> TimetableBlock {
        TimetableBlock {
            id: Uuid::from_u128(id),
            timetable_id: Uuid::from_u128(2),
            academic_term_id: Uuid::from_u128(3),
            academic_year_id: Uuid::from_u128(4),
            bell_schedule_id: Uuid::from_u128(5),
            bell_schedule_period_id: Uuid::from_u128(6),
            period_name: "คาบที่ 1".into(),
            start_time: NaiveTime::from_hms_opt(8, 0, 0).unwrap(),
            end_time: NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
            day_of_week: day.into(),
            block_kind: TimetableBlockKind::Structural,
            scheduling_mode: None,
            learning_offering_id: None,
            offering_code: None,
            offering_name: None,
            structural_kind: Some(TimetableStructuralKind::Break),
            title: Some("พัก".into()),
            note: None,
            series_id: Some(Uuid::from_u128(id)),
            groups: vec![],
            homerooms: vec![],
            teachers: vec![],
            sync_states: vec![],
            row_version: 1,
            is_active: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn missing_source_keeps_lesson_intact_and_returns_its_id() {
        let mut lesson = structural_block(1, "MON");
        lesson.block_kind = TimetableBlockKind::Course;
        lesson.learning_offering_id = Some(Uuid::from_u128(10));
        let findings = source_issues(&DeliveryGraph { offerings: vec![] }, &[lesson.clone()]);
        assert_eq!(
            findings,
            vec![TimetableSourceIssue {
                block_id: lesson.id,
                learning_group_id: None,
                teacher_id: None,
                code: TimetableSourceIssueCode::MissingOffering,
            }]
        );
        assert!(lesson.is_active);
        assert_eq!(lesson.learning_offering_id, Some(Uuid::from_u128(10)));
    }
}
