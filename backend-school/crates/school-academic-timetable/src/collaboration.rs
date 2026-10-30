//! Process-local draft authority. Socket transport never owns reservations or committed state.
mod authority;
mod journal;
mod mutation;
#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use school_authorization::ActorContext;
use school_errors::AppError;
use tokio::sync::{broadcast, OwnedMutexGuard, OwnedSemaphorePermit, Semaphore};
use uuid::Uuid;

use crate::models::collaboration::*;
use crate::models::timetable_block::{
    TimetableBlockPlacementCandidate, TimetableBlockPlacementSource, TimetableBlockWorkspace,
};
use crate::policy::{TimetableAccessFilter, TimetableVisibility};

pub use authority::{CommitPermit, MutationScope};
pub use mutation::{
    CreationPermit, MutationAuthorizationPort, TimetableMutationContext, VersionMutationPermit,
};

pub const LEASE_TTL: Duration = Duration::from_secs(10);
const MAX_ROOMS: usize = 128;
const MAX_CONNECTIONS_PER_ROOM: usize = 128;
const MAX_ACCEPTED_WRITES: usize = 64;
const ROOM_IDLE_TTL: Duration = Duration::from_secs(600);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DraftKey {
    pub tenant_id: Uuid,
    pub academic_term_id: Uuid,
    pub timetable_version_id: Uuid,
}

#[derive(Debug, Clone)]
pub enum DraftSignal {
    Presence(Uuid),
    Left(Uuid),
    Reservations,
    Committed(u64),
    Invalidated,
}

#[derive(Clone)]
pub struct DraftMember {
    pub connection_id: Uuid,
    pub actor: ActorContext,
    pub session_id: Uuid,
    pub name: String,
    pub color: String,
    pub access: TimetableAccessFilter,
    pub manage_access: Option<TimetableAccessFilter>,
    pub resource_access: Arc<crate::policy::TimetableResourceAccessSnapshot>,
    /// Already authorized for this viewer by the Staff provider at join.
    pub portraits: BTreeMap<Uuid, Uuid>,
    pub context: Option<DraftContext>,
    pub anchor: Option<DraftAnchor>,
}

#[derive(Default)]
struct VisibilityIndex {
    blocks: HashSet<Uuid>,
    ordinary_demands: HashSet<(Uuid, Uuid)>,
    synchronized_offerings: HashSet<Uuid>,
    homerooms: HashSet<Uuid>,
    staff: HashSet<Uuid>,
    groups: HashSet<Uuid>,
    periods: HashSet<Uuid>,
}

impl VisibilityIndex {
    fn build(workspace: &TimetableBlockWorkspace) -> Self {
        Self {
            blocks: workspace.blocks.iter().map(|entry| entry.id).collect(),
            ordinary_demands: workspace
                .ordinary_demands
                .iter()
                .map(|entry| (entry.learning_group_id, entry.learning_offering_id))
                .collect(),
            synchronized_offerings: workspace
                .synchronized_demands
                .iter()
                .map(|entry| entry.learning_offering_id)
                .collect(),
            homerooms: workspace.homerooms.iter().map(|entry| entry.id).collect(),
            staff: workspace.staff.iter().map(|entry| entry.id).collect(),
            groups: workspace
                .learning_groups
                .iter()
                .map(|entry| entry.id)
                .collect(),
            periods: workspace
                .bell_periods
                .iter()
                .map(|entry| entry.id)
                .collect(),
        }
    }
}

struct MemberVisibility {
    read: VisibilityIndex,
    manage: Option<VisibilityIndex>,
}

fn member_visibility(
    workspace: &TimetableBlockWorkspace,
    visibility: &TimetableVisibility,
    member: &DraftMember,
) -> MemberVisibility {
    MemberVisibility {
        read: VisibilityIndex::build(&crate::policy::project_workspace(
            workspace,
            visibility,
            &member.access,
        )),
        manage: member.manage_access.as_ref().map(|access| {
            let mut projected = crate::policy::project_workspace(workspace, visibility, access);
            projected.blocks.retain(|block| {
                let mut resources = crate::policy::TimetableResourceSet::default();
                resources.include_block(block);
                member
                    .resource_access
                    .require_resources(&resources, access)
                    .is_ok()
            });
            VisibilityIndex::build(&projected)
        }),
    }
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
    member_visibility: BTreeMap<Uuid, MemberVisibility>,
    projection_rebuilds: u64,
    leases: BTreeMap<Uuid, Lease>,
    active_leases: BTreeMap<Uuid, Uuid>,
    committing: BTreeMap<Uuid, authority::CommitRecord>,
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
        self.state.lock().map_err(|_| {
            AppError::InternalServerError("Draft collaboration state unavailable".into())
        })
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn signal_subscriber_count(&self) -> usize {
        self.signals.receiver_count()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<DraftSignal> {
        self.signals.subscribe()
    }

    pub fn resource_inputs(&self) -> Result<crate::policy::TimetableResourceSet, AppError> {
        let state = self.lock()?;
        let workspace = &state.workspace;
        let mut resources = crate::policy::TimetableResourceSet {
            timetable_version_ids: vec![workspace.version.id],
            learning_offering_ids: state
                .visibility
                .source
                .offerings
                .iter()
                .map(|entry| entry.id)
                .collect(),
            learning_group_ids: workspace
                .learning_groups
                .iter()
                .map(|entry| entry.id)
                .collect(),
            homeroom_ids: workspace.homerooms.iter().map(|entry| entry.id).collect(),
            teacher_ids: workspace.staff.iter().map(|entry| entry.id).collect(),
            room_ids: workspace.rooms.iter().map(|entry| entry.id).collect(),
            requires_school_scope: false,
        };
        for block in &workspace.blocks {
            resources.include_block(block);
        }
        Ok(resources)
    }

    pub fn members(&self) -> Result<Vec<DraftMember>, AppError> {
        Ok(self.lock()?.members.values().cloned().collect())
    }

    pub fn set_portraits(
        &self,
        connection_id: Uuid,
        portraits: BTreeMap<Uuid, Uuid>,
    ) -> Result<(), AppError> {
        if let Some(member) = self.lock()?.members.get_mut(&connection_id) {
            member.portraits = portraits;
        }
        Ok(())
    }

    pub async fn mutation_gate(&self) -> OwnedMutexGuard<()> {
        self.gate.clone().lock_owned().await
    }

    pub fn join(&self, member: DraftMember) -> Result<DraftEventEnvelope, AppError> {
        let mut state = self.lock()?;
        if !state.valid {
            return Err(stale());
        }
        if state.members.len() >= MAX_CONNECTIONS_PER_ROOM {
            return Err(AppError::Conflict("Draft connection limit reached".into()));
        }
        let connection_id = member.connection_id;
        let indices = member_visibility(&state.workspace, &state.visibility, &member);
        state.member_visibility.insert(connection_id, indices);
        state.projection_rebuilds += 1;
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
            state.member_visibility.remove(&connection_id);
            state.active_leases.remove(&connection_id);
            let pinned = state
                .committing
                .values()
                .filter_map(|record| record.lease_token)
                .collect::<HashSet<_>>();
            state.leases.retain(|token, lease| {
                lease.public.owner_connection_id != connection_id || pinned.contains(token)
            });
            // In-flight committing footprints are owned by their permit, never by socket lifetime.
            if state.members.is_empty() {
                state.empty_since = Some(Instant::now());
            }
        }
        let _ = self.signals.send(DraftSignal::Left(connection_id));
        let _ = self.signals.send(DraftSignal::Reservations);
    }

    pub fn invalidate(&self) {
        if let Ok(mut state) = self.lock() {
            state.valid = false;
            state.leases.clear();
        }
        let _ = self.signals.send(DraftSignal::Invalidated);
    }

    pub fn update_presence(
        &self,
        connection_id: Uuid,
        update: DraftPresenceUpdate,
    ) -> Result<(), AppError> {
        let mut state = self.lock()?;
        if !state.valid {
            return Err(stale());
        }
        let projected = &state
            .member_visibility
            .get(&connection_id)
            .ok_or_else(stale)?
            .read;
        // A peer mutation can remove an anchor/context before this in-flight frame arrives.
        // Sanitize against current authorized indices without closing healthy membership or
        // retaining/rebroadcasting identifiers that are no longer visible.
        let context = context_visible(projected, &update.context).then_some(update.context);
        let anchor = update
            .anchor
            .filter(|anchor| context.is_some() && anchor_visible(projected, anchor));
        let member = state.members.get_mut(&connection_id).ok_or_else(stale)?;
        member.context = context;
        member.anchor = anchor;
        drop(state);
        let _ = self.signals.send(DraftSignal::Presence(connection_id));
        Ok(())
    }

    pub fn expire(&self, now: Instant) {
        let changed = self
            .lock()
            .map(|mut state| {
                let before = state.leases.len();
                let pinned = state
                    .committing
                    .values()
                    .filter_map(|record| record.lease_token)
                    .collect::<HashSet<_>>();
                state
                    .leases
                    .retain(|token, lease| lease.deadline > now || pinned.contains(token));
                let live_tokens = state.leases.keys().copied().collect::<HashSet<_>>();
                state
                    .active_leases
                    .retain(|_, token| live_tokens.contains(token));
                before != state.leases.len()
            })
            .unwrap_or(false);
        if changed {
            let _ = self.signals.send(DraftSignal::Reservations);
        }
    }

    pub fn signal_events(
        &self,
        viewer: Uuid,
        signal: DraftSignal,
    ) -> Result<Vec<DraftEventEnvelope>, AppError> {
        let state = self.lock()?;
        let member = state.members.get(&viewer).ok_or_else(stale)?;
        let event = |event| DraftEventEnvelope {
            epoch: state.epoch,
            seq: state.seq,
            event,
        };
        match signal {
            DraftSignal::Presence(connection_id) => {
                let Some(changed) = state.members.get(&connection_id) else {
                    return Ok(Vec::new());
                };
                Ok(vec![event(DraftServerMessage::DraftPresenceChanged(
                    participant(&state, member, changed.actor.user_id),
                ))])
            }
            DraftSignal::Left(connection_id) => {
                Ok(vec![event(DraftServerMessage::DraftPresenceLeft {
                    connection_id,
                })])
            }
            DraftSignal::Reservations => {
                let mut events = vec![event(DraftServerMessage::DraftReservationsChanged {
                    reservations: reservations(&state, member),
                })];
                if let Some(availability) = authority::availability(&state, member) {
                    events.push(event(DraftServerMessage::DraftAvailability(availability)));
                }
                Ok(events)
            }
            DraftSignal::Committed(seq) => journal::events_after(
                &state,
                member,
                state.epoch,
                seq.saturating_sub(1),
                Some(seq),
            ),
            DraftSignal::Invalidated => {
                Ok(vec![event(DraftServerMessage::DraftReconcileRequired {
                    reason: "draft_invalidated".into(),
                })])
            }
        }
    }

    pub fn command(
        &self,
        connection_id: Uuid,
        command: DraftClientMessage,
    ) -> Result<Vec<DraftEventEnvelope>, AppError> {
        self.expire(Instant::now());
        match command {
            DraftClientMessage::ClientPresenceUpdate(update) => {
                self.update_presence(connection_id, update)?;
                Ok(Vec::new())
            }
            DraftClientMessage::ClientReplay(replay) => {
                let state = self.lock()?;
                let member = state.members.get(&connection_id).ok_or_else(stale)?;
                journal::events_after(&state, member, replay.epoch, replay.after_seq, None)
            }
            command => {
                let (request_id, kind) = match &command {
                    DraftClientMessage::ClientClaim(value) => {
                        (value.request_id, DraftCommandKind::Claim)
                    }
                    DraftClientMessage::ClientTarget(value) => {
                        (value.request_id, DraftCommandKind::Target)
                    }
                    DraftClientMessage::ClientRenew(value) => {
                        (value.request_id, DraftCommandKind::Renew)
                    }
                    DraftClientMessage::ClientRelease(value) => {
                        (value.request_id, DraftCommandKind::Release)
                    }
                    _ => return Err(stale()),
                };
                match authority::lease_command(self, connection_id, command) {
                    Ok(events) => Ok(events),
                    Err(error) => {
                        let state = self.lock()?;
                        if !state.valid || !state.members.contains_key(&connection_id) {
                            return Err(error);
                        }
                        let code = match error {
                            AppError::Forbidden(_) => "forbidden",
                            AppError::ValidationError(_) => "invalid_candidate",
                            AppError::Conflict(_) | AppError::NotFound(_) => "stale_reservation",
                            _ => return Err(error),
                        };
                        Ok(vec![DraftEventEnvelope {
                            epoch: state.epoch,
                            seq: state.seq,
                            event: DraftServerMessage::DraftCommandResult(DraftCommandResult {
                                request_id,
                                command: kind,
                                accepted: false,
                                code: Some(code.into()),
                                source: None,
                                target: None,
                                reservation_id: None,
                                lease_token: None,
                                expires_at: None,
                            }),
                        }])
                    }
                }
            }
        }
    }
}

pub struct TimetableCollaborationRuntime {
    rooms: Mutex<BTreeMap<DraftKey, Arc<DraftRoom>>>,
    writes: Arc<Semaphore>,
    cold_load: tokio::sync::Mutex<()>,
    cold_generation: AtomicU64,
}

impl Default for TimetableCollaborationRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl TimetableCollaborationRuntime {
    pub fn new() -> Self {
        Self {
            rooms: Mutex::new(BTreeMap::new()),
            writes: Arc::new(Semaphore::new(MAX_ACCEPTED_WRITES)),
            cold_load: tokio::sync::Mutex::new(()),
            cold_generation: AtomicU64::new(0),
        }
    }

    pub fn has_tenant_rooms(&self, tenant_id: Uuid) -> bool {
        self.rooms
            .lock()
            .map(|rooms| rooms.keys().any(|key| key.tenant_id == tenant_id))
            .unwrap_or(true)
    }

    pub fn invalidate_all(&self) {
        if let Ok(rooms) = self.rooms.lock() {
            self.cold_generation.fetch_add(1, Ordering::AcqRel);
            for room in rooms.values() {
                room.invalidate();
            }
        }
    }

    pub fn room(&self, key: DraftKey) -> Result<Option<Arc<DraftRoom>>, AppError> {
        Ok(self.rooms.lock().map_err(|_| stale())?.get(&key).cloned())
    }

    /// Only cold joins/writes perform these reads. All interaction commands use the cached room.
    pub async fn load_room(
        &self,
        tenant_id: Uuid,
        pool: &sqlx::PgPool,
        version_id: Uuid,
        staff_directory: &dyn crate::ports::StaffDirectoryPort,
    ) -> Result<Arc<DraftRoom>, AppError> {
        let _load = self.cold_load.lock().await;
        if let Some(room) = self
            .rooms
            .lock()
            .map_err(|_| stale())?
            .iter()
            .find(|(key, _)| key.tenant_id == tenant_id && key.timetable_version_id == version_id)
            .map(|(_, room)| room.clone())
        {
            if room.lock()?.valid {
                return Ok(room);
            }
        }
        let generation = self.cold_generation.load(Ordering::Acquire);
        let mut connection = school_tenancy::TenantPool::begin_observed(
            pool,
            "academic_timetable.collaboration.load_room",
        )
        .await?;
        // Every placement/lifecycle writer locks this version before changing its graph. A cold
        // or uncertain-COMMIT recovery read must wait for that transaction's final outcome.
        school_database::DbSession::new(&mut *connection)
            .scalar::<Uuid>(
                "academic_timetable.collaboration.load_room.version_fence",
                "SELECT id FROM academic_timetable_versions WHERE id=$1 FOR SHARE",
            )
            .bind(version_id)
            .fetch_optional()
            .await?
            .ok_or_else(|| AppError::NotFound("Timetable version not found".into()))?;
        let version = crate::services::timetable_version_service::get_version_on_connection(
            &mut connection,
            version_id,
            Utc::now().date_naive(),
        )
        .await?;
        let key = DraftKey {
            tenant_id,
            academic_term_id: version.academic_term_id,
            timetable_version_id: version_id,
        };
        let query = crate::models::timetable_block::TimetableBlockWorkspaceQuery {
            academic_year_id: version.academic_year_id,
            academic_term_id: version.academic_term_id,
            timetable_version_id: version_id,
        };
        let (workspace, visibility) =
            crate::services::timetable_block_queries::get_canonical_workspace(
                &mut connection,
                query,
                staff_directory,
            )
            .await?;
        connection.commit().await?;
        self.install_fenced(key, workspace, visibility, Some(generation))
    }

    pub fn install(
        &self,
        key: DraftKey,
        workspace: TimetableBlockWorkspace,
        visibility: TimetableVisibility,
    ) -> Result<Arc<DraftRoom>, AppError> {
        self.install_fenced(key, workspace, visibility, None)
    }

    fn install_fenced(
        &self,
        key: DraftKey,
        workspace: TimetableBlockWorkspace,
        visibility: TimetableVisibility,
        generation: Option<u64>,
    ) -> Result<Arc<DraftRoom>, AppError> {
        if workspace.version.id != key.timetable_version_id
            || workspace.version.academic_term_id != key.academic_term_id
        {
            return Err(stale());
        }
        let mut rooms = self.rooms.lock().map_err(|_| stale())?;
        // Invalidation advances under this same map lock, including when no room exists yet.
        // Check and insertion are one atomic cache-fill decision.
        if generation
            .is_some_and(|accepted| accepted != self.cold_generation.load(Ordering::Acquire))
        {
            return Err(AppError::ServiceUnavailable(
                "Draft cache changed during loading; reconnect".into(),
            ));
        }
        rooms.retain(|_, room| {
            let Ok(mut state) = room.lock() else {
                return true;
            };
            // External owners include accepted writes waiting for the mutation gate.
            // Evicting a still-valid referenced room would split the draft authority.
            let retained = Arc::strong_count(room) > 1
                || !state.committing.is_empty()
                || state
                    .empty_since
                    .is_none_or(|at| at.elapsed() < ROOM_IDLE_TTL);
            if !retained {
                // Retire under the state lock before removal, including a concurrent Weak
                // upgrade after the reference-count check. An old handle can never write.
                state.valid = false;
                state.leases.clear();
                state.active_leases.clear();
                let _ = room.signals.send(DraftSignal::Invalidated);
            }
            retained
        });
        if let Some(room) = rooms.get(&key) {
            if room.lock()?.valid {
                return Ok(room.clone());
            }
            if !room.lock()?.committing.is_empty() {
                return Err(stale());
            }
        }
        if rooms.len() >= MAX_ROOMS && !rooms.contains_key(&key) {
            return Err(AppError::Conflict("Draft cache limit reached".into()));
        }
        let (signals, _) = broadcast::channel(256);
        let room = Arc::new(DraftRoom {
            key,
            gate: Arc::new(tokio::sync::Mutex::new(())),
            signals,
            state: Mutex::new(RoomState {
                workspace: Arc::new(workspace),
                visibility: Arc::new(visibility),
                members: BTreeMap::new(),
                member_visibility: BTreeMap::new(),
                projection_rebuilds: 0,
                leases: BTreeMap::new(),
                active_leases: BTreeMap::new(),
                committing: BTreeMap::new(),
                epoch: Uuid::new_v4(),
                seq: 0,
                journal: VecDeque::new(),
                journal_bytes: 0,
                valid: true,
                empty_since: Some(Instant::now()),
            }),
        });
        rooms.insert(key, room.clone());
        Ok(room)
    }

    pub fn expire(&self) {
        if let Ok(rooms) = self.rooms.lock() {
            for room in rooms.values() {
                room.expire(Instant::now());
            }
        }
    }

    pub fn invalidate_term(&self, tenant_id: Uuid, term_id: Option<Uuid>) {
        if let Ok(rooms) = self.rooms.lock() {
            self.cold_generation.fetch_add(1, Ordering::AcqRel);
            for (key, room) in rooms.iter() {
                if key.tenant_id == tenant_id
                    && term_id.is_none_or(|term| term == key.academic_term_id)
                {
                    room.invalidate();
                }
            }
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn accepted_write_count(&self) -> usize {
        MAX_ACCEPTED_WRITES - self.writes.available_permits()
    }

    pub fn admit_write(&self) -> Result<OwnedSemaphorePermit, AppError> {
        self.writes
            .clone()
            .try_acquire_owned()
            .map_err(|_| AppError::Conflict("Too many accepted timetable writes".into()))
    }
}

fn state_sync(state: &RoomState, connection_id: Uuid) -> Result<DraftEventEnvelope, AppError> {
    let member = state.members.get(&connection_id).ok_or_else(stale)?;
    let users = state
        .members
        .values()
        .map(|member| member.actor.user_id)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .map(|user| participant(state, member, user))
        .collect();
    Ok(DraftEventEnvelope {
        epoch: state.epoch,
        seq: state.seq,
        event: DraftServerMessage::DraftStateSync(DraftStateSync {
            connection_id,
            workspace: crate::policy::project_workspace(
                &state.workspace,
                &state.visibility,
                &member.access,
            ),
            users,
            reservations: reservations(state, member),
        }),
    })
}

fn participant(state: &RoomState, viewer: &DraftMember, user_id: Uuid) -> DraftParticipant {
    let members = state
        .members
        .values()
        .filter(|member| member.actor.user_id == user_id)
        .collect::<Vec<_>>();
    let projected = &state.member_visibility[&viewer.connection_id].read;
    let first = members[0];
    DraftParticipant {
        user_id,
        name: first.name.clone(),
        color: first.color.clone(),
        profile_image_file_id: viewer.portraits.get(&user_id).copied(),
        connections: members
            .iter()
            .map(|member| {
                let context = member
                    .context
                    .as_ref()
                    .filter(|context| context_visible(&projected, context))
                    .cloned();
                let anchor = member
                    .anchor
                    .as_ref()
                    .filter(|anchor| context.is_some() && anchor_visible(&projected, anchor))
                    .cloned();
                DraftConnectionPresence {
                    connection_id: member.connection_id,
                    active_reservation_id: state
                        .active_leases
                        .get(&member.connection_id)
                        .and_then(|token| state.leases.get(token))
                        .filter(|lease| {
                            source_visible(projected, &lease.public.source)
                                && lease
                                    .public
                                    .target
                                    .as_ref()
                                    .and_then(|target| target.block_id)
                                    .is_none_or(|id| projected.blocks.contains(&id))
                        })
                        .map(|lease| lease.public.reservation_id),
                    context,
                    anchor,
                }
            })
            .collect(),
    }
}

fn reservations(state: &RoomState, viewer: &DraftMember) -> Vec<DraftReservation> {
    let projected = &state.member_visibility[&viewer.connection_id].read;
    state
        .leases
        .values()
        .filter(|lease| source_visible(&projected, &lease.public.source))
        .filter(|lease| {
            lease
                .public
                .target
                .as_ref()
                .and_then(|target| target.block_id)
                .is_none_or(|id| projected.blocks.contains(&id))
        })
        .map(|lease| lease.public.clone())
        .collect()
}

fn source_visible(index: &VisibilityIndex, source: &TimetableBlockPlacementSource) -> bool {
    match source {
        TimetableBlockPlacementSource::ExistingBlock { block_id, .. } => {
            index.blocks.contains(block_id)
        }
        TimetableBlockPlacementSource::OrdinaryDemand {
            learning_group_id,
            learning_offering_id,
        } => index
            .ordinary_demands
            .contains(&(*learning_group_id, *learning_offering_id)),
        TimetableBlockPlacementSource::SynchronizedOffering {
            learning_offering_id,
        } => index.synchronized_offerings.contains(learning_offering_id),
    }
}

fn context_visible(index: &VisibilityIndex, context: &DraftContext) -> bool {
    if context
        .day_of_week
        .as_ref()
        .is_some_and(|day| !authority::valid_day(day))
    {
        return false;
    }
    match context.view_mode {
        DraftViewMode::WholeSchool => context
            .view_id
            .is_none_or(|id| index.homerooms.contains(&id)),
        DraftViewMode::Homeroom => context
            .view_id
            .is_some_and(|id| index.homerooms.contains(&id)),
        DraftViewMode::Teacher => context.view_id.is_some_and(|id| index.staff.contains(&id)),
        DraftViewMode::LearningGroup => {
            context.view_id.is_some_and(|id| index.groups.contains(&id))
        }
    }
}

fn anchor_visible(index: &VisibilityIndex, anchor: &DraftAnchor) -> bool {
    anchor.has_valid_coordinates()
        && match anchor {
            DraftAnchor::Cell {
                day_of_week,
                bell_schedule_period_id,
                ..
            } => {
                authority::valid_day(day_of_week) && index.periods.contains(bell_schedule_period_id)
            }
            DraftAnchor::Block { block_id, .. } => index.blocks.contains(block_id),
            DraftAnchor::Tray { source, .. } => source_visible(index, source),
        }
}

fn stale() -> AppError {
    AppError::Conflict("Draft collaboration must be reconciled".into())
}
fn forbidden() -> AppError {
    AppError::Forbidden("Draft resource is outside your authorized scope".into())
}

fn expires_at() -> DateTime<Utc> {
    Utc::now() + chrono::Duration::seconds(LEASE_TTL.as_secs() as i64)
}
