pub mod assessment_service;
#[cfg(test)]
mod assessment_service_tests;
pub mod daily_teaching_service;
pub mod effective_teacher_service;
pub mod exam_schedule_service;
mod timetable_block_conflicts;
mod timetable_block_queries;
pub mod timetable_block_service;
mod timetable_block_sync;
pub mod timetable_realtime_service;
pub mod timetable_service;
pub mod timetable_template_service;
pub mod timetable_version_service;

#[cfg(test)]
mod exam_schedule_service_tests;

#[cfg(test)]
mod timetable_block_service_tests;
#[cfg(test)]
mod timetable_service_tests;
#[cfg(test)]
mod timetable_template_service_tests;
#[cfg(test)]
mod timetable_version_service_tests;
