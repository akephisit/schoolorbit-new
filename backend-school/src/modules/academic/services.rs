#[cfg(test)]
mod assessment_service_tests;
pub mod exam_schedule_service;

#[cfg(test)]
pub use school_academic_timetable::services::{
    timetable_block_service, timetable_template_service, timetable_version_service,
};

#[cfg(test)]
mod exam_schedule_service_tests;

#[cfg(test)]
mod timetable_block_service_tests;
#[cfg(test)]
mod timetable_template_service_tests;
#[cfg(test)]
mod timetable_version_service_tests;
