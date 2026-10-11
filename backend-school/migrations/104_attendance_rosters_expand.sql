-- Compact immutable participants replace unchecked result placeholders.
CREATE TABLE attendance_module_state (
    singleton BOOLEAN PRIMARY KEY DEFAULT true CHECK (singleton),
    generation UUID NOT NULL DEFAULT gen_random_uuid(),
    coverage_start DATE NOT NULL DEFAULT (now() AT TIME ZONE 'Asia/Bangkok')::date
);
INSERT INTO attendance_module_state(singleton) VALUES (true);
UPDATE attendance_module_state SET coverage_start=LEAST(coverage_start,
    COALESCE((SELECT min(date) FROM attendance_sessions),coverage_start));

ALTER TABLE attendance_sessions ADD COLUMN roster JSONB NOT NULL DEFAULT '[]'::jsonb;
ALTER TABLE attendance_sessions ADD CONSTRAINT attendance_session_roster_array
    CHECK (jsonb_typeof(roster) = 'array' AND jsonb_array_length(roster) <= 2000);

DO $$ BEGIN
    IF EXISTS(SELECT 1 FROM attendance_records GROUP BY session_id HAVING count(*)>2000) THEN
        RAISE EXCEPTION 'ATTENDANCE_ROSTER_TOO_LARGE';
    END IF;
END $$;

UPDATE attendance_sessions s SET roster = COALESCE((
    SELECT jsonb_agg(jsonb_build_object(
        'studentId', r.student_id,
        'studentAcademicYearId', r.student_academic_year_id,
        'classNumber', r.class_number
    ) ORDER BY r.student_id)
    FROM attendance_records r WHERE r.session_id = s.id
), '[]'::jsonb);

-- Migration provenance can be unknown; never invent the original creator.
ALTER TABLE attendance_special_templates ALTER COLUMN created_by DROP NOT NULL;
ALTER TABLE attendance_special_rounds ADD COLUMN participants JSONB NOT NULL DEFAULT '[]'::jsonb
    CHECK (jsonb_typeof(participants)='array');
