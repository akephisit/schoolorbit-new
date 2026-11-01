use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Instant;

use chrono::NaiveTime;
use school_errors::AppError;
use uuid::Uuid;

use super::*;
use crate::models::timetable_block::*;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Identity {
    Block(Uuid),
    Group(Uuid),
    Offering(Uuid),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Resource {
    Teacher(Uuid),
    Homeroom(Uuid),
    Room(Uuid),
    Group(Uuid),
}

#[derive(Debug, Clone)]
struct Slot {
    resource: Resource,
    day: String,
    start: NaiveTime,
    end: NaiveTime,
}

impl Slot {
    fn overlaps(&self, day: &str, start: NaiveTime, end: NaiveTime) -> bool {
        self.day == day && self.start < end && start < self.end
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct Footprint {
    whole_draft: bool,
    identities: BTreeSet<Identity>,
    slots: Vec<Slot>,
}

impl Footprint {
    fn conflicts(&self, other: &Self) -> bool {
        self.whole_draft
            || other.whole_draft
            || !self.identities.is_disjoint(&other.identities)
            || self.slots.iter().any(|left| {
                other.slots.iter().any(|right| {
                    left.resource == right.resource
                        && left.overlaps(&right.day, right.start, right.end)
                })
            })
    }
    fn extend(&mut self, other: Self) {
        self.whole_draft |= other.whole_draft;
        self.identities.extend(other.identities);
        self.slots.extend(other.slots);
    }
}

pub enum MutationScope {
    Ordinary(CreateOrdinaryTimetableBlockRequest),
    Synchronized(CreateSynchronizedTimetableBlockRequest),
    Structural(CreateStructuralTimetableBlocksRequest),
    Update {
        block_id: Uuid,
        request: UpdateTimetableBlockRequest,
    },
    Swap(SwapTimetableBlocksRequest),
    Series(Uuid),
    WholeDraft,
    Blocks(Vec<Uuid>),
    Placement {
        source: TimetableBlockPlacementSource,
        candidate: TimetableBlockPlacementCandidate,
        targets: Vec<DraftTarget>,
    },
}

/// Pinned exclusion owned by the accepted write, independent of connection and lease expiry.
pub struct CommitPermit {
    room: Arc<DraftRoom>,
    id: Uuid,
    pub actor_id: Uuid,
    phase: AtomicU8,
    capture_content_revision: AtomicBool,
    settled: AtomicBool,
    committed_version: std::sync::Mutex<Option<crate::models::timetable_version::TimetableVersion>>,
}

pub(super) struct CommitRecord {
    pub footprint: Footprint,
    pub lease_token: Option<Uuid>,
    pub reservation_id: Uuid,
    pub owner_connection_id: Uuid,
    pub purpose: DraftReservationPurpose,
}

impl CommitPermit {
    pub fn require_version(&self, version_id: Uuid) -> Result<(), AppError> {
        if self.room.key.timetable_version_id == version_id {
            Ok(())
        } else {
            Err(forbidden())
        }
    }
    pub(crate) fn settle(&self) {
        self.settled.store(true, Ordering::Release);
    }
    pub fn possibly_committed(&self) -> bool {
        self.phase.load(Ordering::Acquire) != 0
    }
    pub(crate) fn disable_content_revision(&self) {
        self.capture_content_revision
            .store(false, Ordering::Release);
    }
    pub fn version_snapshot(
        &self,
    ) -> Result<Option<crate::models::timetable_version::TimetableVersion>, AppError> {
        Ok(self.committed_version.lock().map_err(|_| stale())?.clone())
    }
    pub async fn commit(
        &self,
        mut transaction: school_tenancy::ObservedTransaction,
    ) -> Result<(), AppError> {
        if self.capture_content_revision.load(Ordering::Acquire) {
            let cached = self.room.lock()?.workspace.version.clone();
            let version =
                crate::services::timetable_version_service::refresh_content_revision_on_connection(
                    &mut transaction,
                    cached,
                )
                .await?;
            *self.committed_version.lock().map_err(|_| stale())? = Some(version);
        }
        self.phase.store(1, Ordering::Release);
        transaction.commit().await?;
        self.phase.store(2, Ordering::Release);
        Ok(())
    }
}

impl Drop for CommitPermit {
    fn drop(&mut self) {
        if self.possibly_committed() && !self.settled.load(Ordering::Acquire) {
            self.room.invalidate();
        }
        if let Ok(mut state) = self.room.lock() {
            if let Some(record) = state.committing.remove(&self.id) {
                if let Some(token) = record.lease_token {
                    state.leases.remove(&token);
                    state.active_leases.retain(|_, active| *active != token);
                }
            }
        }
        let _ = self.room.signals.send(DraftSignal::Reservations);
    }
}

impl DraftRoom {
    pub fn begin_commit(
        self: &Arc<Self>,
        actor_id: Uuid,
        session_id: Uuid,
        identity: Option<&DraftMutationIdentity>,
        scope: MutationScope,
    ) -> Result<CommitPermit, AppError> {
        self.expire(Instant::now());
        let mut state = self.lock()?;
        if !state.valid {
            return Err(stale());
        }
        if !matches!(&scope, MutationScope::WholeDraft)
            && state.workspace.version.delivery_version_id
                != state.workspace.latest_delivery_version_id
        {
            return Err(crate::services::timetable_source::outdated_draft_source());
        }
        let expected_purpose = match &scope {
            MutationScope::Blocks(_) | MutationScope::Series(_) => {
                Some(DraftReservationPurpose::Edit)
            }
            MutationScope::Update { request, .. }
                if request.day_of_week.is_none() && request.bell_schedule_period_id.is_none() =>
            {
                Some(DraftReservationPurpose::Edit)
            }
            MutationScope::Update { .. }
            | MutationScope::Ordinary(_)
            | MutationScope::Synchronized(_)
            | MutationScope::Swap(_) => Some(DraftReservationPurpose::Drag),
            _ => None,
        };
        let scope = match resolve_scope(&state.workspace, scope)? {
            MutationScope::Placement {
                source,
                candidate,
                targets,
            } => {
                let candidate = normalize_candidate(&state.workspace, &source, Some(candidate))?;
                MutationScope::Placement {
                    source,
                    candidate,
                    targets,
                }
            }
            scope => scope,
        };
        let own_token = if let Some(identity) = identity {
            let member = state
                .members
                .get(&identity.connection_id)
                .ok_or_else(stale)?;
            if member.actor.user_id != actor_id || member.session_id != session_id {
                return Err(forbidden());
            }
            let lease = state.leases.get(&identity.lease_token).ok_or_else(stale)?;
            if lease.public.owner_connection_id != identity.connection_id {
                return Err(forbidden());
            }
            if expected_purpose.is_some_and(|purpose| purpose != lease.public.purpose) {
                return Err(forbidden());
            }
            let covers_operation = match &scope {
                MutationScope::WholeDraft | MutationScope::Structural(_) => false,
                MutationScope::Blocks(ids) => match &lease.public.source {
                    TimetableBlockPlacementSource::ExistingBlock { block_id, .. } => {
                        ids.contains(block_id)
                            && ids.iter().all(|id| {
                                *id == *block_id
                                    || lease
                                        .public
                                        .target
                                        .as_ref()
                                        .and_then(|target| target.block_id)
                                        == Some(*id)
                            })
                    }
                    _ => false,
                },
                MutationScope::Placement {
                    source,
                    candidate,
                    targets,
                } => {
                    *source == lease.public.source
                        && lease.candidate.as_ref() == Some(candidate)
                        && targets.len() == 1
                        && (lease.public.target.as_ref() == targets.first()
                            || (lease.public.purpose == DraftReservationPurpose::Edit
                                && matches!(source, TimetableBlockPlacementSource::ExistingBlock { block_id, .. }
                                    if state.workspace.blocks.iter().any(|block| block.id == *block_id
                                        && targets[0].day_of_week == block.day_of_week
                                        && targets[0].bell_schedule_period_id == block.bell_schedule_period_id
                                        && targets[0].block_id.is_none()))))
                }
                _ => false,
            };
            if !covers_operation {
                return Err(AppError::Conflict(
                    "Reservation does not cover this mutation".into(),
                ));
            }
            Some(identity.lease_token)
        } else {
            None
        };
        let footprint = match scope {
            MutationScope::Structural(request) => structural_footprint(&state.workspace, &request)?,
            MutationScope::WholeDraft => Footprint {
                whole_draft: true,
                ..Default::default()
            },
            MutationScope::Blocks(ids) => {
                let mut result = Footprint::default();
                for id in ids {
                    result.extend(block_footprint(
                        state
                            .workspace
                            .blocks
                            .iter()
                            .find(|block| block.id == id)
                            .ok_or_else(stale)?,
                    ));
                }
                result
            }
            MutationScope::Placement {
                source,
                candidate,
                targets,
            } => {
                let candidate = normalize_candidate(&state.workspace, &source, Some(candidate))?;
                let mut result = source_footprint(&state.workspace, &source)?;
                for target in targets {
                    result.extend(target_footprint(
                        &state.workspace,
                        &source,
                        &candidate,
                        &target,
                    )?);
                }
                result
            }
            _ => return Err(stale()),
        };
        if state.leases.iter().any(|(token, lease)| {
            Some(*token) != own_token && footprint.conflicts(&lease.footprint)
        }) || state
            .committing
            .values()
            .any(|other| footprint.conflicts(&other.footprint))
        {
            return Err(AppError::Conflict(
                "Timetable resources are reserved by another editor".into(),
            ));
        }
        let id = Uuid::new_v4();
        let public = own_token
            .and_then(|token| state.leases.get_mut(&token))
            .map(|lease| {
                lease.public.expires_at = Utc::now() + chrono::Duration::seconds(65);
                lease.public.clone()
            });
        state.committing.insert(
            id,
            CommitRecord {
                footprint,
                lease_token: own_token,
                reservation_id: public
                    .as_ref()
                    .map(|lease| lease.reservation_id)
                    .unwrap_or(id),
                owner_connection_id: public
                    .as_ref()
                    .map(|lease| lease.owner_connection_id)
                    .unwrap_or(Uuid::nil()),
                purpose: public
                    .as_ref()
                    .map(|lease| lease.purpose)
                    .unwrap_or(DraftReservationPurpose::Edit),
            },
        );
        drop(state);
        let _ = self.signals.send(DraftSignal::Reservations);
        Ok(CommitPermit {
            room: self.clone(),
            id,
            actor_id,
            phase: AtomicU8::new(0),
            capture_content_revision: AtomicBool::new(true),
            settled: AtomicBool::new(false),
            committed_version: std::sync::Mutex::new(None),
        })
    }
}

fn resolve_scope(
    workspace: &TimetableBlockWorkspace,
    scope: MutationScope,
) -> Result<MutationScope, AppError> {
    let target = |day: &str, period| DraftTarget {
        day_of_week: day.trim().to_ascii_uppercase(),
        bell_schedule_period_id: period,
        block_id: None,
    };
    Ok(match scope {
        MutationScope::Ordinary(request) => {
            let demand = workspace
                .ordinary_demands
                .iter()
                .find(|demand| demand.learning_group_id == request.learning_group_id)
                .ok_or_else(stale)?;
            let group = workspace
                .learning_groups
                .iter()
                .find(|group| group.id == request.learning_group_id)
                .ok_or_else(stale)?;
            MutationScope::Placement {
                source: TimetableBlockPlacementSource::OrdinaryDemand {
                    learning_group_id: demand.learning_group_id,
                    learning_offering_id: demand.learning_offering_id,
                },
                candidate: TimetableBlockPlacementCandidate {
                    block_kind: if group.offering_kind == "course" {
                        TimetableBlockKind::Course
                    } else {
                        TimetableBlockKind::Activity
                    },
                    learning_group_id: Some(demand.learning_group_id),
                    learning_offering_id: Some(demand.learning_offering_id),
                    room_id: request.room_id,
                    homeroom_ids: demand.homeroom_ids.clone(),
                    teacher_ids: Vec::new(),
                    instructor_ids: request.instructor_ids,
                },
                targets: vec![target(
                    &request.day_of_week,
                    request.bell_schedule_period_id,
                )],
            }
        }
        MutationScope::Synchronized(request) => MutationScope::Placement {
            source: TimetableBlockPlacementSource::SynchronizedOffering {
                learning_offering_id: request.learning_offering_id,
            },
            candidate: TimetableBlockPlacementCandidate {
                block_kind: TimetableBlockKind::Activity,
                learning_group_id: None,
                learning_offering_id: Some(request.learning_offering_id),
                room_id: request.room_id,
                homeroom_ids: request.intended_homeroom_ids,
                teacher_ids: request.teacher_ids,
                instructor_ids: Vec::new(),
            },
            targets: vec![target(
                &request.day_of_week,
                request.bell_schedule_period_id,
            )],
        },
        MutationScope::Update { block_id, request } => {
            let block = workspace
                .blocks
                .iter()
                .find(|block| block.id == block_id)
                .ok_or_else(stale)?;
            let mut candidate = candidate_from_block(block);
            if request.clear_room {
                candidate.room_id = None;
            } else if request.room_id.is_some() {
                candidate.room_id = request.room_id;
            }
            if let Some(ids) = request.instructor_ids {
                candidate.instructor_ids = ids;
            }
            if let Some(ids) = request.teacher_ids {
                candidate.teacher_ids = ids;
            }
            MutationScope::Placement {
                source: TimetableBlockPlacementSource::ExistingBlock {
                    block_id,
                    row_version: request.row_version,
                },
                candidate,
                targets: vec![target(
                    request.day_of_week.as_deref().unwrap_or(&block.day_of_week),
                    request
                        .bell_schedule_period_id
                        .unwrap_or(block.bell_schedule_period_id),
                )],
            }
        }
        MutationScope::Swap(request) => {
            let block = workspace
                .blocks
                .iter()
                .find(|block| block.id == request.block_a_id)
                .ok_or_else(stale)?;
            let other = workspace
                .blocks
                .iter()
                .find(|block| {
                    block.id == request.block_b_id
                        && block.row_version == request.block_b_row_version
                })
                .ok_or_else(stale)?;
            MutationScope::Placement {
                source: TimetableBlockPlacementSource::ExistingBlock {
                    block_id: block.id,
                    row_version: request.block_a_row_version,
                },
                candidate: candidate_from_block(block),
                targets: vec![DraftTarget {
                    day_of_week: other.day_of_week.clone(),
                    bell_schedule_period_id: other.bell_schedule_period_id,
                    block_id: Some(other.id),
                }],
            }
        }
        MutationScope::Series(id) => MutationScope::Blocks(
            workspace
                .blocks
                .iter()
                .filter(|block| block.series_id == Some(id))
                .map(|block| block.id)
                .collect(),
        ),
        scope => scope,
    })
}

fn structural_footprint(
    workspace: &TimetableBlockWorkspace,
    request: &CreateStructuralTimetableBlocksRequest,
) -> Result<Footprint, AppError> {
    let homerooms = if request.all_homerooms {
        workspace
            .homerooms
            .iter()
            .filter(|entry| entry.is_active)
            .map(|entry| entry.id)
            .collect()
    } else {
        request.homeroom_ids.clone()
    };
    let teachers = if request.all_teachers {
        workspace
            .staff
            .iter()
            .filter(|entry| entry.status == "active")
            .map(|entry| entry.id)
            .collect()
    } else {
        request.teacher_ids.clone()
    };
    let resources = homerooms
        .into_iter()
        .map(Resource::Homeroom)
        .chain(teachers.into_iter().map(Resource::Teacher))
        .chain(request.room_id.map(Resource::Room))
        .collect::<BTreeSet<_>>();
    let mut result = Footprint::default();
    for slot in &request.slots {
        let period = workspace
            .bell_periods
            .iter()
            .find(|period| period.id == slot.bell_schedule_period_id)
            .ok_or_else(stale)?;
        let day = slot.day_of_week.trim().to_ascii_uppercase();
        if !valid_day(&day) {
            return Err(stale());
        }
        result
            .slots
            .extend(resources.iter().cloned().map(|resource| Slot {
                resource,
                day: day.clone(),
                start: period.start_time,
                end: period.end_time,
            }));
    }
    Ok(result)
}

pub(super) fn valid_day(day: &str) -> bool {
    matches!(day, "MON" | "TUE" | "WED" | "THU" | "FRI" | "SAT" | "SUN")
}

fn block_footprint(block: &TimetableBlock) -> Footprint {
    let mut footprint = Footprint::default();
    footprint.identities.insert(Identity::Block(block.id));
    let resources = block
        .groups
        .iter()
        .filter(|group| group.is_active)
        .flat_map(|group| {
            std::iter::once(Resource::Group(group.learning_group_id))
                .chain(group.homeroom_ids.iter().copied().map(Resource::Homeroom))
                .chain(
                    group
                        .instructors
                        .iter()
                        .map(|teacher| Resource::Teacher(teacher.teacher_id)),
                )
                .chain(group.room_id.map(Resource::Room))
        })
        .chain(
            block
                .homerooms
                .iter()
                .filter(|target| target.is_active)
                .flat_map(|target| {
                    std::iter::once(Resource::Homeroom(target.homeroom_id))
                        .chain(target.room_id.map(Resource::Room))
                }),
        )
        .chain(
            block
                .teachers
                .iter()
                .filter(|teacher| teacher.is_active)
                .map(|teacher| Resource::Teacher(teacher.teacher_id)),
        )
        .collect::<BTreeSet<_>>();
    footprint.slots = resources
        .into_iter()
        .map(|resource| Slot {
            resource,
            day: block.day_of_week.clone(),
            start: block.start_time,
            end: block.end_time,
        })
        .collect();
    footprint
}

fn source_footprint(
    workspace: &TimetableBlockWorkspace,
    source: &TimetableBlockPlacementSource,
) -> Result<Footprint, AppError> {
    match source {
        TimetableBlockPlacementSource::ExistingBlock {
            block_id,
            row_version,
        } => {
            let block = workspace
                .blocks
                .iter()
                .find(|block| block.id == *block_id && block.row_version == *row_version)
                .ok_or_else(stale)?;
            Ok(block_footprint(block))
        }
        TimetableBlockPlacementSource::OrdinaryDemand {
            learning_group_id,
            learning_offering_id,
        } => {
            if !workspace.ordinary_demands.iter().any(|demand| {
                demand.learning_group_id == *learning_group_id
                    && demand.learning_offering_id == *learning_offering_id
            }) {
                return Err(stale());
            }
            Ok(Footprint {
                identities: BTreeSet::from([Identity::Group(*learning_group_id)]),
                ..Default::default()
            })
        }
        TimetableBlockPlacementSource::SynchronizedOffering {
            learning_offering_id,
        } => {
            if !workspace
                .synchronized_demands
                .iter()
                .any(|demand| demand.learning_offering_id == *learning_offering_id)
            {
                return Err(stale());
            }
            Ok(Footprint {
                identities: BTreeSet::from([Identity::Offering(*learning_offering_id)]),
                ..Default::default()
            })
        }
    }
}

pub(super) fn candidate_from_block(block: &TimetableBlock) -> TimetableBlockPlacementCandidate {
    TimetableBlockPlacementCandidate {
        block_kind: block.block_kind,
        learning_group_id: block.groups.first().map(|group| group.learning_group_id),
        learning_offering_id: block.learning_offering_id,
        room_id: block
            .groups
            .iter()
            .find_map(|group| group.room_id)
            .or_else(|| block.homerooms.iter().find_map(|target| target.room_id)),
        homeroom_ids: block
            .groups
            .iter()
            .flat_map(|group| group.homeroom_ids.iter().copied())
            .chain(block.homerooms.iter().map(|target| target.homeroom_id))
            .collect(),
        teacher_ids: block
            .teachers
            .iter()
            .map(|teacher| teacher.teacher_id)
            .collect(),
        instructor_ids: block
            .groups
            .iter()
            .flat_map(|group| group.instructors.iter().map(|teacher| teacher.teacher_id))
            .collect(),
    }
}

fn normalize_candidate(
    workspace: &TimetableBlockWorkspace,
    source: &TimetableBlockPlacementSource,
    candidate: Option<TimetableBlockPlacementCandidate>,
) -> Result<TimetableBlockPlacementCandidate, AppError> {
    let candidate = match source {
        TimetableBlockPlacementSource::ExistingBlock { block_id, .. } => {
            candidate.unwrap_or(candidate_from_block(
                workspace
                    .blocks
                    .iter()
                    .find(|block| block.id == *block_id)
                    .ok_or_else(stale)?,
            ))
        }
        _ => candidate.ok_or_else(|| {
            AppError::ValidationError("A demand claim needs its private candidate".into())
        })?,
    };
    let committed = match source {
        TimetableBlockPlacementSource::ExistingBlock { block_id, .. } => workspace
            .blocks
            .iter()
            .find(|block| block.id == *block_id)
            .map(candidate_from_block),
        _ => None,
    };
    if candidate.room_id.is_some_and(|id| {
        !workspace.rooms.iter().any(|room| {
            room.id == id
                && (room.status == "ACTIVE"
                    || committed
                        .as_ref()
                        .is_some_and(|old| old.room_id == Some(id)))
        })
    }) || candidate.homeroom_ids.iter().any(|id| {
        !workspace
            .homerooms
            .iter()
            .any(|homeroom| homeroom.id == *id)
    }) || candidate
        .teacher_ids
        .iter()
        .chain(&candidate.instructor_ids)
        .any(|id| {
            !workspace.staff.iter().any(|staff| {
                staff.id == *id
                    && (staff.status == "active"
                        || committed.as_ref().is_some_and(|old| {
                            old.teacher_ids.contains(id) || old.instructor_ids.contains(id)
                        }))
            })
        })
    {
        return Err(forbidden());
    }
    let mut candidate = match source {
        TimetableBlockPlacementSource::OrdinaryDemand {
            learning_group_id,
            learning_offering_id,
        } => {
            let group = workspace
                .learning_groups
                .iter()
                .find(|group| {
                    group.id == *learning_group_id
                        && group.learning_offering_id == *learning_offering_id
                })
                .ok_or_else(stale)?;
            if candidate.learning_group_id != Some(*learning_group_id)
                || candidate.learning_offering_id != Some(*learning_offering_id)
                || candidate.instructor_ids.iter().any(|id| {
                    !group
                        .eligible_instructors
                        .iter()
                        .any(|teacher| teacher.teacher_id == *id)
                })
            {
                return Err(forbidden());
            }
            Ok(TimetableBlockPlacementCandidate {
                homeroom_ids: group.homeroom_ids.clone(),
                ..candidate
            })
        }
        TimetableBlockPlacementSource::SynchronizedOffering {
            learning_offering_id,
        } if candidate.learning_offering_id != Some(*learning_offering_id) => Err(forbidden()),
        _ => Ok(candidate),
    }?;
    candidate.homeroom_ids.sort_unstable();
    candidate.homeroom_ids.dedup();
    candidate.teacher_ids.sort_unstable();
    candidate.teacher_ids.dedup();
    candidate.instructor_ids.sort_unstable();
    candidate.instructor_ids.dedup();
    Ok(candidate)
}

fn target_footprint(
    workspace: &TimetableBlockWorkspace,
    source: &TimetableBlockPlacementSource,
    candidate: &TimetableBlockPlacementCandidate,
    target: &DraftTarget,
) -> Result<Footprint, AppError> {
    if !valid_day(&target.day_of_week) {
        return Err(stale());
    }
    let period = workspace
        .bell_periods
        .iter()
        .find(|period| period.id == target.bell_schedule_period_id)
        .ok_or_else(stale)?;
    let resources = candidate
        .homeroom_ids
        .iter()
        .copied()
        .map(Resource::Homeroom)
        .chain(
            candidate
                .teacher_ids
                .iter()
                .chain(&candidate.instructor_ids)
                .copied()
                .map(Resource::Teacher),
        )
        .chain(candidate.room_id.map(Resource::Room))
        .chain(candidate.learning_group_id.map(Resource::Group))
        .collect::<BTreeSet<_>>();
    let mut footprint = Footprint {
        slots: resources
            .into_iter()
            .map(|resource| Slot {
                resource,
                day: target.day_of_week.clone(),
                start: period.start_time,
                end: period.end_time,
            })
            .collect(),
        ..Default::default()
    };
    if let TimetableBlockPlacementSource::ExistingBlock { block_id, .. } = source {
        let block = workspace
            .blocks
            .iter()
            .find(|block| block.id == *block_id)
            .ok_or_else(stale)?;
        let mut shifted = block_footprint(block);
        for slot in &mut shifted.slots {
            slot.day = target.day_of_week.clone();
            slot.start = period.start_time;
            slot.end = period.end_time;
        }
        footprint.extend(shifted);
    }
    if let TimetableBlockPlacementSource::SynchronizedOffering {
        learning_offering_id,
    } = source
    {
        for group in workspace
            .learning_groups
            .iter()
            .filter(|group| group.learning_offering_id == *learning_offering_id)
        {
            let resources = std::iter::once(Resource::Group(group.id))
                .chain(group.homeroom_ids.iter().copied().map(Resource::Homeroom))
                .chain(
                    group
                        .eligible_instructors
                        .iter()
                        .map(|teacher| Resource::Teacher(teacher.teacher_id)),
                )
                .chain(
                    group
                        .preferred_room_ids
                        .first()
                        .copied()
                        .map(Resource::Room),
                );
            footprint.slots.extend(resources.map(|resource| Slot {
                resource,
                day: target.day_of_week.clone(),
                start: period.start_time,
                end: period.end_time,
            }));
        }
    }
    if let Some(block_id) = target.block_id {
        let block = workspace
            .blocks
            .iter()
            .find(|block| {
                block.id == block_id
                    && block.day_of_week == target.day_of_week
                    && block.bell_schedule_period_id == target.bell_schedule_period_id
            })
            .ok_or_else(stale)?;
        footprint.extend(block_footprint(block));
        // A swap occupies the target's resources at the source's original slot too.
        if let TimetableBlockPlacementSource::ExistingBlock { block_id, .. } = source {
            let original = workspace
                .blocks
                .iter()
                .find(|block| block.id == *block_id)
                .ok_or_else(stale)?;
            let mut reverse = block_footprint(block);
            for slot in &mut reverse.slots {
                slot.day = original.day_of_week.clone();
                slot.start = original.start_time;
                slot.end = original.end_time;
            }
            footprint.extend(reverse);
        }
    }
    Ok(footprint)
}

fn conflict(state: &RoomState, footprint: &Footprint, own_token: Option<Uuid>) -> bool {
    state
        .leases
        .iter()
        .any(|(token, lease)| Some(*token) != own_token && footprint.conflicts(&lease.footprint))
        || state
            .committing
            .values()
            .any(|other| footprint.conflicts(&other.footprint))
}

fn require_candidate_access(
    state: &RoomState,
    member: &DraftMember,
    source: &TimetableBlockPlacementSource,
    candidate: &TimetableBlockPlacementCandidate,
) -> Result<(), AppError> {
    let filter = member.manage_access.as_ref().ok_or_else(forbidden)?;
    let mut resources = crate::policy::TimetableResourceSet {
        timetable_version_ids: vec![state.workspace.version.id],
        room_ids: candidate.room_id.into_iter().collect(),
        teacher_ids: candidate
            .teacher_ids
            .iter()
            .chain(&candidate.instructor_ids)
            .copied()
            .collect(),
        ..Default::default()
    };
    match source {
        TimetableBlockPlacementSource::ExistingBlock { block_id, .. } => resources.include_block(
            state
                .workspace
                .blocks
                .iter()
                .find(|block| block.id == *block_id)
                .ok_or_else(stale)?,
        ),
        TimetableBlockPlacementSource::OrdinaryDemand {
            learning_group_id, ..
        } => resources.learning_group_ids.push(*learning_group_id),
        TimetableBlockPlacementSource::SynchronizedOffering {
            learning_offering_id,
        } => {
            // CREATE authorizes the offering and intended homerooms, while all selected
            // teachers still participate in the reservation footprint and DB constraints.
            resources.teacher_ids.clear();
            resources.learning_offering_ids.push(*learning_offering_id);
            resources.homeroom_ids.extend(&candidate.homeroom_ids);
        }
    }
    member.resource_access.require_resources(&resources, filter)
}

pub(super) fn lease_command(
    room: &DraftRoom,
    connection_id: Uuid,
    command: DraftClientMessage,
) -> Result<Vec<DraftEventEnvelope>, AppError> {
    let mut state = room.lock()?;
    if !state.valid {
        return Err(stale());
    }
    let member = state.members.get(&connection_id).ok_or_else(stale)?.clone();
    let projected = state
        .member_visibility
        .get(&connection_id)
        .and_then(|indices| indices.manage.as_ref())
        .ok_or_else(forbidden)?;
    let (request_id, kind) = match &command {
        DraftClientMessage::ClientClaim(value) => (value.request_id, DraftCommandKind::Claim),
        DraftClientMessage::ClientTarget(value) => (value.request_id, DraftCommandKind::Target),
        DraftClientMessage::ClientRenew(value) => (value.request_id, DraftCommandKind::Renew),
        DraftClientMessage::ClientRelease(value) => (value.request_id, DraftCommandKind::Release),
        _ => return Err(stale()),
    };
    let mut result = DraftCommandResult {
        request_id,
        command: kind,
        accepted: false,
        code: None,
        source: None,
        target: None,
        reservation_id: None,
        lease_token: None,
        expires_at: None,
    };
    if !matches!(&command, DraftClientMessage::ClientRelease(_))
        && state.workspace.version.delivery_version_id != state.workspace.latest_delivery_version_id
    {
        result.code = Some("delivery_source_outdated".into());
        return Ok(vec![DraftEventEnvelope {
            epoch: state.epoch,
            seq: state.seq,
            event: DraftServerMessage::DraftCommandResult(result),
        }]);
    }
    match command {
        DraftClientMessage::ClientClaim(claim) => {
            result.source = Some(claim.source.clone());
            if !source_visible(&projected, &claim.source) {
                return Err(forbidden());
            }
            let footprint = source_footprint(&state.workspace, &claim.source)?;
            let candidate = normalize_candidate(&state.workspace, &claim.source, claim.candidate)?;
            require_candidate_access(&state, &member, &claim.source, &candidate)?;
            if state
                .leases
                .values()
                .filter(|lease| lease.public.owner_connection_id == connection_id)
                .count()
                >= 16
                || state.leases.len() >= 512
            {
                result.code = Some("pending_limit".into());
            } else if conflict(&state, &footprint, None) {
                result.code = Some("reserved".into());
            } else {
                let token = Uuid::new_v4();
                let public = DraftReservation {
                    reservation_id: Uuid::new_v4(),
                    owner_connection_id: connection_id,
                    purpose: claim.purpose,
                    source: claim.source,
                    target: None,
                    expires_at: expires_at(),
                };
                result.accepted = true;
                result.reservation_id = Some(public.reservation_id);
                result.lease_token = Some(token);
                result.expires_at = Some(public.expires_at);
                // Pending saves retain their earlier independently owned leases.
                state.active_leases.insert(connection_id, token);
                state.leases.insert(
                    token,
                    Lease {
                        public,
                        token,
                        candidate: Some(candidate),
                        footprint,
                        deadline: Instant::now() + LEASE_TTL,
                    },
                );
            }
        }
        DraftClientMessage::ClientTarget(update) => {
            let lease = state
                .leases
                .get(&update.lease_token)
                .filter(|lease| lease.public.owner_connection_id == connection_id)
                .ok_or_else(stale)?;
            let source = lease.public.source.clone();
            result.source = Some(source.clone());
            result.target = update.target.clone();
            let candidate = normalize_candidate(
                &state.workspace,
                &source,
                update.candidate.or_else(|| lease.candidate.clone()),
            )?;
            require_candidate_access(&state, &member, &source, &candidate)?;
            let mut footprint = source_footprint(&state.workspace, &source)?;
            if let Some(target) = &update.target {
                if target
                    .block_id
                    .is_some_and(|id| !projected.blocks.contains(&id))
                {
                    return Err(forbidden());
                }
                footprint.extend(target_footprint(
                    &state.workspace,
                    &source,
                    &candidate,
                    target,
                )?);
            }
            if conflict(&state, &footprint, Some(update.lease_token)) {
                result.code = Some("reserved".into());
            } else {
                let lease = state
                    .leases
                    .get_mut(&update.lease_token)
                    .ok_or_else(stale)?;
                lease.public.target = update.target;
                lease.public.expires_at = expires_at();
                lease.deadline = Instant::now() + LEASE_TTL;
                lease.candidate = Some(candidate);
                lease.footprint = footprint;
                grant_result(&mut result, lease);
            }
        }
        DraftClientMessage::ClientRenew(command) => {
            let lease = state
                .leases
                .get_mut(&command.lease_token)
                .filter(|lease| lease.public.owner_connection_id == connection_id)
                .ok_or_else(stale)?;
            lease.public.expires_at = expires_at();
            lease.deadline = Instant::now() + LEASE_TTL;
            grant_result(&mut result, lease);
        }
        DraftClientMessage::ClientRelease(command) => {
            let lease = state
                .leases
                .get(&command.lease_token)
                .filter(|lease| lease.public.owner_connection_id == connection_id)
                .ok_or_else(stale)?;
            result.source = Some(lease.public.source.clone());
            result.target = lease.public.target.clone();
            result.accepted = true;
            if !state
                .committing
                .values()
                .any(|record| record.lease_token == Some(command.lease_token))
            {
                state.leases.remove(&command.lease_token);
            }
            if state.active_leases.get(&connection_id) == Some(&command.lease_token) {
                state.active_leases.remove(&connection_id);
            }
        }
        _ => return Err(stale()),
    }
    let event = DraftEventEnvelope {
        epoch: state.epoch,
        seq: state.seq,
        event: DraftServerMessage::DraftCommandResult(result),
    };
    drop(state);
    let _ = room.signals.send(DraftSignal::Reservations);
    let _ = room.signals.send(DraftSignal::Presence(connection_id));
    Ok(vec![event])
}

fn grant_result(result: &mut DraftCommandResult, lease: &Lease) {
    result.accepted = true;
    result.source = Some(lease.public.source.clone());
    result.target = lease.public.target.clone();
    result.reservation_id = Some(lease.public.reservation_id);
    result.lease_token = Some(lease.token);
    result.expires_at = Some(lease.public.expires_at);
}

fn room_options(state: &RoomState, member: &DraftMember, own: &Lease) -> Option<DraftRoomOptions> {
    if own.public.purpose != DraftReservationPurpose::Edit {
        return None;
    }
    let TimetableBlockPlacementSource::ExistingBlock {
        block_id,
        row_version,
    } = own.public.source
    else {
        return None;
    };
    let block = state.workspace.blocks.iter().find(|block| {
        block.id == block_id && block.is_active && block.row_version == row_version
    })?;
    let access = member.manage_access.as_ref()?;
    let visible = &state
        .member_visibility
        .get(&member.connection_id)?
        .read
        .rooms;
    let original_room = candidate_from_block(block).room_id;
    let mut occupied = BTreeSet::new();
    let mut collect = |footprint: &Footprint| {
        for slot in &footprint.slots {
            if let Resource::Room(id) = slot.resource {
                if slot.overlaps(&block.day_of_week, block.start_time, block.end_time) {
                    occupied.insert(id);
                }
            }
        }
    };
    // Scan authority footprints once, including blocks outside this viewer's read scope.
    for other in state
        .workspace
        .blocks
        .iter()
        .filter(|other| other.is_active && other.id != block.id)
    {
        collect(&block_footprint(other));
    }
    let mut whole_draft = false;
    for footprint in state
        .leases
        .values()
        .filter(|lease| lease.token != own.token)
        .map(|lease| &lease.footprint)
        .chain(
            state
                .committing
                .values()
                .filter(|record| record.lease_token != Some(own.token))
                .map(|record| &record.footprint),
        )
    {
        whole_draft |= footprint.whole_draft;
        collect(footprint);
    }
    let available_room_ids = state
        .workspace
        .rooms
        .iter()
        .filter(|room| {
            // Existing edits may retain their original inactive room, as normalize_candidate does.
            (room.status == "ACTIVE" || original_room == Some(room.id))
                && visible.contains(&room.id)
                && !whole_draft
                && !occupied.contains(&room.id)
                && member
                    .resource_access
                    .require_resources(
                        &crate::policy::TimetableResourceSet {
                            room_ids: vec![room.id],
                            ..Default::default()
                        },
                        access,
                    )
                    .is_ok()
        })
        .map(|room| room.id)
        .collect();
    Some(DraftRoomOptions {
        reservation_id: own.public.reservation_id,
        available_room_ids,
    })
}

pub(super) fn availability(state: &RoomState, member: &DraftMember) -> Option<DraftAvailability> {
    let own = state
        .leases
        .get(state.active_leases.get(&member.connection_id)?)?;
    let candidate = own.candidate.as_ref()?;
    let mut blocked_cells = Vec::new();
    for day in ["MON", "TUE", "WED", "THU", "FRI", "SAT", "SUN"] {
        for period in &state.workspace.bell_periods {
            let target = DraftTarget {
                day_of_week: day.into(),
                bell_schedule_period_id: period.id,
                block_id: None,
            };
            let footprint =
                target_footprint(&state.workspace, &own.public.source, candidate, &target).ok()?;
            for record in state
                .committing
                .values()
                .filter(|record| footprint.conflicts(&record.footprint))
            {
                blocked_cells.push(DraftBlockedCell {
                    day_of_week: day.into(),
                    bell_schedule_period_id: period.id,
                    reservation_id: record.reservation_id,
                    owner_connection_id: record.owner_connection_id,
                    purpose: record.purpose,
                });
            }
            for other in state
                .leases
                .values()
                .filter(|lease| lease.token != own.token && footprint.conflicts(&lease.footprint))
            {
                blocked_cells.push(DraftBlockedCell {
                    day_of_week: day.into(),
                    bell_schedule_period_id: period.id,
                    reservation_id: other.public.reservation_id,
                    owner_connection_id: other.public.owner_connection_id,
                    purpose: other.public.purpose,
                });
            }
        }
    }
    Some(DraftAvailability {
        source: own.public.source.clone(),
        context: member.context.clone(),
        blocked_cells,
        room_options: room_options(state, member, own),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn slot(resource: Resource, start: &str, end: &str) -> Footprint {
        Footprint {
            slots: vec![Slot {
                resource,
                day: "MON".into(),
                start: start.parse().unwrap(),
                end: end.parse().unwrap(),
            }],
            ..Default::default()
        }
    }
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
        let source = Footprint {
            identities: BTreeSet::from([Identity::Group(id)]),
            ..Default::default()
        };
        assert!(source.conflicts(&source));
        assert!(source.conflicts(&Footprint {
            whole_draft: true,
            ..Default::default()
        }));
        assert!(!source.conflicts(&Footprint::default()));
    }
}
