use crate::error::AppError;
use crate::modules::academic::services::timetable_realtime_service::{
    authorize_socket, TimetableSocketAccess,
};
use crate::modules::auth::{
    audit::{self, SessionFailureReason},
    events::SessionRevocationEvent,
    http::presented_session_token,
    session_crypto::RawSessionToken,
    session_repository::SessionMaintenanceMode,
    session_service::{self, AuthenticatedSession},
};
use crate::modules::notification::events::PermissionChangeEvent;
use crate::utils::request_context::actor_tenant_context_from_session;
use crate::utils::subdomain::parse_realtime_tenant_hint;
use crate::utils::tenant::resolve_auth_tenant_context;
use crate::AppState;
use axum::{
    extract::{
        ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade},
        RawQuery, State,
    },
    http::HeaderMap,
    response::Response,
};
use chrono::Utc;
use dashmap::DashMap;
use futures::stream::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use uuid::Uuid;

/// จำนวน event ที่เก็บใน buffer ต่อ room (สำหรับ replay เมื่อ client reconnect)
const EVENT_BUFFER_SIZE: usize = 200;
const ACADEMIC_CORE_BROADCAST_ROOM_LIMIT: usize = 64;

/// ลบ room ที่ไม่มี subscriber นานเกินเวลานี้
const ROOM_IDLE_TTL: Duration = Duration::from_secs(600); // 10 นาที

/// interval ของ cleanup task
const ROOM_CLEANUP_INTERVAL: Duration = Duration::from_secs(60); // ตรวจทุก 1 นาที

const MAX_TEXT_FRAME_BYTES: usize = 64 * 1024;
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);
const SILENCE_TIMEOUT: Duration = Duration::from_secs(90);

// ==========================================
// Data Structures
// ==========================================

// Context for what the user is looking at
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UserContext {
    pub view_mode: String,
    pub view_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPresence {
    pub user_id: Uuid,
    pub name: String,
    pub color: String,
    pub context: Option<UserContext>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DragInfo {
    pub code: String,
    pub title: String,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum TimetableEvent {
    // System
    StateSync {
        users: Vec<UserPresence>,
        drags: std::collections::HashMap<Uuid, DragState>, // user_id -> drag info
        activities: std::collections::HashMap<Uuid, ActivityState>, // user_id -> dialog activity
        /// current_seq ณ ตอน snapshot — client ใช้เป็นจุดเริ่มต้น tracking seq
        current_seq: u64,
    },

    // Presence
    UserJoined(UserPresence),
    UserLeft {
        user_id: Uuid,
    },

    // Sync Data — legacy fallback (client full-fetch เมื่อได้รับ)
    TableRefresh {
        user_id: Uuid,
    },

    // Patch events (client patch state ตรง ไม่ต้อง fetch DB)
    EntryCreated {
        user_id: Uuid,
        entry: serde_json::Value, // TimetableEntry with joined fields
        /// Phase 2: echo back ของ client_temp_id ที่ส่งมาตอน POST → client correlate temp → real entry
        #[serde(default, skip_serializing_if = "Option::is_none")]
        client_temp_id: Option<String>,
    },
    EntryUpdated {
        user_id: Uuid,
        entry: serde_json::Value, // Full updated entry with joined fields
    },
    EntryDeleted {
        user_id: Uuid,
        entry_id: Uuid,
    },
    EntriesSwapped {
        user_id: Uuid,
        entry_a: serde_json::Value,
        entry_b: serde_json::Value,
    },
    EntryInstructorAdded {
        user_id: Uuid,
        entry_id: Uuid,
        instructor_id: Uuid,
        instructor_name: String,
        role: String,
    },
    EntryInstructorRemoved {
        user_id: Uuid,
        entry_id: Uuid,
        instructor_id: Uuid,
        /// true = entry ถูกลบตามไปด้วย (ครูคนสุดท้าย + regular course)
        entry_deleted: bool,
    },
    /// ทีมครูของ course เปลี่ยน (add/remove/update role) — client re-fetch entries
    /// ของ course นั้นเฉพาะที่เกี่ยวข้อง
    CourseTeamChanged {
        user_id: Uuid,
        course_id: Uuid,
    },
    AcademicCoreChanged {
        user_id: Uuid,
        entity_type: String,
        entity_id: Option<Uuid>,
        academic_year_id: Option<Uuid>,
        academic_term_id: Option<Uuid>,
    },
    LearningDeliveryChanged {
        user_id: Uuid,
        academic_term_id: Uuid,
        learning_offering_id: Uuid,
        learning_group_id: Option<Uuid>,
        revision: i64,
    },

    // Interactions
    CursorMove {
        user_id: Uuid,
        x: f64,
        y: f64,
        context: Option<UserContext>,
    },

    DragStart {
        user_id: Uuid,
        course_id: Option<String>,
        entry_id: Option<String>,
        info: Option<DragInfo>,
    },

    DragEnd {
        user_id: Uuid,
    },

    DragMove {
        user_id: Uuid,
        x: f64,
        y: f64,
        target_day: Option<String>,
        target_period_id: Option<String>,
    },

    // Dialog / activity presence (ephemeral — ไม่ seq)
    UserActivity {
        user_id: Uuid,
        activity_type: String, // "room_picker" | "instructor_edit" | ...
        target: Option<serde_json::Value>, // { entry_id?, day?, period_id? }
    },
    UserActivityEnd {
        user_id: Uuid,
    },

    // === Phase 2: optimistic drop intent + rejection ===
    /// Client → Server: ผู้ใช้ drop เสร็จแล้ว (UI ขยับแล้ว) — relay ให้คนอื่นเห็นทันที
    /// ก่อน DB confirm. Server ไม่ validate, ไม่เขียน DB — แค่ relay
    /// (ephemeral — ไม่ seq เพราะ EntryUpdated/Created/Swapped จะมาตามทีหลังพร้อม seq จริง)
    DropIntent {
        user_id: Uuid,
        kind: String, // "move" | "swap" | "replace"
        entry_id: Uuid,
        day_of_week: String,
        period_id: Uuid,
        room_id: Option<Uuid>,
        /// สำหรับ swap: id ของ entry ที่ถูกสลับด้วย, day/period ของมันก่อน swap
        /// (ตำแหน่งใหม่หลัง swap = ตำแหน่งเดิมของ entry_id)
        swap_partner_id: Option<Uuid>,
        swap_partner_day: Option<String>,
        swap_partner_period_id: Option<Uuid>,
        /// สำหรับ replace: ids ของ course/activity ใหม่ + classroom (ถ้าเปลี่ยนข้ามห้อง)
        /// client receivers lookup local courses[]/activitySlots[] เพื่อ render joined fields
        new_classroom_course_id: Option<Uuid>,
        new_activity_slot_id: Option<Uuid>,
        new_classroom_id: Option<Uuid>,
    },
    /// Server → Clients: DB reject drop ที่ broadcast intent ไปแล้ว → ทุกคน rollback
    /// ตำแหน่งเดิม. Toast แสดงเฉพาะคนที่ drop (`user_id` ตรงกับตัวเอง)
    DropRejected {
        user_id: Uuid, // คนที่ drop (ใช้ filter toast)
        entry_id: Uuid,
        original_day: String,
        original_period_id: Uuid,
        original_room_id: Option<Uuid>,
        /// optional: ถ้า swap → entry คู่สลับที่ต้อง rollback เช่นกัน
        partner_id: Option<Uuid>,
        partner_original_day: Option<String>,
        partner_original_period_id: Option<Uuid>,
        reason: String,
    },

    /// Client → Server: ผู้ใช้ drop NEW (สร้าง entry) — relay ให้คนอื่นเห็น tempEntry ทันที
    /// (ก่อน DB confirm). Lookup joined fields จาก local state ของ client เอง
    EntryIntent {
        user_id: Uuid,
        temp_id: String, // UUID ที่ client gen ขึ้นเอง
        classroom_id: Uuid,
        classroom_course_id: Option<Uuid>,
        activity_slot_id: Option<Uuid>,
        day_of_week: String,
        period_id: Uuid,
        room_id: Option<Uuid>,
        title: Option<String>, // สำหรับ ACTIVITY
        entry_type: String,    // "COURSE" | "ACTIVITY"
    },
    /// Server → Clients: CREATE ถูก reject → ทุก client ลบ tempEntry ที่มี temp_id นี้
    EntryRejected {
        user_id: Uuid, // คนที่สร้าง (ใช้ filter toast)
        temp_id: String,
        reason: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityState {
    pub activity_type: String,
    pub target: Option<serde_json::Value>,
}

impl TimetableEvent {
    /// Event ประเภท mutation (ต้อง seq + buffer). คืน true ถ้าต้อง track
    pub fn is_mutation(&self) -> bool {
        matches!(
            self,
            TimetableEvent::TableRefresh { .. }
                | TimetableEvent::EntryCreated { .. }
                | TimetableEvent::EntryUpdated { .. }
                | TimetableEvent::EntryDeleted { .. }
                | TimetableEvent::EntriesSwapped { .. }
                | TimetableEvent::EntryInstructorAdded { .. }
                | TimetableEvent::EntryInstructorRemoved { .. }
                | TimetableEvent::CourseTeamChanged { .. }
                | TimetableEvent::AcademicCoreChanged { .. }
                | TimetableEvent::LearningDeliveryChanged { .. }
        )
    }
}

/// Envelope for broadcast: seq สำหรับ mutation events, None สำหรับ ephemeral/presence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeqEvent {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seq: Option<u64>,
    #[serde(flatten)]
    pub event: TimetableEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DragState {
    pub course_id: Option<String>,
    pub entry_id: Option<String>,
    pub info: Option<DragInfo>,
}

#[derive(Debug, Deserialize)]
pub struct WsParams {
    pub semester_id: Uuid,
    #[serde(default)]
    pub school_subdomain: Option<String>,
}

fn parse_ws_params(
    raw_query: Option<&str>,
    school_subdomain: Option<String>,
) -> Result<WsParams, AppError> {
    let raw_query =
        raw_query.ok_or_else(|| AppError::BadRequest("Invalid WebSocket query".to_string()))?;
    let mut seen = HashSet::new();
    let mut semester_id = None;

    for (key, value) in url::form_urlencoded::parse(raw_query.as_bytes()) {
        if !seen.insert(key.to_string()) {
            return Err(AppError::BadRequest("Invalid WebSocket query".to_string()));
        }
        if key == "semester_id" {
            semester_id = Some(
                value
                    .parse::<Uuid>()
                    .map_err(|_| AppError::BadRequest("Invalid WebSocket query".to_string()))?,
            );
        }
    }

    Ok(WsParams {
        semester_id: semester_id
            .ok_or_else(|| AppError::BadRequest("Invalid WebSocket query".to_string()))?,
        school_subdomain,
    })
}

// ==========================================
// State Manager
// ==========================================

pub struct WebSocketManager {
    // Room Key -> Broadcast Sender (ส่ง SeqEvent ไปทุก subscriber)
    rooms: DashMap<String, broadcast::Sender<SeqEvent>>,
    // Room Key -> (User ID -> (Presence, tab/connection count))
    // count > 0 = user มีอย่างน้อย 1 tab เปิดอยู่
    room_users: DashMap<String, DashMap<Uuid, (UserPresence, usize)>>,
    // Room Key -> (User ID -> Drag State)
    room_drags: DashMap<String, DashMap<Uuid, DragState>>,
    // Room Key -> (User ID -> Activity State) — ผู้ใช้เปิด dialog อยู่ที่ไหน
    room_activities: DashMap<String, DashMap<Uuid, ActivityState>>,
    // Room Key -> next seq counter (monotonic)
    room_seq: DashMap<String, Arc<AtomicU64>>,
    // Room Key -> ring buffer ของ mutation events (ล่าสุด EVENT_BUFFER_SIZE อัน)
    room_buffer: DashMap<String, Arc<Mutex<VecDeque<SeqEvent>>>>,
    // Room Key -> Instant ที่ว่าง (count=0) ครั้งล่าสุด; None = ยังมี subscriber
    room_empty_since: DashMap<String, Instant>,
}

impl WebSocketManager {
    pub fn new() -> Self {
        Self {
            rooms: DashMap::new(),
            room_users: DashMap::new(),
            room_drags: DashMap::new(),
            room_activities: DashMap::new(),
            room_seq: DashMap::new(),
            room_buffer: DashMap::new(),
            room_empty_since: DashMap::new(),
        }
    }

    /// Spawn background cleanup task — ลบ room ที่ idle > ROOM_IDLE_TTL
    /// เรียกครั้งเดียวตอน startup (ใน main.rs)
    pub fn spawn_cleanup_task(self: Arc<Self>) {
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(ROOM_CLEANUP_INTERVAL).await;
                let now = Instant::now();
                let mut candidates: Vec<String> = Vec::new();
                for entry in self.room_empty_since.iter() {
                    if now.duration_since(*entry.value()) > ROOM_IDLE_TTL {
                        candidates.push(entry.key().clone());
                    }
                }
                for key in candidates {
                    // ใช้ DashMap remove_if atomic — remove เฉพาะถ้า count==0
                    // (ลด race window: ระหว่าง check กับ remove มี entry lock)
                    let removed = self.rooms.remove_if(&key, |_, tx| tx.receiver_count() == 0);
                    if removed.is_some() {
                        self.room_users.remove(&key);
                        self.room_drags.remove(&key);
                        self.room_activities.remove(&key);
                        self.room_seq.remove(&key);
                        self.room_buffer.remove(&key);
                        self.room_empty_since.remove(&key);
                        tracing::error!("[WS cleanup] dropped idle room: {}", key);
                    } else {
                        // มีคน subscribe ระหว่างนั้น → clear empty_since, เก็บ room ไว้
                        self.room_empty_since.remove(&key);
                    }
                }
            }
        });
    }

    fn get_room_key(school_key: String, semester_id: Uuid) -> String {
        format!("{}:{}", school_key, semester_id)
    }

    pub fn get_or_create_room(
        &self,
        school_key: String,
        semester_id: Uuid,
    ) -> broadcast::Sender<SeqEvent> {
        let key = Self::get_room_key(school_key, semester_id);

        if let Some(sender) = self.rooms.get(&key) {
            return sender.clone();
        }

        let (tx, _rx) = broadcast::channel(100);
        self.rooms.insert(key.clone(), tx.clone());
        self.room_users.entry(key.clone()).or_default();
        self.room_drags.entry(key.clone()).or_default();
        self.room_activities.entry(key.clone()).or_default();
        self.room_seq
            .entry(key.clone())
            .or_insert_with(|| Arc::new(AtomicU64::new(0)));
        self.room_buffer
            .entry(key)
            .or_insert_with(|| Arc::new(Mutex::new(VecDeque::with_capacity(EVENT_BUFFER_SIZE))));

        tx
    }

    /// Broadcast ephemeral event (presence, cursor, drag) — ไม่มี seq ไม่ buffer
    pub fn broadcast_ephemeral(
        &self,
        school_key: String,
        semester_id: Uuid,
        event: TimetableEvent,
    ) {
        let tx = self.get_or_create_room(school_key, semester_id);
        let _ = tx.send(SeqEvent { seq: None, event });
    }

    /// Broadcast mutation event — assign seq, push buffer, send.
    /// Skip ทั้งหมดถ้า receiver_count <= 1 (มีแค่ caller เอง หรือไม่มีใคร) — ประหยัด
    /// seq ไม่เพิ่ม, buffer ไม่โต, send ไม่เกิด. Return 0 เมื่อ skip
    pub fn broadcast_mutation(
        &self,
        school_key: String,
        semester_id: Uuid,
        event: TimetableEvent,
    ) -> u64 {
        // Gate: ไม่มี "คนอื่น" ฟัง → skip ทั้ง pipeline
        if !self.has_other_subscribers(school_key.clone(), semester_id) {
            return 0;
        }
        let key = Self::get_room_key(school_key.clone(), semester_id);
        // ensure room exists
        let tx = self.get_or_create_room(school_key, semester_id);

        let seq_counter = self.room_seq.get(&key).map(|v| v.clone());
        let buffer = self.room_buffer.get(&key).map(|v| v.clone());
        let seq = match seq_counter {
            Some(c) => c.fetch_add(1, Ordering::SeqCst) + 1,
            None => 0,
        };

        let seq_event = SeqEvent {
            seq: Some(seq),
            event,
        };

        if let Some(buf) = buffer {
            if let Ok(mut guard) = buf.lock() {
                if guard.len() >= EVENT_BUFFER_SIZE {
                    guard.pop_front();
                }
                guard.push_back(seq_event.clone());
            }
        }

        let _ = tx.send(seq_event);
        seq
    }

    pub fn broadcast_academic_core_changed(
        &self,
        school_key: String,
        user_id: Uuid,
        entity_type: &str,
        entity_id: Option<Uuid>,
        academic_year_id: Option<Uuid>,
        academic_term_id: Option<Uuid>,
    ) {
        let term_ids = if let Some(term_id) = academic_term_id {
            let room_key = Self::get_room_key(school_key.clone(), term_id);
            self.rooms
                .contains_key(&room_key)
                .then_some(term_id)
                .into_iter()
                .collect::<Vec<_>>()
        } else {
            let prefix = format!("{school_key}:");
            self.rooms
                .iter()
                .filter_map(|room| {
                    room.key()
                        .strip_prefix(&prefix)
                        .and_then(|value| Uuid::parse_str(value).ok())
                })
                .take(ACADEMIC_CORE_BROADCAST_ROOM_LIMIT)
                .collect::<Vec<_>>()
        };
        for term_id in term_ids {
            self.broadcast_mutation(
                school_key.clone(),
                term_id,
                TimetableEvent::AcademicCoreChanged {
                    user_id,
                    entity_type: entity_type.to_string(),
                    entity_id,
                    academic_year_id,
                    academic_term_id,
                },
            );
        }
    }

    pub fn broadcast_learning_delivery_changed(
        &self,
        school_key: String,
        user_id: Uuid,
        academic_term_id: Uuid,
        learning_offering_id: Uuid,
        learning_group_id: Option<Uuid>,
        revision: i64,
    ) {
        self.broadcast_mutation(
            school_key,
            academic_term_id,
            TimetableEvent::LearningDeliveryChanged {
                user_id,
                academic_term_id,
                learning_offering_id,
                learning_group_id,
                revision,
            },
        );
    }

    pub fn current_seq(&self, school_key: String, semester_id: Uuid) -> u64 {
        let key = Self::get_room_key(school_key, semester_id);
        self.room_seq
            .get(&key)
            .map(|c| c.load(Ordering::SeqCst))
            .unwrap_or(0)
    }

    /// True ถ้ามี subscriber อย่างน้อย 1 คนใน room (ใครก็ได้)
    pub fn has_subscribers(&self, school_key: String, semester_id: Uuid) -> bool {
        let key = Self::get_room_key(school_key, semester_id);
        self.rooms
            .get(&key)
            .map(|tx| tx.receiver_count() > 0)
            .unwrap_or(false)
    }

    /// True ถ้ามี subscriber **นอกจากตัว caller** (อย่างน้อย 2 คน)
    /// ใช้ skip joined re-fetch เมื่อ mutation มาจากคนเดียวที่อยู่ใน room
    /// (echo กลับให้ตัวเองไม่คุ้ม — client จะ loadTimetable ต่ออยู่แล้ว)
    pub fn has_other_subscribers(&self, school_key: String, semester_id: Uuid) -> bool {
        let key = Self::get_room_key(school_key, semester_id);
        self.rooms
            .get(&key)
            .map(|tx| tx.receiver_count() > 1)
            .unwrap_or(false)
    }

    /// Return events with seq > after_seq, ordered. If buffer doesn't reach back that far,
    /// return None (signal caller: client ต้อง full-fetch)
    pub fn replay(
        &self,
        school_key: String,
        semester_id: Uuid,
        after_seq: u64,
    ) -> Option<Vec<SeqEvent>> {
        let key = Self::get_room_key(school_key, semester_id);
        let buffer = self.room_buffer.get(&key)?.clone();
        let guard = buffer.lock().ok()?;

        // Check ถ้า after_seq น้อยกว่า seq ต่ำสุดใน buffer → ต้อง refetch
        if let Some(first) = guard.front() {
            if let Some(first_seq) = first.seq {
                if after_seq + 1 < first_seq {
                    return None; // buffer ไม่ถึง — ต้อง full-fetch
                }
            }
        }

        let events: Vec<SeqEvent> = guard
            .iter()
            .filter(|e| e.seq.map(|s| s > after_seq).unwrap_or(false))
            .cloned()
            .collect();
        Some(events)
    }

    /// Join room — เพิ่ม count ของ user_id นั้น. Return true ถ้าเป็น "first tab" ของ user
    /// (caller ใช้ตัดสินใจว่าจะ broadcast UserJoined หรือไม่)
    pub fn join_room(&self, school_key: String, semester_id: Uuid, user: UserPresence) -> bool {
        let key = Self::get_room_key(school_key, semester_id);
        let mut is_first = false;
        if let Some(users) = self.room_users.get(&key) {
            users
                .entry(user.user_id)
                .and_modify(|(presence, count)| {
                    *presence = user.clone(); // refresh presence (ชื่อ/สี อัปเดต)
                    *count += 1;
                })
                .or_insert_with(|| {
                    is_first = true;
                    (user, 1)
                });
        }
        // มี subscriber เข้ามา — room ไม่ว่างอีกต่อไป
        self.room_empty_since.remove(&key);
        is_first
    }

    /// Leave room — ลด count. Return true ถ้าเป็น "last tab" ของ user
    /// (caller ใช้ตัดสินใจว่าจะ broadcast UserLeft หรือไม่)
    pub fn leave_room(&self, school_key: String, semester_id: Uuid, user_id: Uuid) -> bool {
        let key = Self::get_room_key(school_key, semester_id);
        let mut is_last = false;
        if let Some(users) = self.room_users.get(&key) {
            let mut should_remove = false;
            if let Some(mut entry) = users.get_mut(&user_id) {
                let (_, count) = entry.value_mut();
                if *count <= 1 {
                    should_remove = true;
                    is_last = true;
                } else {
                    *count -= 1;
                }
            }
            if should_remove {
                users.remove(&user_id);
            }
        }
        // Drag + Activity state ล้างเมื่อ tab สุดท้ายออกเท่านั้น
        if is_last {
            if let Some(drags) = self.room_drags.get(&key) {
                drags.remove(&user_id);
            }
            if let Some(activities) = self.room_activities.get(&key) {
                activities.remove(&user_id);
            }
        }
        // ถ้าไม่มี subscriber เหลือ → mark เวลาเริ่มว่าง (cleanup task จะลบในภายหลัง)
        if let Some(tx) = self.rooms.get(&key) {
            if tx.receiver_count() == 0 {
                self.room_empty_since.insert(key, Instant::now());
            }
        }
        is_last
    }

    pub fn update_drag(
        &self,
        school_key: String,
        semester_id: Uuid,
        user_id: Uuid,
        drag: Option<DragState>,
    ) {
        let key = Self::get_room_key(school_key, semester_id);
        if let Some(drags) = self.room_drags.get(&key) {
            if let Some(d) = drag {
                drags.insert(user_id, d);
            } else {
                drags.remove(&user_id);
            }
        }
    }

    pub fn update_activity(
        &self,
        school_key: String,
        semester_id: Uuid,
        user_id: Uuid,
        activity: Option<ActivityState>,
    ) {
        let key = Self::get_room_key(school_key, semester_id);
        if let Some(activities) = self.room_activities.get(&key) {
            match activity {
                Some(a) => {
                    activities.insert(user_id, a);
                }
                None => {
                    activities.remove(&user_id);
                }
            }
        }
    }

    pub fn update_context(
        &self,
        school_key: String,
        semester_id: Uuid,
        user_id: Uuid,
        context: Option<UserContext>,
    ) {
        let key = Self::get_room_key(school_key, semester_id);
        if let Some(users) = self.room_users.get(&key) {
            if let Some(mut entry) = users.get_mut(&user_id) {
                entry.value_mut().0.context = context;
            }
        }
    }

    pub fn get_state_snapshot(
        &self,
        school_key: String,
        semester_id: Uuid,
    ) -> (
        Vec<UserPresence>,
        std::collections::HashMap<Uuid, DragState>,
        std::collections::HashMap<Uuid, ActivityState>,
    ) {
        let key = Self::get_room_key(school_key, semester_id);

        let users = self
            .room_users
            .get(&key)
            .map(|m| m.iter().map(|kv| kv.value().0.clone()).collect())
            .unwrap_or_default();

        let drags = self
            .room_drags
            .get(&key)
            .map(|m| m.iter().map(|kv| (*kv.key(), kv.value().clone())).collect())
            .unwrap_or_default();

        let activities = self
            .room_activities
            .get(&key)
            .map(|m| m.iter().map(|kv| (*kv.key(), kv.value().clone())).collect())
            .unwrap_or_default();

        (users, drags, activities)
    }
}

// ==========================================
// Handler
// ==========================================

fn text_frame_too_large(bytes: usize) -> bool {
    bytes > MAX_TEXT_FRAME_BYTES
}

fn heartbeat_timed_out(last_inbound: Instant, now: Instant) -> bool {
    now.duration_since(last_inbound) >= SILENCE_TIMEOUT
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum SocketSessionDecision {
    Continue,
    Disconnect,
    Unavailable,
}

fn session_event_decision(
    event: Result<SessionRevocationEvent, broadcast::error::RecvError>,
    session: &AuthenticatedSession,
) -> SocketSessionDecision {
    match event {
        Ok(event)
            if event.applies_to(
                &session.tenant.subdomain,
                session.user_id,
                session.session_id,
            ) =>
        {
            SocketSessionDecision::Disconnect
        }
        Ok(_) => SocketSessionDecision::Continue,
        Err(broadcast::error::RecvError::Lagged(missed_events)) => {
            tracing::warn!(
                missed_events,
                "Timetable WebSocket session receiver lagged; closing session"
            );
            SocketSessionDecision::Unavailable
        }
        Err(broadcast::error::RecvError::Closed) => {
            tracing::warn!("Timetable WebSocket session channel closed; closing session");
            SocketSessionDecision::Unavailable
        }
    }
}

fn queued_session_decision(
    receiver: &mut broadcast::Receiver<SessionRevocationEvent>,
    session: &AuthenticatedSession,
) -> SocketSessionDecision {
    loop {
        match receiver.try_recv() {
            Ok(event)
                if event.applies_to(
                    &session.tenant.subdomain,
                    session.user_id,
                    session.session_id,
                ) =>
            {
                return SocketSessionDecision::Disconnect;
            }
            Ok(_) => continue,
            Err(broadcast::error::TryRecvError::Empty) => {
                return SocketSessionDecision::Continue;
            }
            Err(broadcast::error::TryRecvError::Lagged(missed_events)) => {
                tracing::warn!(
                    missed_events,
                    "Timetable WebSocket queued session receiver lagged; closing session"
                );
                return SocketSessionDecision::Unavailable;
            }
            Err(broadcast::error::TryRecvError::Closed) => {
                tracing::warn!(
                    "Timetable WebSocket queued session channel closed; closing session"
                );
                return SocketSessionDecision::Unavailable;
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum SocketPermissionDecision {
    Continue,
    Disconnect,
}

fn permission_event_decision(
    event: Result<PermissionChangeEvent, broadcast::error::RecvError>,
    tenant: &str,
    user_id: Uuid,
) -> SocketPermissionDecision {
    match event {
        Ok(event) if event.applies_to(tenant, user_id) => SocketPermissionDecision::Disconnect,
        Ok(_) => SocketPermissionDecision::Continue,
        Err(broadcast::error::RecvError::Lagged(missed_events)) => {
            tracing::warn!(
                missed_events,
                "Timetable WebSocket permission receiver lagged; closing session"
            );
            SocketPermissionDecision::Disconnect
        }
        Err(broadcast::error::RecvError::Closed) => {
            tracing::warn!("Timetable WebSocket permission channel closed; closing session");
            SocketPermissionDecision::Disconnect
        }
    }
}

fn drain_queued_permission_events(
    receiver: &mut broadcast::Receiver<PermissionChangeEvent>,
    tenant: &str,
    user_id: Uuid,
) -> SocketPermissionDecision {
    loop {
        match receiver.try_recv() {
            Ok(event) if event.applies_to(tenant, user_id) => {
                return SocketPermissionDecision::Disconnect;
            }
            Ok(_) => continue,
            Err(broadcast::error::TryRecvError::Empty) => {
                return SocketPermissionDecision::Continue;
            }
            Err(broadcast::error::TryRecvError::Lagged(missed_events)) => {
                tracing::warn!(
                    missed_events,
                    "Timetable WebSocket queued permission receiver lagged; closing session"
                );
                return SocketPermissionDecision::Disconnect;
            }
            Err(broadcast::error::TryRecvError::Closed) => {
                tracing::warn!(
                    "Timetable WebSocket queued permission channel closed; closing session"
                );
                return SocketPermissionDecision::Disconnect;
            }
        }
    }
}

fn initialize_socket_if_permissions_current<T>(
    receiver: &mut broadcast::Receiver<PermissionChangeEvent>,
    tenant: &str,
    user_id: Uuid,
    initialize: impl FnOnce() -> T,
) -> Option<T> {
    if drain_queued_permission_events(receiver, tenant, user_id)
        == SocketPermissionDecision::Disconnect
    {
        return None;
    }

    Some(initialize())
}

fn sanitize_client_event(
    event: TimetableEvent,
    authenticated_user_id: Uuid,
    can_manage: bool,
) -> Option<TimetableEvent> {
    match event {
        TimetableEvent::CursorMove { x, y, context, .. } => Some(TimetableEvent::CursorMove {
            user_id: authenticated_user_id,
            x,
            y,
            context,
        }),
        TimetableEvent::DragStart {
            course_id,
            entry_id,
            info,
            ..
        } if can_manage => Some(TimetableEvent::DragStart {
            user_id: authenticated_user_id,
            course_id,
            entry_id,
            info,
        }),
        TimetableEvent::DragEnd { .. } if can_manage => Some(TimetableEvent::DragEnd {
            user_id: authenticated_user_id,
        }),
        TimetableEvent::DragMove {
            x,
            y,
            target_day,
            target_period_id,
            ..
        } if can_manage => Some(TimetableEvent::DragMove {
            user_id: authenticated_user_id,
            x,
            y,
            target_day,
            target_period_id,
        }),
        TimetableEvent::UserActivity {
            activity_type,
            target,
            ..
        } if can_manage => Some(TimetableEvent::UserActivity {
            user_id: authenticated_user_id,
            activity_type,
            target,
        }),
        TimetableEvent::UserActivityEnd { .. } if can_manage => {
            Some(TimetableEvent::UserActivityEnd {
                user_id: authenticated_user_id,
            })
        }
        TimetableEvent::TableRefresh { .. } if can_manage => Some(TimetableEvent::TableRefresh {
            user_id: authenticated_user_id,
        }),
        TimetableEvent::DropIntent {
            kind,
            entry_id,
            day_of_week,
            period_id,
            room_id,
            swap_partner_id,
            swap_partner_day,
            swap_partner_period_id,
            new_classroom_course_id,
            new_activity_slot_id,
            new_classroom_id,
            ..
        } if can_manage => Some(TimetableEvent::DropIntent {
            user_id: authenticated_user_id,
            kind,
            entry_id,
            day_of_week,
            period_id,
            room_id,
            swap_partner_id,
            swap_partner_day,
            swap_partner_period_id,
            new_classroom_course_id,
            new_activity_slot_id,
            new_classroom_id,
        }),
        TimetableEvent::EntryIntent {
            temp_id,
            classroom_id,
            classroom_course_id,
            activity_slot_id,
            day_of_week,
            period_id,
            room_id,
            title,
            entry_type,
            ..
        } if can_manage => Some(TimetableEvent::EntryIntent {
            user_id: authenticated_user_id,
            temp_id,
            classroom_id,
            classroom_course_id,
            activity_slot_id,
            day_of_week,
            period_id,
            room_id,
            title,
            entry_type,
        }),
        _ => None,
    }
}

fn relay_client_event(
    manager: &WebSocketManager,
    tx: &broadcast::Sender<SeqEvent>,
    tenant: &str,
    semester_id: Uuid,
    user_presence: &mut UserPresence,
    event: TimetableEvent,
) {
    let user_id = user_presence.user_id;
    match &event {
        TimetableEvent::CursorMove { context, .. } => {
            if user_presence.context != *context {
                manager.update_context(tenant.to_string(), semester_id, user_id, context.clone());
                user_presence.context = context.clone();
            }
        }
        TimetableEvent::DragStart {
            course_id,
            entry_id,
            info,
            ..
        } => manager.update_drag(
            tenant.to_string(),
            semester_id,
            user_id,
            Some(DragState {
                course_id: course_id.clone(),
                entry_id: entry_id.clone(),
                info: info.clone(),
            }),
        ),
        TimetableEvent::DragEnd { .. } => {
            manager.update_drag(tenant.to_string(), semester_id, user_id, None);
        }
        TimetableEvent::UserActivity {
            activity_type,
            target,
            ..
        } => manager.update_activity(
            tenant.to_string(),
            semester_id,
            user_id,
            Some(ActivityState {
                activity_type: activity_type.clone(),
                target: target.clone(),
            }),
        ),
        TimetableEvent::UserActivityEnd { .. } => {
            manager.update_activity(tenant.to_string(), semester_id, user_id, None);
        }
        _ => {}
    }

    if event.is_mutation() {
        manager.broadcast_mutation(tenant.to_string(), semester_id, event);
    } else if tx.send(SeqEvent { seq: None, event }).is_err() {
        tracing::debug!("Timetable WebSocket room has no event receivers");
    }
}

async fn send_broadcast_event(
    socket: &mut WebSocket,
    broadcast: Result<SeqEvent, broadcast::error::RecvError>,
) -> Result<(), ()> {
    let event = match broadcast {
        Ok(event) => event,
        Err(broadcast::error::RecvError::Lagged(missed_events)) => {
            tracing::warn!(
                missed_events,
                "Timetable WebSocket client lagged; forcing full refresh"
            );
            SeqEvent {
                seq: None,
                event: TimetableEvent::TableRefresh {
                    user_id: Uuid::nil(),
                },
            }
        }
        Err(broadcast::error::RecvError::Closed) => return Err(()),
    };

    let json = serde_json::to_string(&event).map_err(|_| {
        tracing::warn!("Failed to serialize timetable WebSocket event");
    })?;
    socket
        .send(Message::Text(json.into()))
        .await
        .map_err(|_| ())
}

#[derive(Clone, Copy)]
enum SocketCloseReason {
    PermissionChanged,
    SessionInvalid,
    SessionUnavailable,
}

impl SocketCloseReason {
    fn message(self) -> &'static str {
        match self {
            Self::PermissionChanged => "Permission changed",
            Self::SessionInvalid | Self::SessionUnavailable => "Authentication required",
        }
    }

    fn audit_reason(self) -> SessionFailureReason {
        match self {
            Self::PermissionChanged => SessionFailureReason::RealtimePermissionChanged,
            Self::SessionInvalid => SessionFailureReason::RealtimeSessionInvalid,
            Self::SessionUnavailable => SessionFailureReason::RealtimeSessionUnavailable,
        }
    }

    fn send_failure_code(self) -> &'static str {
        match self {
            Self::PermissionChanged => "permission_close_send_failed",
            Self::SessionInvalid | Self::SessionUnavailable => "session_close_send_failed",
        }
    }
}

async fn close_realtime_socket(
    socket: &mut WebSocket,
    session: &AuthenticatedSession,
    reason: SocketCloseReason,
) {
    audit::session_realtime_disconnect(
        session.tenant.tenant_id,
        session.user_id,
        session.session_id,
        reason.audit_reason(),
    );
    if socket
        .send(Message::Close(Some(CloseFrame {
            code: 1008,
            reason: reason.message().into(),
        })))
        .await
        .is_err()
    {
        tracing::debug!(reason = reason.send_failure_code());
    }
}

pub async fn timetable_websocket_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    RawQuery(raw_query): RawQuery,
    ws: WebSocketUpgrade,
) -> Result<Response, AppError> {
    let parsed_dev_hint = parse_realtime_tenant_hint(raw_query.as_deref())?;
    let params = parse_ws_params(raw_query.as_deref(), parsed_dev_hint)?;
    let session_event_receiver = state.auth_runtime.session_events.subscribe();
    let permission_event_receiver = state.permission_event_channel.subscribe();
    let tenant = resolve_auth_tenant_context(
        &state.auth_runtime,
        &headers,
        params.school_subdomain.as_deref(),
    )
    .await?;
    let token = presented_session_token(&headers)?
        .ok_or_else(|| AppError::AuthError("กรุณาเข้าสู่ระบบ".to_string()))?;
    let session_context = state.auth_runtime.service_context(tenant);
    let authenticated = session_service::authenticate(
        &session_context,
        token.token_hash(),
        Utc::now(),
        SessionMaintenanceMode::TouchOnly,
        RawSessionToken::generate,
    )
    .await?
    .map(|authentication| authentication.authenticated)
    .ok_or_else(|| AppError::AuthError("กรุณาเข้าสู่ระบบ".to_string()))?;
    let context = actor_tenant_context_from_session(&state, &authenticated).await?;
    let access = authorize_socket(&context.tenant.pool, &context.actor, params.semester_id).await?;
    if !session_service::revalidate(&authenticated, Utc::now()).await? {
        return Err(AppError::AuthError("กรุณาเข้าสู่ระบบ".to_string()));
    }

    Ok(ws.on_upgrade(move |socket| {
        handle_socket(
            socket,
            state,
            params.semester_id,
            authenticated,
            access,
            session_event_receiver,
            permission_event_receiver,
        )
    }))
}

async fn handle_socket(
    mut socket: WebSocket,
    state: AppState,
    semester_id: Uuid,
    authenticated: AuthenticatedSession,
    access: TimetableSocketAccess,
    mut session_event_receiver: broadcast::Receiver<SessionRevocationEvent>,
    mut permission_event_receiver: broadcast::Receiver<PermissionChangeEvent>,
) {
    let tenant = authenticated.tenant.subdomain.clone();
    let TimetableSocketAccess {
        user_id,
        display_name,
        can_manage,
    } = access;
    let mut user_presence = UserPresence {
        user_id,
        name: display_name,
        color: generate_color_from_uuid(&user_id),
        context: None,
    };

    match queued_session_decision(&mut session_event_receiver, &authenticated) {
        SocketSessionDecision::Continue => {}
        SocketSessionDecision::Disconnect => {
            close_realtime_socket(
                &mut socket,
                &authenticated,
                SocketCloseReason::SessionInvalid,
            )
            .await;
            return;
        }
        SocketSessionDecision::Unavailable => {
            close_realtime_socket(
                &mut socket,
                &authenticated,
                SocketCloseReason::SessionUnavailable,
            )
            .await;
            return;
        }
    }

    let initialization = initialize_socket_if_permissions_current(
        &mut permission_event_receiver,
        &tenant,
        user_id,
        || {
            let tx = state
                .websocket_manager
                .get_or_create_room(tenant.clone(), semester_id);
            let rx = tx.subscribe();
            let is_first_tab = state.websocket_manager.join_room(
                tenant.clone(),
                semester_id,
                user_presence.clone(),
            );

            let (users, drags, activities) = state
                .websocket_manager
                .get_state_snapshot(tenant.clone(), semester_id);
            let current_seq = state
                .websocket_manager
                .current_seq(tenant.clone(), semester_id);
            let sync_event = SeqEvent {
                seq: None,
                event: TimetableEvent::StateSync {
                    users,
                    drags,
                    activities,
                    current_seq,
                },
            };

            (tx, rx, is_first_tab, sync_event)
        },
    );

    let Some((tx, mut rx, is_first_tab, sync_event)) = initialization else {
        close_realtime_socket(
            &mut socket,
            &authenticated,
            SocketCloseReason::PermissionChanged,
        )
        .await;
        return;
    };

    let socket_ready = match serde_json::to_string(&sync_event) {
        Ok(json) => socket.send(Message::Text(json.into())).await.is_ok(),
        Err(_) => {
            tracing::warn!("Failed to serialize timetable WebSocket state sync");
            false
        }
    };

    if is_first_tab
        && tx
            .send(SeqEvent {
                seq: None,
                event: TimetableEvent::UserJoined(user_presence.clone()),
            })
            .is_err()
    {
        tracing::debug!("Timetable WebSocket room has no presence receivers");
    }

    if socket_ready {
        let mut heartbeat = tokio::time::interval_at(
            tokio::time::Instant::now() + HEARTBEAT_INTERVAL,
            HEARTBEAT_INTERVAL,
        );
        heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let mut last_inbound = Instant::now();

        loop {
            tokio::select! {
                biased;
                session_change = session_event_receiver.recv() => {
                    match session_event_decision(session_change, &authenticated) {
                        SocketSessionDecision::Continue => {}
                        SocketSessionDecision::Disconnect => {
                            close_realtime_socket(
                                &mut socket,
                                &authenticated,
                                SocketCloseReason::SessionInvalid,
                            )
                            .await;
                            break;
                        }
                        SocketSessionDecision::Unavailable => {
                            close_realtime_socket(
                                &mut socket,
                                &authenticated,
                                SocketCloseReason::SessionUnavailable,
                            )
                            .await;
                            break;
                        }
                    }
                },
                permission_change = permission_event_receiver.recv() => {
                    if permission_event_decision(permission_change, &tenant, user_id)
                        == SocketPermissionDecision::Disconnect
                    {
                        close_realtime_socket(
                            &mut socket,
                            &authenticated,
                            SocketCloseReason::PermissionChanged,
                        )
                        .await;
                        break;
                    }
                },
                _ = heartbeat.tick() => {
                    match session_service::revalidate(&authenticated, Utc::now()).await {
                        Ok(true) => {}
                        Ok(false) => {
                            close_realtime_socket(
                                &mut socket,
                                &authenticated,
                                SocketCloseReason::SessionInvalid,
                            )
                            .await;
                            break;
                        }
                        Err(_) => {
                            close_realtime_socket(
                                &mut socket,
                                &authenticated,
                                SocketCloseReason::SessionUnavailable,
                            )
                            .await;
                            break;
                        }
                    }
                    if heartbeat_timed_out(last_inbound, Instant::now()) {
                        break;
                    }
                    if socket.send(Message::Ping(Vec::new().into())).await.is_err() {
                        break;
                    }
                },
                incoming = socket.next() => match incoming {
                    Some(Ok(Message::Text(text))) => {
                        last_inbound = Instant::now();
                        if text_frame_too_large(text.len()) {
                            if socket
                                .send(Message::Close(Some(CloseFrame {
                                    code: 1009,
                                    reason: "Message too large".into(),
                                })))
                                .await
                                .is_err()
                            {
                                tracing::debug!("Failed to send timetable WebSocket frame-limit close");
                            }
                            break;
                        }
                        if let Ok(event) = serde_json::from_str::<TimetableEvent>(&text) {
                            if let Some(event) = sanitize_client_event(event, user_id, can_manage) {
                                relay_client_event(
                                    &state.websocket_manager,
                                    &tx,
                                    &tenant,
                                    semester_id,
                                    &mut user_presence,
                                    event,
                                );
                            }
                        }
                    }
                    Some(Ok(Message::Pong(_))) => {
                        last_inbound = Instant::now();
                    }
                    Some(Ok(Message::Ping(payload))) => {
                        last_inbound = Instant::now();
                        if socket.send(Message::Pong(payload)).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                    Some(Ok(_)) => {
                        last_inbound = Instant::now();
                    }
                },
                broadcast = rx.recv() => {
                    if send_broadcast_event(&mut socket, broadcast).await.is_err() {
                        break;
                    }
                },
            }
        }
    }

    drop(rx);
    let is_last_tab = state
        .websocket_manager
        .leave_room(tenant.clone(), semester_id, user_id);
    if is_last_tab
        && tx
            .send(SeqEvent {
                seq: None,
                event: TimetableEvent::UserLeft { user_id },
            })
            .is_err()
    {
        tracing::debug!("Timetable WebSocket room has no presence receivers");
    }
}

fn generate_color_from_uuid(id: &Uuid) -> String {
    let hash = id.as_u128();
    // simple color gen
    // take 3 bytes
    let r = (hash & 0xFF) as u8;
    let g = ((hash >> 8) & 0xFF) as u8;
    let b = ((hash >> 16) & 0xFF) as u8;
    format!("#{:02X}{:02X}{:02X}", r, g, b)
}

#[cfg(test)]
mod security_tests {
    use super::*;
    use crate::modules::auth::events::SessionRevocationEvent;
    use crate::modules::auth::session_service::AuthenticatedSession;
    use crate::modules::notification::events::PermissionChangeEvent;
    use crate::utils::tenant::TenantContext;
    use sqlx::postgres::PgPoolOptions;

    fn authenticated_session(tenant: &str) -> AuthenticatedSession {
        AuthenticatedSession {
            tenant: TenantContext {
                tenant_id: Uuid::new_v4(),
                subdomain: tenant.to_string(),
                pool: PgPoolOptions::new()
                    .connect_lazy("postgres://invalid:invalid@127.0.0.1:1/invalid")
                    .unwrap(),
            },
            session_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            username: "teacher.one".to_string(),
            user_type: "staff".to_string(),
        }
    }

    fn session_channel(
        capacity: usize,
    ) -> (
        broadcast::Sender<SessionRevocationEvent>,
        broadcast::Receiver<SessionRevocationEvent>,
    ) {
        let (sender, receiver) = broadcast::channel(capacity);
        (sender, receiver)
    }

    fn permission_channel(
        capacity: usize,
    ) -> (
        broadcast::Sender<PermissionChangeEvent>,
        broadcast::Receiver<PermissionChangeEvent>,
    ) {
        let (sender, receiver) = broadcast::channel(capacity);
        (sender, receiver)
    }

    #[tokio::test]
    async fn websocket_session_revocation_wins_before_room_initialization() {
        let (sender, mut receiver) = session_channel(8);
        let session = authenticated_session("demo");
        sender
            .send(SessionRevocationEvent::session(
                "demo",
                session.user_id,
                session.session_id,
            ))
            .unwrap();

        assert_eq!(
            queued_session_decision(&mut receiver, &session),
            SocketSessionDecision::Disconnect
        );
    }

    async fn receive_permission_decision(
        sender: &broadcast::Sender<PermissionChangeEvent>,
        receiver: &mut broadcast::Receiver<PermissionChangeEvent>,
        event: PermissionChangeEvent,
        tenant: &str,
        user_id: Uuid,
    ) -> SocketPermissionDecision {
        sender.send(event).unwrap();
        permission_event_decision(receiver.recv().await, tenant, user_id)
    }

    #[test]
    fn legacy_query_identity_is_ignored() {
        let params: WsParams = serde_json::from_value(serde_json::json!({
            "semester_id": "8b391685-4a1c-4f25-a544-b1c5bd0d457e",
            "user_id": "eb22ab8e-4382-4ddb-bcbb-8833b788e362",
            "name": "attacker",
            "school_key": "other"
        }))
        .unwrap();
        assert_eq!(
            params.semester_id.to_string(),
            "8b391685-4a1c-4f25-a544-b1c5bd0d457e"
        );
        assert_eq!(params.school_subdomain, None);
    }

    #[test]
    fn websocket_query_uses_shared_tenant_hint_and_rejects_duplicate_keys() {
        let semester_id = Uuid::new_v4();
        let raw_query = format!("semester_id={semester_id}&school_subdomain=Demo");
        let hint = parse_realtime_tenant_hint(Some(&raw_query)).unwrap();
        let params = parse_ws_params(Some(&raw_query), hint).unwrap();

        assert_eq!(params.semester_id, semester_id);
        assert_eq!(params.school_subdomain.as_deref(), Some("demo"));
        assert!(
            parse_realtime_tenant_hint(Some("school_subdomain=demo&school_subdomain=other"))
                .is_err()
        );
        assert!(parse_ws_params(
            Some(&format!(
                "semester_id={semester_id}&semester_id={semester_id}"
            )),
            None,
        )
        .is_err());
    }

    #[test]
    fn reader_can_move_cursor_but_cannot_relay_edit_intent() {
        let actor = Uuid::new_v4();
        let forged = Uuid::new_v4();
        let cursor = TimetableEvent::CursorMove {
            user_id: forged,
            x: 1.0,
            y: 2.0,
            context: None,
        };
        assert!(matches!(
            sanitize_client_event(cursor, actor, false),
            Some(TimetableEvent::CursorMove { user_id, .. }) if user_id == actor
        ));
        let refresh = TimetableEvent::TableRefresh { user_id: forged };
        assert!(sanitize_client_event(refresh, actor, false).is_none());
    }

    #[test]
    fn manager_identity_replaces_forged_payload_identity() {
        let actor = Uuid::new_v4();
        let drag = TimetableEvent::DragEnd {
            user_id: Uuid::new_v4(),
        };
        assert!(matches!(
            sanitize_client_event(drag, actor, true),
            Some(TimetableEvent::DragEnd { user_id }) if user_id == actor
        ));
    }

    #[test]
    fn server_only_events_are_never_accepted_from_clients() {
        let event = TimetableEvent::UserLeft {
            user_id: Uuid::new_v4(),
        };
        assert!(sanitize_client_event(event, Uuid::new_v4(), true).is_none());
    }

    #[test]
    fn room_key_uses_server_tenant() {
        let semester = Uuid::new_v4();
        assert_eq!(
            WebSocketManager::get_room_key("tenant-a".to_string(), semester),
            format!("tenant-a:{semester}")
        );
    }

    #[test]
    fn academic_core_signal_does_not_create_rooms_without_subscribers() {
        let manager = WebSocketManager::new();
        manager.broadcast_academic_core_changed(
            "tenant-a".to_string(),
            Uuid::new_v4(),
            "academic_year",
            Some(Uuid::new_v4()),
            Some(Uuid::new_v4()),
            None,
        );
        assert!(manager.rooms.is_empty());
        assert!(manager.room_seq.is_empty());
        assert!(manager.room_buffer.is_empty());
    }

    #[test]
    fn term_scoped_academic_core_signal_only_invalidates_the_selected_term_room() {
        let manager = WebSocketManager::new();
        let tenant = "tenant-a".to_string();
        let selected_term = Uuid::new_v4();
        let unrelated_term = Uuid::new_v4();
        let selected_sender = manager.get_or_create_room(tenant.clone(), selected_term);
        let unrelated_sender = manager.get_or_create_room(tenant.clone(), unrelated_term);
        let _selected_receivers = (selected_sender.subscribe(), selected_sender.subscribe());
        let _unrelated_receivers = (unrelated_sender.subscribe(), unrelated_sender.subscribe());

        manager.broadcast_academic_core_changed(
            tenant.clone(),
            Uuid::new_v4(),
            "academic_term",
            Some(selected_term),
            Some(Uuid::new_v4()),
            Some(selected_term),
        );

        assert_eq!(manager.current_seq(tenant.clone(), selected_term), 1);
        assert_eq!(manager.current_seq(tenant, unrelated_term), 0);
    }

    #[test]
    fn learning_delivery_signal_does_not_create_rooms_without_subscribers() {
        let manager = WebSocketManager::new();
        manager.broadcast_learning_delivery_changed(
            "tenant-a".to_string(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            Some(Uuid::new_v4()),
            2,
        );
        assert!(manager.rooms.is_empty());
        assert!(manager.room_seq.is_empty());
        assert!(manager.room_buffer.is_empty());
    }

    #[test]
    fn learning_delivery_signal_is_term_scoped_and_contains_identifiers_only() {
        let manager = WebSocketManager::new();
        let tenant = "tenant-a".to_string();
        let selected_term = Uuid::new_v4();
        let unrelated_term = Uuid::new_v4();
        let offering_id = Uuid::new_v4();
        let group_id = Uuid::new_v4();
        let selected_sender = manager.get_or_create_room(tenant.clone(), selected_term);
        let unrelated_sender = manager.get_or_create_room(tenant.clone(), unrelated_term);
        let mut selected_receiver = selected_sender.subscribe();
        let _selected_guard = selected_sender.subscribe();
        let _unrelated_receivers = (unrelated_sender.subscribe(), unrelated_sender.subscribe());

        manager.broadcast_learning_delivery_changed(
            tenant.clone(),
            Uuid::new_v4(),
            selected_term,
            offering_id,
            Some(group_id),
            7,
        );

        assert_eq!(manager.current_seq(tenant.clone(), selected_term), 1);
        assert_eq!(manager.current_seq(tenant, unrelated_term), 0);
        let event = selected_receiver.try_recv().unwrap();
        assert!(matches!(
            event.event,
            TimetableEvent::LearningDeliveryChanged {
                academic_term_id,
                learning_offering_id,
                learning_group_id: Some(received_group_id),
                revision: 7,
                ..
            } if academic_term_id == selected_term
                && learning_offering_id == offering_id
                && received_group_id == group_id
        ));
        let payload = serde_json::to_string(&event).unwrap();
        assert!(!payload.contains("student"));
        assert!(!payload.contains("roster"));
    }

    #[test]
    fn frame_limit_and_heartbeat_deadline_are_exact() {
        assert!(!text_frame_too_large(64 * 1024));
        assert!(text_frame_too_large(64 * 1024 + 1));
        let last = Instant::now();
        assert!(!heartbeat_timed_out(last, last + Duration::from_secs(89)));
        assert!(heartbeat_timed_out(last, last + Duration::from_secs(90)));
    }

    #[test]
    fn multi_tab_presence_joins_and_leaves_once() {
        let manager = WebSocketManager::new();
        let semester = Uuid::new_v4();
        let tenant = "tenant-a".to_string();
        let user_id = Uuid::new_v4();
        manager.get_or_create_room(tenant.clone(), semester);
        let presence = UserPresence {
            user_id,
            name: "Teacher".into(),
            color: "#112233".into(),
            context: None,
        };
        assert!(manager.join_room(tenant.clone(), semester, presence.clone()));
        assert!(!manager.join_room(tenant.clone(), semester, presence));
        assert!(!manager.leave_room(tenant.clone(), semester, user_id));
        assert!(manager.leave_room(tenant, semester, user_id));
    }

    #[tokio::test]
    async fn targeted_permission_event_disconnects_exact_tenant_user() {
        let (sender, mut receiver) = permission_channel(4);
        let user_id = Uuid::new_v4();

        let decision = receive_permission_decision(
            &sender,
            &mut receiver,
            PermissionChangeEvent::for_user("tenant-a", user_id),
            "tenant-a",
            user_id,
        )
        .await;

        assert_eq!(decision, SocketPermissionDecision::Disconnect);
    }

    #[tokio::test]
    async fn tenant_wide_permission_event_disconnects_user_in_that_tenant() {
        let (sender, mut receiver) = permission_channel(4);
        let user_id = Uuid::new_v4();

        let decision = receive_permission_decision(
            &sender,
            &mut receiver,
            PermissionChangeEvent::for_all_users("tenant-a"),
            "tenant-a",
            user_id,
        )
        .await;

        assert_eq!(decision, SocketPermissionDecision::Disconnect);
    }

    #[tokio::test]
    async fn permission_event_for_wrong_tenant_keeps_socket_open() {
        let (sender, mut receiver) = permission_channel(4);
        let user_id = Uuid::new_v4();

        let decision = receive_permission_decision(
            &sender,
            &mut receiver,
            PermissionChangeEvent::for_user("tenant-b", user_id),
            "tenant-a",
            user_id,
        )
        .await;

        assert_eq!(decision, SocketPermissionDecision::Continue);
    }

    #[tokio::test]
    async fn permission_event_for_wrong_user_keeps_socket_open() {
        let (sender, mut receiver) = permission_channel(4);
        let user_id = Uuid::new_v4();

        let decision = receive_permission_decision(
            &sender,
            &mut receiver,
            PermissionChangeEvent::for_user("tenant-a", Uuid::new_v4()),
            "tenant-a",
            user_id,
        )
        .await;

        assert_eq!(decision, SocketPermissionDecision::Continue);
    }

    #[tokio::test]
    async fn lagged_permission_receiver_disconnects_fail_closed() {
        let (sender, mut receiver) = permission_channel(1);
        let user_id = Uuid::new_v4();
        sender
            .send(PermissionChangeEvent::for_user("tenant-b", Uuid::new_v4()))
            .unwrap();
        sender
            .send(PermissionChangeEvent::for_user("tenant-b", Uuid::new_v4()))
            .unwrap();

        let received = receiver.recv().await;
        assert!(matches!(
            &received,
            Err(broadcast::error::RecvError::Lagged(_))
        ));
        assert_eq!(
            permission_event_decision(received, "tenant-a", user_id),
            SocketPermissionDecision::Disconnect
        );
    }

    #[test]
    fn queued_unrelated_permission_events_are_drained_before_initialization() {
        let (sender, mut receiver) = permission_channel(4);
        let user_id = Uuid::new_v4();
        sender
            .send(PermissionChangeEvent::for_user("tenant-b", user_id))
            .unwrap();
        sender
            .send(PermissionChangeEvent::for_user("tenant-a", Uuid::new_v4()))
            .unwrap();
        let mut initialization_count = 0;

        let initialized =
            initialize_socket_if_permissions_current(&mut receiver, "tenant-a", user_id, || {
                initialization_count += 1
            });

        assert!(initialized.is_some());
        assert_eq!(initialization_count, 1);
        assert!(matches!(
            receiver.try_recv(),
            Err(broadcast::error::TryRecvError::Empty)
        ));
    }

    #[test]
    fn queued_matching_permission_event_prevents_room_initialization() {
        let (sender, mut receiver) = permission_channel(4);
        let manager = WebSocketManager::new();
        let semester_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        sender
            .send(PermissionChangeEvent::for_user("tenant-b", user_id))
            .unwrap();
        sender
            .send(PermissionChangeEvent::for_user("tenant-a", user_id))
            .unwrap();
        let mut initialization_invoked = false;

        let initialized =
            initialize_socket_if_permissions_current(&mut receiver, "tenant-a", user_id, || {
                initialization_invoked = true;
                let tx = manager.get_or_create_room("tenant-a".into(), semester_id);
                let mut presence = UserPresence {
                    user_id,
                    name: "Teacher".into(),
                    color: "#112233".into(),
                    context: None,
                };
                manager.join_room("tenant-a".into(), semester_id, presence.clone());
                manager.get_state_snapshot("tenant-a".into(), semester_id);
                relay_client_event(
                    &manager,
                    &tx,
                    "tenant-a",
                    semester_id,
                    &mut presence,
                    TimetableEvent::CursorMove {
                        user_id,
                        x: 1.0,
                        y: 2.0,
                        context: None,
                    },
                );
            });

        assert!(initialized.is_none());
        assert!(!initialization_invoked);
        assert!(manager.rooms.is_empty());
        assert!(manager.room_users.is_empty());
    }

    #[test]
    fn queued_permission_lag_prevents_room_initialization() {
        let (sender, mut receiver) = permission_channel(1);
        let manager = WebSocketManager::new();
        let user_id = Uuid::new_v4();
        sender
            .send(PermissionChangeEvent::for_user("tenant-b", Uuid::new_v4()))
            .unwrap();
        sender
            .send(PermissionChangeEvent::for_user("tenant-b", Uuid::new_v4()))
            .unwrap();
        let mut initialization_invoked = false;

        let initialized =
            initialize_socket_if_permissions_current(&mut receiver, "tenant-a", user_id, || {
                initialization_invoked = true;
                manager.get_or_create_room("tenant-a".into(), Uuid::new_v4());
            });

        assert!(initialized.is_none());
        assert!(!initialization_invoked);
        assert!(manager.rooms.is_empty());
    }
}
