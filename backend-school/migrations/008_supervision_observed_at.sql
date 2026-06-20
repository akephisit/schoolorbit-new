-- Canonical observed datetime for teaching supervision bookings.
-- Timetable-backed observations also need an exact date, not only day_of_week.

ALTER TABLE supervision_observations
    ADD COLUMN observed_at timestamp with time zone;

UPDATE supervision_observations
SET observed_at = COALESCE(manual_observed_at, requested_at)
WHERE observed_at IS NULL;

ALTER TABLE supervision_observations
    ALTER COLUMN observed_at SET NOT NULL;

ALTER TABLE supervision_observations
    DROP CONSTRAINT supervision_observations_lesson_source_check;

ALTER TABLE supervision_observations
    ADD CONSTRAINT supervision_observations_lesson_source_check CHECK (
        (
            timetable_entry_id IS NOT NULL
            AND manual_subject_name IS NULL
            AND manual_classroom_label IS NULL
            AND manual_period_label IS NULL
            AND manual_reason IS NULL
        )
        OR (
            timetable_entry_id IS NULL
            AND manual_subject_name IS NOT NULL
            AND manual_classroom_label IS NOT NULL
            AND manual_period_label IS NOT NULL
            AND manual_reason IS NOT NULL
        )
    );

ALTER TABLE supervision_observations
    DROP COLUMN manual_observed_at;

CREATE INDEX idx_supervision_observations_observed_at
    ON supervision_observations(observed_at);

COMMENT ON COLUMN supervision_observations.observed_at
    IS 'วันที่และเวลาจริงของการนิเทศ ใช้ทั้งคาบจากตารางสอนและคาบกำหนดเอง';
