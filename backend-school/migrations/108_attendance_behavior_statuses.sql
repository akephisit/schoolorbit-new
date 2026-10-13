-- No historical business data exists in this rollout. Refuse ambiguous leave
-- rather than inventing illness/personal provenance if a tenant violates that premise.
DO $$ BEGIN
    IF EXISTS (SELECT 1 FROM attendance_records WHERE result = 'leave')
        OR EXISTS (SELECT 1 FROM attendance_term_summaries WHERE leave <> 0)
        OR EXISTS (SELECT 1 FROM attendance_term_teacher_summaries WHERE leave <> 0) THEN
        RAISE EXCEPTION 'ATTENDANCE_UNCLASSIFIED_LEAVE';
    END IF;
END $$;

ALTER TABLE attendance_records DROP CONSTRAINT attendance_records_result_check;
ALTER TABLE attendance_records ADD CONSTRAINT attendance_records_result_check
    CHECK (result IN ('present','absent','late','skipped','sick_leave','personal_leave','activity'));
ALTER TABLE attendance_records DROP CONSTRAINT attendance_records_origin_check;
ALTER TABLE attendance_records ADD CONSTRAINT attendance_records_origin_check
    CHECK (origin IN ('none','teacher','scan','automatic'));
ALTER TABLE attendance_records ADD COLUMN processed_at TIMESTAMPTZ;

ALTER TABLE attendance_term_summaries DROP CONSTRAINT attendance_term_summaries_check;
ALTER TABLE attendance_term_teacher_summaries DROP CONSTRAINT attendance_term_teacher_summaries_check;
ALTER TABLE attendance_term_summaries DROP COLUMN leave;
ALTER TABLE attendance_term_teacher_summaries DROP COLUMN leave;
ALTER TABLE attendance_term_summaries
    ADD COLUMN skipped BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN sick_leave BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN personal_leave BIGINT NOT NULL DEFAULT 0;
ALTER TABLE attendance_term_teacher_summaries
    ADD COLUMN skipped BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN sick_leave BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN personal_leave BIGINT NOT NULL DEFAULT 0;
ALTER TABLE attendance_term_summaries ADD CONSTRAINT attendance_term_summaries_check
    CHECK (present >= 0 AND late >= 0 AND absent >= 0 AND skipped >= 0
        AND sick_leave >= 0 AND personal_leave >= 0 AND activity >= 0 AND unchecked >= 0
        AND expected = present + late + absent + skipped + sick_leave + personal_leave + activity + unchecked);
ALTER TABLE attendance_term_teacher_summaries ADD CONSTRAINT attendance_term_teacher_summaries_check
    CHECK (present >= 0 AND late >= 0 AND absent >= 0 AND skipped >= 0
        AND sick_leave >= 0 AND personal_leave >= 0 AND activity >= 0 AND unchecked >= 0
        AND expected = present + late + absent + skipped + sick_leave + personal_leave + activity + unchecked);

ALTER TABLE attendance_module_state ADD COLUMN auto_finalize_from DATE NOT NULL
    DEFAULT (now() AT TIME ZONE 'Asia/Bangkok')::date;
CREATE TABLE attendance_finalized_dates (
    generation UUID NOT NULL,
    academic_term_id UUID NOT NULL REFERENCES academic_terms(id),
    date DATE NOT NULL,
    processed_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (generation, academic_term_id, date)
);
CREATE SEQUENCE attendance_score_revision_seq AS BIGINT;
CREATE TABLE attendance_score_changes (
    generation UUID NOT NULL,
    academic_term_id UUID NOT NULL REFERENCES academic_terms(id),
    date DATE NOT NULL,
    row_version BIGINT NOT NULL DEFAULT nextval('attendance_score_revision_seq') CHECK (row_version > 0),
    queued_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    reset BOOLEAN NOT NULL DEFAULT false,
    PRIMARY KEY (generation, academic_term_id, date)
);
CREATE INDEX attendance_score_changes_pending_idx ON attendance_score_changes(queued_at,generation,academic_term_id,date);
