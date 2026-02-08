use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

// ==================== Time Slot ====================

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TimeSlot {
    pub day: String,      // "MON", "TUE", etc.
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
    pub required_room_type: Option<String>,
    pub fixed_room_id: Option<Uuid>, // From instructor_room_assignments
    
    // Time preferences
    pub preferred_time_of_day: Option<String>, // "MORNING", "AFTERNOON", "ANYTIME"
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
            .or_insert_with(HashSet::new)
            .insert(slot_key.clone());
        
        // Track instructor usage
        if let Some(instructor_id) = assignment.instructor_id {
            self.instructor_slots
                .entry(instructor_id)
                .or_insert_with(HashSet::new)
                .insert(slot_key.clone());
        }
        
        // Track room usage
        if let Some(room_id) = assignment.room_id {
            self.room_slots
                .entry(room_id)
                .or_insert_with(HashSet::new)
                .insert(slot_key);
        }
        
        // Track per-course
        self.course_assignments
            .entry(assignment.classroom_course_id)
            .or_insert_with(Vec::new)
            .push(assignment.clone());
        
        self.assignments.push(assignment);
    }
    
    pub fn remove_last_assignment(&mut self) {
        if let Some(assignment) = self.assignments.pop() {
            let slot_key = assignment.time_slot.key();
            
            // Remove from classroom
            if let Some(slots) = self.classroom_slots.get_mut(&assignment.classroom_id.to_string()) {
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
            if let Some(course_assigns) = self.course_assignments.get_mut(&assignment.classroom_course_id) {
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
    
    // Hard constraints
    pub enforce_period_requirements: bool,
    pub enforce_instructor_unavailability: bool,
    
    // Soft constraints
    pub optimize_distribution: bool,
    pub optimize_consecutive_limit: bool,
    pub optimize_time_of_day: bool,
    pub respect_preferences: bool,
    pub balance_daily_load: bool,
    
    // Options
    pub force_overwrite: bool,
    pub allow_partial: bool,
    pub min_quality_score: f64,
    
    // Weights (for quality scoring)
    pub weight_distribution: f64,      // 30%
    pub weight_consecutive: f64,        // 20%
    pub weight_time_of_day: f64,       // 15%
    pub weight_instructor_pref: f64,   // 15%
    pub weight_daily_load: f64,        // 10%
    pub weight_instructor_load: f64,   // 5%
    pub weight_avoid_edge: f64,        // 3%
    pub weight_spacing: f64,           // 2%
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
            
            enforce_period_requirements: true,
            enforce_instructor_unavailability: true,
            
            optimize_distribution: true,
            optimize_consecutive_limit: true,
            optimize_time_of_day: true,
            respect_preferences: true,
            balance_daily_load: true,
            
            force_overwrite: false,
            allow_partial: false,
            min_quality_score: 70.0,
            
            weight_distribution: 30.0,
            weight_consecutive: 20.0,
            weight_time_of_day: 15.0,
            weight_instructor_pref: 15.0,
            weight_daily_load: 10.0,
            weight_instructor_load: 5.0,
            weight_avoid_edge: 3.0,
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
    pub instructor_id: Uuid,
    pub hard_unavailable: HashSet<String>, // Set of slot keys
    pub preferred_slots: HashSet<String>,  // Set of slot keys
    pub max_periods_per_day: i32,
}

// ==================== Period Info ====================

#[derive(Debug, Clone)]
pub struct PeriodInfo {
    pub id: Uuid,
    pub order: i32,
    pub name: String,
    pub start_time: String,
    pub end_time: String,
}

// ==================== Available Slots ====================

pub type AvailableSlots = Vec<TimeSlot>;
