use axum::response::sse::Event;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use tokio::sync::mpsc;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SseMessage {
    Log {
        level: String,
        message: String,
    },
    Progress {
        step: u8,
        total: u8,
        message: String,
    },
    Complete {
        data: serde_json::Value,
    },
    Error {
        error: String,
    },
}

pub struct SseLogger {
    sender: mpsc::Sender<Result<Event, Infallible>>,
}

impl SseLogger {
    pub fn new(sender: mpsc::Sender<Result<Event, Infallible>>) -> Self {
        Self { sender }
    }

    pub async fn log(&self, level: &str, message: &str) {
        let msg = SseMessage::Log {
            level: level.to_string(),
            message: message.to_string(),
        };
        self.send_message(msg).await;
    }

    pub async fn info(&self, message: &str) {
        self.log("info", message).await;
    }

    pub async fn success(&self, message: &str) {
        self.log("success", message).await;
    }

    pub async fn error(&self, message: &str) {
        self.log("error", message).await;
    }

    pub async fn warning(&self, message: &str) {
        self.log("warning", message).await;
    }

    pub async fn progress(&self, step: u8, total: u8, message: &str) {
        let msg = SseMessage::Progress {
            step,
            total,
            message: message.to_string(),
        };
        self.send_message(msg).await;
    }

    pub async fn complete(&self, data: serde_json::Value) {
        let msg = SseMessage::Complete { data };
        self.send_message(msg).await;
    }

    pub async fn error_complete(&self, error: String) {
        let msg = SseMessage::Error { error };
        self.send_message(msg).await;
    }

    async fn send_message(&self, msg: SseMessage) {
        if let Ok(json) = serde_json::to_string(&msg) {
            let event = Event::default().data(json);
            let _ = self.sender.send(Ok(event)).await;
        }
    }
}
