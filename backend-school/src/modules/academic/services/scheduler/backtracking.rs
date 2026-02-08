use super::types::*;
use super::validator::ConstraintValidator;
use super::quality::QualityScorer;
use std::time::Instant;
use uuid::Uuid;

pub struct BacktrackingScheduler {
    validator: ConstraintValidator,
    scorer: QualityScorer,
    config: SchedulerConfig,
}

impl BacktrackingScheduler {
    pub fn new(
        validator: ConstraintValidator,
        config: SchedulerConfig,
    ) -> Self {
        let scorer = QualityScorer::new(config.clone());
        
        Self {
            validator,
            scorer,
            config,
        }
    }
    
    /// Run backtracking algorithm
    pub fn schedule(
        &self,
        courses: &mut [CourseToSchedule],
        available_slots: &[TimeSlot],
    ) -> SchedulingResult {
        let start_time = Instant::now();
        
        // Sort courses by difficulty (hardest first)
        courses.sort_by_key(|c| self.calculate_difficulty(c));
        courses.reverse(); // Descending order
        
        let mut state = ScheduleState::new();
        let mut best_state: Option<ScheduleState> = None;
        let mut best_score = 0.0;
        let mut iterations = 0;
        let mut failed_courses = Vec::new();
        
        // Try to schedule
        let success = self.backtrack(
            courses,
            0,
            available_slots,
            &mut state,
            &mut best_state,
            &mut best_score,
            &mut iterations,
            &start_time,
        );
        
        let duration_ms = start_time.elapsed().as_millis();
        
        // Use best state if found
        let final_state = if let Some(best) = best_state {
            best
        } else if success {
            state
        } else {
            // Partial schedule
            state
        };
        
        // Calculate final quality
        let quality_score = self.scorer.calculate_quality(&final_state, courses);
        
        // Find failed courses
        for course in courses.iter() {
            let assignments = final_state.get_course_assignments(course.id);
            if assignments.len() < course.periods_needed as usize {
                failed_courses.push(FailedCourse {
                    course_id: course.id,
                    subject_code: course.subject_code.clone(),
                    subject_name: course.subject_name.clone(),
                    classroom: course.classroom_name.clone(),
                    reason: format!(
                        "Only scheduled {}/{} periods",
                        assignments.len(),
                        course.periods_needed
                    ),
                });
            }
        }
        
        let scheduled_count = courses
            .iter()
            .filter(|c| {
                let assigns = final_state.get_course_assignments(c.id);
                assigns.len() == c.periods_needed as usize
            })
            .count();
        
        SchedulingResult {
            success: failed_courses.is_empty(),
            quality_score,
            assignments: final_state.assignments,
            scheduled_courses: scheduled_count,
            total_courses: courses.len(),
            failed_courses,
            duration_ms,
            iterations,
        }
    }
    
    /// Backtracking recursive function
    fn backtrack(
        &self,
        courses: &[CourseToSchedule],
        course_idx: usize,
        available_slots: &[TimeSlot],
        state: &mut ScheduleState,
        best_state: &mut Option<ScheduleState>,
        best_score: &mut f64,
        iterations: &mut u32,
        start_time: &Instant,
    ) -> bool {
        *iterations += 1;
        
        // Check timeout
        if start_time.elapsed().as_secs() >= self.config.timeout_seconds as u64 {
            return false; // Timeout
        }
        
        // Check max iterations
        if *iterations >= self.config.max_iterations {
            return false;
        }
        
        // Base case: all courses scheduled
        if course_idx >= courses.len() {
            // Calculate quality
            let quality = self.scorer.calculate_quality(state, courses);
            
            // Update best if better
            if quality > *best_score {
                *best_score = quality;
                *best_state = Some(state.clone());
            }
            
            // Accept if meets minimum quality
            return quality >= self.config.min_quality_score;
        }
        
        let course = &courses[course_idx];
        let periods_needed = course.periods_remaining;
        
        // Try to schedule this course
        let success = self.schedule_course(
            course,
            periods_needed,
            available_slots,
            state,
            best_state,
            best_score,
            iterations,
            start_time,
        );
        
        if success {
            // Continue to next course
            if self.backtrack(
                courses,
                course_idx + 1,
                available_slots,
                state,
                best_state,
                best_score,
                iterations,
                start_time,
            ) {
                return true; // Found acceptable solution
            }
        }
        
        // Backtrack: remove assignments for this course
        let course_assignments = state.get_course_assignments(course.id).len();
        for _ in 0..course_assignments {
            state.remove_last_assignment();
        }
        
        // If partial scheduling allowed, continue anyway
        if self.config.allow_partial {
            return self.backtrack(
                courses,
                course_idx + 1,
                available_slots,
                state,
                best_state,
                best_score,
                iterations,
                start_time,
            );
        }
        
        false
    }
    
    /// Try to schedule a single course
    fn schedule_course(
        &self,
        course: &CourseToSchedule,
        periods_needed: i32,
        available_slots: &[TimeSlot],
        state: &mut ScheduleState,
        _best_state: &mut Option<ScheduleState>,
        _best_score: &mut f64,
        _iterations: &mut u32,
        _start_time: &Instant,
    ) -> bool {
        // Strategy based on consecutive requirements
        if course.min_consecutive > 1 {
            self.schedule_with_consecutive(course, periods_needed, available_slots, state)
        } else {
            self.schedule_without_consecutive(course, periods_needed, available_slots, state)
        }
    }
    
    /// Schedule course with consecutive requirements
    fn schedule_with_consecutive(
        &self,
        course: &CourseToSchedule,
        periods_needed: i32,
        available_slots: &[TimeSlot],
        state: &mut ScheduleState,
    ) -> bool {
        let mut remaining = periods_needed;
        
        while remaining > 0 {
            let chunk_size = if remaining >= course.min_consecutive {
                course.max_consecutive.min(remaining)
            } else if course.allow_single_period && remaining == 1 {
                1
            } else {
                return false; // Cannot schedule remainder
            };
            
            // Find consecutive slots
            if let Some(slots) = self.find_consecutive_slots(
                course,
                chunk_size,
                available_slots,
                state,
            ) {
                // Assign these slots
                for slot in slots {
                    let room_id = self.determine_room_id(course);
                    let assignment = Assignment::new(course, slot, room_id, false);
                    state.add_assignment(assignment);
                }
                remaining -= chunk_size;
            } else {
                return false; // Cannot find consecutive slots
            }
        }
        
        // Validate consecutive after all assignments
        let assignments = state.get_course_assignments(course.id);
        if let Err(_) = self.validator.validate_consecutive(course, &assignments) {
            return false;
        }
        
        true
    }
    
    /// Schedule course without consecutive requirements
    fn schedule_without_consecutive(
        &self,
        course: &CourseToSchedule,
        periods_needed: i32,
        available_slots: &[TimeSlot],
        state: &mut ScheduleState,
    ) -> bool {
        let mut assigned = 0;
        
        // Try to assign periods, preferring distribution
        for slot in available_slots {
            if assigned >= periods_needed {
                break;
            }
            
            // Check if can assign
            let room_id = self.determine_room_id(course);
            
            // Check max consecutive per day limit locally
            let current_day_count = state.assignments.iter()
                .filter(|a| a.course_id == course.id && a.time_slot.day == slot.day)
                .count() as i32;
                
            // Strict distribution: If ANY class exists today, skip this day
            // This forces subjects to be spread across multiple days
            if current_day_count > 0 {
                continue; 
            }

            match self.validator.can_assign(course, slot, room_id, state) {
                Ok(()) => {
                    // Check instructor daily load
                    if let Some(instructor_id) = course.instructor_id {
                        if !self.validator.check_instructor_daily_load(
                            instructor_id,
                            &slot.day,
                            state,
                        ) {
                            continue; // Skip this slot
                        }
                    }
                    
                    // Check if adding this slot creates a gap (non-consecutive)
                    // If we already have assignments on this day, new slot MUST be adjacent
                    // But for min_consecutive=1, maybe gaps are allowed?
                    // User requirement implies "2 periods consecutive, but not 3" which means NO GAPS usually
                    // Let's defer strict consecutive check to Validator, but at least control COUNT here.
                    
                    // Assign
                    let assignment = Assignment::new(course, slot.clone(), room_id, false);
                    state.add_assignment(assignment);
                    assigned += 1;
                }
                Err(_) => continue,
            }
        }
        
        assigned == periods_needed
    }
    
    /// Find consecutive available slots
    fn find_consecutive_slots(
        &self,
        course: &CourseToSchedule,
        count: i32,
        available_slots: &[TimeSlot],
        state: &ScheduleState,
    ) -> Option<Vec<TimeSlot>> {
        // Group slots by day
        let mut by_day: std::collections::HashMap<String, Vec<TimeSlot>> =
            std::collections::HashMap::new();
        
        for slot in available_slots {
            by_day
                .entry(slot.day.clone())
                .or_insert_with(Vec::new)
                .push(slot.clone());
        }
        
        // Try each day
        for (day_name, mut day_slots) in by_day {
            // Check if already assigned on this day
            let current_day_count = state.assignments.iter()
                .filter(|a| a.course_id == course.id && a.time_slot.day == day_name)
                .count() as i32;
            
            if current_day_count > 0 {
                continue; // Already taught today, skip to force different day
            }

            // Sort by period order
            day_slots.sort_by_key(|s| s.period_order);
            
            // Find consecutive window
            for i in 0..=day_slots.len().saturating_sub(count as usize) {
                let window = &day_slots[i..i + count as usize];
                
                // Check if truly consecutive
                if !self.is_consecutive_periods(window) {
                    continue;
                }
                
                // Check if all can be assigned
                let room_id = self.determine_room_id(course);
                let all_valid = window.iter().all(|slot| {
                    self.validator.can_assign(course, slot, room_id, state).is_ok()
                });
                
                if all_valid {
                    return Some(window.to_vec());
                }
            }
        }
        
        None
    }
    
    fn is_consecutive_periods(&self, slots: &[TimeSlot]) -> bool {
        if slots.len() <= 1 {
            return true;
        }
        
        for i in 1..slots.len() {
            if slots[i].period_order != slots[i - 1].period_order + 1 {
                return false;
            }
        }
        
        true
    }
    
    fn determine_room_id(&self, course: &CourseToSchedule) -> Option<Uuid> {
        // Priority: fixed room > none (use classroom's room)
        course.fixed_room_id
    }
    
    fn calculate_difficulty(&self, course: &CourseToSchedule) -> i32 {
        let mut difficulty = 0;
        
        // More periods = more difficult
        difficulty += course.periods_needed * 10;
        
        // Consecutive requirement = more difficult
        if course.min_consecutive > 1 {
            difficulty += 100;
        }
        
        // Fixed room = more difficult
        if course.fixed_room_id.is_some() {
            difficulty += 50;
        }
        
        // Has instructor = more difficult
        if course.instructor_id.is_some() {
            difficulty += 20;
        }
        
        difficulty
    }
}
