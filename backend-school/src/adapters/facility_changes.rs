use std::sync::Arc;

use school_academic_http::websockets::WebSocketManager;
use school_administration::http::state::FacilityResourceChangePort;
use uuid::Uuid;

pub(crate) struct ApplicationFacilityChanges(pub Arc<WebSocketManager>);

impl FacilityResourceChangePort for ApplicationFacilityChanges {
    fn room_changed(&self, tenant_id: Uuid, school_key: &str, actor_id: Uuid, room_id: Uuid) {
        self.0.collaboration().invalidate_term(tenant_id, None);
        self.0.broadcast_academic_core_changed(
            school_key.to_owned(),
            actor_id,
            "facility_room",
            Some(room_id),
            None,
            None,
        );
    }
}
