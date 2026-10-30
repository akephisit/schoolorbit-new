//! Shared HTTP and collaboration visibility. Entire blocks/offerings retain their existing scope.
use std::collections::{BTreeMap, BTreeSet};

use school_academic_delivery::models::versions::DeliverySnapshot;
use uuid::Uuid;

use super::TimetableAccessFilter;
use crate::models::timetable_block::{
    TimetableBlock, TimetableBlockSyncStatus, TimetableBlockWorkspace,
};

#[derive(Debug, Clone)]
pub struct TimetableVisibility {
    pub source: DeliverySnapshot,
    pub block_offering_owners: BTreeMap<Uuid, Uuid>,
    pub homeroom_advisors: BTreeMap<Uuid, BTreeSet<Uuid>>,
}

impl TimetableVisibility {
    pub fn offering_visible(&self, id: Uuid, access: &TimetableAccessFilter) -> bool {
        self.source
            .offerings
            .iter()
            .find(|offering| offering.id == id)
            .is_some_and(|offering| {
                access.includes_school_owned
                    || access
                        .organization_unit_ids
                        .contains(&offering.owning_organization_unit_id)
                    || access
                        .organization_tree_unit_ids
                        .contains(&offering.owning_organization_unit_id)
                    || access.assigned_actor_id.is_some_and(|actor| {
                        offering.groups.iter().any(|group| {
                            group
                                .teachers
                                .iter()
                                .any(|teacher| teacher.teacher_id == actor)
                        })
                    })
            })
    }

    pub fn block_visible(&self, block: &TimetableBlock, access: &TimetableAccessFilter) -> bool {
        if access.includes_school_owned {
            return true;
        }
        if block.learning_offering_id.and_then(|id| self.block_offering_owners.get(&id)).is_some_and(|owner| access.organization_unit_ids.contains(owner) || access.organization_tree_unit_ids.contains(owner)) { return true; }
        access.assigned_actor_id.is_some_and(|actor| {
            block
                .groups
                .iter()
                .filter(|group| group.is_active)
                .any(|group| {
                    group
                        .instructors
                        .iter()
                        .any(|teacher| teacher.teacher_id == actor)
                })
                || block
                    .teachers
                    .iter()
                    .any(|teacher| teacher.is_active && teacher.teacher_id == actor)
                || block.homerooms.iter().any(|target| {
                    target.is_active
                        && self
                            .homeroom_advisors
                            .get(&target.homeroom_id)
                            .is_some_and(|advisors| advisors.contains(&actor))
                })
        })
    }
}

pub fn project_workspace(
    canonical: &TimetableBlockWorkspace,
    visibility: &TimetableVisibility,
    access: &TimetableAccessFilter,
) -> TimetableBlockWorkspace {
    let mut result = canonical.clone();
    result
        .blocks
        .retain(|block| visibility.block_visible(block, access));
    let offering_ids = visibility
        .source
        .offerings
        .iter()
        .filter(|offering| visibility.offering_visible(offering.id, access))
        .map(|offering| offering.id)
        .collect::<BTreeSet<_>>();
    result
        .version
        .targets
        .retain(|target| offering_ids.contains(&target.learning_offering_id));
    result
        .learning_groups
        .retain(|group| offering_ids.contains(&group.learning_offering_id));
    result
        .ordinary_demands
        .retain(|demand| offering_ids.contains(&demand.learning_offering_id));
    result
        .synchronized_demands
        .retain(|demand| offering_ids.contains(&demand.learning_offering_id));
    let homeroom_ids = result
        .learning_groups
        .iter()
        .flat_map(|group| group.homeroom_ids.iter().copied())
        .chain(
            result
                .blocks
                .iter()
                .flat_map(|block| block.homerooms.iter().map(|target| target.homeroom_id)),
        )
        .collect::<BTreeSet<_>>();
    result.homerooms.retain(|homeroom| {
        (access.includes_school_owned && homeroom.is_active) || homeroom_ids.contains(&homeroom.id)
    });
    let staff_ids =
        result
            .learning_groups
            .iter()
            .flat_map(|group| {
                group
                    .eligible_instructors
                    .iter()
                    .map(|teacher| teacher.teacher_id)
            })
            .chain(result.blocks.iter().flat_map(|block| {
                block
                    .teachers
                    .iter()
                    .map(|teacher| teacher.teacher_id)
                    .chain(block.groups.iter().flat_map(|group| {
                        group.instructors.iter().map(|teacher| teacher.teacher_id)
                    }))
            }))
            .collect::<BTreeSet<_>>();
    result.staff.retain(|staff| staff_ids.contains(&staff.id));
    let room_ids = result
        .blocks
        .iter()
        .flat_map(|block| {
            block
                .groups
                .iter()
                .filter_map(|group| group.room_id)
                .chain(block.homerooms.iter().filter_map(|target| target.room_id))
        })
        .chain(
            result
                .learning_groups
                .iter()
                .flat_map(|group| group.preferred_room_ids.iter().copied()),
        )
        .collect::<BTreeSet<_>>();
    result.rooms.retain(|room| {
        (access.includes_school_owned && room.status == "ACTIVE") || room_ids.contains(&room.id)
    });
    result.offering_subject_groups.retain(|entry| {
        offering_ids.contains(&entry.learning_offering_id)
            || result
                .blocks
                .iter()
                .any(|block| block.learning_offering_id == Some(entry.learning_offering_id))
    });
    let period_ids = result
        .blocks
        .iter()
        .map(|block| block.bell_schedule_period_id)
        .collect::<BTreeSet<_>>();
    result
        .bell_periods
        .retain(|period| period.is_active || period_ids.contains(&period.id));
    if !access.includes_school_owned {
        result.total_draft_block_count = None;
    }
    recompute_workspace(&mut result, visibility);
    result
}

pub fn recompute_workspace(
    workspace: &mut TimetableBlockWorkspace,
    visibility: &TimetableVisibility,
) {
    for demand in &mut workspace.ordinary_demands {
        demand.scheduled_periods = workspace
            .blocks
            .iter()
            .filter(|block| block.is_active)
            .flat_map(|block| &block.groups)
            .filter(|group| group.is_active && group.learning_group_id == demand.learning_group_id)
            .count() as i32;
        demand.remaining_periods = (demand.required_periods - demand.scheduled_periods).max(0);
    }
    for demand in &mut workspace.synchronized_demands {
        let blocks = workspace
            .blocks
            .iter()
            .filter(|block| {
                block.is_active && block.learning_offering_id == Some(demand.learning_offering_id)
            })
            .collect::<Vec<_>>();
        demand.scheduled_periods = blocks.len() as i32;
        let count = |statuses: &[TimetableBlockSyncStatus]| {
            blocks
                .iter()
                .flat_map(|block| &block.sync_states)
                .filter(|state| statuses.contains(&state.status))
                .map(|state| state.learning_group_id)
                .collect::<BTreeSet<_>>()
                .len() as i32
        };
        demand.linked_group_count = count(&[TimetableBlockSyncStatus::Linked]);
        demand.pending_group_count = count(&[
            TimetableBlockSyncStatus::WaitingForData,
            TimetableBlockSyncStatus::OutsideScope,
        ]);
        demand.conflict_group_count = count(&[TimetableBlockSyncStatus::Conflict]);
        demand.excluded_group_count = count(&[TimetableBlockSyncStatus::Excluded]);
    }
    workspace.summary.block_count = workspace.blocks.len() as i32;
    workspace.summary.ordinary_demand_count = workspace.ordinary_demands.len() as i32;
    workspace.summary.synchronized_demand_count = workspace.synchronized_demands.len() as i32;
    workspace.summary.linked_group_count = workspace
        .synchronized_demands
        .iter()
        .map(|demand| demand.linked_group_count)
        .sum();
    workspace.summary.waiting_group_count = workspace
        .synchronized_demands
        .iter()
        .map(|demand| demand.pending_group_count)
        .sum();
    workspace.summary.conflict_group_count = workspace
        .synchronized_demands
        .iter()
        .map(|demand| demand.conflict_group_count)
        .sum();
    workspace.summary.excluded_group_count = workspace
        .synchronized_demands
        .iter()
        .map(|demand| demand.excluded_group_count)
        .sum();
    workspace.source_issues =
        crate::services::timetable_source::source_issues(&visibility.source, &workspace.blocks);
}
