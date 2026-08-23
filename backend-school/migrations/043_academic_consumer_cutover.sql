-- Academic Core Phase A: affected consumer and permission-data cutover.
-- Legacy columns remain inert until the separately gated migration 044 cleanup.

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM academic_core_cutover_audits
        WHERE migration_version = 42
          AND mapping_algorithm_version = 'academic-core-v1'
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_043_PREDECESSOR_AUDIT_MISSING';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM academic_assessment_plans plan
        LEFT JOIN classroom_courses course ON course.id = plan.classroom_course_id
        WHERE plan.classroom_course_id IS NOT NULL
          AND (course.id IS NULL
               OR course.academic_semester_id <> plan.academic_semester_id
               OR course.subject_id <> plan.subject_id)
    ) OR EXISTS (
        SELECT 1
        FROM academic_assessment_plans plan
        LEFT JOIN learning_offerings offering
          ON offering.id = uuid_generate_v5(
              '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
              'course-offering:' || plan.academic_semester_id::text || ':' || plan.subject_id::text
          )
        WHERE offering.id IS NULL OR offering.kind <> 'course'
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_043_ASSESSMENT_OFFERING_MISMATCH';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM academic_timetable_entries entry
        LEFT JOIN classroom_courses course ON course.id = entry.classroom_course_id
        LEFT JOIN activity_slots slot ON slot.id = entry.activity_slot_id
        WHERE (entry.entry_type = 'COURSE' AND (
                   course.id IS NULL
                   OR course.academic_semester_id <> entry.academic_semester_id
                   OR course.classroom_id IS DISTINCT FROM entry.classroom_id
               ))
           OR (entry.entry_type = 'ACTIVITY' AND (
                   slot.id IS NULL
                   OR slot.semester_id <> entry.academic_semester_id
               ))
           OR (entry.entry_type = 'COURSE' AND entry.classroom_course_id IS NULL)
           OR (entry.entry_type = 'ACTIVITY' AND entry.activity_slot_id IS NULL)
           OR (entry.entry_type = 'ACTIVITY' AND (
               SELECT COUNT(*)
               FROM learning_groups activity_group
               JOIN learning_group_homerooms coverage
                 ON coverage.learning_group_id = activity_group.id
               WHERE activity_group.learning_offering_id = entry.activity_slot_id
                 AND (entry.classroom_id IS NULL OR coverage.homeroom_id = entry.classroom_id)
           ) <> 1)
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_043_TIMETABLE_CONTEXT_MISMATCH';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM academic_exam_schedule_items item
        JOIN academic_exam_rounds round ON round.id = item.exam_round_id
        JOIN academic_assessment_plans plan ON plan.id = item.assessment_plan_id
        JOIN classroom_courses course ON course.id = item.classroom_course_id
        JOIN homerooms homeroom ON homeroom.id = item.classroom_id
        JOIN academic_terms term ON term.id = item.academic_semester_id
        WHERE round.academic_semester_id <> item.academic_semester_id
           OR plan.academic_semester_id <> item.academic_semester_id
           OR plan.subject_id <> item.subject_id
           OR course.academic_semester_id <> item.academic_semester_id
           OR course.subject_id <> item.subject_id
           OR course.classroom_id <> item.classroom_id
           OR homeroom.academic_year_id <> term.academic_year_id
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_043_EXAM_CONTEXT_MISMATCH';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM supervision_cycles cycle
        LEFT JOIN academic_terms term ON term.id = cycle.academic_semester_id
        LEFT JOIN academic_years year ON year.id = term.academic_year_id
        WHERE cycle.academic_semester_id IS NOT NULL
          AND (term.id IS NULL
               OR year.year <> cycle.academic_year
               OR academic_normalize_identity(term.legacy_term)
                  <> academic_normalize_identity(cycle.semester))
    ) OR EXISTS (
        SELECT 1
        FROM supervision_cycles cycle
        WHERE cycle.academic_semester_id IS NULL
          AND (
              SELECT COUNT(*)
              FROM academic_terms term
              JOIN academic_years year ON year.id = term.academic_year_id
              WHERE year.year = cycle.academic_year
                AND academic_normalize_identity(term.legacy_term)
                    = academic_normalize_identity(cycle.semester)
          ) <> 1
    ) OR EXISTS (
        SELECT 1
        FROM supervision_observations observation
        JOIN supervision_cycles cycle ON cycle.id = observation.cycle_id
        JOIN academic_timetable_entries entry ON entry.id = observation.timetable_entry_id
        WHERE observation.timetable_entry_id IS NOT NULL
          AND cycle.academic_semester_id IS NOT NULL
          AND entry.academic_semester_id <> cycle.academic_semester_id
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_043_SUPERVISION_CONTEXT_MISMATCH';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM admission_tracks track
        JOIN admission_rounds round ON round.id = track.admission_round_id
        JOIN academic_years round_year ON round_year.id = round.academic_year_id
        WHERE (
            SELECT COUNT(*)
            FROM curriculum_versions version
            JOIN academic_years starts ON starts.id = version.start_academic_year_id
            LEFT JOIN academic_years ends ON ends.id = version.end_academic_year_id
            JOIN study_programs program
              ON program.curriculum_version_id = version.id
             AND program.is_default
            WHERE version.curriculum_id = track.study_plan_id
              AND starts.start_date <= round_year.start_date
              AND (ends.id IS NULL OR ends.end_date >= round_year.end_date)
        ) <> 1
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_043_ADMISSION_PROGRAM_UNRESOLVED';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM admission_room_assignments assignment
        JOIN admission_applications application ON application.id = assignment.application_id
        JOIN homerooms homeroom ON homeroom.id = assignment.class_room_id
        WHERE (application.status = 'enrolled' OR assignment.student_confirmed)
          AND (
              application.created_user_id IS NULL
              OR (
                  SELECT COUNT(*)
                  FROM student_academic_years student_year
                  JOIN homeroom_placements placement
                    ON placement.student_academic_year_id = student_year.id
                   AND placement.homeroom_id = homeroom.id
                  WHERE student_year.student_id = application.created_user_id
                    AND student_year.academic_year_id = homeroom.academic_year_id
              ) <> 1
          )
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_043_ADMISSION_PLACEMENT_UNRESOLVED';
    END IF;

    IF EXISTS (
        WITH granted_permissions AS (
            SELECT permission_id FROM role_permissions
            UNION
            SELECT permission_id FROM organization_permission_grants
            UNION
            SELECT permission_id FROM organization_permission_delegations
        )
        SELECT 1
        FROM granted_permissions grant_row
        JOIN permissions permission ON permission.id = grant_row.permission_id
        WHERE (
              permission.code LIKE 'academic_structure.%'
           OR permission.code LIKE 'academic_classroom.%'
           OR permission.code LIKE 'academic_enrollment.%'
           OR permission.code LIKE 'academic_course_plan.%'
           OR permission.code LIKE 'academic_curriculum.%'
           OR permission.code LIKE 'activity.%'
        )
        AND permission.code NOT IN (
            'academic_structure.read.all',
            'academic_structure.manage.all',
            'academic_classroom.create.all',
            'academic_classroom.delete.all',
            'academic_classroom.read.all',
            'academic_classroom.update.all',
            'academic_enrollment.read.all',
            'academic_enrollment.update.all',
            'academic_course_plan.read.all',
            'academic_course_plan.manage.all',
            'academic_curriculum.read.all',
            'academic_curriculum.read.organization_tree',
            'academic_curriculum.create.all',
            'academic_curriculum.update.all',
            'academic_curriculum.delete.all',
            'academic_curriculum.manage.organization_unit',
            'academic_curriculum.manage.organization_tree',
            'activity.read.all',
            'activity.manage.all',
            'activity.manage_members.all',
            'activity.manage.own'
        )
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_043_PERMISSION_MAPPING_UNRESOLVED';
    END IF;

    IF EXISTS (
        SELECT 1 FROM academic_assessment_categories
        WHERE max_score::numeric <> round(max_score::numeric, 2)
    ) OR EXISTS (
        SELECT 1 FROM academic_assessment_items
        WHERE max_score::numeric <> round(max_score::numeric, 2)
    ) OR EXISTS (
        SELECT 1 FROM academic_question_bank_questions
        WHERE points::numeric <> round(points::numeric, 2)
    ) OR EXISTS (
        SELECT 1 FROM admission_room_assignments
        WHERE (total_score IS NOT NULL AND total_score::numeric <> round(total_score::numeric, 2))
           OR (full_score IS NOT NULL AND full_score::numeric <> round(full_score::numeric, 2))
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_043_DECIMAL_PRECISION_UNREPRESENTABLE';
    END IF;
END;
$$;

ALTER TABLE academic_assessment_plans RENAME TO course_assessment_plans;
ALTER TABLE academic_assessment_categories RENAME TO course_assessment_categories;
ALTER TABLE academic_assessment_items RENAME TO course_assessment_items;

ALTER TABLE course_assessment_plans
    RENAME COLUMN classroom_course_id TO legacy_classroom_course_id;
ALTER TABLE course_assessment_plans
    RENAME COLUMN academic_semester_id TO academic_term_id;
ALTER TABLE course_assessment_plans RENAME COLUMN subject_id TO subject_version_id;
ALTER TABLE course_assessment_plans
    ADD COLUMN learning_offering_id UUID,
    ADD COLUMN academic_year_id UUID,
    ADD COLUMN row_version BIGINT NOT NULL DEFAULT 1,
    ADD COLUMN migration_provenance JSONB NOT NULL DEFAULT '{}'::jsonb;

UPDATE course_assessment_plans plan
SET learning_offering_id = uuid_generate_v5(
        '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
        'course-offering:' || plan.academic_term_id::text || ':' || plan.subject_version_id::text
    ),
    academic_year_id = term.academic_year_id,
    migration_provenance = jsonb_build_object(
        'migration', 43,
        'mappingAlgorithm', 'academic-core-v1',
        'legacyClassroomCourseId', plan.legacy_classroom_course_id
    )
FROM academic_terms term
WHERE term.id = plan.academic_term_id;

ALTER TABLE course_offering_details
    ADD CONSTRAINT course_offering_details_offering_subject_context_key
        UNIQUE (learning_offering_id, subject_version_id, academic_term_id, academic_year_id);

ALTER TABLE learning_groups
    ADD CONSTRAINT learning_groups_id_offering_term_year_key
        UNIQUE (id, learning_offering_id, academic_term_id, academic_year_id);

ALTER TABLE course_assessment_plans
    ALTER COLUMN learning_offering_id SET NOT NULL,
    ALTER COLUMN academic_year_id SET NOT NULL,
    ADD CONSTRAINT course_assessment_plans_row_version_check CHECK (row_version > 0),
    ADD CONSTRAINT course_assessment_plans_offering_context_fkey
        FOREIGN KEY (learning_offering_id, academic_term_id, academic_year_id)
        REFERENCES learning_offerings(id, academic_term_id, academic_year_id) ON DELETE RESTRICT,
    ADD CONSTRAINT course_assessment_plans_offering_subject_context_fkey
        FOREIGN KEY (learning_offering_id, subject_version_id, academic_term_id, academic_year_id)
        REFERENCES course_offering_details(
            learning_offering_id, subject_version_id, academic_term_id, academic_year_id
        ) ON DELETE RESTRICT,
    ADD CONSTRAINT course_assessment_plans_offering_key UNIQUE (learning_offering_id),
    ADD CONSTRAINT course_assessment_plans_id_offering_term_year_key
        UNIQUE (id, learning_offering_id, academic_term_id, academic_year_id);

ALTER TABLE course_assessment_categories
    ALTER COLUMN max_score TYPE NUMERIC(10,2) USING round(max_score::numeric, 2);
ALTER TABLE course_assessment_items
    ALTER COLUMN max_score TYPE NUMERIC(10,2) USING round(max_score::numeric, 2);

ALTER TABLE academic_timetable_entries
    RENAME COLUMN classroom_course_id TO legacy_classroom_course_id;
ALTER TABLE academic_timetable_entries
    RENAME COLUMN academic_semester_id TO academic_term_id;
ALTER TABLE academic_timetable_entries RENAME COLUMN classroom_id TO homeroom_id;
ALTER TABLE academic_timetable_entries
    RENAME COLUMN activity_slot_id TO legacy_activity_slot_id;
ALTER TABLE academic_timetable_entries
    RENAME COLUMN period_id TO bell_schedule_period_id;
ALTER TABLE academic_timetable_entries
    ADD COLUMN academic_year_id UUID,
    ADD COLUMN learning_offering_id UUID,
    ADD COLUMN learning_group_id UUID,
    ADD COLUMN bell_schedule_id UUID,
    ADD COLUMN migration_provenance JSONB NOT NULL DEFAULT '{}'::jsonb;

UPDATE academic_timetable_entries entry
SET academic_year_id = term.academic_year_id,
    bell_schedule_id = term.bell_schedule_id,
    learning_group_id = CASE
        WHEN entry.entry_type = 'COURSE' THEN entry.legacy_classroom_course_id
        WHEN entry.entry_type = 'ACTIVITY' THEN (
            SELECT min(activity_group.id::text)::uuid
            FROM learning_groups activity_group
            JOIN learning_group_homerooms coverage
              ON coverage.learning_group_id = activity_group.id
            WHERE activity_group.learning_offering_id = entry.legacy_activity_slot_id
              AND (entry.homeroom_id IS NULL OR coverage.homeroom_id = entry.homeroom_id)
        )
        ELSE NULL
    END,
    learning_offering_id = CASE
        WHEN entry.entry_type = 'COURSE' THEN (
            SELECT learning_offering_id FROM learning_groups
            WHERE id = entry.legacy_classroom_course_id
        )
        WHEN entry.entry_type = 'ACTIVITY' THEN entry.legacy_activity_slot_id
        ELSE NULL
    END,
    migration_provenance = jsonb_build_object(
        'migration', 43,
        'mappingAlgorithm', 'academic-core-v1',
        'legacyClassroomCourseId', entry.legacy_classroom_course_id,
        'legacyActivitySlotId', entry.legacy_activity_slot_id
    )
FROM academic_terms term
WHERE term.id = entry.academic_term_id;

CREATE OR REPLACE FUNCTION check_entry_move_no_instructor_conflict()
RETURNS trigger
LANGUAGE plpgsql
AS $$
DECLARE
    has_conflict BOOLEAN;
BEGIN
    IF OLD.day_of_week = NEW.day_of_week
       AND OLD.bell_schedule_period_id = NEW.bell_schedule_period_id
       AND (OLD.is_active IS NOT DISTINCT FROM NEW.is_active OR NOT NEW.is_active)
    THEN
        RETURN NEW;
    END IF;

    IF NOT NEW.is_active THEN
        RETURN NEW;
    END IF;

    SELECT EXISTS (
        SELECT 1
        FROM timetable_entry_instructors own_instructor
        JOIN timetable_entry_instructors other_instructor
          ON other_instructor.instructor_id = own_instructor.instructor_id
        JOIN academic_timetable_entries other_entry
          ON other_entry.id = other_instructor.entry_id
        WHERE own_instructor.entry_id = NEW.id
          AND other_entry.id <> NEW.id
          AND other_entry.day_of_week = NEW.day_of_week
          AND other_entry.bell_schedule_period_id = NEW.bell_schedule_period_id
          AND other_entry.is_active
    ) INTO has_conflict;

    IF has_conflict THEN
        RAISE EXCEPTION 'ACADEMIC_TIMETABLE_INSTRUCTOR_MOVE_CONFLICT'
            USING ERRCODE = 'check_violation';
    END IF;

    RETURN NEW;
END;
$$;

CREATE OR REPLACE FUNCTION check_instructor_no_double_book()
RETURNS trigger
LANGUAGE plpgsql
AS $$
DECLARE
    entry_day VARCHAR(10);
    entry_period UUID;
    entry_active BOOLEAN;
    entry_group UUID;
    entry_batch UUID;
    entry_kind VARCHAR(50);
    has_conflict BOOLEAN;
BEGIN
    SELECT day_of_week,
           bell_schedule_period_id,
           is_active,
           learning_group_id,
           batch_id,
           entry_type
      INTO entry_day,
           entry_period,
           entry_active,
           entry_group,
           entry_batch,
           entry_kind
    FROM academic_timetable_entries
    WHERE id = NEW.entry_id;

    IF entry_day IS NULL OR NOT entry_active THEN
        RETURN NEW;
    END IF;

    SELECT EXISTS (
        SELECT 1
        FROM timetable_entry_instructors other_instructor
        JOIN academic_timetable_entries other_entry
          ON other_entry.id = other_instructor.entry_id
        WHERE other_instructor.instructor_id = NEW.instructor_id
          AND other_entry.day_of_week = entry_day
          AND other_entry.bell_schedule_period_id = entry_period
          AND other_entry.id <> NEW.entry_id
          AND other_entry.is_active
          AND NOT (
              entry_kind = 'ACTIVITY'
              AND other_entry.entry_type = 'ACTIVITY'
              AND entry_group IS NOT NULL
              AND other_entry.learning_group_id = entry_group
          )
          AND NOT (entry_batch IS NOT NULL AND other_entry.batch_id = entry_batch)
    ) INTO has_conflict;

    IF has_conflict THEN
        RAISE EXCEPTION 'ACADEMIC_TIMETABLE_INSTRUCTOR_DOUBLE_BOOKED'
            USING ERRCODE = 'check_violation';
    END IF;

    RETURN NEW;
END;
$$;

ALTER TABLE academic_terms
    ADD CONSTRAINT academic_terms_id_bell_schedule_key UNIQUE (id, bell_schedule_id);

ALTER TABLE academic_timetable_entries
    ALTER COLUMN academic_year_id SET NOT NULL,
    ALTER COLUMN bell_schedule_id SET NOT NULL,
    ADD CONSTRAINT academic_timetable_entries_term_context_fkey
        FOREIGN KEY (academic_term_id, academic_year_id)
        REFERENCES academic_terms(id, academic_year_id) ON DELETE RESTRICT,
    ADD CONSTRAINT academic_timetable_entries_term_schedule_fkey
        FOREIGN KEY (academic_term_id, bell_schedule_id)
        REFERENCES academic_terms(id, bell_schedule_id) ON DELETE RESTRICT,
    ADD CONSTRAINT academic_timetable_entries_period_schedule_fkey
        FOREIGN KEY (bell_schedule_period_id, bell_schedule_id)
        REFERENCES bell_schedule_periods(id, bell_schedule_id) ON DELETE RESTRICT,
    ADD CONSTRAINT academic_timetable_entries_group_context_fkey
        FOREIGN KEY (learning_group_id, academic_term_id, academic_year_id)
        REFERENCES learning_groups(id, academic_term_id, academic_year_id) ON DELETE RESTRICT,
    ADD CONSTRAINT academic_timetable_entries_group_offering_context_fkey
        FOREIGN KEY (learning_group_id, learning_offering_id, academic_term_id, academic_year_id)
        REFERENCES learning_groups(
            id, learning_offering_id, academic_term_id, academic_year_id
        ) ON DELETE RESTRICT,
    ADD CONSTRAINT academic_timetable_entries_offering_context_fkey
        FOREIGN KEY (learning_offering_id, academic_term_id, academic_year_id)
        REFERENCES learning_offerings(id, academic_term_id, academic_year_id) ON DELETE RESTRICT,
    ADD CONSTRAINT academic_timetable_entries_homeroom_context_fkey
        FOREIGN KEY (homeroom_id, academic_year_id)
        REFERENCES homerooms(id, academic_year_id) ON DELETE RESTRICT,
    ADD CONSTRAINT academic_timetable_entries_delivery_shape_check CHECK (
        (entry_type IN ('COURSE', 'ACTIVITY')
         AND learning_group_id IS NOT NULL
         AND learning_offering_id IS NOT NULL)
        OR (entry_type IN ('BREAK', 'HOMEROOM', 'ACADEMIC')
            AND learning_group_id IS NULL)
    ),
    ADD CONSTRAINT academic_timetable_entries_id_term_year_key
        UNIQUE (id, academic_term_id, academic_year_id);

ALTER TABLE academic_exam_rounds
    RENAME COLUMN academic_semester_id TO academic_term_id;
ALTER TABLE academic_exam_rounds
    ADD COLUMN academic_year_id UUID,
    ADD COLUMN migration_provenance JSONB NOT NULL DEFAULT '{}'::jsonb;

UPDATE academic_exam_rounds round
SET academic_year_id = term.academic_year_id,
    migration_provenance = jsonb_build_object(
        'migration', 43,
        'mappingAlgorithm', 'academic-core-v1'
    )
FROM academic_terms term
WHERE term.id = round.academic_term_id;

ALTER TABLE academic_exam_rounds
    ALTER COLUMN academic_year_id SET NOT NULL,
    ADD CONSTRAINT academic_exam_rounds_term_context_fkey
        FOREIGN KEY (academic_term_id, academic_year_id)
        REFERENCES academic_terms(id, academic_year_id) ON DELETE RESTRICT,
    ADD CONSTRAINT academic_exam_rounds_id_term_year_key
        UNIQUE (id, academic_term_id, academic_year_id);

ALTER TABLE academic_exam_days
    ADD COLUMN academic_term_id UUID,
    ADD COLUMN academic_year_id UUID,
    ADD COLUMN migration_provenance JSONB NOT NULL DEFAULT '{}'::jsonb;

UPDATE academic_exam_days day
SET academic_term_id = round.academic_term_id,
    academic_year_id = round.academic_year_id,
    migration_provenance = jsonb_build_object(
        'migration', 43,
        'mappingAlgorithm', 'academic-core-v1'
    )
FROM academic_exam_rounds round
WHERE round.id = day.exam_round_id;

ALTER TABLE academic_exam_days
    ALTER COLUMN academic_term_id SET NOT NULL,
    ALTER COLUMN academic_year_id SET NOT NULL,
    ADD CONSTRAINT academic_exam_days_round_context_fkey
        FOREIGN KEY (exam_round_id, academic_term_id, academic_year_id)
        REFERENCES academic_exam_rounds(id, academic_term_id, academic_year_id) ON DELETE CASCADE,
    ADD CONSTRAINT academic_exam_days_id_term_year_key
        UNIQUE (id, academic_term_id, academic_year_id);

ALTER TABLE academic_exam_day_room_assignments RENAME COLUMN classroom_id TO homeroom_id;
ALTER TABLE academic_exam_day_room_assignments
    ADD COLUMN academic_term_id UUID,
    ADD COLUMN academic_year_id UUID,
    ADD COLUMN migration_provenance JSONB NOT NULL DEFAULT '{}'::jsonb;

UPDATE academic_exam_day_room_assignments assignment
SET academic_term_id = round.academic_term_id,
    academic_year_id = round.academic_year_id,
    migration_provenance = jsonb_build_object(
        'migration', 43,
        'mappingAlgorithm', 'academic-core-v1'
    )
FROM academic_exam_days day
JOIN academic_exam_rounds round ON round.id = day.exam_round_id
WHERE day.id = assignment.exam_day_id;

ALTER TABLE academic_exam_day_room_assignments
    ALTER COLUMN academic_term_id SET NOT NULL,
    ALTER COLUMN academic_year_id SET NOT NULL,
    ADD CONSTRAINT academic_exam_day_room_assignments_homeroom_context_fkey
        FOREIGN KEY (homeroom_id, academic_year_id)
        REFERENCES homerooms(id, academic_year_id) ON DELETE RESTRICT,
    ADD CONSTRAINT academic_exam_day_room_assignments_day_context_fkey
        FOREIGN KEY (exam_day_id, academic_term_id, academic_year_id)
        REFERENCES academic_exam_days(id, academic_term_id, academic_year_id) ON DELETE CASCADE;

ALTER TABLE academic_exam_schedule_items
    RENAME COLUMN academic_semester_id TO academic_term_id;
ALTER TABLE academic_exam_schedule_items
    RENAME COLUMN assessment_plan_id TO course_assessment_plan_id;
ALTER TABLE academic_exam_schedule_items
    RENAME COLUMN classroom_course_id TO legacy_classroom_course_id;
ALTER TABLE academic_exam_schedule_items RENAME COLUMN classroom_id TO homeroom_id;
ALTER TABLE academic_exam_schedule_items RENAME COLUMN subject_id TO subject_version_id;
ALTER TABLE academic_exam_schedule_items
    ADD COLUMN academic_year_id UUID,
    ADD COLUMN learning_offering_id UUID,
    ADD COLUMN learning_group_id UUID,
    ADD COLUMN subject_id UUID,
    ADD COLUMN migration_provenance JSONB NOT NULL DEFAULT '{}'::jsonb;

UPDATE academic_exam_schedule_items item
SET academic_year_id = term.academic_year_id,
    learning_group_id = item.legacy_classroom_course_id,
    learning_offering_id = activity_group.learning_offering_id,
    subject_id = version.subject_id,
    migration_provenance = jsonb_build_object(
        'migration', 43,
        'mappingAlgorithm', 'academic-core-v1',
        'legacyClassroomCourseId', item.legacy_classroom_course_id
    )
FROM academic_terms term
JOIN learning_groups activity_group
  ON activity_group.academic_term_id = term.id
JOIN subject_versions version ON true
WHERE term.id = item.academic_term_id
  AND activity_group.id = item.legacy_classroom_course_id
  AND version.id = item.subject_version_id;

ALTER TABLE academic_exam_schedule_items
    ALTER COLUMN academic_year_id SET NOT NULL,
    ALTER COLUMN learning_offering_id SET NOT NULL,
    ALTER COLUMN learning_group_id SET NOT NULL,
    ALTER COLUMN subject_id SET NOT NULL,
    ADD CONSTRAINT academic_exam_schedule_items_group_context_fkey
        FOREIGN KEY (learning_group_id, academic_term_id, academic_year_id)
        REFERENCES learning_groups(id, academic_term_id, academic_year_id) ON DELETE RESTRICT,
    ADD CONSTRAINT academic_exam_schedule_items_group_offering_context_fkey
        FOREIGN KEY (learning_group_id, learning_offering_id, academic_term_id, academic_year_id)
        REFERENCES learning_groups(
            id, learning_offering_id, academic_term_id, academic_year_id
        ) ON DELETE RESTRICT,
    ADD CONSTRAINT academic_exam_schedule_items_round_context_fkey
        FOREIGN KEY (exam_round_id, academic_term_id, academic_year_id)
        REFERENCES academic_exam_rounds(id, academic_term_id, academic_year_id) ON DELETE CASCADE,
    ADD CONSTRAINT academic_exam_schedule_items_homeroom_context_fkey
        FOREIGN KEY (homeroom_id, academic_year_id)
        REFERENCES homerooms(id, academic_year_id) ON DELETE RESTRICT,
    ADD CONSTRAINT academic_exam_schedule_items_offering_context_fkey
        FOREIGN KEY (learning_offering_id, academic_term_id, academic_year_id)
        REFERENCES learning_offerings(id, academic_term_id, academic_year_id) ON DELETE RESTRICT,
    ADD CONSTRAINT academic_exam_schedule_items_plan_offering_context_fkey
        FOREIGN KEY (course_assessment_plan_id, learning_offering_id, academic_term_id, academic_year_id)
        REFERENCES course_assessment_plans(id, learning_offering_id, academic_term_id, academic_year_id)
        ON DELETE RESTRICT,
    ADD CONSTRAINT academic_exam_schedule_items_subject_fkey
        FOREIGN KEY (subject_id) REFERENCES subjects(id) ON DELETE RESTRICT;

ALTER TABLE supervision_cycles
    ADD COLUMN academic_year_id UUID REFERENCES academic_years(id) ON DELETE RESTRICT,
    ADD COLUMN academic_term_id UUID REFERENCES academic_terms(id) ON DELETE RESTRICT,
    ADD COLUMN migration_provenance JSONB NOT NULL DEFAULT '{}'::jsonb;

UPDATE supervision_cycles cycle
SET academic_year_id = context.academic_year_id,
    academic_term_id = context.academic_term_id,
    migration_provenance = jsonb_build_object(
        'migration', 43,
        'mappingAlgorithm', 'academic-core-v1',
        'legacyAcademicYear', cycle.academic_year,
        'legacySemester', cycle.semester
    )
FROM (
    SELECT source.id,
           term.id AS academic_term_id,
           term.academic_year_id
    FROM supervision_cycles source
    JOIN academic_terms term
      ON term.id = source.academic_semester_id
      OR (
          source.academic_semester_id IS NULL
          AND academic_normalize_identity(term.legacy_term)
              = academic_normalize_identity(source.semester)
      )
    JOIN academic_years year
      ON year.id = term.academic_year_id
     AND year.year = source.academic_year
) context
WHERE context.id = cycle.id;

ALTER TABLE supervision_cycles
    ALTER COLUMN academic_year_id SET NOT NULL,
    ALTER COLUMN academic_term_id SET NOT NULL,
    ADD CONSTRAINT supervision_cycles_term_context_fkey
        FOREIGN KEY (academic_term_id, academic_year_id)
        REFERENCES academic_terms(id, academic_year_id) ON DELETE RESTRICT,
    ADD CONSTRAINT supervision_cycles_id_term_year_key
        UNIQUE (id, academic_term_id, academic_year_id);

ALTER TABLE supervision_observations
    ADD COLUMN learning_group_id UUID,
    ADD COLUMN homeroom_id UUID,
    ADD COLUMN academic_term_id UUID,
    ADD COLUMN academic_year_id UUID,
    ADD COLUMN migration_provenance JSONB NOT NULL DEFAULT '{}'::jsonb;

UPDATE supervision_observations observation
SET academic_term_id = cycle.academic_term_id,
    academic_year_id = cycle.academic_year_id,
    migration_provenance = jsonb_build_object(
        'migration', 43,
        'mappingAlgorithm', 'academic-core-v1'
    )
FROM supervision_cycles cycle
WHERE cycle.id = observation.cycle_id;

UPDATE supervision_observations observation
SET learning_group_id = entry.learning_group_id,
    homeroom_id = entry.homeroom_id
FROM academic_timetable_entries entry
WHERE entry.id = observation.timetable_entry_id;

ALTER TABLE supervision_observations
    ALTER COLUMN academic_term_id SET NOT NULL,
    ALTER COLUMN academic_year_id SET NOT NULL,
    ADD CONSTRAINT supervision_observations_cycle_context_fkey
        FOREIGN KEY (cycle_id, academic_term_id, academic_year_id)
        REFERENCES supervision_cycles(id, academic_term_id, academic_year_id) ON DELETE CASCADE,
    ADD CONSTRAINT supervision_observations_timetable_context_fkey
        FOREIGN KEY (timetable_entry_id, academic_term_id, academic_year_id)
        REFERENCES academic_timetable_entries(id, academic_term_id, academic_year_id),
    ADD CONSTRAINT supervision_observations_learning_group_context_fkey
        FOREIGN KEY (learning_group_id, academic_term_id, academic_year_id)
        REFERENCES learning_groups(id, academic_term_id, academic_year_id) ON DELETE RESTRICT,
    ADD CONSTRAINT supervision_observations_homeroom_context_fkey
        FOREIGN KEY (homeroom_id, academic_year_id)
        REFERENCES homerooms(id, academic_year_id) ON DELETE RESTRICT;

ALTER TABLE academic_question_bank_questions
    RENAME COLUMN subject_id TO legacy_subject_version_id;
ALTER TABLE academic_question_bank_questions
    ADD COLUMN subject_id UUID REFERENCES subjects(id) ON DELETE SET NULL,
    ADD COLUMN migration_provenance JSONB NOT NULL DEFAULT '{}'::jsonb;

UPDATE academic_question_bank_questions question
SET subject_id = version.subject_id,
    migration_provenance = jsonb_build_object(
        'migration', 43,
        'mappingAlgorithm', 'academic-core-v1',
        'legacySubjectVersionId', question.legacy_subject_version_id
    )
FROM subject_versions version
WHERE version.id = question.legacy_subject_version_id;

ALTER TABLE academic_question_bank_questions
    ALTER COLUMN points TYPE NUMERIC(10,2) USING round(points::numeric, 2),
    ADD CONSTRAINT academic_question_bank_questions_version_subject_fkey
        FOREIGN KEY (legacy_subject_version_id, subject_id)
        REFERENCES subject_versions(id, subject_id) ON DELETE RESTRICT;

ALTER TABLE admission_tracks
    ADD COLUMN study_program_id UUID REFERENCES study_programs(id) ON DELETE RESTRICT,
    ADD COLUMN curriculum_version_id UUID,
    ADD COLUMN academic_year_id UUID,
    ADD COLUMN migration_provenance JSONB NOT NULL DEFAULT '{}'::jsonb;

WITH resolved_tracks AS (
    SELECT track.id,
           track.study_plan_id,
           version.id AS curriculum_version_id,
           round.academic_year_id,
           program.id AS program_id
    FROM admission_tracks track
    JOIN admission_rounds round ON round.id = track.admission_round_id
    JOIN academic_years round_year ON round_year.id = round.academic_year_id
    JOIN curriculum_versions version ON version.curriculum_id = track.study_plan_id
    JOIN academic_years starts ON starts.id = version.start_academic_year_id
    LEFT JOIN academic_years ends ON ends.id = version.end_academic_year_id
    JOIN study_programs program
      ON program.curriculum_version_id = version.id
     AND program.is_default
    WHERE starts.start_date <= round_year.start_date
      AND (ends.id IS NULL OR ends.end_date >= round_year.end_date)
)
UPDATE admission_tracks track
SET study_program_id = resolved.program_id,
    curriculum_version_id = resolved.curriculum_version_id,
    academic_year_id = resolved.academic_year_id,
    migration_provenance = jsonb_build_object(
        'migration', 43,
        'mappingAlgorithm', 'academic-core-v1',
        'legacyCurriculumId', resolved.study_plan_id
    )
FROM resolved_tracks resolved
WHERE resolved.id = track.id;

ALTER TABLE admission_rounds
    ADD CONSTRAINT admission_rounds_id_year_key UNIQUE (id, academic_year_id);

ALTER TABLE study_programs
    ADD CONSTRAINT study_programs_id_curriculum_version_key
        UNIQUE (id, curriculum_version_id);

ALTER TABLE admission_tracks
    ALTER COLUMN study_program_id SET NOT NULL,
    ALTER COLUMN curriculum_version_id SET NOT NULL,
    ALTER COLUMN academic_year_id SET NOT NULL,
    ADD CONSTRAINT admission_tracks_round_context_fkey
        FOREIGN KEY (admission_round_id, academic_year_id)
        REFERENCES admission_rounds(id, academic_year_id) ON DELETE CASCADE,
    ADD CONSTRAINT admission_tracks_curriculum_version_fkey
        FOREIGN KEY (curriculum_version_id, study_plan_id)
        REFERENCES curriculum_versions(id, curriculum_id) ON DELETE RESTRICT,
    ADD CONSTRAINT admission_tracks_program_version_fkey
        FOREIGN KEY (study_program_id, curriculum_version_id)
        REFERENCES study_programs(id, curriculum_version_id) ON DELETE RESTRICT;

CREATE OR REPLACE FUNCTION check_admission_track_program_context()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM admission_rounds round
        JOIN academic_years round_year ON round_year.id = round.academic_year_id
        JOIN curriculum_versions version
          ON version.id = NEW.curriculum_version_id
         AND version.curriculum_id = NEW.study_plan_id
        JOIN academic_years starts ON starts.id = version.start_academic_year_id
        LEFT JOIN academic_years ends ON ends.id = version.end_academic_year_id
        JOIN study_programs program
          ON program.id = NEW.study_program_id
         AND program.curriculum_version_id = version.id
         AND program.is_default
        WHERE round.id = NEW.admission_round_id
          AND round.academic_year_id = NEW.academic_year_id
          AND starts.start_date <= round_year.start_date
          AND (ends.id IS NULL OR ends.end_date >= round_year.end_date)
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_ADMISSION_TRACK_PROGRAM_CONTEXT_MISMATCH'
            USING ERRCODE = 'check_violation';
    END IF;

    RETURN NEW;
END;
$$;

CREATE TRIGGER admission_tracks_program_context_guard
BEFORE INSERT OR UPDATE OF
    admission_round_id, study_plan_id, study_program_id,
    curriculum_version_id, academic_year_id
ON admission_tracks
FOR EACH ROW EXECUTE FUNCTION check_admission_track_program_context();

ALTER TABLE homeroom_placements
    ADD CONSTRAINT homeroom_placements_id_student_year_key
        UNIQUE (id, student_academic_year_id),
    ADD CONSTRAINT homeroom_placements_full_context_key
        UNIQUE (id, student_academic_year_id, academic_year_id, homeroom_id);

ALTER TABLE student_academic_years
    ADD CONSTRAINT student_academic_years_id_student_key UNIQUE (id, student_id);

ALTER TABLE admission_applications
    ADD CONSTRAINT admission_applications_id_user_key UNIQUE (id, created_user_id);

ALTER TABLE admission_room_assignments RENAME COLUMN class_room_id TO homeroom_id;
ALTER TABLE admission_room_assignments
    ADD COLUMN academic_year_id UUID,
    ADD COLUMN student_id UUID,
    ADD COLUMN student_academic_year_id UUID,
    ADD COLUMN homeroom_placement_id UUID,
    ADD COLUMN migration_provenance JSONB NOT NULL DEFAULT '{}'::jsonb;

WITH resolved_assignments AS (
    SELECT assignment.id,
           homeroom.academic_year_id,
           application.created_user_id AS student_id,
           student_year.id AS student_academic_year_id,
           placement.id AS homeroom_placement_id
    FROM admission_room_assignments assignment
    JOIN admission_applications application ON application.id = assignment.application_id
    JOIN homerooms homeroom ON homeroom.id = assignment.homeroom_id
    LEFT JOIN student_academic_years student_year
      ON student_year.student_id = application.created_user_id
     AND student_year.academic_year_id = homeroom.academic_year_id
    LEFT JOIN homeroom_placements placement
      ON placement.student_academic_year_id = student_year.id
     AND placement.homeroom_id = homeroom.id
)
UPDATE admission_room_assignments assignment
SET academic_year_id = resolved.academic_year_id,
    student_id = resolved.student_id,
    student_academic_year_id = resolved.student_academic_year_id,
    homeroom_placement_id = resolved.homeroom_placement_id,
    migration_provenance = jsonb_build_object(
        'migration', 43,
        'mappingAlgorithm', 'academic-core-v1'
    )
FROM resolved_assignments resolved
WHERE resolved.id = assignment.id;

ALTER TABLE admission_room_assignments
    ALTER COLUMN academic_year_id SET NOT NULL,
    ALTER COLUMN total_score TYPE NUMERIC(12,2) USING round(total_score::numeric, 2),
    ALTER COLUMN full_score TYPE NUMERIC(12,2) USING round(full_score::numeric, 2),
    ADD CONSTRAINT admission_room_assignments_student_year_context_fkey
        FOREIGN KEY (student_academic_year_id, academic_year_id)
        REFERENCES student_academic_years(id, academic_year_id) ON DELETE RESTRICT,
    ADD CONSTRAINT admission_room_assignments_student_identity_fkey
        FOREIGN KEY (student_academic_year_id, academic_year_id, student_id)
        REFERENCES student_academic_years(id, academic_year_id, student_id) ON DELETE RESTRICT,
    ADD CONSTRAINT admission_room_assignments_application_identity_fkey
        FOREIGN KEY (application_id, student_id)
        REFERENCES admission_applications(id, created_user_id) ON DELETE RESTRICT,
    ADD CONSTRAINT admission_room_assignments_homeroom_context_fkey
        FOREIGN KEY (homeroom_id, academic_year_id)
        REFERENCES homerooms(id, academic_year_id) ON DELETE RESTRICT,
    ADD CONSTRAINT admission_room_assignments_placement_context_fkey
        FOREIGN KEY (
            homeroom_placement_id, student_academic_year_id, academic_year_id, homeroom_id
        ) REFERENCES homeroom_placements(
            id, student_academic_year_id, academic_year_id, homeroom_id
        ) ON DELETE RESTRICT,
    ADD CONSTRAINT admission_room_assignments_successful_placement_check CHECK (
        NOT student_confirmed
        OR (
            student_id IS NOT NULL
            AND student_academic_year_id IS NOT NULL
            AND homeroom_placement_id IS NOT NULL
        )
    );

ALTER TABLE admission_applications
    ADD COLUMN student_academic_year_id UUID,
    ADD COLUMN homeroom_placement_id UUID;

UPDATE admission_applications application
SET student_academic_year_id = assignment.student_academic_year_id,
    homeroom_placement_id = assignment.homeroom_placement_id
FROM admission_room_assignments assignment
WHERE assignment.application_id = application.id;

ALTER TABLE admission_applications
    ADD CONSTRAINT admission_applications_placement_student_year_fkey
        FOREIGN KEY (homeroom_placement_id, student_academic_year_id)
        REFERENCES homeroom_placements(id, student_academic_year_id) ON DELETE RESTRICT,
    ADD CONSTRAINT admission_applications_student_identity_fkey
        FOREIGN KEY (student_academic_year_id, created_user_id)
        REFERENCES student_academic_years(id, student_id) ON DELETE RESTRICT,
    ADD CONSTRAINT admission_applications_successful_placement_check CHECK (
        status <> 'enrolled'
        OR (
            created_user_id IS NOT NULL
            AND student_academic_year_id IS NOT NULL
            AND homeroom_placement_id IS NOT NULL
        )
    );

ALTER TABLE calendar_events
    ADD COLUMN academic_year_id UUID REFERENCES academic_years(id) ON DELETE RESTRICT,
    ADD COLUMN academic_term_id UUID,
    ADD COLUMN migration_provenance JSONB NOT NULL DEFAULT '{}'::jsonb;

WITH resolved_events AS (
    SELECT event.id,
           (
               SELECT min(year.id::text)::uuid
               FROM academic_years year
               WHERE event.start_date >= year.start_date
                 AND event.end_date <= year.end_date
               HAVING COUNT(*) = 1
           ) AS academic_year_id,
           (
               SELECT min(term.id::text)::uuid
               FROM academic_terms term
               WHERE event.start_date >= term.start_date
                 AND event.end_date <= term.end_date
               HAVING COUNT(*) = 1
           ) AS academic_term_id
    FROM calendar_events event
)
UPDATE calendar_events event
SET academic_year_id = context.academic_year_id,
    academic_term_id = context.academic_term_id,
    migration_provenance = jsonb_build_object(
        'migration', 43,
        'mappingAlgorithm', 'academic-core-v1'
    )
FROM resolved_events context
WHERE context.id = event.id;

ALTER TABLE calendar_events
    ADD CONSTRAINT calendar_events_term_context_fkey
        FOREIGN KEY (academic_term_id, academic_year_id)
        REFERENCES academic_terms(id, academic_year_id) ON DELETE RESTRICT,
    ADD CONSTRAINT calendar_events_id_year_key UNIQUE (id, academic_year_id);

ALTER TABLE calendar_event_targets RENAME COLUMN class_room_id TO homeroom_id;

ALTER TABLE calendar_event_targets
    ADD COLUMN academic_year_id UUID;

UPDATE calendar_event_targets target
SET academic_year_id = event.academic_year_id
FROM calendar_events event
WHERE event.id = target.event_id;

ALTER TABLE calendar_event_targets
    ADD CONSTRAINT calendar_event_targets_event_year_fkey
        FOREIGN KEY (event_id, academic_year_id)
        REFERENCES calendar_events(id, academic_year_id) ON DELETE CASCADE,
    ADD CONSTRAINT calendar_event_targets_homeroom_context_fkey
        FOREIGN KEY (homeroom_id, academic_year_id)
        REFERENCES homerooms(id, academic_year_id) ON DELETE RESTRICT,
    ADD CONSTRAINT calendar_event_targets_homeroom_year_check
        CHECK (homeroom_id IS NULL OR academic_year_id IS NOT NULL);

CREATE INDEX course_assessment_plans_term_offering_idx
    ON course_assessment_plans(academic_term_id, learning_offering_id);
CREATE INDEX academic_timetable_entries_term_group_offering_idx
    ON academic_timetable_entries(
        academic_term_id, learning_group_id, learning_offering_id
    );
CREATE INDEX academic_timetable_entries_year_homeroom_idx
    ON academic_timetable_entries(academic_year_id, homeroom_id);
CREATE INDEX academic_exam_rounds_year_term_idx
    ON academic_exam_rounds(academic_year_id, academic_term_id);
CREATE INDEX academic_exam_days_year_term_round_idx
    ON academic_exam_days(academic_year_id, academic_term_id, exam_round_id);
CREATE INDEX academic_exam_day_room_assignments_year_term_idx
    ON academic_exam_day_room_assignments(academic_year_id, academic_term_id);
CREATE INDEX academic_exam_schedule_items_term_group_offering_idx
    ON academic_exam_schedule_items(
        academic_term_id, learning_group_id, learning_offering_id
    );
CREATE INDEX supervision_cycles_year_term_idx
    ON supervision_cycles(academic_year_id, academic_term_id);
CREATE INDEX supervision_observations_year_term_group_idx
    ON supervision_observations(academic_year_id, academic_term_id, learning_group_id);
CREATE INDEX supervision_observations_year_homeroom_idx
    ON supervision_observations(academic_year_id, homeroom_id);
CREATE INDEX academic_question_bank_questions_subject_idx
    ON academic_question_bank_questions(subject_id, legacy_subject_version_id);
CREATE INDEX admission_tracks_study_program_idx
    ON admission_tracks(academic_year_id, curriculum_version_id, study_program_id);
CREATE INDEX admission_room_assignments_student_year_placement_idx
    ON admission_room_assignments(student_id, student_academic_year_id, homeroom_placement_id);
CREATE INDEX admission_applications_student_year_placement_idx
    ON admission_applications(student_academic_year_id, homeroom_placement_id);
CREATE INDEX calendar_events_year_term_idx
    ON calendar_events(academic_year_id, academic_term_id);
CREATE INDEX calendar_event_targets_year_homeroom_idx
    ON calendar_event_targets(academic_year_id, homeroom_id);

ALTER TABLE permissions ADD COLUMN is_active BOOLEAN NOT NULL DEFAULT true;

INSERT INTO permissions (code, name, module, action, scope, description, is_active)
VALUES
    ('academic_context.read.school', 'ดูตัวเลือกปีและภาคเรียน', 'academic_context', 'read', 'school', 'Read academic context labels and states', true),
    ('academic_year.read.school', 'ดูปีการศึกษา', 'academic_year', 'read', 'school', 'Read academic years', true),
    ('academic_year.manage.school', 'จัดการปีการศึกษา', 'academic_year', 'manage', 'school', 'Manage planning academic years', true),
    ('academic_term.read.school', 'ดูภาคเรียน', 'academic_term', 'read', 'school', 'Read academic terms', true),
    ('academic_term.manage.school', 'จัดการภาคเรียน', 'academic_term', 'manage', 'school', 'Manage planning academic terms', true),
    ('academic_catalog.read.school', 'ดูคลังวิชาและกิจกรรม', 'academic_catalog', 'read', 'school', 'Read the school academic catalog', true),
    ('academic_catalog.manage.organization_unit', 'จัดการคลังวิชาของหน่วยงาน', 'academic_catalog', 'manage', 'organization_unit', 'Manage catalog owned by the exact organization unit', true),
    ('academic_catalog.manage.organization_tree', 'จัดการคลังวิชาในสายงาน', 'academic_catalog', 'manage', 'organization_tree', 'Manage catalog in the organization tree', true),
    ('academic_catalog.manage.school', 'จัดการคลังวิชาทั้งโรงเรียน', 'academic_catalog', 'manage', 'school', 'Manage the school academic catalog', true),
    ('academic_curriculum.read.organization_unit', 'ดูหลักสูตรของหน่วยงาน', 'academic_curriculum', 'read', 'organization_unit', 'Read curriculum owned by the exact organization unit', true),
    ('academic_curriculum.read.organization_tree', 'ดูหลักสูตรในสายงาน', 'academic_curriculum', 'read', 'organization_tree', 'Read curriculum in the organization tree', true),
    ('academic_curriculum.read.school', 'ดูหลักสูตรทั้งโรงเรียน', 'academic_curriculum', 'read', 'school', 'Read school curriculum', true),
    ('academic_curriculum.manage.organization_unit', 'จัดการหลักสูตรของหน่วยงาน', 'academic_curriculum', 'manage', 'organization_unit', 'Manage curriculum owned by the exact organization unit', true),
    ('academic_curriculum.manage.organization_tree', 'จัดการหลักสูตรในสายงาน', 'academic_curriculum', 'manage', 'organization_tree', 'Manage curriculum in the organization tree', true),
    ('academic_curriculum.manage.school', 'จัดการหลักสูตรทั้งโรงเรียน', 'academic_curriculum', 'manage', 'school', 'Manage school curriculum', true),
    ('homeroom.read.school', 'ดูห้องประจำชั้น', 'homeroom', 'read', 'school', 'Read homerooms', true),
    ('homeroom.manage.school', 'จัดการห้องประจำชั้น', 'homeroom', 'manage', 'school', 'Manage homerooms', true),
    ('student_academic_year.read.school', 'ดูข้อมูลนักเรียนรายปี', 'student_academic_year', 'read', 'school', 'Read student academic-year records', true),
    ('student_academic_year.manage.school', 'จัดการข้อมูลนักเรียนรายปี', 'student_academic_year', 'manage', 'school', 'Manage student academic-year records and placements', true),
    ('learning_offering.read.assigned', 'ดูการเปิดสอนที่รับผิดชอบ', 'learning_offering', 'read', 'assigned', 'Read assigned offerings and groups', true),
    ('learning_offering.read.organization_unit', 'ดูการเปิดสอนของหน่วยงาน', 'learning_offering', 'read', 'organization_unit', 'Read offerings owned by the exact organization unit', true),
    ('learning_offering.read.organization_tree', 'ดูการเปิดสอนในสายงาน', 'learning_offering', 'read', 'organization_tree', 'Read offerings in the organization tree', true),
    ('learning_offering.read.school', 'ดูการเปิดสอนทั้งโรงเรียน', 'learning_offering', 'read', 'school', 'Read school offerings and groups', true),
    ('learning_offering.manage.assigned', 'จัดการการเปิดสอนที่รับผิดชอบ', 'learning_offering', 'manage', 'assigned', 'Manage assigned offerings and groups', true),
    ('learning_offering.manage.organization_unit', 'จัดการการเปิดสอนของหน่วยงาน', 'learning_offering', 'manage', 'organization_unit', 'Manage offerings owned by the exact organization unit', true),
    ('learning_offering.manage.organization_tree', 'จัดการการเปิดสอนในสายงาน', 'learning_offering', 'manage', 'organization_tree', 'Manage offerings in the organization tree', true),
    ('learning_offering.manage.school', 'จัดการการเปิดสอนทั้งโรงเรียน', 'learning_offering', 'manage', 'school', 'Manage school offerings and groups', true)
ON CONFLICT (code) DO UPDATE SET
    name = EXCLUDED.name,
    module = EXCLUDED.module,
    action = EXCLUDED.action,
    scope = EXCLUDED.scope,
    description = EXCLUDED.description,
    is_active = true,
    updated_at = now();

CREATE TEMP TABLE academic_permission_cutover_map (
    source_code TEXT NOT NULL,
    target_code TEXT NOT NULL,
    PRIMARY KEY (source_code, target_code)
) ON COMMIT DROP;

INSERT INTO academic_permission_cutover_map (source_code, target_code)
VALUES
    ('academic_structure.read.all', 'academic_context.read.school'),
    ('academic_structure.read.all', 'academic_year.read.school'),
    ('academic_structure.read.all', 'academic_term.read.school'),
	('academic_structure.read.all', 'academic_catalog.read.school'),
    ('academic_structure.manage.all', 'academic_context.read.school'),
    ('academic_structure.manage.all', 'academic_year.manage.school'),
    ('academic_structure.manage.all', 'academic_term.manage.school'),
	('academic_structure.manage.all', 'academic_catalog.manage.school'),
    ('academic_classroom.read.all', 'homeroom.read.school'),
    ('academic_classroom.create.all', 'homeroom.manage.school'),
    ('academic_classroom.update.all', 'homeroom.manage.school'),
    ('academic_classroom.delete.all', 'homeroom.manage.school'),
    ('academic_enrollment.read.all', 'student_academic_year.read.school'),
    ('academic_enrollment.update.all', 'student_academic_year.manage.school'),
    ('academic_course_plan.read.all', 'learning_offering.read.school'),
    ('academic_course_plan.manage.all', 'learning_offering.manage.school'),
    ('academic_curriculum.read.all', 'academic_curriculum.read.school'),
    ('academic_curriculum.read.organization_tree', 'academic_curriculum.read.organization_tree'),
    ('academic_curriculum.create.all', 'academic_curriculum.manage.school'),
    ('academic_curriculum.update.all', 'academic_curriculum.manage.school'),
    ('academic_curriculum.delete.all', 'academic_curriculum.manage.school'),
    ('academic_curriculum.manage.organization_unit', 'academic_curriculum.manage.organization_unit'),
    ('academic_curriculum.manage.organization_tree', 'academic_curriculum.manage.organization_tree'),
    ('activity.read.all', 'academic_catalog.read.school'),
    ('activity.read.all', 'learning_offering.read.school'),
    ('activity.manage.all', 'academic_catalog.manage.school'),
    ('activity.manage.all', 'learning_offering.manage.school'),
    ('activity.manage_members.all', 'learning_offering.manage.school'),
    ('activity.manage.own', 'learning_offering.manage.assigned');

INSERT INTO role_permissions (role_id, permission_id, created_at)
SELECT old_grant.role_id, target.id, old_grant.created_at
FROM role_permissions old_grant
JOIN permissions source ON source.id = old_grant.permission_id
JOIN academic_permission_cutover_map mapping ON mapping.source_code = source.code
JOIN permissions target ON target.code = mapping.target_code
ON CONFLICT DO NOTHING;

INSERT INTO organization_permission_grants (
    organization_unit_id, permission_id, created_at, created_by, position_code
)
SELECT old_grant.organization_unit_id, target.id, old_grant.created_at,
       old_grant.created_by, old_grant.position_code
FROM organization_permission_grants old_grant
JOIN permissions source ON source.id = old_grant.permission_id
JOIN academic_permission_cutover_map mapping ON mapping.source_code = source.code
JOIN permissions target ON target.code = mapping.target_code
ON CONFLICT DO NOTHING;

INSERT INTO organization_permission_delegations (
    id, from_user_id, to_user_id, permission_id, organization_unit_id,
    reason, started_at, expires_at, revoked_at, created_at
)
SELECT uuid_generate_v5(
           '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
           'permission-delegation:' || old_grant.id::text || ':' || target.code
       ),
       old_grant.from_user_id,
       old_grant.to_user_id,
       target.id,
       old_grant.organization_unit_id,
       old_grant.reason,
       old_grant.started_at,
       old_grant.expires_at,
       old_grant.revoked_at,
       old_grant.created_at
FROM organization_permission_delegations old_grant
JOIN permissions source ON source.id = old_grant.permission_id
JOIN academic_permission_cutover_map mapping ON mapping.source_code = source.code
JOIN permissions target ON target.code = mapping.target_code
ON CONFLICT DO NOTHING;

INSERT INTO role_permissions (role_id, permission_id)
SELECT DISTINCT role_grant.role_id, context_permission.id
FROM role_permissions role_grant
JOIN permissions retained ON retained.id = role_grant.permission_id
CROSS JOIN permissions context_permission
WHERE context_permission.code = 'academic_context.read.school'
  AND retained.is_active
  AND retained.module IN (
      'academic_year', 'academic_term', 'academic_catalog', 'academic_curriculum',
      'homeroom', 'student_academic_year', 'learning_offering', 'academic_assessment',
      'academic_exam_schedule', 'academic_question_bank', 'academic_timetable_today'
  )
ON CONFLICT DO NOTHING;

INSERT INTO organization_permission_grants (
    organization_unit_id, permission_id, created_at, created_by, position_code
)
SELECT DISTINCT grant_row.organization_unit_id, context_permission.id,
       now(), grant_row.created_by, grant_row.position_code
FROM organization_permission_grants grant_row
JOIN permissions retained ON retained.id = grant_row.permission_id
CROSS JOIN permissions context_permission
WHERE context_permission.code = 'academic_context.read.school'
  AND retained.is_active
  AND retained.module IN (
      'academic_year', 'academic_term', 'academic_catalog', 'academic_curriculum',
      'homeroom', 'student_academic_year', 'learning_offering', 'academic_assessment',
      'academic_exam_schedule', 'academic_question_bank', 'academic_timetable_today'
  )
ON CONFLICT DO NOTHING;

WITH context_sources AS (
    SELECT DISTINCT ON (
               delegation.from_user_id,
               delegation.to_user_id,
               delegation.organization_unit_id,
               delegation.started_at,
               delegation.expires_at,
               delegation.revoked_at
           )
           delegation.*
    FROM organization_permission_delegations delegation
    JOIN permissions retained ON retained.id = delegation.permission_id
    WHERE retained.is_active
      AND retained.module IN (
          'academic_year', 'academic_term', 'academic_catalog', 'academic_curriculum',
          'homeroom', 'student_academic_year', 'learning_offering', 'academic_assessment',
          'academic_exam_schedule', 'academic_question_bank', 'academic_timetable_today'
      )
    ORDER BY delegation.from_user_id,
             delegation.to_user_id,
             delegation.organization_unit_id NULLS FIRST,
             delegation.started_at,
             delegation.expires_at NULLS LAST,
             delegation.revoked_at NULLS LAST,
             delegation.id
)
INSERT INTO organization_permission_delegations (
    id, from_user_id, to_user_id, permission_id, organization_unit_id,
    reason, started_at, expires_at, revoked_at, created_at
)
SELECT uuid_generate_v5(
           '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
           'permission-context-delegation:' || source.id::text
       ),
       source.from_user_id,
       source.to_user_id,
       context_permission.id,
       source.organization_unit_id,
       source.reason,
       source.started_at,
       source.expires_at,
       source.revoked_at,
       source.created_at
FROM context_sources source
CROSS JOIN permissions context_permission
WHERE context_permission.code = 'academic_context.read.school'
ON CONFLICT DO NOTHING;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM role_permissions source_grant
        JOIN permissions source ON source.id = source_grant.permission_id
        JOIN academic_permission_cutover_map mapping ON mapping.source_code = source.code
        JOIN permissions target ON target.code = mapping.target_code
        WHERE NOT EXISTS (
            SELECT 1
            FROM role_permissions target_grant
            WHERE target_grant.role_id = source_grant.role_id
              AND target_grant.permission_id = target.id
        )
    ) OR EXISTS (
        SELECT 1
        FROM organization_permission_grants source_grant
        JOIN permissions source ON source.id = source_grant.permission_id
        JOIN academic_permission_cutover_map mapping ON mapping.source_code = source.code
        JOIN permissions target ON target.code = mapping.target_code
        WHERE NOT EXISTS (
            SELECT 1
            FROM organization_permission_grants target_grant
            WHERE target_grant.organization_unit_id = source_grant.organization_unit_id
              AND target_grant.position_code IS NOT DISTINCT FROM source_grant.position_code
              AND target_grant.permission_id = target.id
        )
    ) OR EXISTS (
        SELECT 1
        FROM organization_permission_delegations source_grant
        JOIN permissions source ON source.id = source_grant.permission_id
        JOIN academic_permission_cutover_map mapping ON mapping.source_code = source.code
        JOIN permissions target ON target.code = mapping.target_code
        WHERE NOT EXISTS (
            SELECT 1
            FROM organization_permission_delegations target_grant
            WHERE target_grant.from_user_id = source_grant.from_user_id
              AND target_grant.to_user_id = source_grant.to_user_id
              AND target_grant.organization_unit_id IS NOT DISTINCT FROM source_grant.organization_unit_id
              AND target_grant.permission_id = target.id
              AND target_grant.started_at = source_grant.started_at
              AND target_grant.expires_at IS NOT DISTINCT FROM source_grant.expires_at
              AND target_grant.revoked_at IS NOT DISTINCT FROM source_grant.revoked_at
        )
    ) OR EXISTS (
        SELECT 1
        FROM role_permissions source_grant
        JOIN permissions retained ON retained.id = source_grant.permission_id
        WHERE retained.is_active
          AND retained.module IN (
              'academic_year', 'academic_term', 'academic_catalog',
              'academic_curriculum', 'homeroom', 'student_academic_year',
              'learning_offering', 'academic_assessment',
              'academic_exam_schedule', 'academic_question_bank',
              'academic_timetable_today'
          )
          AND NOT EXISTS (
            SELECT 1
            FROM role_permissions context_grant
            JOIN permissions context_permission ON context_permission.id = context_grant.permission_id
            WHERE context_grant.role_id = source_grant.role_id
              AND context_permission.code = 'academic_context.read.school'
        )
    ) OR EXISTS (
        SELECT 1
        FROM organization_permission_grants source_grant
        JOIN permissions retained ON retained.id = source_grant.permission_id
        WHERE retained.is_active
          AND retained.module IN (
              'academic_year', 'academic_term', 'academic_catalog',
              'academic_curriculum', 'homeroom', 'student_academic_year',
              'learning_offering', 'academic_assessment',
              'academic_exam_schedule', 'academic_question_bank',
              'academic_timetable_today'
          )
          AND NOT EXISTS (
            SELECT 1
            FROM organization_permission_grants context_grant
            JOIN permissions context_permission ON context_permission.id = context_grant.permission_id
            WHERE context_grant.organization_unit_id = source_grant.organization_unit_id
              AND context_grant.position_code IS NOT DISTINCT FROM source_grant.position_code
              AND context_permission.code = 'academic_context.read.school'
        )
    ) OR EXISTS (
        SELECT 1
        FROM organization_permission_delegations source_grant
        JOIN permissions retained ON retained.id = source_grant.permission_id
        WHERE retained.is_active
          AND retained.module IN (
              'academic_year', 'academic_term', 'academic_catalog',
              'academic_curriculum', 'homeroom', 'student_academic_year',
              'learning_offering', 'academic_assessment',
              'academic_exam_schedule', 'academic_question_bank',
              'academic_timetable_today'
          )
          AND NOT EXISTS (
            SELECT 1
            FROM organization_permission_delegations context_grant
            JOIN permissions context_permission ON context_permission.id = context_grant.permission_id
            WHERE context_grant.from_user_id = source_grant.from_user_id
              AND context_grant.to_user_id = source_grant.to_user_id
              AND context_grant.organization_unit_id IS NOT DISTINCT FROM source_grant.organization_unit_id
              AND context_grant.started_at = source_grant.started_at
              AND context_grant.expires_at IS NOT DISTINCT FROM source_grant.expires_at
              AND context_grant.revoked_at IS NOT DISTINCT FROM source_grant.revoked_at
              AND context_permission.code = 'academic_context.read.school'
        )
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_043_PERMISSION_PRINCIPAL_MISMATCH';
    END IF;
END;
$$;

UPDATE permissions
SET is_active = false,
    updated_at = now()
WHERE code LIKE 'academic_structure.%'
   OR code LIKE 'academic_classroom.%'
   OR code LIKE 'academic_enrollment.%'
   OR code LIKE 'academic_course_plan.%'
   OR code IN (
       'academic_curriculum.read.all',
       'academic_curriculum.create.all',
       'academic_curriculum.update.all',
       'academic_curriculum.delete.all',
       'activity.read.all',
       'activity.manage.all',
       'activity.manage_members.all',
       'activity.manage.own',
       'academic_promotion.read.all',
       'academic_promotion.execute.all'
   );

INSERT INTO academic_core_entity_map (
    source_table, source_id, target_table, target_id, mapping_rule, migration_version
)
SELECT 'academic_assessment_plans', id, 'course_assessment_plans', id,
       'rename-preserve-id', 43 FROM course_assessment_plans
UNION ALL
SELECT 'academic_assessment_categories', id, 'course_assessment_categories', id,
       'rename-preserve-id', 43 FROM course_assessment_categories
UNION ALL
SELECT 'academic_assessment_items', id, 'course_assessment_items', id,
       'rename-preserve-id', 43 FROM course_assessment_items
UNION ALL
SELECT 'academic_timetable_entries', id, 'academic_timetable_entries', id,
       'context-backfill-preserve-id', 43 FROM academic_timetable_entries
UNION ALL
SELECT 'timetable_entry_instructors', id, 'timetable_entry_instructors', id,
       'context-retain-preserve-id', 43 FROM timetable_entry_instructors
UNION ALL
SELECT 'academic_exam_rounds', id, 'academic_exam_rounds', id,
       'term-context-preserve-id', 43 FROM academic_exam_rounds
UNION ALL
SELECT 'academic_exam_days', id, 'academic_exam_days', id,
       'round-context-preserve-id', 43 FROM academic_exam_days
UNION ALL
SELECT 'academic_exam_schedule_items', id, 'academic_exam_schedule_items', id,
       'delivery-context-preserve-id', 43 FROM academic_exam_schedule_items
UNION ALL
SELECT 'academic_exam_sessions', id, 'academic_exam_sessions', id,
       'delivery-context-preserve-id', 43 FROM academic_exam_sessions
UNION ALL
SELECT 'academic_exam_day_room_assignments', id, 'academic_exam_day_room_assignments', id,
       'homeroom-context-preserve-id', 43 FROM academic_exam_day_room_assignments
UNION ALL
SELECT 'supervision_cycles', id, 'supervision_cycles', id,
       'year-term-context-preserve-id', 43 FROM supervision_cycles
UNION ALL
SELECT 'supervision_observations', id, 'supervision_observations', id,
       'delivery-context-preserve-id', 43 FROM supervision_observations
UNION ALL
SELECT 'academic_question_bank_questions', id, 'academic_question_bank_questions', id,
       'stable-subject-preserve-id', 43 FROM academic_question_bank_questions
UNION ALL
SELECT 'academic_question_bank_choices', id, 'academic_question_bank_choices', id,
       'stable-subject-owner-preserve-id', 43 FROM academic_question_bank_choices
UNION ALL
SELECT 'admission_tracks', id, 'admission_tracks', id,
       'study-program-context-preserve-id', 43 FROM admission_tracks
UNION ALL
SELECT 'admission_applications', id, 'admission_applications', id,
       'student-year-context-preserve-id', 43 FROM admission_applications
UNION ALL
SELECT 'admission_room_assignments', id, 'admission_room_assignments', id,
       'placement-context-preserve-id', 43 FROM admission_room_assignments
UNION ALL
SELECT 'calendar_events', id, 'calendar_events', id,
       'optional-year-term-context-preserve-id', 43 FROM calendar_events
UNION ALL
SELECT 'calendar_event_targets', id, 'calendar_event_targets', id,
       'homeroom-target-preserve-id', 43 FROM calendar_event_targets
UNION ALL
SELECT 'certificate_campaigns', id, 'certificate_campaigns', id,
       'academic-year-owner-preserve-id', 43 FROM certificate_campaigns;

DO $$
DECLARE
    source_counts JSONB;
    target_counts JSONB;
BEGIN
    source_counts := jsonb_build_object(
        'assessmentPlans', (SELECT COUNT(*) FROM course_assessment_plans),
        'assessmentCategories', (SELECT COUNT(*) FROM course_assessment_categories),
        'assessmentItems', (SELECT COUNT(*) FROM course_assessment_items),
        'timetableEntries', (SELECT COUNT(*) FROM academic_timetable_entries),
        'timetableInstructors', (SELECT COUNT(*) FROM timetable_entry_instructors),
        'examRounds', (SELECT COUNT(*) FROM academic_exam_rounds),
        'examDays', (SELECT COUNT(*) FROM academic_exam_days),
        'examItems', (SELECT COUNT(*) FROM academic_exam_schedule_items),
        'examSessions', (SELECT COUNT(*) FROM academic_exam_sessions),
        'examRoomAssignments', (SELECT COUNT(*) FROM academic_exam_day_room_assignments),
        'supervisionCycles', (SELECT COUNT(*) FROM supervision_cycles),
        'supervisionObservations', (SELECT COUNT(*) FROM supervision_observations),
        'questions', (SELECT COUNT(*) FROM academic_question_bank_questions),
        'questionChoices', (SELECT COUNT(*) FROM academic_question_bank_choices),
        'admissionTracks', (SELECT COUNT(*) FROM admission_tracks),
        'admissionApplications', (SELECT COUNT(*) FROM admission_applications),
        'admissionAssignments', (SELECT COUNT(*) FROM admission_room_assignments),
        'calendarEvents', (SELECT COUNT(*) FROM calendar_events),
        'calendarTargets', (SELECT COUNT(*) FROM calendar_event_targets),
        'certificateCampaigns', (SELECT COUNT(*) FROM certificate_campaigns)
    );

    target_counts := source_counts;

    IF EXISTS (
        SELECT 1 FROM course_assessment_plans plan
        JOIN learning_offerings offering ON offering.id = plan.learning_offering_id
        WHERE offering.academic_term_id <> plan.academic_term_id
           OR offering.academic_year_id <> plan.academic_year_id
           OR offering.kind <> 'course'
    ) OR EXISTS (
        SELECT 1 FROM academic_timetable_entries entry
        JOIN academic_terms term ON term.id = entry.academic_term_id
        WHERE term.academic_year_id <> entry.academic_year_id
           OR (entry.learning_group_id IS NOT NULL AND NOT EXISTS (
               SELECT 1 FROM learning_groups activity_group
               WHERE activity_group.id = entry.learning_group_id
                 AND activity_group.academic_term_id = entry.academic_term_id
                 AND activity_group.academic_year_id = entry.academic_year_id
           ))
    ) OR EXISTS (
        SELECT 1 FROM academic_exam_schedule_items item
        JOIN learning_groups activity_group ON activity_group.id = item.learning_group_id
        WHERE activity_group.learning_offering_id <> item.learning_offering_id
           OR activity_group.academic_term_id <> item.academic_term_id
           OR activity_group.academic_year_id <> item.academic_year_id
    ) OR EXISTS (
        SELECT 1 FROM admission_tracks WHERE study_program_id IS NULL
    ) OR EXISTS (
        SELECT 1 FROM permissions
        WHERE code IN (
            'academic_structure.read.all', 'academic_structure.manage.all',
            'academic_classroom.read.all', 'academic_enrollment.read.all',
            'academic_course_plan.read.all', 'activity.read.all'
        ) AND is_active
    ) OR (SELECT COUNT(*) FROM permissions WHERE is_active AND code IN (
            'academic_context.read.school',
            'academic_year.read.school', 'academic_year.manage.school',
            'academic_term.read.school', 'academic_term.manage.school',
            'academic_catalog.read.school',
            'academic_catalog.manage.organization_unit',
            'academic_catalog.manage.organization_tree',
            'academic_catalog.manage.school',
            'academic_curriculum.read.organization_unit',
            'academic_curriculum.read.organization_tree',
            'academic_curriculum.read.school',
            'academic_curriculum.manage.organization_unit',
            'academic_curriculum.manage.organization_tree',
            'academic_curriculum.manage.school',
            'homeroom.read.school', 'homeroom.manage.school',
            'student_academic_year.read.school', 'student_academic_year.manage.school',
            'learning_offering.read.assigned',
            'learning_offering.read.organization_unit',
            'learning_offering.read.organization_tree',
            'learning_offering.read.school',
            'learning_offering.manage.assigned',
            'learning_offering.manage.organization_unit',
            'learning_offering.manage.organization_tree',
            'learning_offering.manage.school'
        )) <> 27 THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_043_RECONCILIATION_FAILED';
    END IF;

    INSERT INTO academic_core_cutover_audits (
        migration_version, mapping_algorithm_version, source_counts, target_counts,
        source_checksum, target_checksum
    )
    VALUES (
        43,
        'academic-core-v1',
        source_counts,
        target_counts,
        encode(sha256(convert_to(
            (SELECT COALESCE(string_agg(source_table || ':' || source_id::text, ','
                                        ORDER BY source_table, source_id), '')
             FROM academic_core_entity_map WHERE migration_version = 43),
            'UTF8'
        )), 'hex'),
        encode(sha256(convert_to(
            (SELECT COALESCE(string_agg(target_table || ':' || target_id::text, ','
                                        ORDER BY target_table, target_id), '')
             FROM academic_core_entity_map WHERE migration_version = 43),
            'UTF8'
        )), 'hex')
    );
END;
$$;
