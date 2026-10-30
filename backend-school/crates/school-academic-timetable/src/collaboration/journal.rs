use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use school_errors::AppError;
use uuid::Uuid;

use super::*;
use crate::models::timetable_block::TimetableBlock;

const JOURNAL_COUNT: usize = 200;
const JOURNAL_BYTES: usize = 16 * 1024 * 1024;

pub(super) struct JournalEntry {
    seq: u64,
    patches: BTreeMap<Uuid, DraftPatch>,
    bytes: usize,
}

impl DraftRoom {
    /// The caller retains the mutation gate and commit pin. Expensive projections are computed
    /// outside the memory mutex, so presence/lease frames never wait for whole-workspace copies.
    pub fn publish_blocks(
        &self,
        blocks: Vec<TimetableBlock>,
        version: Option<crate::models::timetable_version::TimetableVersion>,
        origin_connection_id: Option<Uuid>,
        mutation_id: Uuid,
    ) -> Result<u64, AppError> {
        let (before, visibility, members, previous_seq) = {
            let state = self.lock()?;
            if !state.valid {
                return Err(stale());
            }
            (
                state.workspace.clone(),
                state.visibility.clone(),
                state.members.values().cloned().collect::<Vec<_>>(),
                state.seq,
            )
        };
        let changed_ids = blocks.iter().map(|block| block.id).collect::<BTreeSet<_>>();
        let mut after = (*before).clone();
        if let Some(version) = version {
            if version.id != self.key.timetable_version_id {
                return Err(stale());
            }
            after.version = version;
        }
        for block in blocks {
            let existing = after.blocks.iter().position(|entry| entry.id == block.id);
            match (existing, block.is_active) {
                (Some(index), true) => after.blocks[index] = block,
                (Some(index), false) => {
                    after.blocks.remove(index);
                }
                (None, true) => {
                    if let Some(count) = after.total_draft_block_count.as_mut() {
                        *count += 1;
                    }
                    after.blocks.push(block);
                }
                (None, false) => {}
            }
        }
        after.blocks.sort_by_key(|block| block.id);
        crate::policy::recompute_workspace(&mut after, &visibility);
        let mut patches = BTreeMap::new();
        let mut indices = BTreeMap::new();
        let mut bytes = 32usize;
        for member in members {
            let before_projection =
                crate::policy::project_workspace(&before, &visibility, &member.access);
            let after_projection =
                crate::policy::project_workspace(&after, &visibility, &member.access);
            let patch = project_patch(
                &before_projection,
                &after_projection,
                &changed_ids,
                origin_connection_id,
                mutation_id,
            )?;
            bytes = bytes.saturating_add(serde_json::to_vec(&patch).map_err(|_| stale())?.len());
            indices.insert(
                member.connection_id,
                member_visibility(&after, &visibility, &member),
            );
            patches.insert(member.connection_id, patch);
        }
        let after = Arc::new(after);
        let mut state = self.lock()?;
        if !state.valid || state.seq != previous_seq || !Arc::ptr_eq(&state.workspace, &before) {
            return Err(stale());
        }
        // A disconnect may occur during projection. Its membership and indices must stay removed.
        indices.retain(|connection, _| state.members.contains_key(connection));
        patches.retain(|connection, _| state.members.contains_key(connection));
        for (connection, index) in indices {
            state.member_visibility.insert(connection, index);
            state.projection_rebuilds += 1;
        }
        state.seq = state.seq.checked_add(1).ok_or_else(stale)?;
        let seq = state.seq;
        state.workspace = after;
        state.journal.push_back(JournalEntry {
            seq,
            patches,
            bytes,
        });
        state.journal_bytes = state.journal_bytes.saturating_add(bytes);
        while state.journal.len() > JOURNAL_COUNT || state.journal_bytes > JOURNAL_BYTES {
            if let Some(entry) = state.journal.pop_front() {
                state.journal_bytes = state.journal_bytes.saturating_sub(entry.bytes);
            } else {
                break;
            }
        }
        drop(state);
        let _ = self.signals.send(DraftSignal::Committed(seq));
        Ok(seq)
    }
}

fn changed<T: serde::Serialize>(left: &T, right: &T) -> Result<bool, AppError> {
    Ok(serde_json::to_value(left).map_err(|_| stale())?
        != serde_json::to_value(right).map_err(|_| stale())?)
}

fn lookup_delta<T: serde::Serialize + Clone>(
    before: &[T],
    after: &[T],
    key: impl Fn(&T) -> Uuid,
) -> Result<(Vec<T>, Vec<Uuid>), AppError> {
    let before_by_id = before
        .iter()
        .map(|entry| (key(entry), entry))
        .collect::<BTreeMap<_, _>>();
    let after_ids = after.iter().map(&key).collect::<BTreeSet<_>>();
    let mut upserts = Vec::new();
    for entry in after {
        if match before_by_id.get(&key(entry)) {
            Some(old) => changed(*old, entry)?,
            None => true,
        } {
            upserts.push(entry.clone());
        }
    }
    Ok((
        upserts,
        before_by_id
            .keys()
            .filter(|id| !after_ids.contains(*id))
            .copied()
            .collect(),
    ))
}

fn lookup_changes(
    before: &TimetableBlockWorkspace,
    after: &TimetableBlockWorkspace,
) -> Result<Option<DraftLookupChanges>, AppError> {
    let (upserted_staff, removed_staff_ids) =
        lookup_delta(&before.staff, &after.staff, |entry| entry.id)?;
    let (upserted_homerooms, removed_homeroom_ids) =
        lookup_delta(&before.homerooms, &after.homerooms, |entry| entry.id)?;
    let (upserted_rooms, removed_room_ids) =
        lookup_delta(&before.rooms, &after.rooms, |entry| entry.id)?;
    let (upserted_bell_periods, removed_bell_period_ids) =
        lookup_delta(&before.bell_periods, &after.bell_periods, |entry| entry.id)?;
    let (upserted_offering_subject_groups, removed_subject_group_offering_ids) = lookup_delta(
        &before.offering_subject_groups,
        &after.offering_subject_groups,
        |entry| entry.learning_offering_id,
    )?;
    if upserted_staff.is_empty()
        && removed_staff_ids.is_empty()
        && upserted_homerooms.is_empty()
        && removed_homeroom_ids.is_empty()
        && upserted_rooms.is_empty()
        && removed_room_ids.is_empty()
        && upserted_bell_periods.is_empty()
        && removed_bell_period_ids.is_empty()
        && upserted_offering_subject_groups.is_empty()
        && removed_subject_group_offering_ids.is_empty()
    {
        return Ok(None);
    }
    let staff_order = (!upserted_staff.is_empty() || !removed_staff_ids.is_empty())
        .then(|| after.staff.iter().map(|entry| entry.id).collect());
    let homeroom_order = (!upserted_homerooms.is_empty() || !removed_homeroom_ids.is_empty())
        .then(|| after.homerooms.iter().map(|entry| entry.id).collect());
    let room_order = (!upserted_rooms.is_empty() || !removed_room_ids.is_empty())
        .then(|| after.rooms.iter().map(|entry| entry.id).collect());
    let bell_period_order = (!upserted_bell_periods.is_empty()
        || !removed_bell_period_ids.is_empty())
    .then(|| after.bell_periods.iter().map(|entry| entry.id).collect());
    let subject_group_offering_order = (!upserted_offering_subject_groups.is_empty()
        || !removed_subject_group_offering_ids.is_empty())
    .then(|| {
        after
            .offering_subject_groups
            .iter()
            .map(|entry| entry.learning_offering_id)
            .collect()
    });
    Ok(Some(DraftLookupChanges {
        staff_order,
        homeroom_order,
        room_order,
        bell_period_order,
        subject_group_offering_order,
        upserted_staff,
        removed_staff_ids,
        upserted_homerooms,
        removed_homeroom_ids,
        upserted_rooms,
        removed_room_ids,
        upserted_bell_periods,
        removed_bell_period_ids,
        upserted_offering_subject_groups,
        removed_subject_group_offering_ids,
    }))
}

fn project_patch(
    before: &TimetableBlockWorkspace,
    after: &TimetableBlockWorkspace,
    changed_ids: &BTreeSet<Uuid>,
    origin_connection_id: Option<Uuid>,
    mutation_id: Uuid,
) -> Result<DraftPatch, AppError> {
    let before_ids = before
        .blocks
        .iter()
        .map(|block| block.id)
        .collect::<BTreeSet<_>>();
    let after_ids = after
        .blocks
        .iter()
        .map(|block| block.id)
        .collect::<BTreeSet<_>>();
    let before_blocks = before
        .blocks
        .iter()
        .map(|block| (block.id, block))
        .collect::<BTreeMap<_, _>>();
    let before_ordinary = before
        .ordinary_demands
        .iter()
        .map(|demand| (demand.learning_group_id, demand))
        .collect::<BTreeMap<_, _>>();
    let before_synchronized = before
        .synchronized_demands
        .iter()
        .map(|demand| (demand.learning_offering_id, demand))
        .collect::<BTreeMap<_, _>>();
    let mut upserted_blocks = Vec::new();
    for block in after
        .blocks
        .iter()
        .filter(|block| changed_ids.contains(&block.id))
    {
        if match before_blocks.get(&block.id) {
            Some(old) => changed(*old, block)?,
            None => true,
        } {
            upserted_blocks.push(block.clone());
        }
    }
    let mut ordinary_demands = Vec::new();
    for demand in &after.ordinary_demands {
        if match before_ordinary.get(&demand.learning_group_id) {
            Some(old) => changed(*old, demand)?,
            None => true,
        } {
            ordinary_demands.push(demand.clone());
        }
    }
    let mut synchronized_demands = Vec::new();
    for demand in &after.synchronized_demands {
        if match before_synchronized.get(&demand.learning_offering_id) {
            Some(old) => changed(*old, demand)?,
            None => true,
        } {
            synchronized_demands.push(demand.clone());
        }
    }
    Ok(DraftPatch {
        timetable_version_id: after.version.id,
        origin_connection_id,
        mutation_id,
        upserted_blocks,
        removed_block_ids: before_ids.difference(&after_ids).copied().collect(),
        ordinary_demands,
        synchronized_demands,
        summary: after.summary.clone(),
        source_issues: after.source_issues.clone(),
        total_draft_block_count: after.total_draft_block_count,
        version: Some(after.version.clone()),
        lookup_changes: lookup_changes(before, after)?,
    })
}

pub(super) fn events_after(
    state: &RoomState,
    member: &DraftMember,
    epoch: Uuid,
    after_seq: u64,
    through: Option<u64>,
) -> Result<Vec<DraftEventEnvelope>, AppError> {
    let reconcile = || {
        vec![DraftEventEnvelope {
            epoch: state.epoch,
            seq: state.seq,
            event: DraftServerMessage::DraftReconcileRequired {
                reason: "journal_gap".into(),
            },
        }]
    };
    if epoch != state.epoch
        || after_seq > state.seq
        || (after_seq < state.seq
            && state
                .journal
                .front()
                .is_none_or(|entry| entry.seq > after_seq.saturating_add(1)))
    {
        return Ok(reconcile());
    }
    let mut events = Vec::new();
    for entry in state
        .journal
        .iter()
        .filter(|entry| entry.seq > after_seq && through.is_none_or(|seq| entry.seq <= seq))
    {
        // Snapshots bootstrap a new connection. Replay never substitutes another user's/tab's
        // authorization for the exact connection whose projection was accepted at publication.
        let Some(patch) = entry.patches.get(&member.connection_id) else {
            return Ok(reconcile());
        };
        events.push(DraftEventEnvelope {
            epoch: state.epoch,
            seq: entry.seq,
            event: DraftServerMessage::DraftPatched(patch.clone()),
        });
    }
    Ok(events)
}
