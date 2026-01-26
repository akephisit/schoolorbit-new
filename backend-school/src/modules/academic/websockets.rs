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
use tokio::sync::broadcast;
use dashmap::DashMap;
use uuid::Uuid;
use crate::AppState;

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
        drags: std::collections::HashMap<Uuid, DragState> // user_id -> drag info
    },

    // Presence
    UserJoined(UserPresence),
    UserLeft { user_id: Uuid },
    
    // Sync Data
    TableRefresh {
        user_id: Uuid
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
    // Room Key -> Broadcast Sender
    rooms: DashMap<String, broadcast::Sender<TimetableEvent>>,
    // Room Key -> (User ID -> User Presence)
    room_users: DashMap<String, DashMap<Uuid, UserPresence>>,
    // Room Key -> (User ID -> Drag State)
    room_drags: DashMap<String, DashMap<Uuid, DragState>>,
}

impl WebSocketManager {
    pub fn new() -> Self {
        Self {
            rooms: DashMap::new(),
            room_users: DashMap::new(),
            room_drags: DashMap::new(),
        }
    }

    fn get_room_key(school_key: String, semester_id: Uuid) -> String {
        format!("{}:{}", school_key, semester_id)
    }

    pub fn get_or_create_room(&self, school_key: String, semester_id: Uuid) -> broadcast::Sender<TimetableEvent> {
        let key = Self::get_room_key(school_key, semester_id);
        
        if let Some(sender) = self.rooms.get(&key) {
            return sender.clone();
        }

        let (tx, _rx) = broadcast::channel(100);
        self.rooms.insert(key.clone(), tx.clone());
        // Init state maps if not exist
        self.room_users.entry(key.clone()).or_insert_with(DashMap::new);
        self.room_drags.entry(key).or_insert_with(DashMap::new);
        
        tx
    }

    pub fn join_room(&self, school_key: String, semester_id: Uuid, user: UserPresence) {
        let key = Self::get_room_key(school_key, semester_id);
        if let Some(users) = self.room_users.get(&key) {
            users.insert(user.user_id, user);
        }
    }

    pub fn leave_room(&self, school_key: String, semester_id: Uuid, user_id: Uuid) {
        let key = Self::get_room_key(school_key, semester_id);
        if let Some(users) = self.room_users.get(&key) {
            users.remove(&user_id);
        }
        if let Some(drags) = self.room_drags.get(&key) {
            drags.remove(&user_id);
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

    // 2. Send Initial State Layout (Snapshot) to THIS user only
    let (users, drags) = state.websocket_manager.get_state_snapshot(school_key.clone(), params.semester_id);
    let sync_event = TimetableEvent::StateSync { users, drags };
    if let Ok(json) = serde_json::to_string(&sync_event) {
        let _ = sender.send(Message::Text(json.into())).await;
    }

    // 3. Broadcast JOIN to others
    let _ = tx.send(TimetableEvent::UserJoined(user_presence.clone()));

    // Spawn a task to handle incoming messages from this client (Broadcast Listener)
    let mut send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            // Send everything (including echo for consistency in state updates, though client filters echo)
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
                    TimetableEvent::TableRefresh { .. } => {
                        valid_event = Some(TimetableEvent::TableRefresh { user_id: params.user_id });
                    },
                    _ => {}
                }

                if let Some(evt) = valid_event {
                    let _ = tx.send(evt);
                }
            }
        }
    }

    // Cleanup
    send_task.abort();
    state.websocket_manager.leave_room(school_key.clone(), params.semester_id, params.user_id);
    let _ = tx.send(TimetableEvent::UserLeft { user_id: params.user_id });
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
