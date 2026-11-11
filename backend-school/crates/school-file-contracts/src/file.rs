use crate::platform_types::{FileLifecycleStatus, FilePurpose, FileVisibility};
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlatformFile {
    pub id: Uuid,
    pub owner_user_id: Option<Uuid>,
    pub purpose: FilePurpose,
    pub visibility: FileVisibility,
    pub lifecycle_status: FileLifecycleStatus,
    pub current_version: Option<u32>,
    pub display_filename: String,
    pub detected_mime_type: String,
    pub byte_size: i64,
}
