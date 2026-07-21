use crate::error::AppError;
use crate::modules::academic::services::timetable_realtime_service::{
    authorize_socket, TimetableSocketAccess,
};
use crate::utils::request_context::actor_tenant_context;
use crate::AppState;
use axum::{
    extract::{
        ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    http::HeaderMap,
    response::Response,
};
use dashmap::DashMap;
use futures::stream::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use uuid::Uuid;

/// จำนวน event ที่เก็บใน buffer ต่อ room (สำหรับ replay เมื่อ client reconnect)
const EVENT_BUFFER_SIZE: usize = 200;

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

pub async fn timetable_websocket_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<WsParams>,
    ws: WebSocketUpgrade,
) -> Result<Response, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let access = authorize_socket(&context.tenant.pool, &context.actor, params.semester_id).await?;
    let tenant = context.tenant.subdomain;

    Ok(ws
        .on_upgrade(move |socket| handle_socket(socket, state, params.semester_id, tenant, access)))
}

async fn handle_socket(
    mut socket: WebSocket,
    state: AppState,
    semester_id: Uuid,
    tenant: String,
    access: TimetableSocketAccess,
) {
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

    let tx = state
        .websocket_manager
        .get_or_create_room(tenant.clone(), semester_id);
    let mut rx = tx.subscribe();
    let is_first_tab =
        state
            .websocket_manager
            .join_room(tenant.clone(), semester_id, user_presence.clone());

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
        let mut heartbeat = tokio::time::interval(HEARTBEAT_INTERVAL);
        heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        heartbeat.tick().await;
        let mut last_inbound = Instant::now();

        loop {
            tokio::select! {
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
                _ = heartbeat.tick() => {
                    if heartbeat_timed_out(last_inbound, Instant::now()) {
                        break;
                    }
                    if socket.send(Message::Ping(Vec::new().into())).await.is_err() {
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
}
