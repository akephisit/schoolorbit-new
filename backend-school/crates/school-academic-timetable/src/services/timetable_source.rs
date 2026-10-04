use std::collections::BTreeMap;

use school_academic_delivery::models::versions::DeliverySnapshot;
use school_academic_delivery::models::{
    ActivitySchedulingMode, LearningOfferingSnapshot, LearningTeacherRole,
};
use school_errors::AppError;
use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::models::timetable_block::{TimetableBlock, TimetableBlockKind, TimetableStructuralKind};
use crate::models::timetable_source::{TimetableSourceIssue, TimetableSourceIssueCode};

/// Reconciliation never edits placements or guesses a replacement resource.
pub fn source_issues(
    snapshot: &DeliverySnapshot,
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
                match group
                    .teachers
                    .iter()
                    .find(|teacher| teacher.teacher_id == instructor.teacher_id)
                {
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

#[derive(Serialize)]
struct Content<'a> {
    delivery_version_id: Uuid,
    blocks: Vec<BlockContent<'a>>,
}

#[derive(Serialize)]
struct BlockContent<'a> {
    day: &'a str,
    period: Uuid,
    kind: TimetableBlockKind,
    mode: Option<ActivitySchedulingMode>,
    offering: Option<Uuid>,
    structural_kind: Option<TimetableStructuralKind>,
    title: &'a Option<String>,
    note: &'a Option<String>,
    groups: Vec<GroupContent<'a>>,
    homerooms: Vec<(Uuid, Option<Uuid>)>,
    teachers: Vec<Uuid>,
}

#[derive(Serialize)]
struct GroupContent<'a> {
    id: Uuid,
    room: Option<Uuid>,
    homerooms: Vec<Uuid>,
    instructors: Vec<(Uuid, &'a str, i32)>,
}

/// Clone IDs, row revisions, labels, series IDs and audit fields cannot establish
/// a changed timetable. Sorted content retains duplicate lessons and exact roles.
pub fn content_hash(
    delivery_version_id: Uuid,
    blocks: &[TimetableBlock],
) -> Result<String, AppError> {
    let mut content = Vec::new();
    for block in blocks.iter().filter(|block| block.is_active) {
        let mut groups = block
            .groups
            .iter()
            .filter(|group| group.is_active)
            .map(|group| {
                let mut instructors = group
                    .instructors
                    .iter()
                    .map(|instructor| {
                        (
                            instructor.teacher_id,
                            instructor.role.as_str(),
                            instructor.order_index,
                        )
                    })
                    .collect::<Vec<_>>();
                instructors.sort_unstable();
                let mut homerooms = group.homeroom_ids.clone();
                homerooms.sort_unstable();
                GroupContent {
                    homerooms,
                    id: group.learning_group_id,
                    room: group.room_id,
                    instructors,
                }
            })
            .collect::<Vec<_>>();
        groups.sort_by_key(|group| group.id);
        let mut homerooms = block
            .homerooms
            .iter()
            .filter(|homeroom| homeroom.is_active)
            .map(|homeroom| (homeroom.homeroom_id, homeroom.room_id))
            .collect::<Vec<_>>();
        homerooms.sort_unstable();
        let mut teachers = block
            .teachers
            .iter()
            .filter(|teacher| teacher.is_active)
            .map(|teacher| teacher.teacher_id)
            .collect::<Vec<_>>();
        teachers.sort_unstable();
        content.push(BlockContent {
            day: &block.day_of_week,
            period: block.bell_schedule_period_id,
            kind: block.block_kind,
            mode: block.scheduling_mode,
            offering: block.learning_offering_id,
            structural_kind: block.structural_kind,
            title: &block.title,
            note: &block.note,
            groups,
            homerooms,
            teachers,
        });
    }
    // Serialize once per block to define an order over the entire semantic shape.
    let mut sorted = content
        .into_iter()
        .map(|block| {
            serde_json::to_vec(&block)
                .map(|key| (key, block))
                .map_err(|_| {
                    AppError::InternalServerError("ไม่สามารถเปรียบเทียบเนื้อหาตารางสอนได้".into())
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    sorted.sort_by(|left, right| left.0.cmp(&right.0));
    let bytes = serde_json::to_vec(&Content {
        delivery_version_id,
        blocks: sorted.into_iter().map(|(_, block)| block).collect(),
    })
    .map_err(|_| AppError::InternalServerError("ไม่สามารถเปรียบเทียบเนื้อหาตารางสอนได้".into()))?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveTime, Utc};

    fn structural_block(id: u128, day: &str) -> TimetableBlock {
        TimetableBlock {
            id: Uuid::from_u128(id),
            timetable_version_id: Uuid::from_u128(2),
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
    fn copied_identity_and_revisions_do_not_count_as_schedule_changes() {
        let original = structural_block(1, "MON");
        let mut copy = original.clone();
        copy.id = Uuid::from_u128(20);
        copy.timetable_version_id = Uuid::from_u128(21);
        copy.series_id = Some(Uuid::from_u128(22));
        copy.row_version = 99;
        copy.period_name = "ชื่อคาบที่แก้ไข".into();
        assert_eq!(
            content_hash(Uuid::nil(), &[original]).unwrap(),
            content_hash(Uuid::nil(), &[copy]).unwrap()
        );
    }

    #[test]
    fn placement_and_delivery_source_changes_are_semantic_changes() {
        let original = structural_block(1, "MON");
        let other = structural_block(2, "TUE");
        assert_ne!(
            content_hash(Uuid::nil(), &[original.clone()]).unwrap(),
            content_hash(Uuid::nil(), &[other]).unwrap()
        );
        assert_ne!(
            content_hash(Uuid::nil(), &[original.clone()]).unwrap(),
            content_hash(Uuid::from_u128(1), &[original]).unwrap()
        );
    }

    #[test]
    fn ordering_is_irrelevant_but_duplicate_lesson_content_is_preserved() {
        let first = structural_block(1, "MON");
        let second = structural_block(2, "TUE");
        assert_eq!(
            content_hash(Uuid::nil(), &[first.clone(), second.clone()]).unwrap(),
            content_hash(Uuid::nil(), &[second, first.clone()]).unwrap()
        );
        assert_ne!(
            content_hash(Uuid::nil(), &[first.clone()]).unwrap(),
            content_hash(Uuid::nil(), &[first.clone(), first]).unwrap()
        );
    }

    #[test]
    fn missing_source_keeps_lesson_intact_and_returns_its_id() {
        let mut lesson = structural_block(1, "MON");
        lesson.block_kind = TimetableBlockKind::Course;
        lesson.learning_offering_id = Some(Uuid::from_u128(10));
        let findings = source_issues(&DeliverySnapshot { offerings: vec![] }, &[lesson.clone()]);
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
