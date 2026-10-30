//! The draft collaboration wire contract. Client commands never carry actor identity.
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
#[cfg(feature = "api-docs")]
use utoipa::ToSchema;
use uuid::Uuid;

use super::timetable_block::{
    TimetableBlock, TimetableBlockPlacementCandidate, TimetableBlockPlacementSource,
    TimetableBlockSummary, TimetableBlockWorkspace, TimetableOrdinaryDemand,
    TimetableSynchronizedDemand,
};
use super::timetable_source::TimetableSourceIssue;
use super::timetable_version::TimetableVersion;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
pub enum DraftViewMode {
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
pub struct DraftContext {
    pub view_mode: DraftViewMode,
    pub view_id: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(tag = "kind", rename_all = "snake_case", rename_all_fields = "camelCase", deny_unknown_fields)]
pub enum DraftAnchor {
    Cell { day_of_week: String, bell_schedule_period_id: Uuid, u: f64, v: f64 },
    Block { block_id: Uuid, u: f64, v: f64 },
    Tray { source: TimetableBlockPlacementSource, u: f64, v: f64 },
}

impl DraftAnchor {
    pub fn has_valid_coordinates(&self) -> bool {
        let (u, v) = match self {
            Self::Cell { u, v, .. } | Self::Block { u, v, .. } | Self::Tray { u, v, .. } => (*u, *v),
        };
        u.is_finite() && v.is_finite() && (0.0..=1.0).contains(&u) && (0.0..=1.0).contains(&v)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DraftTarget {
    pub day_of_week: String,
    pub bell_schedule_period_id: Uuid,
    pub block_id: Option<Uuid>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum DraftReservationPurpose { Drag, Edit }

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DraftPresenceUpdate {
    pub context: DraftContext,
    pub anchor: Option<DraftAnchor>,
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DraftClaim {
    pub request_id: Uuid,
    pub source: TimetableBlockPlacementSource,
    pub purpose: DraftReservationPurpose,
    /// Private configuration, used only for the server's resource footprint.
    pub candidate: Option<TimetableBlockPlacementCandidate>,
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DraftTargetUpdate {
    pub request_id: Uuid,
    pub lease_token: Uuid,
    pub target: Option<DraftTarget>,
    pub candidate: Option<TimetableBlockPlacementCandidate>,
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DraftLeaseCommand {
    pub request_id: Uuid,
    pub lease_token: Uuid,
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DraftReplay {
    pub epoch: Uuid,
    pub after_seq: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(tag = "type", content = "payload", deny_unknown_fields)]
pub enum DraftClientMessage {
    ClientPresenceUpdate(DraftPresenceUpdate),
    ClientClaim(DraftClaim),
    ClientTarget(DraftTargetUpdate),
    ClientRenew(DraftLeaseCommand),
    ClientRelease(DraftLeaseCommand),
    ClientReplay(DraftReplay),
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct DraftConnectionPresence {
    pub connection_id: Uuid,
    pub context: Option<DraftContext>,
    pub anchor: Option<DraftAnchor>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct DraftParticipant {
    pub user_id: Uuid,
    pub name: String,
    pub color: String,
    pub profile_image_file_id: Option<Uuid>,
    pub connections: Vec<DraftConnectionPresence>,
}

/// Public lease identity is distinct from its private ownership token.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct DraftReservation {
    pub reservation_id: Uuid,
    pub owner_connection_id: Uuid,
    pub purpose: DraftReservationPurpose,
    pub source: TimetableBlockPlacementSource,
    pub target: Option<DraftTarget>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum DraftCommandKind { Claim, Target, Renew, Release }

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct DraftCommandResult {
    pub request_id: Uuid,
    pub command: DraftCommandKind,
    pub accepted: bool,
    pub code: Option<String>,
    pub source: Option<TimetableBlockPlacementSource>,
    pub target: Option<DraftTarget>,
    pub reservation_id: Option<Uuid>,
    pub lease_token: Option<Uuid>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct DraftBlockedCell {
    pub day_of_week: String,
    pub bell_schedule_period_id: Uuid,
    pub reservation_id: Uuid,
    pub owner_connection_id: Uuid,
    pub purpose: DraftReservationPurpose,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct DraftAvailability {
    pub source: TimetableBlockPlacementSource,
    pub context: Option<DraftContext>,
    pub blocked_cells: Vec<DraftBlockedCell>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct DraftStateSync {
    pub connection_id: Uuid,
    pub workspace: TimetableBlockWorkspace,
    pub users: Vec<DraftParticipant>,
    pub reservations: Vec<DraftReservation>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct DraftPatch {
    pub timetable_version_id: Uuid,
    pub origin_connection_id: Option<Uuid>,
    pub mutation_id: Uuid,
    pub upserted_blocks: Vec<TimetableBlock>,
    pub removed_block_ids: Vec<Uuid>,
    pub ordinary_demands: Vec<TimetableOrdinaryDemand>,
    pub synchronized_demands: Vec<TimetableSynchronizedDemand>,
    pub summary: TimetableBlockSummary,
    pub source_issues: Vec<TimetableSourceIssue>,
    pub total_draft_block_count: Option<i64>,
    pub version: Option<TimetableVersion>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(tag = "type", content = "payload")]
pub enum DraftServerMessage {
    DraftStateSync(DraftStateSync),
    DraftPresenceChanged(DraftParticipant),
    DraftPresenceLeft { #[serde(rename = "connectionId")] connection_id: Uuid },
    DraftCommandResult(DraftCommandResult),
    DraftReservationsChanged { reservations: Vec<DraftReservation> },
    DraftAvailability(DraftAvailability),
    DraftPatched(DraftPatch),
    DraftReconcileRequired { reason: String },
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct DraftEventEnvelope {
    pub epoch: Uuid,
    /// Snapshot sequence for state sync; committed journal sequence for patches.
    pub seq: u64,
    #[serde(flatten)]
    pub event: DraftServerMessage,
}

/// Optional HTTP ownership proof. Actor/session identity always comes from authentication.
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DraftMutationIdentity {
    pub connection_id: Uuid,
    pub lease_token: Uuid,
    pub mutation_id: Uuid,
}
