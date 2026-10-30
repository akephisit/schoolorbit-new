use std::collections::BTreeSet;
use std::sync::Arc;

use school_errors::AppError;
use uuid::Uuid;

use super::*;
use crate::models::timetable_block::TimetableBlock;

const JOURNAL_COUNT: usize = 200;
const JOURNAL_BYTES: usize = 16 * 1024 * 1024;

pub(super) struct JournalEntry {
    seq: u64,
    before: Arc<TimetableBlockWorkspace>,
    after: Arc<TimetableBlockWorkspace>,
    origin_connection_id: Option<Uuid>,
    mutation_id: Uuid,
    bytes: usize,
}

impl DraftRoom {
    /// Called under the draft mutation gate after a successful transaction, even without listeners.
    pub fn publish_blocks(&self, blocks: Vec<TimetableBlock>, origin_connection_id: Option<Uuid>, mutation_id: Uuid) -> Result<u64, AppError> {
        let mut state = self.lock()?;
        let before = state.workspace.clone();
        let mut after = (*before).clone();
        for block in blocks {
            let existing = after.blocks.iter().position(|entry| entry.id == block.id);
            match (existing, block.is_active) {
                (Some(index), true) => after.blocks[index] = block,
                (Some(index), false) => { after.blocks.remove(index); }
                (None, true) => { if let Some(count) = after.total_draft_block_count.as_mut() { *count += 1; } after.blocks.push(block); }
                (None, false) => {}
            }
        }
        after.blocks.sort_by_key(|block| block.id);
        crate::policy::recompute_workspace(&mut after, &state.visibility);
        // Bound retained snapshots by actual serialized bytes as well as event count.
        let bytes = serde_json::to_vec(&after).map_err(|_| stale())?.len() + serde_json::to_vec(&before).map_err(|_| stale())?.len();
        let after = Arc::new(after);
        state.seq = state.seq.checked_add(1).ok_or_else(stale)?;
        let seq = state.seq;
        state.workspace = after.clone();
        state.journal.push_back(JournalEntry { seq, before, after, origin_connection_id, mutation_id, bytes });
        state.journal_bytes = state.journal_bytes.saturating_add(bytes);
        while state.journal.len() > JOURNAL_COUNT || state.journal_bytes > JOURNAL_BYTES {
            if let Some(entry) = state.journal.pop_front() { state.journal_bytes = state.journal_bytes.saturating_sub(entry.bytes); }
            else { break; }
        }
        drop(state);
        let _ = self.signals.send(DraftSignal::Committed(seq));
        Ok(seq)
    }
}

pub(super) fn events_after(state: &RoomState, member: &DraftMember, epoch: Uuid, after_seq: u64, through: Option<u64>) -> Result<Vec<DraftEventEnvelope>, AppError> {
    if epoch != state.epoch || after_seq > state.seq || (after_seq < state.seq && state.journal.front().is_none_or(|entry| entry.seq > after_seq.saturating_add(1))) {
        return Ok(vec![DraftEventEnvelope { epoch: state.epoch, seq: state.seq, event: DraftServerMessage::DraftReconcileRequired { reason: "journal_gap".into() } }]);
    }
    state.journal.iter().filter(|entry| entry.seq > after_seq && through.is_none_or(|seq| entry.seq <= seq))
        .map(|entry| {
            let before = crate::policy::project_workspace(&entry.before, &state.visibility, &member.access);
            let after = crate::policy::project_workspace(&entry.after, &state.visibility, &member.access);
            let before_ids = before.blocks.iter().map(|block| block.id).collect::<BTreeSet<_>>();
            let after_ids = after.blocks.iter().map(|block| block.id).collect::<BTreeSet<_>>();
            let upserted_blocks = after.blocks.iter().filter(|block| before.blocks.iter().find(|old| old.id == block.id).is_none_or(|old| old.row_version != block.row_version || serde_json::to_value(old).ok() != serde_json::to_value(block).ok())).cloned().collect();
            let ordinary_demands = after.ordinary_demands.iter().filter(|demand| before.ordinary_demands.iter().find(|old| old.learning_group_id == demand.learning_group_id).is_none_or(|old| serde_json::to_value(old).ok() != serde_json::to_value(demand).ok())).cloned().collect();
            let synchronized_demands = after.synchronized_demands.iter().filter(|demand| before.synchronized_demands.iter().find(|old| old.learning_offering_id == demand.learning_offering_id).is_none_or(|old| serde_json::to_value(old).ok() != serde_json::to_value(demand).ok())).cloned().collect();
            Ok(DraftEventEnvelope { epoch: state.epoch, seq: entry.seq, event: DraftServerMessage::DraftPatched(DraftPatch {
                timetable_version_id: state.workspace.version.id, origin_connection_id: entry.origin_connection_id, mutation_id: entry.mutation_id,
                upserted_blocks, removed_block_ids: before_ids.difference(&after_ids).copied().collect(), ordinary_demands, synchronized_demands,
                summary: after.summary, source_issues: after.source_issues, total_draft_block_count: after.total_draft_block_count, version: None,
            }) })
        }).collect()
}
