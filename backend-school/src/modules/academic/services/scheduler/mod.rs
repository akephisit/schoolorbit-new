pub mod types;
pub mod validator;
pub mod quality;
pub mod backtracking;

use types::*;
use validator::{ConstraintValidator, LockedSlotData};
use backtracking::BacktrackingScheduler;

use std::collections::HashMap;
use uuid::Uuid;

/// Main scheduler orchestrator
pub struct TimetableScheduler {
    config: SchedulerConfig,
}

impl TimetableScheduler {
    pub fn new(config: SchedulerConfig) -> Self {
        Self { config }
    }
    
    /// Main entry point for scheduling
    pub fn schedule(
        &self,
        mut courses: Vec<CourseToSchedule>,
        available_slots: Vec<TimeSlot>,
        locked_slots: Vec<LockedSlotData>,
        instructor_prefs: HashMap<Uuid, InstructorPrefData>,
        periods: Vec<PeriodInfo>,
    ) -> SchedulingResult {
        // Build validator
        let validator = ConstraintValidator::new(
            locked_slots,
            instructor_prefs,
            periods,
        );
        
        // Select algorithm
        match self.config.algorithm {
            SchedulingAlgorithm::Greedy => {
                // For now, use backtracking (greedy would be faster but lower quality)
                // TODO: Implement greedy as fast fallback
                let scheduler = BacktrackingScheduler::new(validator, self.config.clone());
                scheduler.schedule(&mut courses, &available_slots)
            }
            SchedulingAlgorithm::Backtracking => {
                let scheduler = BacktrackingScheduler::new(validator, self.config.clone());
                scheduler.schedule(&mut courses, &available_slots)
            }
            SchedulingAlgorithm::Hybrid => {
                // Try greedy first for speed, then backtrack if quality too low
                // TODO: Implement hybrid approach
                let scheduler = BacktrackingScheduler::new(validator, self.config.clone());
                scheduler.schedule(&mut courses, &available_slots)
            }
        }
    }
}

/// Builder for easier configuration
pub struct SchedulerBuilder {
    config: SchedulerConfig,
}

impl SchedulerBuilder {
    pub fn new() -> Self {
        Self {
            config: SchedulerConfig::default(),
        }
    }
    
    pub fn algorithm(mut self, algorithm: SchedulingAlgorithm) -> Self {
        self.config.algorithm = algorithm;
        self
    }
    
    pub fn max_iterations(mut self, max: u32) -> Self {
        self.config.max_iterations = max;
        self
    }
    
    pub fn timeout_seconds(mut self, seconds: u32) -> Self {
        self.config.timeout_seconds = seconds;
        self
    }
    
    pub fn min_quality_score(mut self, score: f64) -> Self {
        self.config.min_quality_score = score;
        self
    }
    
    pub fn allow_partial(mut self, allow: bool) -> Self {
        self.config.allow_partial = allow;
        self
    }
    
    pub fn force_overwrite(mut self, force: bool) -> Self {
        self.config.force_overwrite = force;
        self
    }
    
    pub fn respect_preferences(mut self, respect: bool) -> Self {
        self.config.respect_preferences = respect;
        self
    }
    
    pub fn build(self) -> TimetableScheduler {
        TimetableScheduler::new(self.config)
    }
}

impl Default for SchedulerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_builder() {
        let scheduler = SchedulerBuilder::new()
            .algorithm(SchedulingAlgorithm::Backtracking)
            .max_iterations(5000)
            .timeout_seconds(60)
            .min_quality_score(80.0)
            .build();
        
        assert_eq!(scheduler.config.algorithm, SchedulingAlgorithm::Backtracking);
        assert_eq!(scheduler.config.max_iterations, 5000);
        assert_eq!(scheduler.config.timeout_seconds, 60);
        assert_eq!(scheduler.config.min_quality_score, 80.0);
    }
}
