use crate::modules::academic::models::timetable::{ConflictInfo, TimetableEntry, TimetableQuery};
use serde::Serialize;
use uuid::Uuid;

/// ผลของ create_batch_entries — handler ใช้ semester_id broadcast WS event
pub struct BatchCreateOutcome {
    pub inserted_count: i64,
    pub skipped: Vec<BatchSkippedCell>,
    pub blocked: Vec<BatchBlockedCell>,
    pub deleted: Vec<BatchDeletedEntry>,
    pub excluded_instructors: Vec<BatchExcludedInstructor>,
    pub semester_id: Uuid,
}

#[derive(Serialize)]
pub struct BatchSkippedCell {
    pub classroom_id: Option<Uuid>,
    pub classroom_name: Option<String>,
    pub day_of_week: String,
    pub period_id: Uuid,
    pub period_name: Option<String>,
    pub reason: String,
    pub message: String,
}

#[derive(Serialize)]
pub struct BatchBlockedCell {
    pub classroom_id: Uuid,
    pub classroom_name: Option<String>,
    pub day_of_week: String,
    pub period_id: Uuid,
    pub period_name: Option<String>,
    pub reason: String,
    pub message: String,
}

#[derive(Serialize)]
pub struct BatchDeletedEntry {
    pub id: Uuid,
    pub classroom_name: Option<String>,
    pub day_of_week: String,
    pub period_id: Uuid,
    pub period_name: Option<String>,
    pub title: String,
    pub entry_type: String,
    pub instructor_names: Vec<String>,
}

#[derive(Serialize)]
pub struct BatchInstructorConflict {
    pub day_of_week: String,
    pub period_id: Uuid,
    pub period_name: Option<String>,
    pub existing_title: String,
}

#[derive(Serialize)]
pub struct BatchExcludedInstructor {
    pub instructor_id: Uuid,
    pub instructor_name: String,
    pub conflicting_at: Vec<BatchInstructorConflict>,
}

/// ข้อมูลสำหรับ DropRejected broadcast เมื่อ swap fail
pub struct SwapConflictInfo {
    pub reason: String,
    pub semester_id: Uuid,
    pub a_id: Uuid,
    pub a_day: String,
    pub a_period: Uuid,
    pub a_room: Option<Uuid>,
    pub b_id: Uuid,
    pub b_day: String,
    pub b_period: Uuid,
}

pub enum SwapOutcome {
    Swapped { semester_id: Uuid },
    Conflict(SwapConflictInfo),
}

/// Outcome ของ create_entry — service ตัดสินใจ logic, handler ตัดสินใจ HTTP/WS broadcast
pub enum CreateEntryOutcome {
    Created(Box<TimetableEntry>),
    Conflict(Vec<ConflictInfo>),
}

/// Outcome ของ update_entry — handler ใช้ existing_entry เพื่อ broadcast DropRejected/EntryUpdated
pub enum UpdateEntryOutcome {
    Updated {
        updated: Box<TimetableEntry>,
        existing: Box<TimetableEntry>,
    },
    Conflict {
        conflicts: Vec<ConflictInfo>,
        existing: Box<TimetableEntry>,
    },
}

pub(super) type SwapEntryRow = (
    Uuid,
    String,
    Uuid,
    Option<Uuid>,
    Option<Uuid>,
    Uuid,
    Option<Uuid>,
);
pub(super) type MoveSourceRow = (String, Uuid, Option<Uuid>, Option<Uuid>, Uuid, Uuid);
pub(super) type MoveEntryRow = (Uuid, String, Uuid, Option<Uuid>, Option<Uuid>);
pub(super) type MoveCellKey = (String, Uuid);
pub(super) type MoveEntryRefs<'a> = Vec<&'a MoveEntryRow>;

/// Filter สำหรับ list_entries — รวม use case ทุกมุมมอง:
/// - Staff/Admin: ส่ง classroom_id หรือ semester_id (ไม่ filter user)
/// - Student: ส่ง student_id → filter ตาม student_class_enrollments
/// - Teacher: ส่ง instructor_id → filter ตาม timetable_entry_instructors
/// - Parent → ดูลูก: caller verify parent-child link แล้วส่ง student_id
#[derive(Debug, Default, Clone)]
pub struct TimetableFilter {
    pub classroom_id: Option<Uuid>,
    pub student_id: Option<Uuid>,
    pub instructor_id: Option<Uuid>,
    pub room_id: Option<Uuid>,
    pub academic_semester_id: Option<Uuid>,
    pub day_of_week: Option<String>,
    pub entry_type: Option<String>,
    /// ใช้กับ instructor_id: รวม cell ที่ instructor อยู่ใน team แต่ไม่ใช่ผู้สอนหลักของ cell
    pub include_team_ghosts: bool,
}

impl From<TimetableQuery> for TimetableFilter {
    fn from(q: TimetableQuery) -> Self {
        Self {
            classroom_id: q.classroom_id,
            student_id: q.student_id,
            instructor_id: q.instructor_id,
            room_id: q.room_id,
            academic_semester_id: q.academic_semester_id,
            day_of_week: q.day_of_week,
            entry_type: q.entry_type,
            include_team_ghosts: q.include_team_ghosts.unwrap_or(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_query() -> TimetableQuery {
        TimetableQuery {
            classroom_id: None,
            student_id: None,
            instructor_id: None,
            room_id: None,
            academic_semester_id: None,
            day_of_week: None,
            entry_type: None,
            include_team_ghosts: None,
        }
    }

    #[test]
    fn timetable_filter_defaults_include_team_ghosts_to_false() {
        let filter = TimetableFilter::from(empty_query());

        assert!(!filter.include_team_ghosts);
    }

    #[test]
    fn timetable_filter_preserves_query_fields() {
        let classroom_id = Uuid::new_v4();
        let query = TimetableQuery {
            classroom_id: Some(classroom_id),
            day_of_week: Some("MON".to_string()),
            include_team_ghosts: Some(true),
            ..empty_query()
        };
        let filter = TimetableFilter::from(query);

        assert_eq!(filter.classroom_id, Some(classroom_id));
        assert_eq!(filter.day_of_week.as_deref(), Some("MON"));
        assert!(filter.include_team_ghosts);
    }
}
