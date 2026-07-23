mod batch_mutations;
mod entries;
mod instructors;
mod moves_and_swaps;
mod occupancy;
mod shared;
mod validation;

#[allow(unused_imports)]
pub use batch_mutations::{
    create_batch_entries, delete_batch_group, delete_entries_by_slot, BatchBlockedCell,
    BatchCreateOutcome, BatchDeletedEntry, BatchExcludedInstructor, BatchInstructorConflict,
    BatchSkippedCell,
};
pub use entries::{
    create_entry, delete_entry, fetch_entry_by_id, list_entries,
    resolve_classroom_course_semester_id, update_entry, CreateEntryOutcome, TimetableFilter,
    UpdateEntryOutcome,
};
#[allow(unused_imports)]
pub use instructors::{
    add_entry_instructor, get_my_activity_for_entry, hide_instructor_from_slot,
    hide_instructor_from_slot_period, remove_entry_instructor, restore_instructor_to_slot,
    AddInstructorResult, MyActivityForEntry, MyActivityInstructor, RemoveInstructorResult,
};
#[allow(unused_imports)]
pub use moves_and_swaps::{swap_entries, validate_moves, SwapConflictInfo, SwapOutcome};
#[allow(unused_imports)]
pub use occupancy::{get_occupancy, OccupancyRow};
pub use validation::validate_entry;
