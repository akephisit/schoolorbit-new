-- Split the two assessment editing decisions without changing accepted plan data.
-- Deployment and publication both fill only an unambiguous, currently missing owner.
LOCK TABLE academic_assessment_phase_controls, course_assessment_plans,
    course_assessment_phases, learning_offerings, course_offering_details,
    learning_groups, learning_group_teachers, academic_years, academic_terms,
    academic_course_result_locks, users IN SHARE ROW EXCLUSIVE MODE;

CREATE TEMP TABLE assessment_146_controls_before ON COMMIT DROP AS
SELECT id, to_jsonb(control) AS original FROM academic_assessment_phase_controls control;
CREATE TEMP TABLE assessment_146_plans_before ON COMMIT DROP AS
SELECT id, to_jsonb(plan) AS original FROM course_assessment_plans plan;
CREATE TEMP TABLE assessment_146_phases_before ON COMMIT DROP AS
SELECT id, to_jsonb(phase) AS original FROM course_assessment_phases phase;

ALTER TABLE academic_assessment_phase_controls
    ADD COLUMN exam_arrangement_editing_enabled BOOLEAN NOT NULL DEFAULT false,
    ADD CONSTRAINT academic_assessment_phase_controls_exam_editing_phase_check
        CHECK (phase_code IN ('midterm', 'final') OR NOT exam_arrangement_editing_enabled);
UPDATE academic_assessment_phase_controls
SET exam_arrangement_editing_enabled = plan_editing_enabled
WHERE phase_code IN ('midterm', 'final');

-- Mirror Assessment's load_coordinator_candidates + suggested_coordinator.
-- Count every nonclosed group before joining dated, active teacher episodes.
CREATE TEMP TABLE assessment_146_missing_owners ON COMMIT DROP AS
WITH eligible AS (
    SELECT offering.id AS offering_id, offering.academic_term_id,
           offering.academic_year_id, detail.subject_version_id,
           LEAST(GREATEST(current_date, term.start_date), term.planned_end_date) AS suggestion_date
    FROM learning_offerings offering
    JOIN course_offering_details detail ON detail.learning_offering_id = offering.id
    JOIN academic_terms term ON term.id = offering.academic_term_id
    JOIN academic_years year ON year.id = offering.academic_year_id
    LEFT JOIN course_assessment_plans plan ON plan.learning_offering_id = offering.id
    WHERE offering.kind = 'course' AND offering.status = 'published'
      AND year.status NOT IN ('closed', 'archived')
      AND term.status NOT IN ('closed', 'cancelled')
      AND plan.assessment_coordinator_id IS NULL
      AND NOT EXISTS (
          SELECT 1 FROM academic_course_result_locks result_lock
          WHERE result_lock.subject_id = detail.subject_id
            AND result_lock.academic_term_id = offering.academic_term_id
            AND result_lock.academic_year_id = offering.academic_year_id
      )
), group_totals AS (
    SELECT eligible.offering_id, count(*)::bigint AS group_count
    FROM eligible
    JOIN learning_groups learning_group ON learning_group.learning_offering_id = eligible.offering_id
        AND learning_group.status <> 'closed'
    GROUP BY eligible.offering_id
), candidates AS (
    SELECT eligible.offering_id, teacher.teacher_id,
           count(DISTINCT learning_group.id) FILTER (WHERE teacher.role = 'primary')::bigint AS primary_group_count,
           count(*) FILTER (WHERE teacher.role = 'primary')::bigint AS primary_assignment_count
    FROM eligible
    JOIN learning_groups learning_group ON learning_group.learning_offering_id = eligible.offering_id
        AND learning_group.status <> 'closed'
    JOIN learning_group_teachers teacher ON teacher.learning_group_id = learning_group.id
        AND teacher.starts_on <= eligible.suggestion_date
        AND (teacher.ends_on IS NULL OR teacher.ends_on >= eligible.suggestion_date)
    JOIN users teacher_account ON teacher_account.id = teacher.teacher_id AND teacher_account.status = 'active'
    GROUP BY eligible.offering_id, teacher.teacher_id
), unique_suggestions AS (
    SELECT candidate.offering_id, (array_agg(candidate.teacher_id ORDER BY candidate.teacher_id))[1] AS teacher_id
    FROM candidates candidate
    JOIN group_totals total ON total.offering_id = candidate.offering_id
    WHERE candidate.primary_group_count = total.group_count
      AND candidate.primary_assignment_count = total.group_count
    GROUP BY candidate.offering_id HAVING count(*) = 1
)
SELECT eligible.*, suggestion.teacher_id
FROM eligible JOIN unique_suggestions suggestion ON suggestion.offering_id = eligible.offering_id;

UPDATE course_assessment_plans plan
SET assessment_coordinator_id = suggestion.teacher_id,
    row_version = plan.row_version + 1, updated_at = now()
FROM assessment_146_missing_owners suggestion
WHERE plan.learning_offering_id = suggestion.offering_id
  AND plan.assessment_coordinator_id IS NULL;

CREATE TEMP TABLE assessment_146_created_plans ON COMMIT DROP AS
WITH inserted AS (
    INSERT INTO course_assessment_plans (
        id, academic_term_id, subject_version_id, learning_offering_id, academic_year_id,
        assessment_coordinator_id, row_version, migration_provenance, created_at, updated_at
    )
    SELECT uuidv7(), academic_term_id, subject_version_id, offering_id, academic_year_id,
           teacher_id, 1, '{}'::jsonb, now(), now()
    FROM assessment_146_missing_owners suggestion
    WHERE NOT EXISTS (SELECT 1 FROM course_assessment_plans plan WHERE plan.learning_offering_id = suggestion.offering_id)
    ORDER BY offering_id
    RETURNING id
) SELECT id FROM inserted;

INSERT INTO course_assessment_phases (
    id, plan_id, phase_code, max_score, exam_arrangement, exam_duration_minutes,
    row_version, created_at, updated_at
)
SELECT uuidv7(), plan.id, phase.phase_code, 0, 'none', NULL, 1, now(), now()
FROM assessment_146_created_plans plan
CROSS JOIN (VALUES ('before_midterm'), ('midterm'), ('after_midterm'), ('final')) phase(phase_code);

DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM assessment_146_controls_before before
        FULL JOIN academic_assessment_phase_controls control USING (id)
        WHERE before.original IS DISTINCT FROM to_jsonb(control) - 'exam_arrangement_editing_enabled'
           OR control.exam_arrangement_editing_enabled IS DISTINCT FROM
              (control.phase_code IN ('midterm', 'final') AND control.plan_editing_enabled)
    ) OR EXISTS (
        SELECT 1 FROM assessment_146_phases_before before
        LEFT JOIN course_assessment_phases phase USING (id)
        WHERE before.original IS DISTINCT FROM to_jsonb(phase)
    ) OR EXISTS (
        SELECT 1 FROM assessment_146_plans_before before
        LEFT JOIN course_assessment_plans plan USING (id)
        LEFT JOIN assessment_146_missing_owners suggestion ON suggestion.offering_id = plan.learning_offering_id
        WHERE CASE WHEN suggestion.offering_id IS NOT NULL AND before.original->>'assessment_coordinator_id' IS NULL
            THEN (before.original - 'assessment_coordinator_id' - 'row_version' - 'updated_at')
                   IS DISTINCT FROM (to_jsonb(plan) - 'assessment_coordinator_id' - 'row_version' - 'updated_at')
                 OR plan.assessment_coordinator_id IS DISTINCT FROM suggestion.teacher_id
                 OR plan.row_version <> (before.original->>'row_version')::bigint + 1
            ELSE before.original IS DISTINCT FROM to_jsonb(plan) END
    ) OR (SELECT count(*) FROM course_assessment_plans) <>
         (SELECT count(*) FROM assessment_146_plans_before) + (SELECT count(*) FROM assessment_146_created_plans)
      OR (SELECT count(*) FROM course_assessment_phases) <>
         (SELECT count(*) FROM assessment_146_phases_before) + 4 * (SELECT count(*) FROM assessment_146_created_plans)
    THEN
        RAISE EXCEPTION 'ASSESSMENT_146_PRESERVATION_FAILED' USING ERRCODE = 'check_violation';
    END IF;
END;
$$;
