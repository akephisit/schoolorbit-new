//! Draft transport adapts authenticated sockets to the domain's memory authority.
use std::collections::{BTreeMap, BTreeSet};

use school_academic_timetable::collaboration::{DraftMember, DraftRoom, DraftSignal};
use school_academic_timetable::models::collaboration::{DraftClientMessage, DraftEventEnvelope};
use school_auth_http::context::ActorTenantContext;

use super::*;

pub(super) struct PreparedDraft { room: Arc<DraftRoom>, member: DraftMember }

pub(super) async fn prepare(
    state: &AcademicWebSocketState,
    context: &ActorTenantContext,
    session: &AuthenticatedSession,
    term_id: Uuid,
    version_id: Uuid,
) -> Result<PreparedDraft, AppError> {
    let access = school_academic_timetable::services::timetable_realtime_service::authorize_draft_socket(&context.tenant.pool, &context.actor, term_id, version_id).await?;
    let room = state.collaboration.load_room(context.tenant.tenant_id, &context.tenant.pool, version_id, state.staff_directory).await?;
    if room.key.academic_term_id != term_id { return Err(AppError::NotFound("Draft not found in selected term".into())); }
    let existing = room.members()?;
    let user_ids = existing.iter().map(|member| member.actor.user_id).chain([access.user_id]).collect::<BTreeSet<_>>().into_iter().collect::<Vec<_>>();
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(&context.tenant.pool, "academic_http.websockets.draft.portraits").await.map_err(school_errors::AppError::from)?;
    // Profile access is resolved at join; pointer and lease frames never query Staff or Files.
    for member in &existing {
        let portraits = state.staff_directory.collaboration_portraits(&mut connection, &member.actor, &user_ids).await?.into_iter().collect();
        room.set_portraits(member.connection_id, portraits)?;
    }
    let portraits = state.staff_directory.collaboration_portraits(&mut connection, &context.actor, &user_ids).await?.into_iter().collect::<BTreeMap<_, _>>();
    let member = DraftMember { connection_id: Uuid::new_v4(), actor: context.actor.clone(), session_id: session.session_id,
        name: access.display_name, color: generate_color_from_uuid(&access.user_id), access: access.read_access,
        manage_access: access.manage_access, portraits, context: None, anchor: None };
    Ok(PreparedDraft { room, member })
}

async fn send(socket: &mut WebSocket, events: Vec<DraftEventEnvelope>) -> Result<(), ()> {
    for event in events {
        let wire = serde_json::to_string(&event).map_err(|_| ())?;
        socket.send(Message::Text(wire.into())).await.map_err(|_| ())?;
    }
    Ok(())
}

fn queued_authority(
    sessions: &mut broadcast::Receiver<SessionRevocationEvent>,
    permissions: &mut broadcast::Receiver<PermissionChangeEvent>,
    authenticated: &AuthenticatedSession,
) -> Option<SocketCloseReason> {
    match queued_session_decision(sessions, authenticated) {
        SocketSessionDecision::Disconnect => return Some(SocketCloseReason::SessionInvalid),
        SocketSessionDecision::Unavailable => return Some(SocketCloseReason::SessionUnavailable),
        SocketSessionDecision::Continue => {}
    }
    (drain_queued_permission_events(permissions, &authenticated.tenant.subdomain, authenticated.user_id) == SocketPermissionDecision::Disconnect).then_some(SocketCloseReason::PermissionChanged)
}

pub(super) async fn handle(
    mut socket: WebSocket,
    state: AcademicWebSocketState,
    authenticated: AuthenticatedSession,
    prepared: PreparedDraft,
    mut sessions: broadcast::Receiver<SessionRevocationEvent>,
    mut permissions: broadcast::Receiver<PermissionChangeEvent>,
) {
    let PreparedDraft { room, member } = prepared;
    let connection_id = member.connection_id;
    let tenant = authenticated.tenant.subdomain.clone();
    let term_id = room.key.academic_term_id;
    let mut changes = room.subscribe();
    let legacy_sender = state.websocket_manager.get_or_create_room(tenant.clone(), term_id);
    let mut upstream = legacy_sender.subscribe();
    let gate = room.mutation_gate().await;
    if let Some(reason) = queued_authority(&mut sessions, &mut permissions, &authenticated) {
        close_realtime_socket(&mut socket, &authenticated, reason).await;
        return;
    }
    let sync = match room.join(member) { Ok(sync) => sync, Err(_) => return };
    drop(gate);
    if send(&mut socket, vec![sync]).await.is_err() { room.disconnect(connection_id); return; }
    let mut heartbeat = tokio::time::interval_at(tokio::time::Instant::now() + HEARTBEAT_INTERVAL, HEARTBEAT_INTERVAL);
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut last_inbound = Instant::now();
    let mut last_presence = Instant::now() - Duration::from_secs(1);
    loop {
        tokio::select! {
            biased;
            event = sessions.recv() => {
                let reason = match session_event_decision(event, &authenticated) {
                    SocketSessionDecision::Continue => continue,
                    SocketSessionDecision::Disconnect => SocketCloseReason::SessionInvalid,
                    SocketSessionDecision::Unavailable => SocketCloseReason::SessionUnavailable,
                };
                close_realtime_socket(&mut socket, &authenticated, reason).await;
                break;
            }
            event = permissions.recv() => {
                if permission_event_decision(event, &tenant, authenticated.user_id) == SocketPermissionDecision::Disconnect {
                    close_realtime_socket(&mut socket, &authenticated, SocketCloseReason::PermissionChanged).await;
                    break;
                }
            }
            _ = heartbeat.tick() => {
                let reason = match school_auth_http::tenant::revalidate_realtime_session(&state.auth_runtime, &authenticated).await {
                    Ok(true) => None,
                    Ok(false) => Some(SocketCloseReason::SessionInvalid),
                    Err(_) => Some(SocketCloseReason::SessionUnavailable),
                };
                if let Some(reason) = reason { close_realtime_socket(&mut socket, &authenticated, reason).await; break; }
                if heartbeat_timed_out(last_inbound, Instant::now()) || socket.send(Message::Ping(Vec::new().into())).await.is_err() { break; }
            }
            event = upstream.recv() => {
                match event {
                    Ok(SeqEvent { event: TimetableEvent::AcademicCoreChanged { .. } | TimetableEvent::LearningDeliveryChanged { .. }, .. }) => { room.invalidate(); }
                    Err(_) => { room.invalidate(); }
                    _ => {}
                }
            }
            signal = changes.recv() => {
                let invalidated = matches!(signal, Ok(DraftSignal::Invalidated) | Err(_));
                let signal = signal.unwrap_or(DraftSignal::Invalidated);
                match room.signal_events(connection_id, signal) {
                    Ok(events) => { if send(&mut socket, events).await.is_err() { break; } }
                    Err(_) => break,
                }
                if invalidated { break; }
            }
            incoming = socket.next() => match incoming {
                Some(Ok(Message::Text(wire))) => {
                    last_inbound = Instant::now();
                    if text_frame_too_large(wire.len()) { break; }
                    let Ok(command) = serde_json::from_str::<DraftClientMessage>(&wire) else { continue; };
                    if matches!(command, DraftClientMessage::ClientPresenceUpdate(_)) {
                        if last_presence.elapsed() < Duration::from_millis(16) { continue; }
                        last_presence = Instant::now();
                    }
                    // Check queued identity changes before accepting any memory authority action.
                    if let Some(reason) = queued_authority(&mut sessions, &mut permissions, &authenticated) {
                        close_realtime_socket(&mut socket, &authenticated, reason).await; break;
                    }
                    match room.command(connection_id, command) {
                        Ok(events) => { if send(&mut socket, events).await.is_err() { break; } }
                        Err(_) => break,
                    }
                }
                Some(Ok(Message::Ping(payload))) => { last_inbound = Instant::now(); if socket.send(Message::Pong(payload)).await.is_err() { break; } }
                Some(Ok(Message::Pong(_))) => last_inbound = Instant::now(),
                Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                Some(Ok(_)) => {}
            }
        }
    }
    room.disconnect(connection_id);
}
