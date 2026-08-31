-- Replace free-form assessment categories with one fixed four-phase plan and
-- establish the per-learning-group score-item boundary. This migration is
-- forward-only and refuses to guess how to normalize non-canonical plans.

CREATE TEMP TABLE academic_056_preflight_counts ON COMMIT DROP AS
SELECT (SELECT count(*) FROM course_assessment_plans) AS plan_count,
       (SELECT count(*) FROM course_assessment_categories) AS phase_count,
       (SELECT count(*) FROM course_assessment_items) AS source_item_count,
       (SELECT count(*) FROM academic_exam_schedule_items) AS exam_item_count,
       (
           SELECT count(*)
           FROM course_assessment_items item
           JOIN course_assessment_categories category ON category.id = item.category_id
           JOIN course_assessment_plans plan ON plan.id = category.plan_id
           JOIN learning_groups learning_group
             ON learning_group.learning_offering_id = plan.learning_offering_id
            AND learning_group.academic_term_id = plan.academic_term_id
            AND learning_group.academic_year_id = plan.academic_year_id
       ) AS expected_group_item_count;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM course_assessment_plans plan
        LEFT JOIN course_assessment_categories category ON category.plan_id = plan.id
        GROUP BY plan.id
        HAVING count(category.id) <> 4
            OR count(DISTINCT category.code) <> 4
            OR count(*) FILTER (
                WHERE category.code IN (
                    'before_midterm', 'midterm', 'after_midterm', 'final'
                )
            ) <> 4
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_056_NON_CANONICAL_ASSESSMENT_PHASES'
            USING ERRCODE = 'check_violation';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM course_assessment_categories category
        WHERE category.code IS NULL
           OR category.code NOT IN (
               'before_midterm', 'midterm', 'after_midterm', 'final'
           )
           OR category.exam_mode = 'practical'
           OR (
               category.code IN ('before_midterm', 'after_midterm')
               AND category.exam_mode <> 'none'
           )
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_056_UNSUPPORTED_ASSESSMENT_PHASE_VALUE'
            USING ERRCODE = 'check_violation';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM course_assessment_items item
        JOIN course_assessment_categories category ON category.id = item.category_id
        JOIN course_assessment_plans plan ON plan.id = category.plan_id
        WHERE NOT EXISTS (
            SELECT 1
            FROM learning_groups learning_group
            WHERE learning_group.learning_offering_id = plan.learning_offering_id
              AND learning_group.academic_term_id = plan.academic_term_id
              AND learning_group.academic_year_id = plan.academic_year_id
        )
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_056_SCORE_ITEM_WITHOUT_LEARNING_GROUP'
            USING ERRCODE = 'check_violation';
    END IF;
END;
$$;

ALTER TABLE course_assessment_plans
    DROP CONSTRAINT IF EXISTS academic_assessment_plans_status_check,
    ADD COLUMN assessment_coordinator_id UUID REFERENCES users(id) ON DELETE SET NULL;

ALTER TABLE course_assessment_plans
    DROP COLUMN status,
    DROP COLUMN submitted_at,
    DROP COLUMN submitted_by,
    DROP COLUMN locked_at,
    DROP COLUMN locked_by;

ALTER TABLE course_assessment_categories
    RENAME TO course_assessment_phases;
ALTER TABLE course_assessment_phases
    RENAME COLUMN code TO phase_code;
ALTER TABLE course_assessment_phases
    RENAME COLUMN exam_mode TO exam_arrangement;

ALTER TABLE academic_exam_schedule_items
    DROP CONSTRAINT academic_exam_schedule_items_category_plan_fkey,
    DROP CONSTRAINT academic_exam_schedule_items_round_category_group_homeroom_key;

ALTER TABLE course_assessment_phases
    DROP CONSTRAINT academic_assessment_categories_id_plan_id_key,
    DROP CONSTRAINT academic_assessment_categories_name_check,
    DROP CONSTRAINT academic_assessment_categories_max_score_check,
    DROP CONSTRAINT academic_assessment_categories_code_check,
    DROP CONSTRAINT academic_assessment_categories_exam_mode_check,
    DROP CONSTRAINT IF EXISTS academic_assessment_categories_exam_duration_check,
    DROP COLUMN name,
    DROP COLUMN display_order,
    ALTER COLUMN phase_code SET NOT NULL,
    ADD COLUMN row_version BIGINT NOT NULL DEFAULT 1,
    ADD CONSTRAINT course_assessment_phases_phase_code_check CHECK (
        phase_code IN ('before_midterm', 'midterm', 'after_midterm', 'final')
    ),
    ADD CONSTRAINT course_assessment_phases_exam_arrangement_check CHECK (
        exam_arrangement IN ('none', 'in_timetable', 'outside_timetable')
    ),
    ADD CONSTRAINT course_assessment_phases_phase_arrangement_check CHECK (
        phase_code IN ('midterm', 'final') OR exam_arrangement = 'none'
    ),
    ADD CONSTRAINT course_assessment_phases_max_score_check CHECK (max_score >= 0),
    ADD CONSTRAINT course_assessment_phases_exam_duration_check CHECK (
        exam_duration_minutes IS NULL OR exam_duration_minutes > 0
    ),
    ADD CONSTRAINT course_assessment_phases_row_version_check CHECK (row_version > 0),
    ADD CONSTRAINT course_assessment_phases_plan_phase_key UNIQUE (plan_id, phase_code),
    ADD CONSTRAINT course_assessment_phases_id_plan_key UNIQUE (id, plan_id);

DROP INDEX IF EXISTS idx_academic_assessment_categories_plan;
DROP INDEX IF EXISTS idx_academic_assessment_categories_exam_mode;
CREATE INDEX course_assessment_phases_plan_idx
    ON course_assessment_phases(plan_id, phase_code);
CREATE INDEX course_assessment_phases_exam_arrangement_idx
    ON course_assessment_phases(exam_arrangement)
    WHERE exam_arrangement = 'in_timetable';

ALTER TABLE academic_exam_schedule_items
    RENAME COLUMN assessment_category_id TO assessment_phase_id;
ALTER TABLE academic_exam_schedule_items
    ADD CONSTRAINT academic_exam_schedule_items_phase_plan_fkey
        FOREIGN KEY (assessment_phase_id, course_assessment_plan_id)
        REFERENCES course_assessment_phases(id, plan_id) ON DELETE RESTRICT,
    ADD CONSTRAINT academic_exam_schedule_items_round_phase_group_homeroom_key
        UNIQUE (exam_round_id, assessment_phase_id, learning_group_id, homeroom_id);

ALTER TABLE academic_exam_rounds
    ADD COLUMN row_version BIGINT NOT NULL DEFAULT 1,
    ADD CONSTRAINT academic_exam_rounds_row_version_check CHECK (row_version > 0);

CREATE TABLE academic_assessment_phase_controls (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    academic_term_id UUID NOT NULL,
    academic_year_id UUID NOT NULL,
    phase_code TEXT NOT NULL,
    item_editing_enabled BOOLEAN NOT NULL DEFAULT false,
    score_entry_enabled BOOLEAN NOT NULL DEFAULT false,
    row_version BIGINT NOT NULL DEFAULT 1,
    updated_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT academic_assessment_phase_controls_phase_code_check CHECK (
        phase_code IN ('before_midterm', 'midterm', 'after_midterm', 'final')
    ),
    CONSTRAINT academic_assessment_phase_controls_row_version_check CHECK (row_version > 0),
    CONSTRAINT academic_assessment_phase_controls_term_context_fkey
        FOREIGN KEY (academic_term_id, academic_year_id)
        REFERENCES academic_terms(id, academic_year_id) ON DELETE CASCADE,
    CONSTRAINT academic_assessment_phase_controls_term_phase_key
        UNIQUE (academic_term_id, phase_code),
    CONSTRAINT academic_assessment_phase_controls_id_context_key
        UNIQUE (id, academic_term_id, academic_year_id)
);

INSERT INTO academic_assessment_phase_controls (
    academic_term_id,
    academic_year_id,
    phase_code
)
SELECT term.id, term.academic_year_id, phase.phase_code
FROM academic_terms term
CROSS JOIN (
    VALUES
        ('before_midterm'::text),
        ('midterm'::text),
        ('after_midterm'::text),
        ('final'::text)
) phase(phase_code);

CREATE TABLE learning_group_score_items (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    learning_group_id UUID NOT NULL,
    learning_offering_id UUID NOT NULL,
    course_assessment_plan_id UUID NOT NULL,
    assessment_phase_id UUID NOT NULL,
    academic_term_id UUID NOT NULL,
    academic_year_id UUID NOT NULL,
    name TEXT NOT NULL,
    max_score NUMERIC(10,2) NOT NULL DEFAULT 0,
    display_order INTEGER NOT NULL DEFAULT 0,
    row_version BIGINT NOT NULL DEFAULT 1,
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    updated_by UUID REFERENCES users(id) ON DELETE SET NULL,
    migration_provenance JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT learning_group_score_items_name_check CHECK (btrim(name) <> ''),
    CONSTRAINT learning_group_score_items_max_score_check CHECK (max_score >= 0),
    CONSTRAINT learning_group_score_items_row_version_check CHECK (row_version > 0),
    CONSTRAINT learning_group_score_items_group_context_fkey
        FOREIGN KEY (
            learning_group_id,
            learning_offering_id,
            academic_term_id,
            academic_year_id
        ) REFERENCES learning_groups(
            id,
            learning_offering_id,
            academic_term_id,
            academic_year_id
        ) ON DELETE RESTRICT,
    CONSTRAINT learning_group_score_items_plan_context_fkey
        FOREIGN KEY (
            course_assessment_plan_id,
            learning_offering_id,
            academic_term_id,
            academic_year_id
        ) REFERENCES course_assessment_plans(
            id,
            learning_offering_id,
            academic_term_id,
            academic_year_id
        ) ON DELETE RESTRICT,
    CONSTRAINT learning_group_score_items_phase_plan_fkey
        FOREIGN KEY (assessment_phase_id, course_assessment_plan_id)
        REFERENCES course_assessment_phases(id, plan_id) ON DELETE RESTRICT
);

CREATE INDEX learning_group_score_items_group_phase_idx
    ON learning_group_score_items(learning_group_id, assessment_phase_id, display_order);

INSERT INTO learning_group_score_items (
    id,
    learning_group_id,
    learning_offering_id,
    course_assessment_plan_id,
    assessment_phase_id,
    academic_term_id,
    academic_year_id,
    name,
    max_score,
    display_order,
    migration_provenance,
    created_at,
    updated_at
)
SELECT uuid_generate_v5(
           '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
           'group-score-item:' || item.id::text || ':' || learning_group.id::text
       ),
       learning_group.id,
       learning_group.learning_offering_id,
       plan.id,
       phase.id,
       learning_group.academic_term_id,
       learning_group.academic_year_id,
       item.name,
       item.max_score,
       item.display_order,
       jsonb_build_object(
           'migration', 56,
           'mappingAlgorithm', 'assessment-phase-v1',
           'legacyCourseAssessmentItemId', item.id
       ),
       item.created_at,
       item.updated_at
FROM course_assessment_items item
JOIN course_assessment_phases phase ON phase.id = item.category_id
JOIN course_assessment_plans plan ON plan.id = phase.plan_id
JOIN learning_groups learning_group
  ON learning_group.learning_offering_id = plan.learning_offering_id
 AND learning_group.academic_term_id = plan.academic_term_id
 AND learning_group.academic_year_id = plan.academic_year_id;

WITH group_primary AS (
    SELECT plan.id AS plan_id,
           learning_group.id AS learning_group_id,
           count(teacher.id) AS primary_count,
           min(teacher.teacher_id::text)::uuid AS teacher_id
    FROM course_assessment_plans plan
    JOIN academic_terms term ON term.id = plan.academic_term_id
    JOIN learning_groups learning_group
      ON learning_group.learning_offering_id = plan.learning_offering_id
     AND learning_group.academic_term_id = plan.academic_term_id
     AND learning_group.academic_year_id = plan.academic_year_id
     AND learning_group.status <> 'closed'
    LEFT JOIN learning_group_teachers teacher
      ON teacher.learning_group_id = learning_group.id
     AND teacher.role = 'primary'
     AND teacher.starts_on <= LEAST(GREATEST(current_date, term.start_date), term.planned_end_date)
     AND (
         teacher.ends_on IS NULL
         OR teacher.ends_on >= LEAST(
             GREATEST(current_date, term.start_date),
             term.planned_end_date
         )
     )
    GROUP BY plan.id, learning_group.id
), common_primary AS (
    SELECT plan_id,
           min(teacher_id::text)::uuid AS teacher_id
    FROM group_primary
    GROUP BY plan_id
    HAVING count(*) > 0
       AND count(*) FILTER (WHERE primary_count = 1) = count(*)
       AND count(DISTINCT teacher_id) = 1
)
UPDATE course_assessment_plans plan
SET assessment_coordinator_id = common_primary.teacher_id,
    row_version = plan.row_version + 1,
    updated_at = now()
FROM common_primary
WHERE common_primary.plan_id = plan.id
  AND plan.assessment_coordinator_id IS NULL;

DO $$
DECLARE
    before_counts RECORD;
    after_plan_count BIGINT;
    after_phase_count BIGINT;
    after_exam_item_count BIGINT;
    after_group_item_count BIGINT;
    invalid_plan_count BIGINT;
    orphaned_exam_item_count BIGINT;
BEGIN
    SELECT * INTO before_counts FROM academic_056_preflight_counts;
    SELECT count(*) INTO after_plan_count FROM course_assessment_plans;
    SELECT count(*) INTO after_phase_count FROM course_assessment_phases;
    SELECT count(*) INTO after_exam_item_count FROM academic_exam_schedule_items;
    SELECT count(*) INTO after_group_item_count FROM learning_group_score_items;

    IF before_counts.plan_count <> after_plan_count
       OR before_counts.phase_count <> after_phase_count
       OR before_counts.exam_item_count <> after_exam_item_count
       OR before_counts.expected_group_item_count <> after_group_item_count
    THEN
        RAISE EXCEPTION 'ACADEMIC_056_ROW_RECONCILIATION_FAILED';
    END IF;

    SELECT count(*) INTO invalid_plan_count
    FROM (
        SELECT plan.id
        FROM course_assessment_plans plan
        LEFT JOIN course_assessment_phases phase ON phase.plan_id = plan.id
        GROUP BY plan.id
        HAVING count(phase.id) <> 4 OR count(DISTINCT phase.phase_code) <> 4
    ) invalid;
    IF invalid_plan_count <> 0 THEN
        RAISE EXCEPTION 'ACADEMIC_056_PHASE_RECONCILIATION_FAILED';
    END IF;

    SELECT count(*) INTO orphaned_exam_item_count
    FROM academic_exam_schedule_items item
    LEFT JOIN course_assessment_phases phase ON phase.id = item.assessment_phase_id
    WHERE phase.id IS NULL;
    IF orphaned_exam_item_count <> 0 THEN
        RAISE EXCEPTION 'ACADEMIC_056_EXAM_PHASE_RECONCILIATION_FAILED';
    END IF;
END;
$$;

DROP TABLE course_assessment_items;
