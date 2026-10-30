//! Process-local draft authority. Socket transport never owns reservations or committed state.
mod authority;
mod journal;
#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use school_authorization::ActorContext;
use school_errors::AppError;
use tokio::sync::{broadcast, OwnedMutexGuard, OwnedSemaphorePermit, Semaphore};
use uuid::Uuid;

use crate::models::collaboration::*;
use crate::models::timetable_block::{TimetableBlockPlacementCandidate, TimetableBlockPlacementSource, TimetableBlockWorkspace};
use crate::policy::{TimetableAccessFilter, TimetableVisibility};

pub use authority::{CommitPermit, MutationScope};

pub const LEASE_TTL: Duration = Duration::from_secs(10);
const MAX_ROOMS: usize = 128;
const MAX_CONNECTIONS_PER_ROOM: usize = 128;
const MAX_ACCEPTED_WRITES: usize = 64;
const ROOM_IDLE_TTL: Duration = Duration::from_secs(600);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DraftKey { pub tenant_id: Uuid, pub academic_term_id: Uuid, pub timetable_version_id: Uuid }

#[derive(Debug, Clone)]
pub enum DraftSignal { Presence(Uuid), Left(Uuid), Reservations, Committed(u64), Invalidated }

#[derive(Clone)]
pub struct DraftMember {
    pub connection_id: Uuid,
    pub actor: ActorContext,
    pub session_id: Uuid,
    pub name: String,
    pub color: String,
    pub access: TimetableAccessFilter,
    pub manage_access: Option<TimetableAccessFilter>,
    /// Already authorized for this viewer by the Staff provider at join.
    pub portraits: BTreeMap<Uuid, Uuid>,
    pub context: Option<DraftContext>,
    pub anchor: Option<DraftAnchor>,
}

struct Lease {
    public: DraftReservation,
    token: Uuid,
    candidate: Option<TimetableBlockPlacementCandidate>,
    footprint: authority::Footprint,
    deadline: Instant,
}

struct RoomState {
    workspace: Arc<TimetableBlockWorkspace>,
    visibility: Arc<TimetableVisibility>,
    members: BTreeMap<Uuid, DraftMember>,
    leases: BTreeMap<Uuid, Lease>,
    committing: BTreeMap<Uuid, authority::Footprint>,
    epoch: Uuid,
    seq: u64,
    journal: VecDeque<journal::JournalEntry>,
    journal_bytes: usize,
    valid: bool,
    empty_since: Option<Instant>,
}

pub struct DraftRoom {
    pub key: DraftKey,
    state: Mutex<RoomState>,
    gate: Arc<tokio::sync::Mutex<()>>,
    signals: broadcast::Sender<DraftSignal>,
}

impl DraftRoom {
    fn lock(&self) -> Result<MutexGuard<'_, RoomState>, AppError> {
        self.state.lock().map_err(|_| AppError::InternalServerError("Draft collaboration state unavailable".into()))
    }

    pub fn subscribe(&self) -> broadcast::Receiver<DraftSignal> { self.signals.subscribe() }

    pub async fn mutation_gate(&self) -> OwnedMutexGuard<()> { self.gate.clone().lock_owned().await }

    pub fn join(&self, member: DraftMember) -> Result<DraftEventEnvelope, AppError> {
        let mut state = self.lock()?;
        if !state.valid { return Err(stale()); }
        if state.members.len() >= MAX_CONNECTIONS_PER_ROOM { return Err(AppError::Conflict("Draft connection limit reached".into())); }
        let connection_id = member.connection_id;
        state.members.insert(connection_id, member);
        state.empty_since = None;
        let event = state_sync(&state, connection_id)?;
        drop(state);
        let _ = self.signals.send(DraftSignal::Presence(connection_id));
        Ok(event)
    }

    pub fn disconnect(&self, connection_id: Uuid) {
        if let Ok(mut state) = self.lock() {
            state.members.remove(&connection_id);
            state.leases.retain(|_, lease| lease.public.owner_connection_id != connection_id);
            // In-flight committing footprints are owned by their permit, never by socket lifetime.
            if state.members.is_empty() { state.empty_since = Some(Instant::now()); }
        }
        let _ = self.signals.send(DraftSignal::Left(connection_id));
        let _ = self.signals.send(DraftSignal::Reservations);
    }

    pub fn invalidate(&self) {
        if let Ok(mut state) = self.lock() { state.valid = false; state.leases.clear(); }
        let _ = self.signals.send(DraftSignal::Invalidated);
    }

    pub fn update_presence(&self, connection_id: Uuid, update: DraftPresenceUpdate) -> Result<(), AppError> {
        let mut state = self.lock()?;
        let member = state.members.get(&connection_id).ok_or_else(stale)?;
        let projected = crate::policy::project_workspace(&state.workspace, &state.visibility, &member.access);
        if !context_visible(&projected, &update.context) || update.anchor.as_ref().is_some_and(|anchor| !anchor_visible(&projected, anchor)) { return Err(forbidden()); }
        let member = state.members.get_mut(&connection_id).ok_or_else(stale)?;
        member.context = Some(update.context);
        member.anchor = update.anchor;
        drop(state);
        let _ = self.signals.send(DraftSignal::Presence(connection_id));
        Ok(())
    }

    pub fn expire(&self, now: Instant) {
        let changed = self.lock().map(|mut state| {
            let before = state.leases.len();
            state.leases.retain(|_, lease| lease.deadline > now);
            before != state.leases.len()
        }).unwrap_or(false);
        if changed { let _ = self.signals.send(DraftSignal::Reservations); }
    }

    pub fn signal_events(&self, viewer: Uuid, signal: DraftSignal) -> Result<Vec<DraftEventEnvelope>, AppError> {
        let state = self.lock()?;
        let member = state.members.get(&viewer).ok_or_else(stale)?;
        let event = |event| DraftEventEnvelope { epoch: state.epoch, seq: state.seq, event };
        match signal {
            DraftSignal::Presence(connection_id) => {
                let Some(changed) = state.members.get(&connection_id) else { return Ok(Vec::new()); };
                Ok(vec![event(DraftServerMessage::DraftPresenceChanged(participant(&state, member, changed.actor.user_id)))])
            }
            DraftSignal::Left(connection_id) => Ok(vec![event(DraftServerMessage::DraftPresenceLeft { connection_id })]),
            DraftSignal::Reservations => {
                let mut events = vec![event(DraftServerMessage::DraftReservationsChanged { reservations: reservations(&state, member) })];
                if let Some(availability) = authority::availability(&state, member) { events.push(event(DraftServerMessage::DraftAvailability(availability))); }
                Ok(events)
            }
            DraftSignal::Committed(seq) => journal::events_after(&state, member, state.epoch, seq.saturating_sub(1), Some(seq)),
            DraftSignal::Invalidated => Ok(vec![event(DraftServerMessage::DraftReconcileRequired { reason: "draft_invalidated".into() })]),
        }
    }

    pub fn command(&self, connection_id: Uuid, command: DraftClientMessage) -> Result<Vec<DraftEventEnvelope>, AppError> {
        self.expire(Instant::now());
        match command {
            DraftClientMessage::ClientPresenceUpdate(update) => { self.update_presence(connection_id, update)?; Ok(Vec::new()) }
            DraftClientMessage::ClientReplay(replay) => {
                let state = self.lock()?;
                let member = state.members.get(&connection_id).ok_or_else(stale)?;
                journal::events_after(&state, member, replay.epoch, replay.after_seq, None)
            }
            command => authority::lease_command(self, connection_id, command),
        }
    }
}

pub struct TimetableCollaborationRuntime {
    rooms: Mutex<BTreeMap<DraftKey, Arc<DraftRoom>>>,
    writes: Arc<Semaphore>,
}

impl Default for TimetableCollaborationRuntime { fn default() -> Self { Self::new() } }

impl TimetableCollaborationRuntime {
    pub fn new() -> Self { Self { rooms: Mutex::new(BTreeMap::new()), writes: Arc::new(Semaphore::new(MAX_ACCEPTED_WRITES)) } }

    pub fn room(&self, key: DraftKey) -> Result<Option<Arc<DraftRoom>>, AppError> {
        Ok(self.rooms.lock().map_err(|_| stale())?.get(&key).cloned())
    }

    pub fn install(&self, key: DraftKey, workspace: TimetableBlockWorkspace, visibility: TimetableVisibility) -> Result<Arc<DraftRoom>, AppError> {
        if workspace.version.id != key.timetable_version_id || workspace.version.academic_term_id != key.academic_term_id { return Err(stale()); }
        let mut rooms = self.rooms.lock().map_err(|_| stale())?;
        rooms.retain(|_, room| room.lock().map(|state| !state.committing.is_empty() || state.empty_since.is_none_or(|at| at.elapsed() < ROOM_IDLE_TTL)).unwrap_or(true));
        if let Some(room) = rooms.get(&key) {
            if room.lock()?.valid { return Ok(room.clone()); }
            if !room.lock()?.committing.is_empty() { return Err(stale()); }
        }
        if rooms.len() >= MAX_ROOMS && !rooms.contains_key(&key) { return Err(AppError::Conflict("Draft cache limit reached".into())); }
        let (signals, _) = broadcast::channel(256);
        let room = Arc::new(DraftRoom { key, gate: Arc::new(tokio::sync::Mutex::new(())), signals,
            state: Mutex::new(RoomState { workspace: Arc::new(workspace), visibility: Arc::new(visibility), members: BTreeMap::new(), leases: BTreeMap::new(), committing: BTreeMap::new(), epoch: Uuid::new_v4(), seq: 0, journal: VecDeque::new(), journal_bytes: 0, valid: true, empty_since: Some(Instant::now()) }) });
        rooms.insert(key, room.clone());
        Ok(room)
    }

    pub fn expire(&self) {
        if let Ok(rooms) = self.rooms.lock() { for room in rooms.values() { room.expire(Instant::now()); } }
    }

    pub fn invalidate_term(&self, tenant_id: Uuid, term_id: Option<Uuid>) {
        if let Ok(rooms) = self.rooms.lock() { for (key, room) in rooms.iter() {
            if key.tenant_id == tenant_id && term_id.is_none_or(|term| term == key.academic_term_id) { room.invalidate(); }
        } }
    }

    pub fn admit_write(&self) -> Result<OwnedSemaphorePermit, AppError> {
        self.writes.clone().try_acquire_owned().map_err(|_| AppError::Conflict("Too many accepted timetable writes".into()))
    }
}

fn state_sync(state: &RoomState, connection_id: Uuid) -> Result<DraftEventEnvelope, AppError> {
    let member = state.members.get(&connection_id).ok_or_else(stale)?;
    let users = state.members.values().map(|member| member.actor.user_id).collect::<std::collections::BTreeSet<_>>().into_iter()
        .map(|user| participant(state, member, user)).collect();
    Ok(DraftEventEnvelope { epoch: state.epoch, seq: state.seq, event: DraftServerMessage::DraftStateSync(DraftStateSync {
        connection_id, workspace: crate::policy::project_workspace(&state.workspace, &state.visibility, &member.access), users, reservations: reservations(state, member),
    }) })
}

fn participant(state: &RoomState, viewer: &DraftMember, user_id: Uuid) -> DraftParticipant {
    let members = state.members.values().filter(|member| member.actor.user_id == user_id).collect::<Vec<_>>();
    let projected = crate::policy::project_workspace(&state.workspace, &state.visibility, &viewer.access);
    let first = members[0];
    DraftParticipant { user_id, name: first.name.clone(), color: first.color.clone(), profile_image_file_id: viewer.portraits.get(&user_id).copied(),
        connections: members.iter().map(|member| {
            let context = member.context.as_ref().filter(|context| context_visible(&projected, context)).cloned();
            let anchor = member.anchor.as_ref().filter(|anchor| context.is_some() && anchor_visible(&projected, anchor)).cloned();
            DraftConnectionPresence { connection_id: member.connection_id, context, anchor }
        }).collect() }
}

fn reservations(state: &RoomState, viewer: &DraftMember) -> Vec<DraftReservation> {
    let projected = crate::policy::project_workspace(&state.workspace, &state.visibility, &viewer.access);
    state.leases.values().filter(|lease| source_visible(&projected, &lease.public.source))
        .filter(|lease| lease.public.target.as_ref().and_then(|target| target.block_id).is_none_or(|id| projected.blocks.iter().any(|block| block.id == id)))
        .map(|lease| lease.public.clone()).collect()
}

fn source_visible(workspace: &TimetableBlockWorkspace, source: &TimetableBlockPlacementSource) -> bool {
    match source {
        TimetableBlockPlacementSource::ExistingBlock { block_id, .. } => workspace.blocks.iter().any(|block| block.id == *block_id),
        TimetableBlockPlacementSource::OrdinaryDemand { learning_group_id, learning_offering_id } => workspace.ordinary_demands.iter().any(|demand| demand.learning_group_id == *learning_group_id && demand.learning_offering_id == *learning_offering_id),
        TimetableBlockPlacementSource::SynchronizedOffering { learning_offering_id } => workspace.synchronized_demands.iter().any(|demand| demand.learning_offering_id == *learning_offering_id),
    }
}

fn context_visible(workspace: &TimetableBlockWorkspace, context: &DraftContext) -> bool {
    match context.view_mode {
        DraftViewMode::WholeSchool => context.view_id.is_none(),
        DraftViewMode::Homeroom => context.view_id.is_some_and(|id| workspace.homerooms.iter().any(|entry| entry.id == id)),
        DraftViewMode::Teacher => context.view_id.is_some_and(|id| workspace.staff.iter().any(|entry| entry.id == id)),
        DraftViewMode::LearningGroup => context.view_id.is_some_and(|id| workspace.learning_groups.iter().any(|entry| entry.id == id)),
    }
}

fn anchor_visible(workspace: &TimetableBlockWorkspace, anchor: &DraftAnchor) -> bool {
    anchor.has_valid_coordinates() && match anchor {
        DraftAnchor::Cell { day_of_week, bell_schedule_period_id, .. } => authority::valid_day(day_of_week) && workspace.bell_periods.iter().any(|period| period.id == *bell_schedule_period_id),
        DraftAnchor::Block { block_id, .. } => workspace.blocks.iter().any(|block| block.id == *block_id),
        DraftAnchor::Tray { source, .. } => source_visible(workspace, source),
    }
}

fn stale() -> AppError { AppError::Conflict("Draft collaboration must be reconciled".into()) }
fn forbidden() -> AppError { AppError::Forbidden("Draft resource is outside your authorized scope".into()) }

fn expires_at() -> DateTime<Utc> { Utc::now() + chrono::Duration::seconds(LEASE_TTL.as_secs() as i64) }
