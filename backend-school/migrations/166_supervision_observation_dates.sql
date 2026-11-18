-- Observation dates are explicit inclusive Bangkok civil dates. Existing cycles
-- and preparation copies remain unconfigured; do not infer dates from visits.
ALTER TABLE supervision_cycles
    ADD COLUMN observation_start_date date,
    ADD COLUMN observation_end_date date,
    ADD CONSTRAINT supervision_cycles_observation_dates_check CHECK (
        (observation_start_date IS NULL AND observation_end_date IS NULL)
        OR (observation_start_date IS NOT NULL AND observation_end_date IS NOT NULL
            AND observation_start_date <= observation_end_date)
    );
