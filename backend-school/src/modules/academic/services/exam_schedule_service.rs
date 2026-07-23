#![allow(dead_code)]

mod invigilation;
mod published_views;
mod publishing;
mod room_assignments;
mod rounds_and_days;
mod sessions_and_conflicts;
mod shared;
mod workspace;

pub use self::invigilation::{
    assign_invigilator_to_assignment, get_invigilator_workspace, list_invigilator_staff_options,
    remove_invigilator_from_assignment, update_assignment_invigilators,
};
pub use self::published_views::{
    list_child_published_exam_schedule, list_my_published_exam_schedule,
    list_staff_published_exam_schedule,
};
pub use self::publishing::publish_round;
pub use self::room_assignments::{
    generate_seats_for_assignment, list_day_room_assignments, upsert_day_room_assignment,
};
pub use self::rounds_and_days::{
    create_round, delete_exam_day, list_rounds, update_exam_day, update_round, upsert_exam_day,
};
pub use self::sessions_and_conflicts::{delete_exam_session, place_exam_session};
pub use self::workspace::{clear_mismatched_exam_items, get_workspace, import_exam_items};

#[cfg(test)]
mod tests;
