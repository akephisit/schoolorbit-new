pub mod exam_schedule;

#[cfg(test)]
pub mod timetable_block {
    pub use school_academic_timetable::models::timetable_block::*;
}

#[cfg(test)]
pub mod timetable_version {
    pub use school_academic_timetable::models::timetable_version::*;
}
