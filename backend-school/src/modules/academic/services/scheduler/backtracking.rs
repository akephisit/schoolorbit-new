use super::quality::QualityScorer;
use super::types::*;
use super::validator::ConstraintValidator;
use std::time::Instant;
use uuid::Uuid;

pub struct BacktrackingScheduler {
    validator: ConstraintValidator,
    scorer: QualityScorer,
    config: SchedulerConfig,
}

struct BacktrackSearch<'a> {
    courses: &'a [CourseToSchedule],
    available_slots: &'a [TimeSlot],
    best_state: &'a mut Option<ScheduleState>,
    best_score: &'a mut f64,
    iterations: &'a mut u32,
    start_time: &'a Instant,
}

impl BacktrackingScheduler {
    pub fn new(validator: ConstraintValidator, config: SchedulerConfig) -> Self {
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
        let success = {
            let mut search = BacktrackSearch {
                courses,
                available_slots,
                best_state: &mut best_state,
                best_score: &mut best_score,
                iterations: &mut iterations,
                start_time: &start_time,
            };
            self.backtrack(&mut search, 0, &mut state)
        };

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
        search: &mut BacktrackSearch<'_>,
        course_idx: usize,
        state: &mut ScheduleState,
    ) -> bool {
        *search.iterations += 1;

        // Check timeout
        if search.start_time.elapsed().as_secs() >= self.config.timeout_seconds as u64 {
            return false; // Timeout
        }

        // Check max iterations
        if *search.iterations >= self.config.max_iterations {
            return false;
        }

        // Base case: all courses scheduled
        if course_idx >= search.courses.len() {
            // Calculate quality
            let quality = self.scorer.calculate_quality(state, search.courses);

            // Update best if better
            if quality > *search.best_score {
                *search.best_score = quality;
                *search.best_state = Some(state.clone());
            }

            // Accept if meets minimum quality
            return quality >= self.config.min_quality_score;
        }

        let course = &search.courses[course_idx];
        let periods_needed = course.periods_remaining;

        // Try to schedule this course
        let success = self.schedule_course(course, periods_needed, search.available_slots, state);

        if success {
            // Continue to next course
            if self.backtrack(search, course_idx + 1, state) {
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
            return self.backtrack(search, course_idx + 1, state);
        }

        false
    }

    /// Filter slots based on course's flexible constraints (allowed_period_ids, allowed_days)
    fn filter_allowed_slots<'a>(
        &self,
        course: &CourseToSchedule,
        available_slots: &'a [TimeSlot],
    ) -> Vec<&'a TimeSlot> {
        available_slots
            .iter()
            .filter(|slot| {
                // Check allowed_days constraint
                if let Some(ref allowed_days) = course.allowed_days {
                    if !allowed_days.is_empty() && !allowed_days.contains(&slot.day) {
                        return false;
                    }
                }

                // Check allowed_period_ids constraint
                if let Some(ref allowed_periods) = course.allowed_period_ids {
                    if !allowed_periods.is_empty() && !allowed_periods.contains(&slot.period_id) {
                        return false;
                    }
                }

                true
            })
            .collect()
    }

    /// Try to schedule a single course
    fn schedule_course(
        &self,
        course: &CourseToSchedule,
        periods_needed: i32,
        available_slots: &[TimeSlot],
        state: &mut ScheduleState,
    ) -> bool {
        // Filter slots based on course constraints (allowed_period_ids, allowed_days)
        let filtered_slots = self.filter_allowed_slots(course, available_slots);

        // If no valid slots after filtering, cannot schedule
        if filtered_slots.is_empty() {
            return false;
        }

        // Convert back to owned Vec for easier handling
        let filtered_owned: Vec<TimeSlot> = filtered_slots.iter().map(|s| (*s).clone()).collect();

        // Strategy:
        // 1. ถ้ามี cc.consecutive_pattern → ใช้ pattern strategy (Phase B)
        // 2. else: legacy min/max consecutive
        if let Some(ref pattern) = course.consecutive_pattern {
            // Validate sum == periods_needed (defensive — backend ตรวจไว้แล้ว)
            let pattern_sum: i32 = pattern.iter().sum();
            if pattern_sum != periods_needed {
                // Pattern ไม่ตรง periods_needed → fallback ไป legacy
                if course.max_consecutive > 1 || course.min_consecutive > 1 {
                    return self.schedule_with_consecutive(
                        course,
                        periods_needed,
                        &filtered_owned,
                        state,
                    );
                } else {
                    return self.schedule_without_consecutive(
                        course,
                        periods_needed,
                        &filtered_owned,
                        state,
                    );
                }
            }
            self.schedule_with_pattern(course, pattern, &filtered_owned, state)
        } else if course.max_consecutive > 1 || course.min_consecutive > 1 {
            self.schedule_with_consecutive(course, periods_needed, &filtered_owned, state)
        } else {
            self.schedule_without_consecutive(course, periods_needed, &filtered_owned, state)
        }
    }

    /// Phase B: Schedule course ตาม consecutive_pattern (e.g. [1,1,1], [2,1], [3])
    /// แต่ละ chunk_size ใน pattern → หา slot ติดกัน chunk_size อันที่ว่าง
    /// Default: chunks ต่างกันต้องอยู่ต่างวัน (ยกเว้น allow_multiple_sessions_per_day)
    fn schedule_with_pattern(
        &self,
        course: &CourseToSchedule,
        pattern: &[i32],
        available_slots: &[TimeSlot],
        state: &mut ScheduleState,
    ) -> bool {
        // เรียง chunks จากใหญ่ → เล็ก เพื่อจัดอันที่ยากก่อน (chunks ใหญ่ต้องการ
        // consecutive slots — หาช่องยากกว่า)
        let mut chunks: Vec<i32> = pattern.to_vec();
        chunks.sort_by(|a, b| b.cmp(a));

        for chunk_size in chunks {
            // หา slots ติดกัน chunk_size อัน — ห้ามอยู่ในวันที่ course นี้มี
            // assignment อยู่แล้ว (บังคับ chunks กระจายต่างวัน)
            if let Some(slots) =
                self.find_consecutive_slots(course, chunk_size, available_slots, state)
            {
                for slot in slots {
                    let room_id = self.determine_room_id(course, &slot, state);
                    let assignment = Assignment::new(course, slot, room_id, false);
                    state.add_assignment(assignment);
                }
            } else {
                // Chunk นี้จัดไม่ได้ → fail ทั้ง course
                return false;
            }
        }

        true
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
            // Determine the ideal chunk size (prefer max_consecutive)
            let ideal_chunk_size = course.max_consecutive.min(remaining);
            let min_chunk_size = if remaining >= course.min_consecutive {
                course.min_consecutive
            } else if course.allow_single_period && remaining == 1 {
                1
            } else {
                return false; // Cannot schedule remainder
            };

            // Try to find consecutive slots, starting from ideal and falling back to smaller sizes
            let mut assigned = false;
            for chunk_size in (min_chunk_size..=ideal_chunk_size).rev() {
                if let Some(slots) =
                    self.find_consecutive_slots(course, chunk_size, available_slots, state)
                {
                    // Assign these slots — pick room ต่อ slot (รองรับ fallback iteration)
                    // Rationale: ใน chunk เดียวกัน ห้องอาจต่างกันได้ถ้าจำเป็น (rare case)
                    for slot in slots {
                        let room_id = self.determine_room_id(course, &slot, state);
                        let assignment = Assignment::new(course, slot, room_id, false);
                        state.add_assignment(assignment);
                    }
                    remaining -= chunk_size;
                    assigned = true;
                    break; // Successfully assigned this chunk
                }
            }

            if !assigned {
                return false; // Cannot find any valid chunk
            }
        }

        // Validate consecutive after all assignments
        let assignments = state.get_course_assignments(course.id);
        if self
            .validator
            .validate_consecutive(course, &assignments)
            .is_err()
        {
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
            let room_id = self.determine_room_id(course, slot, state);

            // Check max consecutive per day limit locally
            let current_day_count = state
                .assignments
                .iter()
                .filter(|a| {
                    a.classroom_course_id == course.classroom_course_id
                        && a.time_slot.day == slot.day
                })
                .count() as i32;

            // Strict distribution: If ANY class exists today, skip this day
            // Only if allow_multiple_sessions_per_day is FALSE (default)
            if !self.config.allow_multiple_sessions_per_day && current_day_count > 0 {
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
                .or_default()
                .push(slot.clone());
        }

        // Try each day
        for (day_name, mut day_slots) in by_day {
            // Check if already assigned on this day
            let current_day_count = state
                .assignments
                .iter()
                .filter(|a| {
                    a.classroom_course_id == course.classroom_course_id
                        && a.time_slot.day == day_name
                })
                .count() as i32;

            if !self.config.allow_multiple_sessions_per_day && current_day_count > 0 {
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

                // Check if all can be assigned (pick room per-slot to support fallback)
                let all_valid = window.iter().all(|slot| {
                    let room_id = self.determine_room_id(course, slot, state);
                    self.validator
                        .can_assign(course, slot, room_id, state)
                        .is_ok()
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

    /// Phase D: room hierarchy + iteration fallback
    /// 1. ลอง cc.preferred_rooms ตาม rank — return ห้องแรกที่ว่างที่ slot นี้
    /// 2. ถ้าทุกห้องใน preferred_rooms เต็ม:
    ///    - ถ้ามีห้อง is_required → return None (scheduler ต้อง fail slot นี้)
    ///    - else → fallback ไป instructor's fixed_room_id (ถ้าว่าง) → None
    /// 3. ถ้าไม่มี preferred_rooms เลย → ใช้ instructor's fixed_room_id (เดิม)
    ///
    /// `slot` + `state` ใช้เช็คห้องว่าง — ห้องเดียวกันคนละชั้นใน slot เดียวกันถือว่าเต็ม
    fn determine_room_id(
        &self,
        course: &CourseToSchedule,
        slot: &TimeSlot,
        state: &ScheduleState,
    ) -> Option<Uuid> {
        let slot_key = slot.key();

        // ไม่มี preferred_rooms → fallback ไป instructor (เดิม)
        if course.preferred_rooms.is_empty() {
            return course.fixed_room_id;
        }

        // ลองแต่ละ preferred room ตาม rank
        let mut has_required = false;
        for pref in &course.preferred_rooms {
            if pref.is_required {
                has_required = true;
            }
            if !state.is_room_slot_occupied(pref.room_id, &slot_key) {
                return Some(pref.room_id);
            }
        }

        // ทุกห้อง preferred เต็ม
        if has_required {
            // ห้ามใช้ห้องอื่น → fail
            return None;
        }

        // Fallback ไป instructor's room ถ้ามีและว่าง
        if let Some(fallback) = course.fixed_room_id {
            if !state.is_room_slot_occupied(fallback, &slot_key) {
                return Some(fallback);
            }
        }

        None
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};

    fn scheduler() -> BacktrackingScheduler {
        let validator = ConstraintValidator::with_settings(Vec::new(), HashMap::new(), 4);
        BacktrackingScheduler::new(validator, SchedulerConfig::default())
    }

    fn sample_course() -> CourseToSchedule {
        CourseToSchedule {
            id: Uuid::new_v4(),
            classroom_course_id: Uuid::new_v4(),
            classroom_id: Uuid::new_v4(),
            classroom_name: "ม.1/1".to_string(),
            subject_id: Uuid::new_v4(),
            subject_code: "MATH".to_string(),
            subject_name: "Mathematics".to_string(),
            instructor_id: None,
            instructor_name: None,
            periods_needed: 1,
            periods_remaining: 1,
            min_consecutive: 1,
            max_consecutive: 1,
            allow_single_period: true,
            fixed_room_id: None,
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
    fn filter_allowed_slots_applies_day_and_period_constraints() {
        let scheduler = scheduler();
        let period_a = Uuid::new_v4();
        let period_b = Uuid::new_v4();
        let mut course = sample_course();
        course.allowed_days = Some(vec!["MON".to_string()]);
        course.allowed_period_ids = Some(vec![period_b]);
        let slots = vec![
            TimeSlot {
                day: "MON".to_string(),
                period_id: period_a,
                period_order: 1,
            },
            TimeSlot {
                day: "MON".to_string(),
                period_id: period_b,
                period_order: 2,
            },
            TimeSlot {
                day: "TUE".to_string(),
                period_id: period_b,
                period_order: 2,
            },
        ];

        let filtered = scheduler.filter_allowed_slots(&course, &slots);

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].day, "MON");
        assert_eq!(filtered[0].period_id, period_b);
    }

    #[test]
    fn is_consecutive_periods_accepts_adjacent_period_order_only() {
        let scheduler = scheduler();
        let slots = vec![
            TimeSlot {
                day: "MON".to_string(),
                period_id: Uuid::new_v4(),
                period_order: 1,
            },
            TimeSlot {
                day: "MON".to_string(),
                period_id: Uuid::new_v4(),
                period_order: 2,
            },
        ];
        let gap_slots = vec![
            TimeSlot {
                day: "MON".to_string(),
                period_id: Uuid::new_v4(),
                period_order: 1,
            },
            TimeSlot {
                day: "MON".to_string(),
                period_id: Uuid::new_v4(),
                period_order: 3,
            },
        ];

        assert!(scheduler.is_consecutive_periods(&slots));
        assert!(!scheduler.is_consecutive_periods(&gap_slots));
    }

    #[test]
    fn calculate_difficulty_weights_periods_consecutive_room_and_instructor() {
        let scheduler = scheduler();
        let easy = sample_course();
        let mut hard = sample_course();
        hard.periods_needed = 3;
        hard.min_consecutive = 2;
        hard.fixed_room_id = Some(Uuid::new_v4());
        hard.instructor_id = Some(Uuid::new_v4());

        assert_eq!(scheduler.calculate_difficulty(&easy), 10);
        assert_eq!(scheduler.calculate_difficulty(&hard), 200);
    }
}
