use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State, Query,
    },
    response::IntoResponse,
};
use futures::{sink::SinkExt, stream::StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::collections::VecDeque;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use dashmap::DashMap;
use uuid::Uuid;
use crate::AppState;

/// จำนวน event ที่เก็บใน buffer ต่อ room (สำหรับ replay เมื่อ client reconnect)
const EVENT_BUFFER_SIZE: usize = 200;

/// ลบ room ที่ไม่มี subscriber นานเกินเวลานี้
const ROOM_IDLE_TTL: Duration = Duration::from_secs(600); // 10 นาที

/// interval ของ cleanup task
const ROOM_CLEANUP_INTERVAL: Duration = Duration::from_secs(60); // ตรวจทุก 1 นาที

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
    UserLeft { user_id: Uuid },

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

    // Interactions
    CursorMove {
        user_id: Uuid,
        x: f64,
        y: f64,
        context: Option<UserContext>
    },

    DragStart {
        user_id: Uuid,
        course_id: Option<String>,
        entry_id: Option<String>,
        info: Option<DragInfo>
    },

    DragEnd {
        user_id: Uuid
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
        activity_type: String,                   // "room_picker" | "instructor_edit" | ...
        target: Option<serde_json::Value>,       // { entry_id?, day?, period_id? }
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
        kind: String,                            // "move" | "swap" | "replace"
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
        user_id: Uuid,                           // คนที่ drop (ใช้ filter toast)
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
        temp_id: String,                         // UUID ที่ client gen ขึ้นเอง
        classroom_id: Uuid,
        classroom_course_id: Option<Uuid>,
        activity_slot_id: Option<Uuid>,
        day_of_week: String,
        period_id: Uuid,
        room_id: Option<Uuid>,
        title: Option<String>,                   // สำหรับ ACTIVITY
        entry_type: String,                      // "COURSE" | "ACTIVITY"
    },
    /// Server → Clients: CREATE ถูก reject → ทุก client ลบ tempEntry ที่มี temp_id นี้
    EntryRejected {
        user_id: Uuid,                           // คนที่สร้าง (ใช้ filter toast)
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
    pub user_id: Uuid,
    pub name: String,
    pub school_key: Option<String>,
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
                        eprintln!("[WS cleanup] dropped idle room: {}", key);
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

    pub fn get_or_create_room(&self, school_key: String, semester_id: Uuid) -> broadcast::Sender<SeqEvent> {
        let key = Self::get_room_key(school_key, semester_id);

        if let Some(sender) = self.rooms.get(&key) {
            return sender.clone();
        }

        let (tx, _rx) = broadcast::channel(100);
        self.rooms.insert(key.clone(), tx.clone());
        self.room_users.entry(key.clone()).or_insert_with(DashMap::new);
        self.room_drags.entry(key.clone()).or_insert_with(DashMap::new);
        self.room_activities.entry(key.clone()).or_insert_with(DashMap::new);
        self.room_seq.entry(key.clone()).or_insert_with(|| Arc::new(AtomicU64::new(0)));
        self.room_buffer.entry(key).or_insert_with(|| Arc::new(Mutex::new(VecDeque::with_capacity(EVENT_BUFFER_SIZE))));

        tx
    }

    /// Broadcast ephemeral event (presence, cursor, drag) — ไม่มี seq ไม่ buffer
    pub fn broadcast_ephemeral(&self, school_key: String, semester_id: Uuid, event: TimetableEvent) {
        let tx = self.get_or_create_room(school_key, semester_id);
        let _ = tx.send(SeqEvent { seq: None, event });
    }

    /// Broadcast mutation event — assign seq, push buffer, send.
    /// Skip ทั้งหมดถ้า receiver_count <= 1 (มีแค่ caller เอง หรือไม่มีใคร) — ประหยัด
    /// seq ไม่เพิ่ม, buffer ไม่โต, send ไม่เกิด. Return 0 เมื่อ skip
    pub fn broadcast_mutation(&self, school_key: String, semester_id: Uuid, event: TimetableEvent) -> u64 {
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

        let seq_event = SeqEvent { seq: Some(seq), event };

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

    pub fn current_seq(&self, school_key: String, semester_id: Uuid) -> u64 {
        let key = Self::get_room_key(school_key, semester_id);
        self.room_seq.get(&key).map(|c| c.load(Ordering::SeqCst)).unwrap_or(0)
    }

    /// True ถ้ามี subscriber อย่างน้อย 1 คนใน room (ใครก็ได้)
    pub fn has_subscribers(&self, school_key: String, semester_id: Uuid) -> bool {
        let key = Self::get_room_key(school_key, semester_id);
        self.rooms.get(&key).map(|tx| tx.receiver_count() > 0).unwrap_or(false)
    }

    /// True ถ้ามี subscriber **นอกจากตัว caller** (อย่างน้อย 2 คน)
    /// ใช้ skip joined re-fetch เมื่อ mutation มาจากคนเดียวที่อยู่ใน room
    /// (echo กลับให้ตัวเองไม่คุ้ม — client จะ loadTimetable ต่ออยู่แล้ว)
    pub fn has_other_subscribers(&self, school_key: String, semester_id: Uuid) -> bool {
        let key = Self::get_room_key(school_key, semester_id);
        self.rooms.get(&key).map(|tx| tx.receiver_count() > 1).unwrap_or(false)
    }

    /// Return events with seq > after_seq, ordered. If buffer doesn't reach back that far,
    /// return None (signal caller: client ต้อง full-fetch)
    pub fn replay(&self, school_key: String, semester_id: Uuid, after_seq: u64) -> Option<Vec<SeqEvent>> {
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
            users.entry(user.user_id)
                .and_modify(|(presence, count)| {
                    *presence = user.clone(); // refresh presence (ชื่อ/สี อัปเดต)
                    *count += 1;
                })
                .or_insert_with(|| { is_first = true; (user, 1) });
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

    pub fn update_drag(&self, school_key: String, semester_id: Uuid, user_id: Uuid, drag: Option<DragState>) {
        let key = Self::get_room_key(school_key, semester_id);
        if let Some(drags) = self.room_drags.get(&key) {
            if let Some(d) = drag {
                drags.insert(user_id, d);
            } else {
                drags.remove(&user_id);
            }
        }
    }
    
    pub fn update_activity(&self, school_key: String, semester_id: Uuid, user_id: Uuid, activity: Option<ActivityState>) {
        let key = Self::get_room_key(school_key, semester_id);
        if let Some(activities) = self.room_activities.get(&key) {
            match activity {
                Some(a) => { activities.insert(user_id, a); }
                None => { activities.remove(&user_id); }
            }
        }
    }

    pub fn update_context(&self, school_key: String, semester_id: Uuid, user_id: Uuid, context: Option<UserContext>) {
         let key = Self::get_room_key(school_key, semester_id);
         if let Some(users) = self.room_users.get(&key) {
             if let Some(mut entry) = users.get_mut(&user_id) {
                 entry.value_mut().0.context = context;
             }
         }
    }

    pub fn get_state_snapshot(&self, school_key: String, semester_id: Uuid) -> (Vec<UserPresence>, std::collections::HashMap<Uuid, DragState>, std::collections::HashMap<Uuid, ActivityState>) {
        let key = Self::get_room_key(school_key, semester_id);

        let users = self.room_users.get(&key)
            .map(|m| m.iter().map(|kv| kv.value().0.clone()).collect())
            .unwrap_or_default();

        let drags = self.room_drags.get(&key)
            .map(|m| m.iter().map(|kv| (*kv.key(), kv.value().clone())).collect())
            .unwrap_or_default();

        let activities = self.room_activities.get(&key)
            .map(|m| m.iter().map(|kv| (*kv.key(), kv.value().clone())).collect())
            .unwrap_or_default();

        (users, drags, activities)
    }
}

// ==========================================
// Handler
// ==========================================

pub async fn timetable_websocket_handler(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Query(params): Query<WsParams>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    // 1. Resolve Subdomain from Host
    let host = headers
        .get("host")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("localhost");

    // Simple subdomain extraction (excludes port)
    let domain = host.split(':').next().unwrap_or(host);
    let parts: Vec<&str> = domain.split('.').collect();
    
    // Logic: if "school.orbit.com", subdomain is "school". 
    // If localhost, maybe "default" or handled differently.
    let subdomain = if parts.len() >= 3 {
        parts[0]
    } else {
        // Fallback for localhost or dev environments
        "default" 
    };

    // Priority: Query Param > Host Header
    // This fixes the issue where Socket connects to API domain (school-api) but App is on Subdomain (snwsb)
    let school_key = if let Some(key) = params.school_key.clone() {
        key
    } else {
        subdomain.to_string()
    };

    ws.on_upgrade(move |socket| handle_socket(socket, state, params, school_key))
}

async fn handle_socket(socket: WebSocket, state: AppState, params: WsParams, school_key: String) {
    let (mut sender, mut receiver) = socket.split();
    
    // Assign a random color
    let color = generate_color_from_uuid(&params.user_id);

    let mut user_presence = UserPresence {
        user_id: params.user_id,
        name: params.name.clone(),
        color,
        context: None,
    };

    // 1. Join Room & Store Presence
    let tx = state.websocket_manager.get_or_create_room(school_key.clone(), params.semester_id);
    let mut rx = tx.subscribe();

    let is_first_tab = state.websocket_manager.join_room(school_key.clone(), params.semester_id, user_presence.clone());

    // 2. Send Initial State (with current_seq — client ใช้เป็นจุดเริ่ม tracking)
    let (users, drags, activities) = state.websocket_manager.get_state_snapshot(school_key.clone(), params.semester_id);
    let current_seq = state.websocket_manager.current_seq(school_key.clone(), params.semester_id);
    let sync_event = SeqEvent {
        seq: None,
        event: TimetableEvent::StateSync { users, drags, activities, current_seq },
    };
    if let Ok(json) = serde_json::to_string(&sync_event) {
        let _ = sender.send(Message::Text(json.into())).await;
    }

    // 3. Broadcast UserJoined เฉพาะถ้าเป็น tab แรกของ user (tab ที่ 2+ ไม่ส่ง — user เดิม)
    if is_first_tab {
        let _ = tx.send(SeqEvent { seq: None, event: TimetableEvent::UserJoined(user_presence.clone()) });
    }

    // Spawn a task to handle incoming messages from this client (Broadcast Listener)
    let mut send_task = tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(msg) => {
                    if let Ok(json) = serde_json::to_string(&msg) {
                        if sender.send(Message::Text(json.into())).await.is_err() {
                            break;
                        }
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    // Client ตามไม่ทัน → miss n events — ส่ง TableRefresh ให้ full-refetch
                    eprintln!("[WS] client lagged, missed {} events — forcing full refresh", n);
                    let refresh = SeqEvent {
                        seq: None,
                        event: TimetableEvent::TableRefresh { user_id: Uuid::nil() },
                    };
                    if let Ok(json) = serde_json::to_string(&refresh) {
                        if sender.send(Message::Text(json.into())).await.is_err() {
                            break;
                        }
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    break;
                }
            }
        }
    });

    // Handle incoming messages from the client
    while let Some(Ok(msg)) = receiver.next().await {
        if let Message::Text(text) = msg {
            if let Ok(event) = serde_json::from_str::<TimetableEvent>(&text) {
                
                let mut valid_event = None;

                match &event {
                    TimetableEvent::CursorMove { x, y, context, .. } => {
                        // Update Context in memory if changed
                        if user_presence.context != *context {
                             state.websocket_manager.update_context(school_key.clone(), params.semester_id, params.user_id, context.clone());
                             user_presence.context = context.clone();
                        }
                        
                        valid_event = Some(TimetableEvent::CursorMove { 
                            user_id: params.user_id, 
                            x: *x, 
                            y: *y, 
                            context: context.clone() 
                        });
                    },
                    TimetableEvent::DragStart { course_id, entry_id, info, .. } => {
                        // Update Drag in memory
                        let drag_state = DragState { 
                            course_id: course_id.clone(), 
                            entry_id: entry_id.clone(),
                            info: info.clone() 
                        };
                        state.websocket_manager.update_drag(school_key.clone(), params.semester_id, params.user_id, Some(drag_state));
                        
                        valid_event = Some(TimetableEvent::DragStart { 
                            user_id: params.user_id, 
                            course_id: course_id.clone(), 
                            entry_id: entry_id.clone(),
                            info: info.clone()
                        });
                    },
                    TimetableEvent::DragEnd { .. } => {
                        // Clear Drag
                        state.websocket_manager.update_drag(school_key.clone(), params.semester_id, params.user_id, None);

                        valid_event = Some(TimetableEvent::DragEnd { user_id: params.user_id });
                    },
                    TimetableEvent::DragMove { x, y, target_day, target_period_id, .. } => {
                        // Relay drag position — no state storage needed (ephemeral)
                        valid_event = Some(TimetableEvent::DragMove {
                            user_id: params.user_id,
                            x: *x,
                            y: *y,
                            target_day: target_day.clone(),
                            target_period_id: target_period_id.clone(),
                        });
                    },
                    TimetableEvent::TableRefresh { .. } => {
                        valid_event = Some(TimetableEvent::TableRefresh { user_id: params.user_id });
                    },
                    TimetableEvent::UserActivity { activity_type, target, .. } => {
                        state.websocket_manager.update_activity(
                            school_key.clone(),
                            params.semester_id,
                            params.user_id,
                            Some(ActivityState {
                                activity_type: activity_type.clone(),
                                target: target.clone(),
                            }),
                        );
                        valid_event = Some(TimetableEvent::UserActivity {
                            user_id: params.user_id,
                            activity_type: activity_type.clone(),
                            target: target.clone(),
                        });
                    },
                    TimetableEvent::UserActivityEnd { .. } => {
                        state.websocket_manager.update_activity(
                            school_key.clone(),
                            params.semester_id,
                            params.user_id,
                            None,
                        );
                        valid_event = Some(TimetableEvent::UserActivityEnd { user_id: params.user_id });
                    },
                    // Phase 2: client broadcast optimistic drop intent
                    TimetableEvent::DropIntent {
                        kind, entry_id, day_of_week, period_id, room_id,
                        swap_partner_id, swap_partner_day, swap_partner_period_id,
                        new_classroom_course_id, new_activity_slot_id, new_classroom_id, ..
                    } => {
                        valid_event = Some(TimetableEvent::DropIntent {
                            user_id: params.user_id,
                            kind: kind.clone(),
                            entry_id: *entry_id,
                            day_of_week: day_of_week.clone(),
                            period_id: *period_id,
                            room_id: *room_id,
                            swap_partner_id: *swap_partner_id,
                            swap_partner_day: swap_partner_day.clone(),
                            swap_partner_period_id: *swap_partner_period_id,
                            new_classroom_course_id: *new_classroom_course_id,
                            new_activity_slot_id: *new_activity_slot_id,
                            new_classroom_id: *new_classroom_id,
                        });
                    },
                    // Phase 2: client broadcast optimistic create intent (NEW drop on empty)
                    TimetableEvent::EntryIntent {
                        temp_id, classroom_id, classroom_course_id, activity_slot_id,
                        day_of_week, period_id, room_id, title, entry_type, ..
                    } => {
                        valid_event = Some(TimetableEvent::EntryIntent {
                            user_id: params.user_id,
                            temp_id: temp_id.clone(),
                            classroom_id: *classroom_id,
                            classroom_course_id: *classroom_course_id,
                            activity_slot_id: *activity_slot_id,
                            day_of_week: day_of_week.clone(),
                            period_id: *period_id,
                            room_id: *room_id,
                            title: title.clone(),
                            entry_type: entry_type.clone(),
                        });
                    },
                    _ => {}
                }

                if let Some(evt) = valid_event {
                    // Relayed client events: TableRefresh = mutation (ต้อง seq + buffer),
                    // อื่นๆ (CursorMove, DragStart/End/Move) = ephemeral
                    if evt.is_mutation() {
                        state.websocket_manager.broadcast_mutation(
                            school_key.clone(),
                            params.semester_id,
                            evt,
                        );
                    } else {
                        let _ = tx.send(SeqEvent { seq: None, event: evt });
                    }
                }
            }
        }
    }

    // Cleanup
    send_task.abort();
    let is_last_tab = state.websocket_manager.leave_room(school_key.clone(), params.semester_id, params.user_id);
    // Broadcast UserLeft เฉพาะถ้าเป็น tab สุดท้ายของ user
    if is_last_tab {
        let _ = tx.send(SeqEvent { seq: None, event: TimetableEvent::UserLeft { user_id: params.user_id } });
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
