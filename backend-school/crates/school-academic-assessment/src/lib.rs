pub mod assessment;
pub mod evidence;
pub mod exam_schedule;
pub mod gradebook;
pub mod learner_evaluation;
pub mod policy;
pub mod ports;
mod student_roster;

pub mod deletion;

mod permission_codes_generated;

#[cfg(test)]
#[path = "test_support/permission_codes_generated.rs"]
mod test_permission_codes;
