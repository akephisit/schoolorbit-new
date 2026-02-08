pub mod scheduler;
pub mod scheduler_data;

// Re-export main types for convenience
pub use scheduler::{
    TimetableScheduler,
    SchedulerBuilder,
    types::{
        SchedulingAlgorithm,
        SchedulingResult,
        CourseToSchedule,
        TimeSlot,
        Assignment,
        FailedCourse,
    },
};
