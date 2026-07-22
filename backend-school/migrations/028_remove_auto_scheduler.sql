-- Remove the retired automatic timetable scheduler and its configuration data.
-- Existing timetable entries are intentionally preserved; only their scheduler
-- provenance link is removed.

ALTER TABLE academic_timetable_entries
    DROP COLUMN scheduler_job_id;

DROP TABLE timetable_scheduling_jobs;
DROP TABLE timetable_locked_slots;
DROP TABLE scheduler_settings;
DROP TABLE classroom_course_preferred_rooms;
DROP TABLE instructor_preferences;
DROP TABLE instructor_room_assignments;

ALTER TABLE classroom_courses
    DROP COLUMN consecutive_pattern,
    DROP COLUMN same_day_unique,
    DROP COLUMN hard_unavailable_slots;

ALTER TABLE subjects
    DROP COLUMN min_consecutive_periods,
    DROP COLUMN max_consecutive_periods,
    DROP COLUMN allow_single_period,
    DROP COLUMN allowed_period_ids,
    DROP COLUMN allowed_days;

DROP FUNCTION validate_allowed_days(jsonb);
DROP TYPE scheduling_algorithm;
DROP TYPE scheduling_status;
