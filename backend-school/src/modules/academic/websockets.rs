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
    // Room Key -> (User ID -> User Presence)
    room_users: DashMap<String, DashMap<Uuid, UserPresence>>,
    // Room Key -> (User ID -> Drag State)
    room_drags: DashMap<String, DashMap<Uuid, DragState>>,
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
                let mut to_remove: Vec<String> = Vec::new();
                for entry in self.room_empty_since.iter() {
                    if now.duration_since(*entry.value()) > ROOM_IDLE_TTL {
                        // Double-check ไม่มี subscriber ณ ตอนนี้
                        if let Some(tx) = self.rooms.get(entry.key()) {
                            if tx.receiver_count() == 0 {
                                to_remove.push(entry.key().clone());
                            }
                        }
                    }
                }
                for key in to_remove {
                    self.rooms.remove(&key);
                    self.room_users.remove(&key);
                    self.room_drags.remove(&key);
                    self.room_seq.remove(&key);
                    self.room_buffer.remove(&key);
                    self.room_empty_since.remove(&key);
                    eprintln!("[WS cleanup] dropped idle room: {}", key);
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

    pub fn join_room(&self, school_key: String, semester_id: Uuid, user: UserPresence) {
        let key = Self::get_room_key(school_key, semester_id);
        if let Some(users) = self.room_users.get(&key) {
            users.insert(user.user_id, user);
        }
        // มี subscriber เข้ามา — room ไม่ว่างอีกต่อไป
        self.room_empty_since.remove(&key);
    }

    pub fn leave_room(&self, school_key: String, semester_id: Uuid, user_id: Uuid) {
        let key = Self::get_room_key(school_key, semester_id);
        if let Some(users) = self.room_users.get(&key) {
            users.remove(&user_id);
        }
        if let Some(drags) = self.room_drags.get(&key) {
            drags.remove(&user_id);
        }
        // ถ้าไม่มี subscriber เหลือ → mark เวลาเริ่มว่าง (cleanup task จะลบในภายหลัง)
        if let Some(tx) = self.rooms.get(&key) {
            if tx.receiver_count() == 0 {
                self.room_empty_since.insert(key, Instant::now());
            }
        }
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
    
    pub fn update_context(&self, school_key: String, semester_id: Uuid, user_id: Uuid, context: Option<UserContext>) {
         let key = Self::get_room_key(school_key, semester_id);
         if let Some(users) = self.room_users.get(&key) {
             if let Some(mut user) = users.get_mut(&user_id) {
                 user.context = context;
             }
         }
    }

    pub fn get_state_snapshot(&self, school_key: String, semester_id: Uuid) -> (Vec<UserPresence>, std::collections::HashMap<Uuid, DragState>) {
        let key = Self::get_room_key(school_key, semester_id);
        
        let users = self.room_users.get(&key)
            .map(|m| m.iter().map(|kv| kv.value().clone()).collect())
            .unwrap_or_default();
            
        let drags = self.room_drags.get(&key)
            .map(|m| m.iter().map(|kv| (*kv.key(), kv.value().clone())).collect())
            .unwrap_or_default();
            
        (users, drags)
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

    state.websocket_manager.join_room(school_key.clone(), params.semester_id, user_presence.clone());

    // 2. Send Initial State (with current_seq — client ใช้เป็นจุดเริ่ม tracking)
    let (users, drags) = state.websocket_manager.get_state_snapshot(school_key.clone(), params.semester_id);
    let current_seq = state.websocket_manager.current_seq(school_key.clone(), params.semester_id);
    let sync_event = SeqEvent {
        seq: None,
        event: TimetableEvent::StateSync { users, drags, current_seq },
    };
    if let Ok(json) = serde_json::to_string(&sync_event) {
        let _ = sender.send(Message::Text(json.into())).await;
    }

    // 3. Broadcast JOIN to others
    let _ = tx.send(SeqEvent { seq: None, event: TimetableEvent::UserJoined(user_presence.clone()) });

    // Spawn a task to handle incoming messages from this client (Broadcast Listener)
    let mut send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if let Ok(json) = serde_json::to_string(&msg) {
                 if sender.send(Message::Text(json.into())).await.is_err() {
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
    state.websocket_manager.leave_room(school_key.clone(), params.semester_id, params.user_id);
    let _ = tx.send(SeqEvent { seq: None, event: TimetableEvent::UserLeft { user_id: params.user_id } });
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
