//! The draft collaboration wire contract. Client commands never carry actor identity.
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
#[cfg(feature = "api-docs")]
use utoipa::ToSchema;
use uuid::Uuid;

use super::timetable::Timetable;
use super::timetable_block::{
    TimetableBlock, TimetableBlockPlacementCandidate, TimetableBlockPlacementSource,
    TimetableBlockSummary, TimetableBlockWorkspace, TimetableBlockWorkspaceHomeroom,
    TimetableBlockWorkspaceRoom, TimetableBlockWorkspaceStaff, TimetableOfferingSubjectGroup,
    TimetableOrdinaryDemand, TimetableSynchronizedDemand,
};
use super::timetable_source::TimetableSourceIssue;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
pub enum TimetableViewMode {
    #[serde(rename = "homeroom")]
    Homeroom,
    #[serde(rename = "teacher")]
    Teacher,
    #[serde(rename = "learning_group")]
    LearningGroup,
    #[serde(rename = "wholeSchool")]
    WholeSchool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TimetableContext {
    pub view_mode: TimetableViewMode,
    pub view_id: Option<Uuid>,
    pub day_of_week: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum TimetableAnchor {
    Cell {
        #[serde(rename = "dayOfWeek")]
        day_of_week: String,
        #[serde(rename = "bellSchedulePeriodId")]
        bell_schedule_period_id: Uuid,
        u: f64,
        v: f64,
    },
    Block {
        #[serde(rename = "blockId")]
        block_id: Uuid,
        u: f64,
        v: f64,
    },
    Tray {
        source: TimetableBlockPlacementSource,
        u: f64,
        v: f64,
    },
}

impl TimetableAnchor {
    pub fn has_valid_coordinates(&self) -> bool {
        let (u, v) = match self {
            Self::Cell { u, v, .. } | Self::Block { u, v, .. } | Self::Tray { u, v, .. } => {
                (*u, *v)
            }
        };
        u.is_finite() && v.is_finite() && (0.0..=1.0).contains(&u) && (0.0..=1.0).contains(&v)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TimetableTarget {
    pub day_of_week: String,
    pub bell_schedule_period_id: Uuid,
    pub block_id: Option<Uuid>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum TimetableReservationPurpose {
    Drag,
    Edit,
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TimetablePresenceUpdate {
    pub context: TimetableContext,
    pub anchor: Option<TimetableAnchor>,
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TimetableClaim {
    pub request_id: Uuid,
    pub source: TimetableBlockPlacementSource,
    pub purpose: TimetableReservationPurpose,
    /// Private configuration, used only for the server's resource footprint.
    pub candidate: Option<TimetableBlockPlacementCandidate>,
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TimetableTargetUpdate {
    pub request_id: Uuid,
    pub lease_token: Uuid,
    pub target: Option<TimetableTarget>,
    pub candidate: Option<TimetableBlockPlacementCandidate>,
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TimetableLeaseCommand {
    pub request_id: Uuid,
    pub lease_token: Uuid,
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TimetableReplay {
    pub epoch: Uuid,
    pub after_seq: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(tag = "type", content = "payload", deny_unknown_fields)]
pub enum TimetableClientMessage {
    ClientPresenceUpdate(TimetablePresenceUpdate),
    ClientClaim(TimetableClaim),
    ClientTarget(TimetableTargetUpdate),
    ClientRenew(TimetableLeaseCommand),
    ClientRelease(TimetableLeaseCommand),
    ClientReplay(TimetableReplay),
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct TimetableConnectionPresence {
    pub connection_id: Uuid,
    pub active_reservation_id: Option<Uuid>,
    pub context: Option<TimetableContext>,
    pub anchor: Option<TimetableAnchor>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct TimetableParticipant {
    pub user_id: Uuid,
    pub name: String,
    pub color: String,
    pub profile_image_file_id: Option<Uuid>,
    pub connections: Vec<TimetableConnectionPresence>,
}

/// Public lease identity is distinct from its private ownership token.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct TimetableReservation {
    pub reservation_id: Uuid,
    pub owner_connection_id: Uuid,
    pub purpose: TimetableReservationPurpose,
    pub source: TimetableBlockPlacementSource,
    pub target: Option<TimetableTarget>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum TimetableCommandKind {
    Claim,
    Target,
    Renew,
    Release,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct TimetableCommandResult {
    pub request_id: Uuid,
    pub command: TimetableCommandKind,
    pub accepted: bool,
    pub code: Option<String>,
    pub source: Option<TimetableBlockPlacementSource>,
    pub target: Option<TimetableTarget>,
    pub reservation_id: Option<Uuid>,
    pub lease_token: Option<Uuid>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct TimetableBlockedCell {
    pub day_of_week: String,
    pub bell_schedule_period_id: Uuid,
    pub reservation_id: Uuid,
    pub owner_connection_id: Uuid,
    pub purpose: TimetableReservationPurpose,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct TimetableRoomOptions {
    pub reservation_id: Uuid,
    pub available_room_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct TimetableAvailability {
    pub source: TimetableBlockPlacementSource,
    pub context: Option<TimetableContext>,
    pub blocked_cells: Vec<TimetableBlockedCell>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room_options: Option<TimetableRoomOptions>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct TimetableStateSync {
    pub connection_id: Uuid,
    pub workspace: TimetableBlockWorkspace,
    pub users: Vec<TimetableParticipant>,
    pub reservations: Vec<TimetableReservation>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct TimetablePatch {
    pub timetable_id: Uuid,
    pub origin_connection_id: Option<Uuid>,
    pub mutation_id: Uuid,
    pub upserted_blocks: Vec<TimetableBlock>,
    pub removed_block_ids: Vec<Uuid>,
    pub ordinary_demands: Vec<TimetableOrdinaryDemand>,
    pub synchronized_demands: Vec<TimetableSynchronizedDemand>,
    pub summary: TimetableBlockSummary,
    pub source_issues: Vec<TimetableSourceIssue>,
    pub total_block_count: Option<i64>,
    pub timetable: Option<Timetable>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lookup_changes: Option<TimetableLookupChanges>,
}

/// Recipient-specific lookup membership/metadata changes caused by a normal block mutation.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct TimetableLookupChanges {
    pub upserted_staff: Vec<TimetableBlockWorkspaceStaff>,
    pub removed_staff_ids: Vec<Uuid>,
    pub upserted_homerooms: Vec<TimetableBlockWorkspaceHomeroom>,
    pub removed_homeroom_ids: Vec<Uuid>,
    pub upserted_rooms: Vec<TimetableBlockWorkspaceRoom>,
    pub removed_room_ids: Vec<Uuid>,
    pub upserted_bell_periods: Vec<school_academic_core::models::BellSchedulePeriod>,
    pub removed_bell_period_ids: Vec<Uuid>,
    pub upserted_offering_subject_groups: Vec<TimetableOfferingSubjectGroup>,
    pub removed_subject_group_offering_ids: Vec<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub staff_order: Option<Vec<Uuid>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub homeroom_order: Option<Vec<Uuid>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room_order: Option<Vec<Uuid>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bell_period_order: Option<Vec<Uuid>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject_group_offering_order: Option<Vec<Uuid>>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(tag = "type", content = "payload")]
pub enum TimetableServerMessage {
    TimetableStateSync(TimetableStateSync),
    TimetablePresenceChanged(TimetableParticipant),
    TimetablePresenceLeft {
        #[serde(rename = "connectionId")]
        connection_id: Uuid,
    },
    TimetableCommandResult(TimetableCommandResult),
    TimetableReservationsChanged {
        reservations: Vec<TimetableReservation>,
    },
    TimetableAvailability(TimetableAvailability),
    TimetablePatched(TimetablePatch),
    TimetableReconcileRequired {
        reason: String,
    },
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct TimetableEventEnvelope {
    pub epoch: Uuid,
    /// Snapshot sequence for state sync; committed journal sequence for patches.
    pub seq: u64,
    #[serde(flatten)]
    pub event: TimetableServerMessage,
}

/// Optional HTTP ownership proof. Actor/session identity always comes from authentication.
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TimetableMutationIdentity {
    pub connection_id: Uuid,
    pub lease_token: Uuid,
    pub mutation_id: Uuid,
}
