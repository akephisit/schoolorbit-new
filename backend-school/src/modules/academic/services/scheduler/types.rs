use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

// ==================== Time Slot ====================

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TimeSlot {
    pub day: String, // "MON", "TUE", etc.
    pub period_id: Uuid,
    pub period_order: i32, // For consecutive checking
}

impl TimeSlot {
    pub fn key(&self) -> String {
        format!("{}__{}", self.day, self.period_id)
    }
}

// ==================== Course to Schedule ====================

#[derive(Debug, Clone)]
pub struct CourseToSchedule {
    pub id: Uuid,
    pub classroom_course_id: Uuid,
    pub classroom_id: Uuid,
    pub classroom_name: String,
    pub subject_id: Uuid,
    pub subject_code: String,
    pub subject_name: String,
    pub instructor_id: Option<Uuid>,
    pub instructor_name: Option<String>,

    // Scheduling requirements
    pub periods_needed: i32,
    pub periods_remaining: i32,

    // Consecutive requirements
    pub min_consecutive: i32,
    pub max_consecutive: i32,
    pub allow_single_period: bool,

    // Room requirements
    pub fixed_room_id: Option<Uuid>, // From instructor_room_assignments

    // Flexible constraints (new)
    pub allowed_period_ids: Option<Vec<Uuid>>, // NULL = all periods allowed
    pub allowed_days: Option<Vec<String>>,     // NULL = all days allowed

    // Phase B: classroom_course-level constraints
    /// คาบที่ห้ามจัดวิชานี้ในห้องนี้ (cc-level, ไม่รวมจากครู — scheduler รวมเอง)
    pub cc_hard_unavailable: HashSet<String>, // key: "DAY__period_id"
    /// true → วันเดียวกันห้ามมีรหัสวิชาซ้ำ (default true)
    pub same_day_unique: bool,
    /// รูปแบบการจัดคาบ — None = fallback [1; periods_needed]
    pub consecutive_pattern: Option<Vec<i32>>,

    // Phase D: room hierarchy
    /// ห้องที่ classroom_course นี้ใช้สอน — เรียงตาม rank (index 0 = ลองก่อน)
    /// scheduler ใช้ตัวแรกเป็นค่า default; ถ้าเต็มลองตัวถัดไป (TODO: iteration)
    pub preferred_rooms: Vec<RoomPref>,

    // Phase C: ถ้าเป็น activity slot (indep) แทนที่จะเป็น classroom_course
    /// Some = นี่คือ activity slot — insert เป็น ACTIVITY entry, ไม่ใช่ COURSE
    pub activity_slot_id: Option<Uuid>,
}

/// Phase D: Room preference entry — used as cc-level preferred room list
#[derive(Debug, Clone)]
pub struct RoomPref {
    pub room_id: Uuid,
    pub is_required: bool,
}

// ==================== Assignment ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Assignment {
    pub id: Uuid,
    pub classroom_course_id: Uuid,
    pub classroom_id: Uuid,
    pub subject_id: Uuid,
    pub instructor_id: Option<Uuid>,
    pub time_slot: TimeSlot,
    pub room_id: Option<Uuid>,
    pub is_locked: bool, // From pre-assigned slots
    /// Phase C: ถ้า Some = ACTIVITY entry (insert ต่างจาก COURSE)
    pub activity_slot_id: Option<Uuid>,
}

impl Assignment {
    pub fn new(
        course: &CourseToSchedule,
        time_slot: TimeSlot,
        room_id: Option<Uuid>,
        is_locked: bool,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            classroom_course_id: course.classroom_course_id,
            classroom_id: course.classroom_id,
            subject_id: course.subject_id,
            instructor_id: course.instructor_id,
            time_slot,
            room_id,
            activity_slot_id: course.activity_slot_id,
            is_locked,
        }
    }
}

// ==================== Schedule State ====================

#[derive(Debug, Clone)]
pub struct ScheduleState {
    pub assignments: Vec<Assignment>,

    // Fast lookup maps
    pub classroom_slots: HashMap<String, HashSet<String>>, // classroom_id -> set of slot keys
    pub instructor_slots: HashMap<Uuid, HashSet<String>>,  // instructor_id -> set of slot keys
    pub room_slots: HashMap<Uuid, HashSet<String>>,        // room_id -> set of slot keys

    // Per-course assignments
    pub course_assignments: HashMap<Uuid, Vec<Assignment>>, // course_id -> assignments
}

impl ScheduleState {
    pub fn new() -> Self {
        Self {
            assignments: Vec::new(),
            classroom_slots: HashMap::new(),
            instructor_slots: HashMap::new(),
            room_slots: HashMap::new(),
            course_assignments: HashMap::new(),
        }
    }

    pub fn add_assignment(&mut self, assignment: Assignment) {
        let slot_key = assignment.time_slot.key();

        // Track classroom usage
        self.classroom_slots
            .entry(assignment.classroom_id.to_string())
            .or_default()
            .insert(slot_key.clone());

        // Track instructor usage
        if let Some(instructor_id) = assignment.instructor_id {
            self.instructor_slots
                .entry(instructor_id)
                .or_default()
                .insert(slot_key.clone());
        }

        // Track room usage
        if let Some(room_id) = assignment.room_id {
            self.room_slots.entry(room_id).or_default().insert(slot_key);
        }

        // Track per-course
        self.course_assignments
            .entry(assignment.classroom_course_id)
            .or_default()
            .push(assignment.clone());

        self.assignments.push(assignment);
    }

    pub fn remove_last_assignment(&mut self) {
        if let Some(assignment) = self.assignments.pop() {
            let slot_key = assignment.time_slot.key();

            // Remove from classroom
            if let Some(slots) = self
                .classroom_slots
                .get_mut(&assignment.classroom_id.to_string())
            {
                slots.remove(&slot_key);
            }

            // Remove from instructor
            if let Some(instructor_id) = assignment.instructor_id {
                if let Some(slots) = self.instructor_slots.get_mut(&instructor_id) {
                    slots.remove(&slot_key);
                }
            }

            // Remove from room
            if let Some(room_id) = assignment.room_id {
                if let Some(slots) = self.room_slots.get_mut(&room_id) {
                    slots.remove(&slot_key);
                }
            }

            // Remove from course
            if let Some(course_assigns) = self
                .course_assignments
                .get_mut(&assignment.classroom_course_id)
            {
                course_assigns.pop();
            }
        }
    }

    pub fn is_classroom_slot_occupied(&self, classroom_id: Uuid, slot_key: &str) -> bool {
        self.classroom_slots
            .get(&classroom_id.to_string())
            .map(|slots| slots.contains(slot_key))
            .unwrap_or(false)
    }

    pub fn is_instructor_slot_occupied(&self, instructor_id: Uuid, slot_key: &str) -> bool {
        self.instructor_slots
            .get(&instructor_id)
            .map(|slots| slots.contains(slot_key))
            .unwrap_or(false)
    }

    pub fn is_room_slot_occupied(&self, room_id: Uuid, slot_key: &str) -> bool {
        self.room_slots
            .get(&room_id)
            .map(|slots| slots.contains(slot_key))
            .unwrap_or(false)
    }

    pub fn get_course_assignments(&self, course_id: Uuid) -> Vec<Assignment> {
        self.course_assignments
            .get(&course_id)
            .cloned()
            .unwrap_or_default()
    }
}

// ==================== Conflict Types ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConflictType {
    ClassroomOccupied,
    InstructorOccupied,
    RoomOccupied,
    InstructorUnavailable,
    InvalidConsecutive,
    LockedSlot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conflict {
    pub conflict_type: ConflictType,
    pub message: String,
}

// ==================== Scheduling Configuration ====================

#[derive(Debug, Clone)]
pub struct SchedulerConfig {
    // Algorithm
    pub algorithm: SchedulingAlgorithm,
    pub max_iterations: u32,
    pub timeout_seconds: u32,

    // Soft constraints
    pub optimize_distribution: bool,
    pub optimize_consecutive_limit: bool,
    pub optimize_time_of_day: bool,
    pub balance_daily_load: bool,

    // Options
    pub allow_partial: bool,
    pub min_quality_score: f64,

    // Custom constraints
    pub allow_multiple_sessions_per_day: bool, // If false, forces spread days

    // Weights (for quality scoring)
    pub weight_distribution: f64, // 30%
    pub weight_consecutive: f64,  // 20%
    pub weight_time_of_day: f64,  // 15%
    pub weight_daily_load: f64,   // 10%
    pub weight_spacing: f64,      // 2%
}

#[derive(Debug, Clone, PartialEq)]
pub enum SchedulingAlgorithm {
    Greedy,
    Backtracking,
    Hybrid,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            algorithm: SchedulingAlgorithm::Backtracking,
            max_iterations: 10000,
            timeout_seconds: 300,

            optimize_distribution: true,
            optimize_consecutive_limit: true,
            optimize_time_of_day: true,
            balance_daily_load: true,

            allow_partial: false,
            min_quality_score: 70.0,

            allow_multiple_sessions_per_day: false, // Default = Force Spread Days

            weight_distribution: 30.0,
            weight_consecutive: 20.0,
            weight_time_of_day: 15.0,
            weight_daily_load: 10.0,
            weight_spacing: 2.0,
        }
    }
}

// ==================== Scheduling Result ====================

#[derive(Debug, Clone, Serialize)]
pub struct SchedulingResult {
    pub success: bool,
    pub quality_score: f64,
    pub assignments: Vec<Assignment>,
    pub scheduled_courses: usize,
    pub total_courses: usize,
    pub failed_courses: Vec<FailedCourse>,
    pub duration_ms: u128,
    pub iterations: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailedCourse {
    pub course_id: Uuid,
    pub subject_code: String,
    pub subject_name: String,
    pub classroom: String,
    pub reason: String,
}

// ==================== Instructor Preference Data ====================

#[derive(Debug, Clone)]
pub struct InstructorPrefData {
    pub hard_unavailable: HashSet<String>, // Set of slot keys
    pub max_periods_per_day: i32,
}

// ==================== Period Info ====================

#[derive(Debug, Clone)]
pub struct PeriodInfo {
    pub id: Uuid,
    pub order: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_course() -> CourseToSchedule {
        CourseToSchedule {
            id: Uuid::new_v4(),
            classroom_course_id: Uuid::new_v4(),
            classroom_id: Uuid::new_v4(),
            classroom_name: "ม.1/1".to_string(),
            subject_id: Uuid::new_v4(),
            subject_code: "MATH".to_string(),
            subject_name: "Mathematics".to_string(),
            instructor_id: Some(Uuid::new_v4()),
            instructor_name: Some("Teacher".to_string()),
            periods_needed: 1,
            periods_remaining: 1,
            min_consecutive: 1,
            max_consecutive: 1,
            allow_single_period: true,
            fixed_room_id: Some(Uuid::new_v4()),
            allowed_period_ids: None,
            allowed_days: None,
            cc_hard_unavailable: HashSet::new(),
            same_day_unique: true,
            consecutive_pattern: None,
            preferred_rooms: Vec::new(),
            activity_slot_id: None,
        }
    }

    #[test]
    fn time_slot_key_uses_day_and_period_id() {
        let period_id = Uuid::new_v4();
        let slot = TimeSlot {
            day: "MON".to_string(),
            period_id,
            period_order: 1,
        };

        assert_eq!(slot.key(), format!("MON__{period_id}"));
    }

    #[test]
    fn schedule_state_indexes_assignment_by_classroom_instructor_room_and_course() {
        let course = sample_course();
        let room_id = course.fixed_room_id;
        let instructor_id = course.instructor_id;
        let slot = TimeSlot {
            day: "MON".to_string(),
            period_id: Uuid::new_v4(),
            period_order: 1,
        };
        let slot_key = slot.key();
        let assignment = Assignment::new(&course, slot, room_id, false);
        let mut state = ScheduleState::new();

        state.add_assignment(assignment);

        assert!(state.is_classroom_slot_occupied(course.classroom_id, &slot_key));
        assert!(state.is_instructor_slot_occupied(instructor_id.unwrap(), &slot_key));
        assert!(state.is_room_slot_occupied(room_id.unwrap(), &slot_key));
        assert_eq!(
            state
                .get_course_assignments(course.classroom_course_id)
                .len(),
            1
        );
    }

    #[test]
    fn schedule_state_remove_last_assignment_clears_indexes() {
        let course = sample_course();
        let room_id = course.fixed_room_id.unwrap();
        let instructor_id = course.instructor_id.unwrap();
        let slot = TimeSlot {
            day: "MON".to_string(),
            period_id: Uuid::new_v4(),
            period_order: 1,
        };
        let slot_key = slot.key();
        let assignment = Assignment::new(&course, slot, Some(room_id), false);
        let mut state = ScheduleState::new();

        state.add_assignment(assignment);
        state.remove_last_assignment();

        assert!(!state.is_classroom_slot_occupied(course.classroom_id, &slot_key));
        assert!(!state.is_instructor_slot_occupied(instructor_id, &slot_key));
        assert!(!state.is_room_slot_occupied(room_id, &slot_key));
        assert!(state
            .get_course_assignments(course.classroom_course_id)
            .is_empty());
    }
}
