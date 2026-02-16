use super::types::*;
use std::collections::HashMap;
use uuid::Uuid;

pub struct QualityScorer {
    config: SchedulerConfig,
}

impl QualityScorer {
    pub fn new(config: SchedulerConfig) -> Self {
        Self { config }
    }
    
    /// Calculate overall quality score (0-100)
    pub fn calculate_quality(&self, state: &ScheduleState, courses: &[CourseToSchedule]) -> f64 {
        // If no assignments at all, quality is 0
        if state.assignments.is_empty() {
            return 0.0;
        }
        
        // Calculate completion rate (how many periods were scheduled vs needed)
        let total_periods_needed: i32 = courses.iter().map(|c| c.periods_needed).sum();
        let total_periods_scheduled = state.assignments.len() as i32;
        
        let completion_rate = if total_periods_needed > 0 {
            (total_periods_scheduled as f64 / total_periods_needed as f64).min(1.0)
        } else {
            1.0
        };
        
        // Calculate base quality score from various metrics
        let mut total_score = 0.0;
        let mut total_weight = 0.0;
        
        // SC-1: Subject Distribution (30%)
        if self.config.optimize_distribution {
            let score = self.score_distribution(state);
            total_score += score * self.config.weight_distribution;
            total_weight += self.config.weight_distribution;
        }
        
        // SC-2: Consecutive Period Limit (20%)
        if self.config.optimize_consecutive_limit {
            let score = self.score_consecutive(state, courses);
            total_score += score * self.config.weight_consecutive;
            total_weight += self.config.weight_consecutive;
        }
        
        // SC-3: Time of Day Preference (15%)
        if self.config.optimize_time_of_day {
            let score = self.score_time_of_day(state, courses);
            total_score += score * self.config.weight_time_of_day;
            total_weight += self.config.weight_time_of_day;
        }
        
        // SC-5: Daily Load Balance (10%)
        if self.config.balance_daily_load {
            let score = self.score_daily_load_balance(state);
            total_score += score * self.config.weight_daily_load;
            total_weight += self.config.weight_daily_load;
        }
        
        // SC-8: Subject Spacing (2%)
        let score = self.score_subject_spacing(state);
        total_score += score * self.config.weight_spacing;
        total_weight += self.config.weight_spacing;
        
        let base_quality = if total_weight == 0.0 {
            100.0
        } else {
            (total_score / total_weight).min(100.0).max(0.0)
        };
        
        // Apply completion penalty: final score = base_quality * completion_rate
        // This ensures partial schedules get lower scores
        (base_quality * completion_rate).min(100.0).max(0.0)
    }
    
    /// SC-1: Score how well subjects are distributed across days
    fn score_distribution(&self, state: &ScheduleState) -> f64 {
        let mut total_score = 0.0;
        let mut count = 0;
        
        for (_course_id, assignments) in &state.course_assignments {
            if assignments.len() <= 1 {
                total_score += 100.0; // Single period = perfect
                count += 1;
                continue;
            }
            
            // Group by day
            let mut days_used: Vec<&str> = assignments
                .iter()
                .map(|a| a.time_slot.day.as_str())
                .collect();
            days_used.sort();
            days_used.dedup();
            
            // Count consecutive days
            let day_numbers: Vec<i32> = days_used
                .iter()
                .map(|d| Self::day_to_number(d))
                .collect();
            
            let max_consecutive = Self::find_max_consecutive_days(&day_numbers);
            
            // Scoring logic
            let score = match max_consecutive {
                1 => 100.0,      // Perfect - no consecutive days
                2 => 90.0,       // Good - some spread
                3 => 70.0,       // OK - a bit clustered
                4 => 50.0,       // Poor - very clustered
                _ => 30.0,       // Bad - all consecutive
            };
            
            total_score += score;
            count += 1;
        }
        
        if count == 0 {
            return 100.0;
        }
        
        total_score / count as f64
    }
    
    /// SC-2: Score consecutive period usage
    fn score_consecutive(&self, state: &ScheduleState, courses: &[CourseToSchedule]) -> f64 {
        let mut total_score = 0.0;
        let mut count = 0;
        
        for course in courses {
            let assignments = state.get_course_assignments(course.id);
            if assignments.is_empty() {
                continue;
            }
            
            // Group by day
            let mut by_day: HashMap<String, Vec<i32>> = HashMap::new();
            for assign in &assignments {
                by_day
                    .entry(assign.time_slot.day.clone())
                    .or_insert_with(Vec::new)
                    .push(assign.time_slot.period_order);
            }
            
            let mut day_scores = Vec::new();
            
            for (_day, mut periods) in by_day {
                periods.sort();
                let period_count = periods.len() as i32;
                
                let score = if period_count == 1 {
                    if course.allow_single_period {
                        80.0 // OK but not ideal
                    } else {
                        0.0 // Violation!
                    }
                } else {
                    // Check if consecutive
                    if !Self::is_consecutive(&periods) {
                        0.0 // Not consecutive = bad
                    } else if period_count >= course.min_consecutive
                        && period_count <= course.max_consecutive
                    {
                        100.0 // Perfect match
                    } else if period_count < course.min_consecutive {
                        50.0 // Too few
                    } else {
                        70.0 // Too many (but still consecutive)
                    }
                };
                
                day_scores.push(score);
            }
            
            if !day_scores.is_empty() {
                total_score += day_scores.iter().sum::<f64>() / day_scores.len() as f64;
                count += 1;
            }
        }
        
        if count == 0 {
            return 100.0;
        }
        
        total_score / count as f64
    }
    
    /// SC-3: Score time of day matching (Deprecated/Removed)
    fn score_time_of_day(&self, _state: &ScheduleState, _courses: &[CourseToSchedule]) -> f64 {
        100.0 // Feature removed
    }
    
    /// SC-5: Score daily load balance for classrooms
    fn score_daily_load_balance(&self, state: &ScheduleState) -> f64 {
        let mut total_score = 0.0;
        let mut count = 0;
        
        // Group by classroom
        let mut by_classroom: HashMap<Uuid, HashMap<String, i32>> = HashMap::new();
        
        for assign in &state.assignments {
            *by_classroom
                .entry(assign.classroom_id)
                .or_insert_with(HashMap::new)
                .entry(assign.time_slot.day.clone())
                .or_insert(0) += 1;
        }
        
        // Score each classroom's balance
        for (_classroom_id, day_counts) in by_classroom {
            let counts: Vec<i32> = day_counts.values().copied().collect();
            
            if counts.is_empty() {
                continue;
            }
            
            // Calculate variance
            let mean = counts.iter().sum::<i32>() as f64 / counts.len() as f64;
            let variance: f64 = counts
                .iter()
                .map(|&c| {
                    let diff = c as f64 - mean;
                    diff * diff
                })
                .sum::<f64>()
                / counts.len() as f64;
            
            let std_dev = variance.sqrt();
            
            // Lower variance = better score
            // Perfect balance (var=0) = 100, high variance = lower score
            let score = 100.0 - (std_dev * 10.0).min(100.0);
            
            total_score += score;
            count += 1;
        }
        
        if count == 0 {
            return 100.0;
        }
        
        total_score / count as f64
    }
    
    /// SC-8: Score subject spacing (same subject should be spread out)
    fn score_subject_spacing(&self, state: &ScheduleState) -> f64 {
        let mut total_score = 0.0;
        let mut count = 0;
        
        for (_course_id, assignments) in &state.course_assignments {
            if assignments.len() <= 1 {
                total_score += 100.0;
                count += 1;
                continue;
            }
            
            // Get days (as numbers)
            let mut days: Vec<i32> = assignments
                .iter()
                .map(|a| Self::day_to_number(&a.time_slot.day))
                .collect();
            days.sort();
            days.dedup();
            
            // Calculate minimum gap
            let mut min_gap = 7;
            for i in 1..days.len() {
                let gap = days[i] - days[i - 1];
                min_gap = min_gap.min(gap);
            }
            
            // Scoring
            let score = match min_gap {
                0 => 50.0,       // Same day (allowed if not consecutive periods)
                1 => 70.0,       // Next day (not ideal)
                2..=3 => 100.0,  // Perfect spacing
                _ => 90.0,       // Well spread (OK)
            };
            
            total_score += score;
            count += 1;
        }
        
        if count == 0 {
            return 100.0;
        }
        
        total_score / count as f64
    }
    
    // Helper functions
    
    fn day_to_number(day: &str) -> i32 {
        match day {
            "MON" => 1,
            "TUE" => 2,
            "WED" => 3,
            "THU" => 4,
            "FRI" => 5,
            "SAT" => 6,
            "SUN" => 7,
            _ => 0,
        }
    }
    
    fn find_max_consecutive_days(days: &[i32]) -> i32 {
        if days.is_empty() {
            return 0;
        }
        if days.len() == 1 {
            return 1;
        }
        
        let mut max_consecutive = 1;
        let mut current_consecutive = 1;
        
        for i in 1..days.len() {
            if days[i] == days[i - 1] + 1 {
                current_consecutive += 1;
                max_consecutive = max_consecutive.max(current_consecutive);
            } else {
                current_consecutive = 1;
            }
        }
        
        max_consecutive
    }
    
    fn is_consecutive(periods: &[i32]) -> bool {
        if periods.len() <= 1 {
            return true;
        }
        
        for i in 1..periods.len() {
            if periods[i] != periods[i - 1] + 1 {
                return false;
            }
        }
        
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_day_to_number() {
        assert_eq!(QualityScorer::day_to_number("MON"), 1);
        assert_eq!(QualityScorer::day_to_number("FRI"), 5);
    }
    
    #[test]
    fn test_find_max_consecutive() {
        assert_eq!(QualityScorer::find_max_consecutive_days(&[1, 2, 3]), 3);
        assert_eq!(QualityScorer::find_max_consecutive_days(&[1, 3, 5]), 1);
        assert_eq!(QualityScorer::find_max_consecutive_days(&[1, 2, 4, 5, 6]), 3);
    }
}
