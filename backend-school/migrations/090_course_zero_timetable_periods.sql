-- Zero means an opened course has no standalone weekly timetable periods.
-- Keep curriculum workload, delivery graphs and existing timetable history intact.
ALTER TABLE course_offering_details
    DROP CONSTRAINT course_offering_details_weekly_period_target_check,
    ADD CONSTRAINT course_offering_details_weekly_period_target_check
        CHECK (weekly_period_target >= 0);

ALTER TABLE academic_term_change_items
    DROP CONSTRAINT academic_term_change_items_weekly_period_target_check,
    ADD CONSTRAINT academic_term_change_items_weekly_period_target_check
        CHECK (weekly_period_target >= 0);
