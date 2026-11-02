use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
pub struct FileTransferFile {
    pub name: String,
    pub size: u64,
    pub mime_type: String,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
pub struct FileTransferDevice {
    pub id: Uuid,
    pub user_id: Uuid,
    pub display_name: String,
    pub label: String,
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
pub enum FileTransferPairStatus {
    Pending,
    Accepted,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
pub struct FileTransferPair {
    pub id: Uuid,
    pub initiator: FileTransferDevice,
    pub recipient: FileTransferDevice,
    pub files: Vec<FileTransferFile>,
    pub client_request_id: Uuid,
    pub status: FileTransferPairStatus,
    pub expires_at: DateTime<Utc>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
pub struct FileTransferLimits {
    pub max_outgoing_transfers: u8,
    pub max_files_per_batch: u8,
    pub max_file_bytes: u64,
    pub max_batch_bytes: u64,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
pub struct FileTransferSnapshot {
    pub protocol_version: u8,
    pub limits: FileTransferLimits,
    pub device: FileTransferDevice,
    pub pairing_code: String,
    pub expires_at: DateTime<Utc>,
    pub peers: Vec<FileTransferDevice>,
    pub pairs: Vec<FileTransferPair>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
pub struct RegisterFileTransferDevice {
    pub label: String,
    pub protocol_version: Option<u8>,
}

#[derive(Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
pub enum FileTransferTarget {
    Device {
        #[serde(rename = "deviceId")]
        device_id: Uuid,
    },
    Code {
        code: String,
    },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
pub struct CreateFileTransferPair {
    pub device_id: Uuid,
    pub target: FileTransferTarget,
    pub files: Vec<FileTransferFile>,
    pub client_request_id: Uuid,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
pub struct FileTransferDeviceRequest {
    pub device_id: Uuid,
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
pub enum FileTransferSignalType {
    Offer,
    Answer,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
pub struct SendFileTransferSignal {
    pub device_id: Uuid,
    #[serde(rename = "type")]
    pub signal_type: FileTransferSignalType,
    pub sdp: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
pub struct FileTransferSignal {
    pub pair_id: Uuid,
    #[serde(rename = "type")]
    pub signal_type: FileTransferSignalType,
    pub sdp: String,
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
pub enum FileTransferCloseReason {
    Declined,
    Cancelled,
    Expired,
    Disconnected,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
pub struct FileTransferPairClosed {
    pub pair_id: Uuid,
    pub reason: FileTransferCloseReason,
}

#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
pub struct FileTransferEmpty {}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
pub enum FileTransferEvent {
    Ready(FileTransferSnapshot),
    PresenceChanged(FileTransferEmpty),
    PairChanged(FileTransferPair),
    PairClosed(FileTransferPairClosed),
    Signal(FileTransferSignal),
    SessionInvalid(FileTransferEmpty),
    SessionUnavailable(FileTransferEmpty),
}
