use std::sync::{Arc, Barrier};

use school_academic_core::models::{BellSchedulePeriod, TimetableVersionStatus};
use school_academic_delivery::models::versions::DeliverySnapshot;

use super::*;
use crate::models::timetable_block::*;
use crate::models::timetable_version::TimetableVersion;

fn block(id: Uuid, version: &TimetableVersion, period: Uuid, teacher: Uuid) -> TimetableBlock {
    TimetableBlock {
        id,
        timetable_version_id: version.id,
        academic_term_id: version.academic_term_id,
        academic_year_id: version.academic_year_id,
        bell_schedule_id: version.bell_schedule_id,
        bell_schedule_period_id: period,
        period_name: "Period".into(),
        start_time: "08:00".parse().unwrap(),
        end_time: "09:00".parse().unwrap(),
        day_of_week: "MON".into(),
        block_kind: TimetableBlockKind::Structural,
        scheduling_mode: None,
        learning_offering_id: None,
        offering_code: None,
        offering_name: None,
        structural_kind: Some(TimetableStructuralKind::Other),
        title: Some("Lesson".into()),
        note: None,
        series_id: None,
        groups: Vec::new(),
        homerooms: Vec::new(),
        teachers: vec![TimetableBlockTeacher {
            id: Uuid::new_v4(),
            teacher_id: teacher,
            display_name: "Teacher".into(),
            row_version: 1,
            is_active: true,
        }],
        sync_states: Vec::new(),
        row_version: 1,
        is_active: true,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

fn fixture() -> (Arc<DraftRoom>, TimetableCollaborationRuntime, Uuid) {
    let version = TimetableVersion {
        publication_no: None,
        id: Uuid::new_v4(),
        academic_term_id: Uuid::new_v4(),
        academic_year_id: Uuid::new_v4(),
        effective_from: None,
        effective_until: None,
        status: TimetableVersionStatus::Draft,
        display_state: None,
        source_version_id: None,
        delivery_version_id: Uuid::new_v4(),
        bell_schedule_id: Uuid::new_v4(),
        row_version: 1,
        created_by: None,
        published_by: None,
        published_at: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        targets: Vec::new(),
    };
    let teacher = Uuid::new_v4();
    let period_id = Uuid::new_v4();
    let period = BellSchedulePeriod {
        id: period_id,
        bell_schedule_id: version.bell_schedule_id,
        name: Some("Period".into()),
        start_time: "08:00".parse().unwrap(),
        end_time: "09:00".parse().unwrap(),
        order_index: 1,
        applicable_days: None,
        is_active: true,
    };
    let second_teacher = Uuid::new_v4();
    let blocks = vec![
        block(Uuid::new_v4(), &version, period_id, teacher),
        block(Uuid::new_v4(), &version, period_id, second_teacher),
    ];
    let publication = school_academic_delivery::models::versions::DeliveryPublicationContext {
        id: version.delivery_version_id,
        publication_no: 1,
        effective_from: Utc::now().date_naive(),
        effective_until: None,
        published_at: Some(Utc::now()),
        is_superseded: false,
    };
    let workspace = TimetableBlockWorkspace {
        publication_date_rules: school_academic_delivery::models::versions::PublicationDateRules {
            today: Utc::now().date_naive(),
            minimum_effective_from: None,
        },
        delivery_publication: publication.clone(),
        latest_delivery_publication: publication,
        total_draft_block_count: Some(2),
        source_issues: Vec::new(),
        latest_delivery_version_id: version.delivery_version_id,
        version,
        bell_periods: vec![period],
        blocks,
        ordinary_demands: Vec::new(),
        synchronized_demands: Vec::new(),
        learning_groups: Vec::new(),
        homerooms: Vec::new(),
        rooms: Vec::new(),
        staff: [teacher, second_teacher]
            .into_iter()
            .map(|id| TimetableBlockWorkspaceStaff {
                id,
                subject_groups: Vec::new(),
                display_name: "Teacher".into(),
                status: "active".into(),
            })
            .collect(),
        offering_subject_groups: Vec::new(),
        summary: TimetableBlockSummary {
            block_count: 2,
            ordinary_demand_count: 0,
            synchronized_demand_count: 0,
            linked_group_count: 0,
            waiting_group_count: 0,
            conflict_group_count: 0,
            excluded_group_count: 0,
        },
    };
    let key = DraftKey {
        tenant_id: Uuid::new_v4(),
        academic_term_id: workspace.version.academic_term_id,
        timetable_version_id: workspace.version.id,
    };
    let runtime = TimetableCollaborationRuntime::new();
    let room = runtime
        .install(
            key,
            workspace,
            TimetableVisibility {
                source: DeliverySnapshot {
                    offerings: Vec::new(),
                },
                block_offering_owners: BTreeMap::new(),
                homeroom_advisors: BTreeMap::new(),
            },
        )
        .unwrap();
    (room, runtime, teacher)
}

fn member(actor_id: Uuid, access: TimetableAccessFilter) -> DraftMember {
    DraftMember {
        connection_id: Uuid::new_v4(),
        actor: ActorContext {
            user_id: actor_id,
            permissions: Vec::new(),
        },
        session_id: Uuid::new_v4(),
        name: "Editor".into(),
        color: "#123456".into(),
        manage_access: Some(access.clone()),
        resource_access: Arc::new(crate::policy::TimetableResourceAccessSnapshot::default()),
        access,
        portraits: BTreeMap::new(),
        context: None,
        anchor: None,
    }
}
fn full_access() -> TimetableAccessFilter {
    TimetableAccessFilter {
        includes_school_owned: true,
        ..Default::default()
    }
}
fn source(room: &DraftRoom) -> TimetableBlockPlacementSource {
    let state = room.lock().unwrap();
    TimetableBlockPlacementSource::ExistingBlock {
        block_id: state.workspace.blocks[0].id,
        row_version: 1,
    }
}
fn claim(
    room: &DraftRoom,
    connection: Uuid,
    source: TimetableBlockPlacementSource,
) -> DraftCommandResult {
    claim_with_purpose(room, connection, source, DraftReservationPurpose::Drag)
}

fn claim_with_purpose(
    room: &DraftRoom,
    connection: Uuid,
    source: TimetableBlockPlacementSource,
    purpose: DraftReservationPurpose,
) -> DraftCommandResult {
    let events = room
        .command(
            connection,
            DraftClientMessage::ClientClaim(DraftClaim {
                request_id: Uuid::new_v4(),
                source,
                purpose,
                candidate: None,
            }),
        )
        .unwrap();
    let DraftServerMessage::DraftCommandResult(result) = events[0].event.clone() else {
        panic!("command result expected")
    };
    result
}

#[test]
fn stale_source_blocks_cold_claim_and_content_admission_but_keeps_lifecycle_authority() {
    let (room, _, actor) = fixture();
    {
        let mut state = room.lock().unwrap();
        Arc::make_mut(&mut state.workspace).latest_delivery_version_id = Uuid::new_v4();
    }
    let viewer = member(actor, full_access());
    room.join(viewer.clone()).unwrap();
    let result = claim(&room, viewer.connection_id, source(&room));
    assert!(!result.accepted);
    assert_eq!(result.code.as_deref(), Some("delivery_source_outdated"));
    assert!(room.lock().unwrap().leases.is_empty());
    let block_id = room.lock().unwrap().workspace.blocks[0].id;
    assert!(matches!(
        room.begin_commit(
            actor,
            viewer.session_id,
            None,
            MutationScope::Blocks(vec![block_id])
        ),
        Err(AppError::Conflict(_))
    ));
    let permit = room
        .begin_commit(actor, viewer.session_id, None, MutationScope::WholeDraft)
        .unwrap();
    drop(permit);
    assert!(room.lock().unwrap().valid);
}

#[test]
fn stale_source_refuses_renew_but_allows_the_owner_to_release_an_existing_lease() {
    let (room, _, actor) = fixture();
    let viewer = member(actor, full_access());
    room.join(viewer.clone()).unwrap();
    let granted = claim(&room, viewer.connection_id, source(&room));
    let token = granted.lease_token.unwrap();
    {
        let mut state = room.lock().unwrap();
        Arc::make_mut(&mut state.workspace).latest_delivery_version_id = Uuid::new_v4();
    }
    let renew = room
        .command(
            viewer.connection_id,
            DraftClientMessage::ClientRenew(DraftLeaseCommand {
                request_id: Uuid::new_v4(),
                lease_token: token,
            }),
        )
        .unwrap();
    let DraftServerMessage::DraftCommandResult(result) = &renew[0].event else {
        panic!("command result expected")
    };
    assert!(!result.accepted);
    assert_eq!(result.code.as_deref(), Some("delivery_source_outdated"));
    assert!(room.lock().unwrap().leases.contains_key(&token));
    let released = room
        .command(
            viewer.connection_id,
            DraftClientMessage::ClientRelease(DraftLeaseCommand {
                request_id: Uuid::new_v4(),
                lease_token: token,
            }),
        )
        .unwrap();
    let DraftServerMessage::DraftCommandResult(result) = &released[0].event else {
        panic!("command result expected")
    };
    assert!(result.accepted);
    assert!(room.lock().unwrap().leases.is_empty());
}

#[test]
fn simultaneous_first_claim_has_exactly_one_winner_even_for_same_user_tabs() {
    let (room, _, actor) = fixture();
    let first = member(actor, full_access());
    let second = member(actor, full_access());
    room.join(first.clone()).unwrap();
    room.join(second.clone()).unwrap();
    let source = source(&room);
    let barrier = Arc::new(Barrier::new(3));
    let mut jobs = Vec::new();
    for connection in [first.connection_id, second.connection_id] {
        let room = room.clone();
        let source = source.clone();
        let barrier = barrier.clone();
        jobs.push(std::thread::spawn(move || {
            barrier.wait();
            claim(&room, connection, source).accepted
        }));
    }
    barrier.wait();
    assert_eq!(
        jobs.into_iter()
            .map(|job| job.join().unwrap())
            .filter(|accepted| *accepted)
            .count(),
        1
    );
}

#[test]
fn separate_blocks_can_be_claimed_concurrently_and_private_tokens_never_reach_peers() {
    let (room, _, actor) = fixture();
    let first = member(actor, full_access());
    let second = member(actor, full_access());
    room.join(first.clone()).unwrap();
    room.join(second.clone()).unwrap();
    let first_claim = claim(&room, first.connection_id, source(&room));
    assert!(first_claim.accepted);
    let second_source = {
        let state = room.lock().unwrap();
        TimetableBlockPlacementSource::ExistingBlock {
            block_id: state.workspace.blocks[1].id,
            row_version: 1,
        }
    };
    assert!(claim(&room, second.connection_id, second_source).accepted);
    let peer = room
        .signal_events(second.connection_id, DraftSignal::Reservations)
        .unwrap();
    let wire = serde_json::to_string(&peer).unwrap();
    assert!(!wire.contains(&first_claim.lease_token.unwrap().to_string()));
    assert!(!wire.contains("candidate"));
    assert!(!wire.contains("leaseToken"));
}

#[test]
fn wrong_tab_cannot_renew_or_release_and_expiry_frees_source() {
    let (room, _, actor) = fixture();
    let first = member(actor, full_access());
    let second = member(actor, full_access());
    room.join(first.clone()).unwrap();
    room.join(second.clone()).unwrap();
    let grant = claim(&room, first.connection_id, source(&room));
    let token = grant.lease_token.unwrap();
    for command in [
        DraftClientMessage::ClientRenew(DraftLeaseCommand {
            request_id: Uuid::new_v4(),
            lease_token: token,
        }),
        DraftClientMessage::ClientRelease(DraftLeaseCommand {
            request_id: Uuid::new_v4(),
            lease_token: token,
        }),
    ] {
        let response = room.command(second.connection_id, command).unwrap();
        assert!(
            matches!(&response[0].event, DraftServerMessage::DraftCommandResult(result) if !result.accepted && result.lease_token.is_none())
        );
    }
    room.expire(Instant::now() + LEASE_TTL);
    assert!(claim(&room, second.connection_id, source(&room)).accepted);
}

#[test]
fn disconnect_does_not_release_pinned_commit_exclusion() {
    let (room, _, actor) = fixture();
    let first = member(actor, full_access());
    let second = member(actor, full_access());
    room.join(first.clone()).unwrap();
    room.join(second.clone()).unwrap();
    let grant = claim_with_purpose(
        &room,
        first.connection_id,
        source(&room),
        DraftReservationPurpose::Edit,
    );
    let identity = DraftMutationIdentity {
        connection_id: first.connection_id,
        lease_token: grant.lease_token.unwrap(),
        mutation_id: Uuid::new_v4(),
    };
    let id = room.lock().unwrap().workspace.blocks[0].id;
    let permit = room
        .begin_commit(
            actor,
            first.session_id,
            Some(&identity),
            MutationScope::Blocks(vec![id]),
        )
        .unwrap();
    room.disconnect(first.connection_id);
    room.expire(Instant::now() + LEASE_TTL * 2);
    assert!(!claim(&room, second.connection_id, source(&room)).accepted);
    drop(permit);
    assert!(claim(&room, second.connection_id, source(&room)).accepted);
}

#[test]
fn direct_http_commit_cannot_bypass_another_connections_reservation() {
    let (room, _, actor) = fixture();
    let first = member(actor, full_access());
    room.join(first.clone()).unwrap();
    assert!(claim(&room, first.connection_id, source(&room)).accepted);
    assert!(room
        .begin_commit(
            Uuid::new_v4(),
            Uuid::new_v4(),
            None,
            MutationScope::WholeDraft
        )
        .is_err());
}

#[test]
fn zero_subscriber_commit_advances_journal_and_snapshot_atomically() {
    let (room, _, actor) = fixture();
    let mut changed = room.lock().unwrap().workspace.blocks[0].clone();
    changed.row_version += 1;
    assert_eq!(
        room.publish_blocks(vec![changed.clone()], None, None, Uuid::new_v4())
            .unwrap(),
        1
    );
    let viewer = member(actor, full_access());
    let sync = room.join(viewer.clone()).unwrap();
    assert_eq!(sync.seq, 1);
    let DraftServerMessage::DraftStateSync(sync) = sync.event else {
        panic!("state sync")
    };
    assert_eq!(
        sync.workspace
            .blocks
            .iter()
            .find(|block| block.id == changed.id)
            .unwrap()
            .row_version,
        2
    );
    let epoch = room.lock().unwrap().epoch;
    let replay = room
        .command(
            viewer.connection_id,
            DraftClientMessage::ClientReplay(DraftReplay {
                epoch,
                after_seq: 0,
            }),
        )
        .unwrap();
    assert_eq!(replay.len(), 1);
    assert_eq!(replay[0].seq, 1);
    assert!(matches!(
        replay[0].event,
        DraftServerMessage::DraftReconcileRequired { .. }
    ));
    changed.row_version += 1;
    room.publish_blocks(vec![changed], None, None, Uuid::new_v4())
        .unwrap();
    let replay = room
        .command(
            viewer.connection_id,
            DraftClientMessage::ClientReplay(DraftReplay {
                epoch,
                after_seq: 1,
            }),
        )
        .unwrap();
    assert_eq!(replay.len(), 1);
    assert!(matches!(
        replay[0].event,
        DraftServerMessage::DraftPatched(_)
    ));
    assert_eq!(replay[0].seq, 2);
}

#[test]
fn unrelated_large_canonical_graph_does_not_overflow_small_patch_journal() {
    let (room, _, actor) = fixture();
    {
        let mut state = room.lock().unwrap();
        let mut workspace = (*state.workspace).clone();
        workspace.blocks[0].note = Some("x".repeat(9 * 1024 * 1024));
        state.workspace = Arc::new(workspace);
    }
    let viewer = member(actor, full_access());
    room.join(viewer.clone()).unwrap();
    let mut changed = room.lock().unwrap().workspace.blocks[1].clone();
    changed.row_version += 1;
    changed.title = Some("Small changed lesson".into());
    let changed_id = changed.id;
    room.publish_blocks(vec![changed], None, None, Uuid::new_v4())
        .unwrap();
    let events = room
        .signal_events(viewer.connection_id, DraftSignal::Committed(1))
        .unwrap();
    let DraftServerMessage::DraftPatched(patch) = &events[0].event else {
        panic!("small patch must not reconcile")
    };
    assert_eq!(patch.upserted_blocks.len(), 1);
    assert_eq!(patch.upserted_blocks[0].id, changed_id);
    assert!(room.lock().unwrap().journal_bytes < 100_000);
    assert!(serde_json::to_vec(&events).unwrap().len() < 100_000);
}

#[test]
fn before_after_scope_change_emits_removal_without_new_unauthorized_resource() {
    let (room, _, actor) = fixture();
    let offering_id = Uuid::new_v4();
    let period_id = {
        let mut state = room.lock().unwrap();
        let workspace = Arc::make_mut(&mut state.workspace);
        workspace.bell_periods[0].is_active = false;
        let block = &mut workspace.blocks[0];
        block.block_kind = TimetableBlockKind::Course;
        block.structural_kind = None;
        block.learning_offering_id = Some(offering_id);
        block.teachers.clear();
        block.groups.push(TimetableBlockGroup {
            id: Uuid::new_v4(),
            learning_group_id: Uuid::new_v4(),
            learning_offering_id: offering_id,
            code: "Retained".into(),
            name: "Retained source group".into(),
            homeroom_ids: Vec::new(),
            homeroom_names: Vec::new(),
            room_id: None,
            room_code: None,
            room_name: None,
            instructors: vec![TimetableBlockInstructor {
                teacher_id: actor,
                display_name: "Teacher".into(),
                role: "primary".into(),
                order_index: 1,
            }],
            sync_status: None,
            row_version: 1,
            is_active: true,
        });
        workspace
            .offering_subject_groups
            .push(TimetableOfferingSubjectGroup {
                learning_offering_id: offering_id,
                subject_group: TimetableSubjectGroup {
                    id: Uuid::new_v4(),
                    name: "Retained subject".into(),
                    display_order: Some(1),
                },
            });
        workspace.bell_periods[0].id
    };
    let viewer = member(
        actor,
        TimetableAccessFilter {
            assigned_actor_id: Some(actor),
            ..Default::default()
        },
    );
    room.join(viewer.clone()).unwrap();
    let mut changed = room.lock().unwrap().workspace.blocks[0].clone();
    let id = changed.id;
    let mut restored = changed.clone();
    let delayed_presence = DraftPresenceUpdate {
        context: DraftContext {
            view_mode: DraftViewMode::Teacher,
            view_id: Some(actor),
            day_of_week: Some("MON".into()),
        },
        anchor: Some(DraftAnchor::Block {
            block_id: id,
            u: 0.4,
            v: 0.6,
        }),
    };
    room.update_presence(viewer.connection_id, delayed_presence.clone())
        .unwrap();
    changed.groups[0].instructors[0].teacher_id = Uuid::new_v4();
    changed.row_version += 1;
    room.publish_blocks(vec![changed], None, None, Uuid::new_v4())
        .unwrap();
    let events = room
        .signal_events(viewer.connection_id, DraftSignal::Committed(1))
        .unwrap();
    let DraftServerMessage::DraftPatched(patch) = &events[0].event else {
        panic!("patch")
    };
    assert_eq!(patch.removed_block_ids, vec![id]);
    assert!(patch.upserted_blocks.is_empty());
    assert_eq!(patch.summary.block_count, 0);
    assert_eq!(events[0].seq, 1);
    room.update_presence(viewer.connection_id, delayed_presence)
        .unwrap();
    let state = room.lock().unwrap();
    assert!(state.members[&viewer.connection_id].context.is_none());
    assert!(state.members[&viewer.connection_id].anchor.is_none());
    drop(state);
    let lookups = patch.lookup_changes.as_ref().unwrap();
    assert_eq!(lookups.removed_staff_ids, vec![actor]);
    assert_eq!(lookups.removed_bell_period_ids, vec![period_id]);
    assert_eq!(
        lookups.removed_subject_group_offering_ids,
        vec![offering_id]
    );
    assert_eq!(lookups.staff_order, Some(Vec::new()));
    assert_eq!(lookups.bell_period_order, Some(Vec::new()));
    assert_eq!(lookups.subject_group_offering_order, Some(Vec::new()));
    assert!(lookups.upserted_offering_subject_groups.is_empty());
    restored.row_version += 2;
    room.publish_blocks(vec![restored], None, None, Uuid::new_v4())
        .unwrap();
    let events = room
        .signal_events(viewer.connection_id, DraftSignal::Committed(2))
        .unwrap();
    let DraftServerMessage::DraftPatched(patch) = &events[0].event else {
        panic!("patch")
    };
    let lookups = patch.lookup_changes.as_ref().unwrap();
    assert_eq!(lookups.staff_order, Some(vec![actor]));
    assert_eq!(lookups.bell_period_order, Some(vec![period_id]));
    assert_eq!(
        lookups.subject_group_offering_order,
        Some(vec![offering_id])
    );
    assert_eq!(
        lookups.upserted_offering_subject_groups[0].learning_offering_id,
        offering_id
    );
}

#[test]
fn invisible_commit_still_emits_empty_sequenced_patch_and_epoch_mismatch_reconciles() {
    let (room, _, actor) = fixture();
    let viewer = member(actor, TimetableAccessFilter::default());
    room.join(viewer.clone()).unwrap();
    let mut changed = room.lock().unwrap().workspace.blocks[0].clone();
    changed.row_version += 1;
    room.publish_blocks(vec![changed], None, None, Uuid::new_v4())
        .unwrap();
    let events = room
        .signal_events(viewer.connection_id, DraftSignal::Committed(1))
        .unwrap();
    let DraftServerMessage::DraftPatched(patch) = &events[0].event else {
        panic!("patch")
    };
    assert!(patch.upserted_blocks.is_empty());
    assert_eq!(events[0].seq, 1);
    let events = room
        .command(
            viewer.connection_id,
            DraftClientMessage::ClientReplay(DraftReplay {
                epoch: Uuid::new_v4(),
                after_seq: 0,
            }),
        )
        .unwrap();
    assert!(matches!(
        events[0].event,
        DraftServerMessage::DraftReconcileRequired { .. }
    ));
}

#[test]
fn client_cannot_forge_server_events_or_actor_fields_and_coordinates_are_bounded() {
    assert!(serde_json::from_value::<DraftClientMessage>(
        serde_json::json!({"type":"DraftPatched","payload":{}})
    )
    .is_err());
    assert!(serde_json::from_value::<DraftPresenceUpdate>(serde_json::json!({"context":{"viewMode":"wholeSchool","viewId":null},"anchor":null,"userId":Uuid::new_v4()})).is_err());
    assert!(!DraftAnchor::Block {
        block_id: Uuid::new_v4(),
        u: f64::NAN,
        v: 0.5
    }
    .has_valid_coordinates());
    assert!(!DraftAnchor::Block {
        block_id: Uuid::new_v4(),
        u: 1.1,
        v: 0.5
    }
    .has_valid_coordinates());
}

#[test]
fn high_frequency_presence_and_recipient_projection_do_not_rebuild_workspace_visibility() {
    let (room, _, actor) = fixture();
    let viewer = member(actor, full_access());
    let other = member(Uuid::new_v4(), full_access());
    room.join(viewer.clone()).unwrap();
    room.join(other.clone()).unwrap();
    let (block_id, rebuilds) = {
        let mut state = room.lock().unwrap();
        let mut workspace = (*state.workspace).clone();
        let original = workspace.blocks[0].clone();
        for _ in 0..2_000 {
            let mut block = original.clone();
            block.id = Uuid::new_v4();
            workspace.blocks.push(block);
        }
        state.workspace = Arc::new(workspace);
        (original.id, state.projection_rebuilds)
    };
    for _ in 0..250 {
        room.update_presence(
            viewer.connection_id,
            DraftPresenceUpdate {
                context: DraftContext {
                    view_mode: DraftViewMode::Teacher,
                    view_id: Some(actor),
                    day_of_week: Some("MON".into()),
                },
                anchor: Some(DraftAnchor::Block {
                    block_id,
                    u: 0.2,
                    v: 0.8,
                }),
            },
        )
        .unwrap();
        let events = room
            .signal_events(
                other.connection_id,
                DraftSignal::Presence(viewer.connection_id),
            )
            .unwrap();
        assert_eq!(events.len(), 1);
    }
    assert_eq!(room.lock().unwrap().projection_rebuilds, rebuilds);
}

#[test]
fn second_claim_keeps_first_captured_drop_proof_and_active_pointer_is_unambiguous() {
    let (room, _, actor) = fixture();
    let editor = member(actor, full_access());
    room.join(editor.clone()).unwrap();
    let first_source = source(&room);
    let first = claim(&room, editor.connection_id, first_source.clone());
    let target = DraftTarget {
        day_of_week: "TUE".into(),
        bell_schedule_period_id: room.lock().unwrap().workspace.bell_periods[0].id,
        block_id: None,
    };
    room.command(
        editor.connection_id,
        DraftClientMessage::ClientTarget(DraftTargetUpdate {
            request_id: Uuid::new_v4(),
            lease_token: first.lease_token.unwrap(),
            target: Some(target.clone()),
            candidate: None,
        }),
    )
    .unwrap();
    let second_source = {
        let state = room.lock().unwrap();
        TimetableBlockPlacementSource::ExistingBlock {
            block_id: state.workspace.blocks[1].id,
            row_version: 1,
        }
    };
    let second = claim(&room, editor.connection_id, second_source);
    assert!(second.accepted);
    let candidate = room.lock().unwrap().leases[&first.lease_token.unwrap()]
        .candidate
        .clone()
        .unwrap();
    let identity = DraftMutationIdentity {
        connection_id: editor.connection_id,
        lease_token: first.lease_token.unwrap(),
        mutation_id: Uuid::new_v4(),
    };
    let permit = room
        .begin_commit(
            actor,
            editor.session_id,
            Some(&identity),
            MutationScope::Placement {
                source: first_source,
                candidate,
                targets: vec![target],
            },
        )
        .unwrap();
    assert_eq!(
        room.lock().unwrap().active_leases[&editor.connection_id],
        second.lease_token.unwrap()
    );
    drop(permit);
    assert!(room
        .lock()
        .unwrap()
        .leases
        .contains_key(&second.lease_token.unwrap()));
}

#[test]
fn token_for_another_source_or_unconfirmed_room_cannot_authorize_actual_save() {
    let (room, _, actor) = fixture();
    let editor = member(actor, full_access());
    room.join(editor.clone()).unwrap();
    let grant = claim(&room, editor.connection_id, source(&room));
    let identity = DraftMutationIdentity {
        connection_id: editor.connection_id,
        lease_token: grant.lease_token.unwrap(),
        mutation_id: Uuid::new_v4(),
    };
    let other_id = room.lock().unwrap().workspace.blocks[1].id;
    assert!(room
        .begin_commit(
            actor,
            editor.session_id,
            Some(&identity),
            MutationScope::Blocks(vec![other_id])
        )
        .is_err());
    let mut candidate = room.lock().unwrap().leases[&grant.lease_token.unwrap()]
        .candidate
        .clone()
        .unwrap();
    candidate.room_id = Some(Uuid::new_v4());
    assert!(room
        .begin_commit(
            actor,
            editor.session_id,
            Some(&identity),
            MutationScope::Placement {
                source: source(&room),
                candidate,
                targets: Vec::new()
            }
        )
        .is_err());
}

#[test]
fn pinned_reservation_remains_public_after_disconnect_and_expiry_until_permit_finishes() {
    let (room, _, actor) = fixture();
    let editor = member(actor, full_access());
    let viewer = member(Uuid::new_v4(), full_access());
    room.join(editor.clone()).unwrap();
    room.join(viewer.clone()).unwrap();
    let grant = claim_with_purpose(
        &room,
        editor.connection_id,
        source(&room),
        DraftReservationPurpose::Edit,
    );
    let block_id = room.lock().unwrap().workspace.blocks[0].id;
    let proof = DraftMutationIdentity {
        connection_id: editor.connection_id,
        lease_token: grant.lease_token.unwrap(),
        mutation_id: Uuid::new_v4(),
    };
    let permit = room
        .begin_commit(
            actor,
            editor.session_id,
            Some(&proof),
            MutationScope::Blocks(vec![block_id]),
        )
        .unwrap();
    room.disconnect(editor.connection_id);
    room.expire(Instant::now() + LEASE_TTL * 2);
    let events = room
        .signal_events(viewer.connection_id, DraftSignal::Reservations)
        .unwrap();
    let DraftServerMessage::DraftReservationsChanged { reservations } = &events[0].event else {
        panic!("reservations")
    };
    assert_eq!(
        reservations[0].reservation_id,
        grant.reservation_id.unwrap()
    );
    drop(permit);
    let events = room
        .signal_events(viewer.connection_id, DraftSignal::Reservations)
        .unwrap();
    let DraftServerMessage::DraftReservationsChanged { reservations } = &events[0].event else {
        panic!("reservations")
    };
    assert!(reservations.is_empty());
}

#[test]
fn private_authorized_new_teacher_does_not_need_to_be_in_public_workspace_projection() {
    let (room, _, actor) = fixture();
    let added = Uuid::new_v4();
    {
        let mut state = room.lock().unwrap();
        Arc::make_mut(&mut state.workspace)
            .staff
            .push(TimetableBlockWorkspaceStaff {
                id: added,
                display_name: "Private new choice".into(),
                status: "active".into(),
                subject_groups: Vec::new(),
            });
    }
    let editor = member(actor, full_access());
    room.join(editor.clone()).unwrap();
    assert!(
        !room.lock().unwrap().member_visibility[&editor.connection_id]
            .read
            .staff
            .contains(&added)
    );
    let mut candidate = authority::candidate_from_block(&room.lock().unwrap().workspace.blocks[0]);
    candidate.teacher_ids.push(added);
    let response = room
        .command(
            editor.connection_id,
            DraftClientMessage::ClientClaim(DraftClaim {
                request_id: Uuid::new_v4(),
                source: source(&room),
                purpose: DraftReservationPurpose::Edit,
                candidate: Some(candidate),
            }),
        )
        .unwrap();
    assert!(
        matches!(&response[0].event, DraftServerMessage::DraftCommandResult(result) if result.accepted)
    );
}

#[test]
fn existing_inactive_teacher_can_be_claimed_for_repair_but_cannot_be_newly_selected() {
    let (room, _, actor) = fixture();
    let other_teacher = {
        let mut state = room.lock().unwrap();
        for teacher in &mut Arc::make_mut(&mut state.workspace).staff {
            teacher.status = "inactive".into();
        }
        state.workspace.staff[1].id
    };
    let editor = member(actor, full_access());
    room.join(editor.clone()).unwrap();
    let accepted = claim_with_purpose(
        &room,
        editor.connection_id,
        source(&room),
        DraftReservationPurpose::Edit,
    );
    assert!(
        accepted.accepted,
        "existing inactive references must remain repairable"
    );
    let mut candidate = authority::candidate_from_block(&room.lock().unwrap().workspace.blocks[0]);
    candidate.teacher_ids.push(other_teacher);
    let response = room
        .command(
            editor.connection_id,
            DraftClientMessage::ClientTarget(DraftTargetUpdate {
                request_id: Uuid::new_v4(),
                lease_token: accepted.lease_token.unwrap(),
                target: None,
                candidate: Some(candidate),
            }),
        )
        .unwrap();
    assert!(
        matches!(&response[0].event, DraftServerMessage::DraftCommandResult(result) if !result.accepted)
    );
}

#[test]
fn idle_cache_eviction_does_not_split_an_externally_held_draft_authority() {
    let (held, runtime, _) = fixture();
    let original = held.key;
    held.lock().unwrap().empty_since = Some(Instant::now() - ROOM_IDLE_TTL * 2);
    let (other, _, _) = fixture();
    let state = other.lock().unwrap();
    runtime
        .install(
            other.key,
            (*state.workspace).clone(),
            (*state.visibility).clone(),
        )
        .unwrap();
    assert!(
        runtime.room(original).unwrap().is_some(),
        "an admission/connection still holding the old Arc must retain its unique valid authority"
    );
    drop(held);
    let (third, _, _) = fixture();
    let state = third.lock().unwrap();
    runtime
        .install(
            third.key,
            (*state.workspace).clone(),
            (*state.visibility).clone(),
        )
        .unwrap();
    assert!(
        runtime.room(original).unwrap().is_none(),
        "truly unreferenced idle rooms remain evictable"
    );
}

#[test]
fn public_staff_cap_reconciles_only_overflowing_viewer_and_requires_a_fresh_baseline() {
    let (room, _, actor) = fixture();
    // All identities exist privately before publication, including not-yet-referenced staff.
    {
        let mut state = room.lock().unwrap();
        let workspace = Arc::make_mut(&mut state.workspace);
        workspace
            .staff
            .extend((0..1999).map(|_| TimetableBlockWorkspaceStaff {
                id: Uuid::new_v4(),
                display_name: "Additional teacher".into(),
                status: "active".into(),
                subject_groups: Vec::new(),
            }));
    }
    let school = member(actor, full_access());
    let mut assigned = member(
        actor,
        TimetableAccessFilter {
            assigned_actor_id: Some(actor),
            ..Default::default()
        },
    );
    // Manage indices can be broader than the public read scope without public caps.
    assigned.manage_access = Some(full_access());
    room.join(school.clone()).unwrap();
    room.join(assigned.clone()).unwrap();
    let original = room.lock().unwrap().workspace.blocks[1].clone();
    let mut changed = original.clone();
    {
        let state = room.lock().unwrap();
        changed.teachers = state
            .workspace
            .staff
            .iter()
            .filter(|teacher| teacher.id != actor)
            .map(|teacher| TimetableBlockTeacher {
                id: Uuid::new_v4(),
                teacher_id: teacher.id,
                display_name: teacher.display_name.clone(),
                row_version: 1,
                is_active: true,
            })
            .collect();
    }
    assert_eq!(changed.teachers.len(), 2000);
    // Exactly 2000 public identities remains valid, regardless of private directory size.
    let mut boundary = changed.clone();
    boundary.teachers.pop();
    room.publish_blocks(vec![boundary], None, None, Uuid::new_v4())
        .unwrap();
    let school_at_limit = member(actor, full_access());
    let sync = room.join(school_at_limit.clone()).unwrap();
    let DraftServerMessage::DraftStateSync(sync) = sync.event else {
        panic!("sync")
    };
    assert_eq!(sync.workspace.staff.len(), 2000);
    room.disconnect(school_at_limit.connection_id);
    let grant = claim(&room, school.connection_id, source(&room));
    assert!(grant.accepted);
    let token = grant.lease_token.unwrap();
    let seq = room
        .publish_blocks(vec![changed], None, None, Uuid::new_v4())
        .unwrap();
    let fresh_school = member(actor, full_access());
    assert!(matches!(
        room.join(fresh_school.clone()),
        Err(AppError::ValidationError(_))
    ));
    {
        let state = room.lock().unwrap();
        assert!(state.valid);
        assert_eq!(state.workspace.staff.len(), 2001);
        assert!(!state.members.contains_key(&fresh_school.connection_id));
        assert!(!state
            .member_visibility
            .contains_key(&fresh_school.connection_id));
        let school_index = &state.member_visibility[&school.connection_id];
        assert!(school_index.requires_reconcile);
        assert!(school_index.read.staff.is_empty());
        assert!(school_index.manage.is_none());
        assert!(!state.leases.contains_key(&token));
        assert!(!state.active_leases.contains_key(&school.connection_id));
    }
    let events = room
        .signal_events(school.connection_id, DraftSignal::Committed(seq))
        .unwrap();
    assert!(
        matches!(&events[0].event, DraftServerMessage::DraftReconcileRequired {reason} if reason == "workspace_limit")
    );
    let events = room
        .signal_events(assigned.connection_id, DraftSignal::Committed(seq))
        .unwrap();
    assert!(matches!(
        events[0].event,
        DraftServerMessage::DraftPatched(_)
    ));
    assert!(!claim(&room, school.connection_id, source(&room)).accepted);
    assert!(claim(&room, assigned.connection_id, source(&room)).accepted);
    // Shrinking again cannot revive a stale recipient or its cleared reservations.
    let seq = room
        .publish_blocks(vec![original], None, None, Uuid::new_v4())
        .unwrap();
    let events = room
        .signal_events(school.connection_id, DraftSignal::Committed(seq))
        .unwrap();
    assert!(
        matches!(&events[0].event, DraftServerMessage::DraftReconcileRequired {reason} if reason == "workspace_limit")
    );
    let epoch = room.lock().unwrap().epoch;
    let replay = room
        .command(
            school.connection_id,
            DraftClientMessage::ClientReplay(DraftReplay {
                epoch,
                after_seq: 0,
            }),
        )
        .unwrap();
    assert!(
        matches!(&replay[0].event, DraftServerMessage::DraftReconcileRequired {reason} if reason == "workspace_limit")
    );
    room.disconnect(school.connection_id);
    room.join(fresh_school).unwrap();
    assert!(!room.lock().unwrap().leases.contains_key(&token));
}

fn room_group(room: Uuid) -> TimetableBlockGroup {
    TimetableBlockGroup {
        id: Uuid::new_v4(),
        learning_group_id: Uuid::new_v4(),
        learning_offering_id: Uuid::new_v4(),
        code: "ROOM".into(),
        name: "Room assignment".into(),
        homeroom_ids: Vec::new(),
        homeroom_names: Vec::new(),
        room_id: Some(room),
        room_code: None,
        room_name: None,
        instructors: Vec::new(),
        sync_status: None,
        row_version: 1,
        is_active: true,
    }
}

fn edit_rooms(room: &DraftRoom, member: &DraftMember) -> DraftRoomOptions {
    authority::availability(&room.lock().unwrap(), member)
        .unwrap()
        .room_options
        .unwrap()
}

#[test]
fn edit_room_options_use_interval_occupancy_and_keep_own_inactive_room_without_leaking_peer_tokens()
{
    let (room, _, actor) = fixture();
    let ids = (0..5).map(|_| Uuid::new_v4()).collect::<Vec<_>>();
    let (own_block, peer_block, period) = {
        let mut state = room.lock().unwrap();
        let workspace = Arc::make_mut(&mut state.workspace);
        workspace.rooms = ids
            .iter()
            .enumerate()
            .map(|(i, id)| TimetableBlockWorkspaceRoom {
                id: *id,
                code: None,
                name: format!("Room {i}"),
                status: if i == 0 || i == 4 {
                    "INACTIVE"
                } else {
                    "ACTIVE"
                }
                .into(),
            })
            .collect();
        workspace.blocks[0].groups = vec![room_group(ids[0])];
        workspace.blocks[1].groups = vec![room_group(ids[1])];
        // A distinct period still overlaps the source's 08:00–09:00 interval.
        workspace.blocks[1].bell_schedule_period_id = Uuid::new_v4();
        workspace.blocks[1].start_time = "08:30".parse().unwrap();
        workspace.blocks[1].end_time = "09:30".parse().unwrap();
        let mut adjacent = workspace.blocks[1].clone();
        adjacent.id = Uuid::new_v4();
        adjacent.groups = vec![room_group(ids[2])];
        adjacent.start_time = "09:00".parse().unwrap();
        adjacent.end_time = "10:00".parse().unwrap();
        adjacent.bell_schedule_period_id = Uuid::new_v4();
        let result = (
            workspace.blocks[0].clone(),
            adjacent.clone(),
            workspace.bell_periods[0].id,
        );
        workspace.blocks.push(adjacent);
        result
    };
    let owner = member(actor, full_access());
    let peer = member(actor, full_access());
    room.join(owner.clone()).unwrap();
    room.join(peer.clone()).unwrap();
    let owned = claim_with_purpose(
        &room,
        owner.connection_id,
        source(&room),
        DraftReservationPurpose::Edit,
    );
    assert!(owned.accepted);
    let options = edit_rooms(&room, &owner);
    assert_eq!(options.reservation_id, owned.reservation_id.unwrap());
    assert_eq!(
        options.available_room_ids,
        vec![ids[0], ids[2], ids[3]],
        "self is ignored, unchanged inactive retained, overlap excluded, adjacent interval free"
    );
    let peer_source = TimetableBlockPlacementSource::ExistingBlock {
        block_id: peer_block.id,
        row_version: 1,
    };
    let peer_grant = claim_with_purpose(
        &room,
        peer.connection_id,
        peer_source.clone(),
        DraftReservationPurpose::Drag,
    );
    assert!(peer_grant.accepted);
    assert!(authority::availability(&room.lock().unwrap(), &peer)
        .unwrap()
        .room_options
        .is_none());
    let token = peer_grant.lease_token.unwrap();
    let mut candidate = authority::candidate_from_block(&peer_block);
    candidate.room_id = Some(ids[3]);
    let target = DraftTarget {
        day_of_week: own_block.day_of_week.clone(),
        bell_schedule_period_id: period,
        block_id: None,
    };
    let response = room
        .command(
            peer.connection_id,
            DraftClientMessage::ClientTarget(DraftTargetUpdate {
                request_id: Uuid::new_v4(),
                lease_token: token,
                target: Some(target.clone()),
                candidate: Some(candidate.clone()),
            }),
        )
        .unwrap();
    assert!(
        matches!(&response[0].event, DraftServerMessage::DraftCommandResult(result) if result.accepted)
    );
    assert_eq!(
        edit_rooms(&room, &owner).available_room_ids,
        vec![ids[0]],
        "peer target footprint reserves both shifted original and proposed rooms"
    );
    let identity = DraftMutationIdentity {
        connection_id: peer.connection_id,
        lease_token: token,
        mutation_id: Uuid::new_v4(),
    };
    let permit = room
        .begin_commit(
            actor,
            peer.session_id,
            Some(&identity),
            MutationScope::Placement {
                source: peer_source,
                candidate,
                targets: vec![target],
            },
        )
        .unwrap();
    room.disconnect(peer.connection_id);
    assert_eq!(
        edit_rooms(&room, &owner).available_room_ids,
        vec![ids[0]],
        "committing interval remains occupied after disconnect"
    );
    drop(permit);
    assert_eq!(
        edit_rooms(&room, &owner).available_room_ids,
        vec![ids[0], ids[2], ids[3]]
    );
    let mut invalid = authority::candidate_from_block(&own_block);
    invalid.room_id = Some(ids[4]);
    let rejected = room
        .command(
            owner.connection_id,
            DraftClientMessage::ClientTarget(DraftTargetUpdate {
                request_id: Uuid::new_v4(),
                lease_token: owned.lease_token.unwrap(),
                target: None,
                candidate: Some(invalid),
            }),
        )
        .unwrap();
    assert!(
        matches!(&rejected[0].event, DraftServerMessage::DraftCommandResult(result) if !result.accepted)
    );
    assert!(
        matches!(&rejected[1].event, DraftServerMessage::DraftAvailability(availability) if availability.room_options.as_ref().unwrap().available_room_ids == vec![ids[0], ids[2], ids[3]])
    );
    let wire = serde_json::to_string(
        &room
            .signal_events(owner.connection_id, DraftSignal::Reservations)
            .unwrap(),
    )
    .unwrap();
    assert!(!wire.contains(&token.to_string()));
    assert!(!wire.contains("candidate"));
    let mut changed = room.lock().unwrap().workspace.blocks[1].clone();
    changed.groups[0].room_id = Some(ids[3]);
    changed.row_version += 1;
    let seq = room
        .publish_blocks(vec![changed], None, None, Uuid::new_v4())
        .unwrap();
    let committed = room
        .signal_events(owner.connection_id, DraftSignal::Committed(seq))
        .unwrap();
    assert!(committed.iter().any(|event| matches!(&event.event, DraftServerMessage::DraftAvailability(availability) if availability.room_options.as_ref().unwrap().available_room_ids == vec![ids[0], ids[1], ids[2]])), "canonical commit refreshes existing edit room options");
}
