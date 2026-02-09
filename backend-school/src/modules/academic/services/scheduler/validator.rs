use super::types::*;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

pub struct ConstraintValidator {
    // Locked slots
    locked_slots: HashMap<String, LockedSlotInfo>, // key -> info
    
    // Instructor preferences
    instructor_prefs: HashMap<Uuid, InstructorPrefData>,
    
    // Periods by day
    periods_by_day: HashMap<String, Vec<PeriodInfo>>,
    
    // Rooms info
    rooms: HashMap<Uuid, RoomInfo>,
}

#[derive(Debug, Clone)]
struct LockedSlotInfo {
    pub subject_id: Uuid,
    pub classroom_ids: Vec<Uuid>, // Empty = all
    pub scope_type: String,
}

impl ConstraintValidator {
    pub fn new(
        locked_slots: Vec<LockedSlotData>,
        instructor_prefs: HashMap<Uuid, InstructorPrefData>,
        periods: Vec<PeriodInfo>,
        rooms: HashMap<Uuid, RoomInfo>,
    ) -> Self {
        let mut locked_map = HashMap::new();
        
        for locked in locked_slots {
            for period_id in &locked.period_ids {
                let key = format!("{}__{}", locked.day, period_id);
                locked_map.insert(
                    key,
                    LockedSlotInfo {
                        subject_id: locked.subject_id,
                        classroom_ids: locked.classroom_ids.clone(),
                        scope_type: locked.scope_type.clone(),
                    },
                );
            }
        }
        
        let mut periods_by_day = HashMap::new();
        for day in &["MON", "TUE", "WED", "THU", "FRI"] {
            periods_by_day.insert(day.to_string(), periods.clone());
        }
        
        Self {
            locked_slots: locked_map,
            instructor_prefs,
            periods_by_day,
            rooms,
        }
    }
    
    /// Check all hard constraints for an assignment
    pub fn can_assign(
        &self,
        course: &CourseToSchedule,
        time_slot: &TimeSlot,
        room_id: Option<Uuid>,
        state: &ScheduleState,
    ) -> Result<(), Conflict> {
        let slot_key = time_slot.key();
        
        // HC-1: Classroom conflict
        if state.is_classroom_slot_occupied(course.classroom_id, &slot_key) {
            return Err(Conflict {
                conflict_type: ConflictType::ClassroomOccupied,
                message: format!(
                    "Classroom {} is occupied at {}",
                    course.classroom_name, slot_key
                ),
            });
        }
        
        // HC-2: Instructor conflict
        if let Some(instructor_id) = course.instructor_id {
            if state.is_instructor_slot_occupied(instructor_id, &slot_key) {
                return Err(Conflict {
                    conflict_type: ConflictType::InstructorOccupied,
                    message: format!(
                        "Instructor {} is already teaching at {}",
                        course.instructor_name.as_deref().unwrap_or("Unknown"),
                        slot_key
                    ),
                });
            }
        }
        
        // HC-3: Room conflict
        if let Some(rid) = room_id {
            // Check occupancy
            if state.is_room_slot_occupied(rid, &slot_key) {
                return Err(Conflict {
                    conflict_type: ConflictType::RoomOccupied,
                    message: format!("Room is occupied at {}", slot_key),
                });
            }
            
            // HC-4: Room Type Compatibility
            if let Some(req_type) = &course.required_room_type {
                if let Some(room) = self.rooms.get(&rid) {
                    let room_type = room.room_type.as_deref().unwrap_or("STANDARD");
                    // Compare types (Exact match for now, could be improved)
                    // If required type is STANDARD, usually any room is OK? Or strict?
                    // Let's assume strict: If subject requires LAB_SCIENCE, Room MUST be LAB_SCIENCE.
                    if room_type != req_type {
                         return Err(Conflict {
                            conflict_type: ConflictType::RoomOccupied,
                            message: format!("Room type mismatch. Needed: {}, Room is: {}", req_type, room_type),
                         });
                    }
                }
            }
        }
        
        // HC-6: Instructor unavailability (hard)
        if let Some(instructor_id) = course.instructor_id {
            if let Some(prefs) = self.instructor_prefs.get(&instructor_id) {
                if prefs.hard_unavailable.contains(&slot_key) {
                    return Err(Conflict {
                        conflict_type: ConflictType::InstructorUnavailable,
                        message: format!(
                            "Instructor is unavailable at {}",
                            slot_key
                        ),
                    });
                }
            }
        }
        
        // HC-9: Locked slot check
        if let Some(locked_info) = self.locked_slots.get(&slot_key) {
            // Check if this slot is locked for a different subject
            if locked_info.subject_id != course.subject_id {
                return Err(Conflict {
                    conflict_type: ConflictType::LockedSlot,
                    message: format!(
                        "Slot {} is locked for another subject",
                        slot_key
                    ),
                });
            }
            
            // Check if classroom is in scope
            if !locked_info.classroom_ids.is_empty()
                && !locked_info.classroom_ids.contains(&course.classroom_id)
            {
                return Err(Conflict {
                    conflict_type: ConflictType::LockedSlot,
                    message: format!(
                        "Slot {} is locked for different classrooms",
                        slot_key
                    ),
                });
            }
        }
        
        Ok(())
    }
    
    /// Validate consecutive period requirements after assignment
    pub fn validate_consecutive(
        &self,
        course: &CourseToSchedule,
        assignments: &[Assignment],
    ) -> Result<(), Conflict> {
        // If no consecutive requirement, OK
        if course.min_consecutive <= 1 {
            return Ok(());
        }
        
        // Group by day
        let mut by_day: HashMap<String, Vec<i32>> = HashMap::new();
        for assign in assignments {
            by_day
                .entry(assign.time_slot.day.clone())
                .or_insert_with(Vec::new)
                .push(assign.time_slot.period_order);
        }
        
        // Check each day
        for (day, mut periods) in by_day {
            periods.sort();
            let count = periods.len() as i32;
            
            // If only 1 period
            if count == 1 {
                if !course.allow_single_period {
                    return Err(Conflict {
                        conflict_type: ConflictType::InvalidConsecutive,
                        message: format!(
                            "Subject {} requires at least {} consecutive periods on {}, got 1",
                            course.subject_code, course.min_consecutive, day
                        ),
                    });
                }
                continue; // OK - single period allowed
            }
            
            // If 2+ periods, check if consecutive
            if !Self::is_consecutive(&periods) {
                return Err(Conflict {
                    conflict_type: ConflictType::InvalidConsecutive,
                    message: format!(
                        "Subject {} periods on {} must be consecutive, got {:?}",
                        course.subject_code, day, periods
                    ),
                });
            }
            
            // Check min/max
            if count < course.min_consecutive {
                return Err(Conflict {
                    conflict_type: ConflictType::InvalidConsecutive,
                    message: format!(
                        "Subject {} requires at least {} consecutive periods on {}, got {}",
                        course.subject_code, course.min_consecutive, day, count
                    ),
                });
            }
            
            if count > course.max_consecutive {
                return Err(Conflict {
                    conflict_type: ConflictType::InvalidConsecutive,
                    message: format!(
                        "Subject {} allows max {} consecutive periods on {}, got {}",
                        course.subject_code, course.max_consecutive, day, count
                    ),
                });
            }
        }
        
        Ok(())
    }
    
    fn is_consecutive(periods: &[i32]) -> bool {
        if periods.len() <= 1 {
            return true;
        }
        
        for i in 1..periods.len() {
            if periods[i] != periods[i - 1] + 1 {
                return false; // Gap found
            }
        }
        
        true
    }
    
    /// Check if instructor has exceeded daily load
    pub fn check_instructor_daily_load(
        &self,
        instructor_id: Uuid,
        day: &str,
        state: &ScheduleState,
    ) -> bool {
        if let Some(prefs) = self.instructor_prefs.get(&instructor_id) {
            // Count periods for this instructor on this day
            let count = state
                .assignments
                .iter()
                .filter(|a| {
                    a.instructor_id == Some(instructor_id) && a.time_slot.day == day
                })
                .count() as i32;
            
            if count >= prefs.max_periods_per_day {
                return false; // Exceeded
            }
        }
        
        true // OK
    }
}

// Helper struct for locked slot data
#[derive(Debug, Clone)]
pub struct LockedSlotData {
    pub subject_id: Uuid,
    pub day: String,
    pub period_ids: Vec<Uuid>,
    pub classroom_ids: Vec<Uuid>, // Empty = all
    pub scope_type: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_is_consecutive() {
        assert!(ConstraintValidator::is_consecutive(&[1, 2, 3]));
        assert!(ConstraintValidator::is_consecutive(&[5, 6]));
        assert!(ConstraintValidator::is_consecutive(&[1]));
        assert!(!ConstraintValidator::is_consecutive(&[1, 3]));
        assert!(!ConstraintValidator::is_consecutive(&[1, 2, 4]));
    }
}
