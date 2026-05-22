pub mod scheduler;
pub mod scheduler_data;
pub mod timetable_service;
pub mod period_service;
pub mod activity_service;
pub mod study_plan_service;
pub mod subject_service;
pub mod scheduling_config_service;
pub mod scheduling_service;
pub mod timetable_template_service;

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
