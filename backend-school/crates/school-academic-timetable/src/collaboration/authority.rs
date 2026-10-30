use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Instant;

use chrono::NaiveTime;
use school_errors::AppError;
use uuid::Uuid;

use super::*;
use crate::models::timetable_block::TimetableBlock;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Identity { Block(Uuid), Group(Uuid), Offering(Uuid) }

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Resource { Teacher(Uuid), Homeroom(Uuid), Room(Uuid), Group(Uuid) }

#[derive(Debug, Clone)]
struct Slot { resource: Resource, day: String, start: NaiveTime, end: NaiveTime }

#[derive(Debug, Clone, Default)]
pub(super) struct Footprint { whole_draft: bool, identities: BTreeSet<Identity>, slots: Vec<Slot> }

impl Footprint {
    fn conflicts(&self, other: &Self) -> bool {
        self.whole_draft || other.whole_draft || !self.identities.is_disjoint(&other.identities)
            || self.slots.iter().any(|left| other.slots.iter().any(|right| left.resource == right.resource && left.day == right.day && left.start < right.end && right.start < left.end))
    }
    fn extend(&mut self, other: Self) { self.whole_draft |= other.whole_draft; self.identities.extend(other.identities); self.slots.extend(other.slots); }
}

pub enum MutationScope {
    WholeDraft,
    Blocks(Vec<Uuid>),
    Placement { source: TimetableBlockPlacementSource, candidate: TimetableBlockPlacementCandidate, targets: Vec<DraftTarget> },
}

/// Pinned exclusion owned by the accepted write, independent of connection and lease expiry.
pub struct CommitPermit { room: Arc<DraftRoom>, id: Uuid, pub actor_id: Uuid }

impl Drop for CommitPermit {
    fn drop(&mut self) { if let Ok(mut state) = self.room.lock() { state.committing.remove(&self.id); } let _ = self.room.signals.send(DraftSignal::Reservations); }
}

impl DraftRoom {
    pub fn begin_commit(self: &Arc<Self>, actor_id: Uuid, session_id: Uuid, identity: Option<&DraftMutationIdentity>, scope: MutationScope) -> Result<CommitPermit, AppError> {
        self.expire(Instant::now());
        let mut state = self.lock()?;
        if !state.valid { return Err(stale()); }
        let own_token = if let Some(identity) = identity {
            let member = state.members.get(&identity.connection_id).ok_or_else(stale)?;
            if member.actor.user_id != actor_id || member.session_id != session_id { return Err(forbidden()); }
            let lease = state.leases.get(&identity.lease_token).ok_or_else(stale)?;
            if lease.public.owner_connection_id != identity.connection_id { return Err(forbidden()); }
            Some(identity.lease_token)
        } else { None };
        let footprint = match scope {
            MutationScope::WholeDraft => Footprint { whole_draft: true, ..Default::default() },
            MutationScope::Blocks(ids) => {
                let mut result = Footprint::default();
                for id in ids { result.extend(block_footprint(state.workspace.blocks.iter().find(|block| block.id == id).ok_or_else(stale)?)); }
                result
            }
            MutationScope::Placement { source, candidate, targets } => {
                let candidate = normalize_candidate(&state.workspace, &source, Some(candidate))?;
                let mut result = source_footprint(&state.workspace, &source)?;
                for target in targets { result.extend(target_footprint(&state.workspace, &source, &candidate, &target)?); }
                result
            }
        };
        if state.leases.iter().any(|(token, lease)| Some(*token) != own_token && footprint.conflicts(&lease.footprint))
            || state.committing.values().any(|other| footprint.conflicts(other)) { return Err(AppError::Conflict("Timetable resources are reserved by another editor".into())); }
        let id = Uuid::new_v4();
        state.committing.insert(id, footprint);
        Ok(CommitPermit { room: self.clone(), id, actor_id })
    }
}

pub(super) fn valid_day(day: &str) -> bool { matches!(day, "MON" | "TUE" | "WED" | "THU" | "FRI" | "SAT" | "SUN") }

fn block_footprint(block: &TimetableBlock) -> Footprint {
    let mut footprint = Footprint::default();
    footprint.identities.insert(Identity::Block(block.id));
    let resources = block.groups.iter().filter(|group| group.is_active).flat_map(|group| {
        std::iter::once(Resource::Group(group.learning_group_id)).chain(group.homeroom_ids.iter().copied().map(Resource::Homeroom))
            .chain(group.instructors.iter().map(|teacher| Resource::Teacher(teacher.teacher_id))).chain(group.room_id.map(Resource::Room))
    }).chain(block.homerooms.iter().filter(|target| target.is_active).flat_map(|target| std::iter::once(Resource::Homeroom(target.homeroom_id)).chain(target.room_id.map(Resource::Room))))
        .chain(block.teachers.iter().filter(|teacher| teacher.is_active).map(|teacher| Resource::Teacher(teacher.teacher_id))).collect::<BTreeSet<_>>();
    footprint.slots = resources.into_iter().map(|resource| Slot { resource, day: block.day_of_week.clone(), start: block.start_time, end: block.end_time }).collect();
    footprint
}

fn source_footprint(workspace: &TimetableBlockWorkspace, source: &TimetableBlockPlacementSource) -> Result<Footprint, AppError> {
    match source {
        TimetableBlockPlacementSource::ExistingBlock { block_id, row_version } => {
            let block = workspace.blocks.iter().find(|block| block.id == *block_id && block.row_version == *row_version).ok_or_else(stale)?;
            Ok(block_footprint(block))
        }
        TimetableBlockPlacementSource::OrdinaryDemand { learning_group_id, learning_offering_id } => {
            if !workspace.ordinary_demands.iter().any(|demand| demand.learning_group_id == *learning_group_id && demand.learning_offering_id == *learning_offering_id) { return Err(stale()); }
            Ok(Footprint { identities: BTreeSet::from([Identity::Group(*learning_group_id)]), ..Default::default() })
        }
        TimetableBlockPlacementSource::SynchronizedOffering { learning_offering_id } => {
            if !workspace.synchronized_demands.iter().any(|demand| demand.learning_offering_id == *learning_offering_id) { return Err(stale()); }
            Ok(Footprint { identities: BTreeSet::from([Identity::Offering(*learning_offering_id)]), ..Default::default() })
        }
    }
}

fn candidate_from_block(block: &TimetableBlock) -> TimetableBlockPlacementCandidate {
    TimetableBlockPlacementCandidate { block_kind: block.block_kind, learning_group_id: block.groups.first().map(|group| group.learning_group_id), learning_offering_id: block.learning_offering_id,
        room_id: block.groups.iter().find_map(|group| group.room_id).or_else(|| block.homerooms.iter().find_map(|target| target.room_id)),
        homeroom_ids: block.groups.iter().flat_map(|group| group.homeroom_ids.iter().copied()).chain(block.homerooms.iter().map(|target| target.homeroom_id)).collect(),
        teacher_ids: block.teachers.iter().map(|teacher| teacher.teacher_id).collect(), instructor_ids: block.groups.iter().flat_map(|group| group.instructors.iter().map(|teacher| teacher.teacher_id)).collect() }
}

fn normalize_candidate(workspace: &TimetableBlockWorkspace, source: &TimetableBlockPlacementSource, candidate: Option<TimetableBlockPlacementCandidate>) -> Result<TimetableBlockPlacementCandidate, AppError> {
    let candidate = match source {
        TimetableBlockPlacementSource::ExistingBlock { block_id, .. } => candidate.unwrap_or(candidate_from_block(workspace.blocks.iter().find(|block| block.id == *block_id).ok_or_else(stale)?)),
        _ => candidate.ok_or_else(|| AppError::ValidationError("A demand claim needs its private candidate".into()))?,
    };
    if candidate.room_id.is_some_and(|id| !workspace.rooms.iter().any(|room| room.id == id && room.status == "ACTIVE"))
        || candidate.homeroom_ids.iter().any(|id| !workspace.homerooms.iter().any(|homeroom| homeroom.id == *id))
        || candidate.teacher_ids.iter().chain(&candidate.instructor_ids).any(|id| !workspace.staff.iter().any(|staff| staff.id == *id && staff.status == "active")) { return Err(forbidden()); }
    match source {
        TimetableBlockPlacementSource::OrdinaryDemand { learning_group_id, learning_offering_id } => {
            let group = workspace.learning_groups.iter().find(|group| group.id == *learning_group_id && group.learning_offering_id == *learning_offering_id).ok_or_else(stale)?;
            if candidate.learning_group_id != Some(*learning_group_id) || candidate.learning_offering_id != Some(*learning_offering_id)
                || candidate.instructor_ids.iter().any(|id| !group.eligible_instructors.iter().any(|teacher| teacher.teacher_id == *id)) { return Err(forbidden()); }
            Ok(TimetableBlockPlacementCandidate { homeroom_ids: group.homeroom_ids.clone(), ..candidate })
        }
        TimetableBlockPlacementSource::SynchronizedOffering { learning_offering_id } if candidate.learning_offering_id != Some(*learning_offering_id) => Err(forbidden()),
        _ => Ok(candidate),
    }
}

fn target_footprint(workspace: &TimetableBlockWorkspace, source: &TimetableBlockPlacementSource, candidate: &TimetableBlockPlacementCandidate, target: &DraftTarget) -> Result<Footprint, AppError> {
    if !valid_day(&target.day_of_week) { return Err(stale()); }
    let period = workspace.bell_periods.iter().find(|period| period.id == target.bell_schedule_period_id).ok_or_else(stale)?;
    let resources = candidate.homeroom_ids.iter().copied().map(Resource::Homeroom).chain(candidate.teacher_ids.iter().chain(&candidate.instructor_ids).copied().map(Resource::Teacher))
        .chain(candidate.room_id.map(Resource::Room)).chain(candidate.learning_group_id.map(Resource::Group)).collect::<BTreeSet<_>>();
    let mut footprint = Footprint { slots: resources.into_iter().map(|resource| Slot { resource, day: target.day_of_week.clone(), start: period.start_time, end: period.end_time }).collect(), ..Default::default() };
    if let Some(block_id) = target.block_id {
        let block = workspace.blocks.iter().find(|block| block.id == block_id && block.day_of_week == target.day_of_week && block.bell_schedule_period_id == target.bell_schedule_period_id).ok_or_else(stale)?;
        footprint.extend(block_footprint(block));
        // A swap occupies the target's resources at the source's original slot too.
        if let TimetableBlockPlacementSource::ExistingBlock { block_id, .. } = source {
            let original = workspace.blocks.iter().find(|block| block.id == *block_id).ok_or_else(stale)?;
            let mut reverse = block_footprint(block);
            for slot in &mut reverse.slots { slot.day = original.day_of_week.clone(); slot.start = original.start_time; slot.end = original.end_time; }
            footprint.extend(reverse);
        }
    }
    Ok(footprint)
}

fn conflict(state: &RoomState, footprint: &Footprint, own_token: Option<Uuid>) -> bool {
    state.leases.iter().any(|(token, lease)| Some(*token) != own_token && footprint.conflicts(&lease.footprint)) || state.committing.values().any(|other| footprint.conflicts(other))
}

pub(super) fn lease_command(room: &DraftRoom, connection_id: Uuid, command: DraftClientMessage) -> Result<Vec<DraftEventEnvelope>, AppError> {
    let mut state = room.lock()?;
    if !state.valid { return Err(stale()); }
    let member = state.members.get(&connection_id).ok_or_else(stale)?.clone();
    let manage = member.manage_access.as_ref().ok_or_else(forbidden)?;
    let projected = crate::policy::project_workspace(&state.workspace, &state.visibility, manage);
    let (request_id, kind) = match &command {
        DraftClientMessage::ClientClaim(value) => (value.request_id, DraftCommandKind::Claim),
        DraftClientMessage::ClientTarget(value) => (value.request_id, DraftCommandKind::Target),
        DraftClientMessage::ClientRenew(value) => (value.request_id, DraftCommandKind::Renew),
        DraftClientMessage::ClientRelease(value) => (value.request_id, DraftCommandKind::Release),
        _ => return Err(stale()),
    };
    let mut result = DraftCommandResult { request_id, command: kind, accepted: false, code: None, source: None, target: None, reservation_id: None, lease_token: None, expires_at: None };
    match command {
        DraftClientMessage::ClientClaim(claim) => {
            result.source = Some(claim.source.clone());
            if !source_visible(&projected, &claim.source) { return Err(forbidden()); }
            let footprint = source_footprint(&state.workspace, &claim.source)?;
            let candidate = normalize_candidate(&projected, &claim.source, claim.candidate)?;
            if conflict(&state, &footprint, None) { result.code = Some("reserved".into()); }
            else {
                let token = Uuid::new_v4();
                let public = DraftReservation { reservation_id: Uuid::new_v4(), owner_connection_id: connection_id, purpose: claim.purpose, source: claim.source, target: None, expires_at: expires_at() };
                result.accepted = true; result.reservation_id = Some(public.reservation_id); result.lease_token = Some(token); result.expires_at = Some(public.expires_at);
                // One active interaction per connection, after new claim succeeds atomically.
                state.leases.retain(|_, lease| lease.public.owner_connection_id != connection_id);
                state.leases.insert(token, Lease { public, token, candidate: Some(candidate), footprint, deadline: Instant::now() + LEASE_TTL });
            }
        }
        DraftClientMessage::ClientTarget(update) => {
            let lease = state.leases.get(&update.lease_token).filter(|lease| lease.public.owner_connection_id == connection_id).ok_or_else(stale)?;
            let source = lease.public.source.clone(); result.source = Some(source.clone()); result.target = update.target.clone();
            let candidate = normalize_candidate(&projected, &source, update.candidate.or_else(|| lease.candidate.clone()))?;
            let mut footprint = source_footprint(&state.workspace, &source)?;
            if let Some(target) = &update.target {
                if target.block_id.is_some_and(|id| !projected.blocks.iter().any(|block| block.id == id)) { return Err(forbidden()); }
                footprint.extend(target_footprint(&state.workspace, &source, &candidate, target)?);
            }
            if conflict(&state, &footprint, Some(update.lease_token)) { result.code = Some("reserved".into()); }
            else {
                let lease = state.leases.get_mut(&update.lease_token).ok_or_else(stale)?;
                lease.public.target = update.target; lease.public.expires_at = expires_at(); lease.deadline = Instant::now() + LEASE_TTL; lease.candidate = Some(candidate); lease.footprint = footprint;
                grant_result(&mut result, lease);
            }
        }
        DraftClientMessage::ClientRenew(command) => {
            let lease = state.leases.get_mut(&command.lease_token).filter(|lease| lease.public.owner_connection_id == connection_id).ok_or_else(stale)?;
            lease.public.expires_at = expires_at(); lease.deadline = Instant::now() + LEASE_TTL; grant_result(&mut result, lease);
        }
        DraftClientMessage::ClientRelease(command) => {
            let lease = state.leases.get(&command.lease_token).filter(|lease| lease.public.owner_connection_id == connection_id).ok_or_else(stale)?;
            result.source = Some(lease.public.source.clone()); result.target = lease.public.target.clone(); result.accepted = true;
            state.leases.remove(&command.lease_token);
        }
        _ => return Err(stale()),
    }
    let event = DraftEventEnvelope { epoch: state.epoch, seq: state.seq, event: DraftServerMessage::DraftCommandResult(result) };
    drop(state);
    let _ = room.signals.send(DraftSignal::Reservations);
    Ok(vec![event])
}

fn grant_result(result: &mut DraftCommandResult, lease: &Lease) {
    result.accepted = true; result.source = Some(lease.public.source.clone()); result.target = lease.public.target.clone(); result.reservation_id = Some(lease.public.reservation_id); result.lease_token = Some(lease.token); result.expires_at = Some(lease.public.expires_at);
}

pub(super) fn availability(state: &RoomState, member: &DraftMember) -> Option<DraftAvailability> {
    let own = state.leases.values().find(|lease| lease.public.owner_connection_id == member.connection_id)?;
    let candidate = own.candidate.as_ref()?;
    let mut blocked_cells = Vec::new();
    for day in ["MON", "TUE", "WED", "THU", "FRI", "SAT", "SUN"] {
        for period in &state.workspace.bell_periods {
            let target = DraftTarget { day_of_week: day.into(), bell_schedule_period_id: period.id, block_id: None };
            let footprint = target_footprint(&state.workspace, &own.public.source, candidate, &target).ok()?;
            for other in state.leases.values().filter(|lease| lease.token != own.token && footprint.conflicts(&lease.footprint)) {
                blocked_cells.push(DraftBlockedCell { day_of_week: day.into(), bell_schedule_period_id: period.id, reservation_id: other.public.reservation_id, owner_connection_id: other.public.owner_connection_id, purpose: other.public.purpose });
            }
        }
    }
    Some(DraftAvailability { source: own.public.source.clone(), context: member.context.clone(), blocked_cells })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn slot(resource: Resource, start: &str, end: &str) -> Footprint { Footprint { slots: vec![Slot { resource, day: "MON".into(), start: start.parse().unwrap(), end: end.parse().unwrap() }], ..Default::default() } }
    #[test]
    fn resource_intervals_conflict_across_views_but_adjacent_periods_do_not() {
        let id = Uuid::new_v4();
        let teacher = slot(Resource::Teacher(id), "08:00", "09:00");
        assert!(teacher.conflicts(&slot(Resource::Teacher(id), "08:30", "09:30")));
        assert!(!teacher.conflicts(&slot(Resource::Teacher(id), "09:00", "10:00")));
        assert!(!teacher.conflicts(&slot(Resource::Room(id), "08:00", "09:00")));
    }
    #[test]
    fn source_identities_and_draft_wide_exclusion_are_atomic_footprints() {
        let id = Uuid::new_v4();
        let source = Footprint { identities: BTreeSet::from([Identity::Group(id)]), ..Default::default() };
        assert!(source.conflicts(&source));
        assert!(source.conflicts(&Footprint { whole_draft: true, ..Default::default() }));
        assert!(!source.conflicts(&Footprint::default()));
    }
}
