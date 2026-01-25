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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum TimetableEvent {
    // Presence
    UserJoined(UserPresence),
    UserLeft { user_id: Uuid },
    
    // Interactions
    CursorMove { 
        user_id: Uuid, 
        x: f64, 
        y: f64,
        day: Option<String>,
        period_id: Option<String> 
    },
    
    DragStart { 
        user_id: Uuid, 
        course_id: Option<String>,
        entry_id: Option<String> 
    },
    
    DragEnd { 
        user_id: Uuid 
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPresence {
    pub user_id: Uuid,
    pub name: String,
    pub color: String, // Hex color for cursor/border
}

#[derive(Debug, Deserialize)]
pub struct WsParams {
    pub school_id: Uuid,
    pub semester_id: Uuid,
    pub user_id: Uuid,
    pub name: String,
}

// ==========================================
// State Manager
// ==========================================

pub struct WebSocketManager {
    // Key: "school_id:semester_id" -> Broadcast Sender
    rooms: DashMap<String, broadcast::Sender<TimetableEvent>>,
}

impl WebSocketManager {
    pub fn new() -> Self {
        Self {
            rooms: DashMap::new(),
        }
    }

    fn get_room_key(school_id: Uuid, semester_id: Uuid) -> String {
        format!("{}:{}", school_id, semester_id)
    }

    pub fn get_or_create_room(&self, school_id: Uuid, semester_id: Uuid) -> broadcast::Sender<TimetableEvent> {
        let key = Self::get_room_key(school_id, semester_id);
        
        if let Some(sender) = self.rooms.get(&key) {
            return sender.clone();
        }

        let (tx, _rx) = broadcast::channel(100);
        self.rooms.insert(key, tx.clone());
        tx
    }
}

// ==========================================
// Handler
// ==========================================

pub async fn timetable_websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Query(params): Query<WsParams>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state, params))
}

async fn handle_socket(socket: WebSocket, state: AppState, params: WsParams) {
    let (mut sender, mut receiver) = socket.split();
    
    // Assign a random color if not provided (or use hash of name/id)
    // For now simple reliable logic: Generate on client or backend? Backend is safer.
    // We assume backend assigns color OR client sends it. For now, let's deterministic hash user_id to color.
    let color = generate_color_from_uuid(&params.user_id);

    let user_presence = UserPresence {
        user_id: params.user_id,
        name: params.name.clone(),
        color,
    };

    // join room
    let tx = state.websocket_manager.get_or_create_room(params.school_id, params.semester_id);
    let mut rx = tx.subscribe();

    // Broadcast JOIN event
    let _ = tx.send(TimetableEvent::UserJoined(user_presence.clone()));

    // Spawn a task to handle incoming messages from this client
    let mut send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            // Filter out messages from self? 
            // Usually broadcast sends to everyone including active sender.
            // But we filter in the loop logic below.
            
            // Serialize and send
            if let Ok(json) = serde_json::to_string(&msg) {
                 if sender.send(Message::Text(json)).await.is_err() {
                     break;
                 }
            }
        }
    });

    // Handle incoming messages from the client
    // We only expect "Action" messages to re-broadcast.
    while let Some(Ok(msg)) = receiver.next().await {
        if let Message::Text(text) = msg {
            if let Ok(event) = serde_json::from_str::<TimetableEvent>(&text) {
                // Determine if we should broadcast this
                // Ideally, we trust the client logic, but we should override 'user_id' to prevent spoofing
                
                let valid_event = match event {
                    TimetableEvent::CursorMove { x, y, day, period_id, .. } => {
                        TimetableEvent::CursorMove { user_id: params.user_id, x, y, day, period_id }
                    },
                    TimetableEvent::DragStart { course_id, entry_id, .. } => {
                        TimetableEvent::DragStart { user_id: params.user_id, course_id, entry_id }
                    },
                    TimetableEvent::DragEnd { .. } => {
                        TimetableEvent::DragEnd { user_id: params.user_id }
                    },
                    _ => continue, // Ignore other events sent by client (like Join/Left which are system managed)
                };

                // Broadcast to room
                // Note: The sender activity will trigger the rx loop above, sending echo back to self.
                // Client must handle echo (ignore own user_id).
                let _ = tx.send(valid_event);
            }
        }
    }

    // Cleanup when disconnected
    send_task.abort();
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
