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
    /// Trusted application routing identity, never supplied by a client frame.
    fn register_tenant(
        &self,
        school_key: &str,
        tenant_id: Uuid,
    ) -> Result<(), school_errors::AppError>;
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn protocol_envelopes_round_trip_without_changing_wire_fields() {
        let user_id = Uuid::from_u128(1);
        let term_id = Uuid::from_u128(2);
        let entity_id = Uuid::from_u128(3);
        let context = UserContext {
            view_mode: "term".into(),
            view_id: Some(term_id.to_string()),
        };
        let presence = UserPresence {
            user_id,
            name: "Teacher".into(),
            color: "blue".into(),
            context: Some(context.clone()),
        };
        let cases = [
            (
                TimetableEvent::StateSync {
                    users: vec![presence.clone()],
                    current_seq: 12,
                },
                json!({"type":"StateSync","payload":{"users":[{"user_id":user_id,"name":"Teacher","color":"blue","context":{"view_mode":"term","view_id":term_id.to_string()}}],"current_seq":12}}),
                false,
            ),
            (
                TimetableEvent::UserJoined(presence),
                json!({"type":"UserJoined","payload":{"user_id":user_id,"name":"Teacher","color":"blue","context":{"view_mode":"term","view_id":term_id.to_string()}}}),
                false,
            ),
            (
                TimetableEvent::UserLeft { user_id },
                json!({"type":"UserLeft","payload":{"user_id":user_id}}),
                false,
            ),
            (
                TimetableEvent::AcademicCoreChanged {
                    user_id,
                    entity_type: "term".into(),
                    entity_id: Some(entity_id),
                    academic_year_id: None,
                    academic_term_id: Some(term_id),
                },
                json!({"type":"AcademicCoreChanged","payload":{"user_id":user_id,"entity_type":"term","entity_id":entity_id,"academic_year_id":null,"academic_term_id":term_id}}),
                true,
            ),
            (
                TimetableEvent::LearningDeliveryChanged {
                    user_id,
                    academic_term_id: term_id,
                    learning_offering_id: entity_id,
                    learning_group_id: None,
                    revision: 7,
                },
                json!({"type":"LearningDeliveryChanged","payload":{"user_id":user_id,"academic_term_id":term_id,"learning_offering_id":entity_id,"learning_group_id":null,"revision":7}}),
                true,
            ),
            (
                TimetableEvent::TimetableChanged {
                    user_id,
                    academic_term_id: term_id,
                    timetable_version_id: entity_id,
                    block_id: None,
                    revision: 8,
                },
                json!({"type":"TimetableChanged","payload":{"user_id":user_id,"academic_term_id":term_id,"timetable_version_id":entity_id,"block_id":null,"revision":8}}),
                true,
            ),
            (
                TimetableEvent::CursorMove {
                    user_id,
                    x: 1.5,
                    y: 2.5,
                    context: Some(context),
                },
                json!({"type":"CursorMove","payload":{"user_id":user_id,"x":1.5,"y":2.5,"context":{"view_mode":"term","view_id":term_id.to_string()}}}),
                false,
            ),
        ];
        for (event, expected, is_mutation) in cases {
            assert_eq!(event.is_mutation(), is_mutation);
            assert_eq!(serde_json::to_value(&event).unwrap(), expected);
            let restored: TimetableEvent = serde_json::from_value(expected.clone()).unwrap();
            assert_eq!(serde_json::to_value(&restored).unwrap(), expected);
            assert_eq!(restored.is_mutation(), is_mutation);
        }
    }
}
