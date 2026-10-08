use serde::{Deserialize, Serialize};
use uuid::Uuid;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum TimetableEvent {
    StateSync {
        users: Vec<UserPresence>,
        current_seq: u64,
    },
    UserJoined(UserPresence),
    UserLeft {
        user_id: Uuid,
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
    TimetableChanged {
        user_id: Uuid,
        academic_term_id: Uuid,
        timetable_version_id: Uuid,
        block_id: Option<Uuid>,
        revision: i64,
    },

    CursorMove {
        user_id: Uuid,
        x: f64,
        y: f64,
        context: Option<UserContext>,
    },
}

impl TimetableEvent {
    /// Event ประเภท mutation (ต้อง seq + buffer). คืน true ถ้าต้อง track
    pub fn is_mutation(&self) -> bool {
        matches!(
            self,
            TimetableEvent::AcademicCoreChanged { .. }
                | TimetableEvent::LearningDeliveryChanged { .. }
                | TimetableEvent::TimetableChanged { .. }
        )
    }
}

pub trait AcademicRealtimePort: Send + Sync {
    fn broadcast_mutation(
        &self,
        school_key: String,
        academic_term_id: Uuid,
        event: TimetableEvent,
    ) -> u64;
    fn broadcast_academic_core_changed(
        &self,
        school_key: String,
        user_id: Uuid,
        entity_type: &str,
        entity_id: Option<Uuid>,
        academic_year_id: Option<Uuid>,
        academic_term_id: Option<Uuid>,
    );
    fn broadcast_learning_delivery_changed(
        &self,
        school_key: String,
        user_id: Uuid,
        academic_term_id: Uuid,
        learning_offering_id: Uuid,
        learning_group_id: Option<Uuid>,
        revision: i64,
    );
}
