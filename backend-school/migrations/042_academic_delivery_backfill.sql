-- Academic Core Phase A: student-year, placement, and learning delivery backfill.
-- This migration preserves the legacy delivery relations for migration 043 reconciliation.

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM academic_core_cutover_audits
        WHERE migration_version = 41
          AND mapping_algorithm_version = 'academic-core-v1'
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_042_PREDECESSOR_AUDIT_MISSING';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM student_class_enrollments enrollment
        JOIN class_rooms homeroom ON homeroom.id = enrollment.class_room_id
        JOIN academic_years year ON year.id = homeroom.academic_year_id
        WHERE NOT (
            (enrollment.status = 'active' AND year.status IN ('active', 'planning', 'ready'))
            OR (enrollment.status = 'completed' AND year.status IN ('closed', 'archived'))
            OR enrollment.status IN ('transferred', 'dropped')
        )
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_042_ENROLLMENT_STATUS_UNRESOLVED';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM (
            SELECT enrollment.student_id, homeroom.academic_year_id
            FROM student_class_enrollments enrollment
            JOIN class_rooms homeroom ON homeroom.id = enrollment.class_room_id
            WHERE enrollment.status = 'active'
            GROUP BY enrollment.student_id, homeroom.academic_year_id
            HAVING COUNT(*) > 1
        ) duplicate_current
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_042_DUPLICATE_CURRENT_PLACEMENT';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM class_rooms homeroom
        LEFT JOIN study_programs program
          ON program.curriculum_version_id = homeroom.study_plan_version_id
         AND program.is_default
        WHERE program.id IS NULL
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_042_HOMEROOM_PROGRAM_UNRESOLVED';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM classroom_courses course
        JOIN class_rooms homeroom ON homeroom.id = course.classroom_id
        JOIN academic_terms term ON term.id = course.academic_semester_id
        WHERE homeroom.academic_year_id <> term.academic_year_id
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_042_COURSE_CONTEXT_MISMATCH';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM classroom_courses course
        JOIN class_rooms homeroom ON homeroom.id = course.classroom_id
        JOIN academic_terms term ON term.id = course.academic_semester_id
        LEFT JOIN curriculum_course_requirements requirement
          ON requirement.subject_version_id = course.subject_id
         AND requirement.study_program_id = uuid_generate_v5(
                '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
                'program:' || homeroom.study_plan_version_id::text
             )
         AND requirement.grade_level_id = homeroom.grade_level_id
         AND academic_normalize_identity(requirement.recommended_term_code)
             = academic_normalize_identity(term.legacy_term)
        WHERE requirement.id IS NULL
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_042_COURSE_REQUIREMENT_UNRESOLVED';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM classroom_courses course
        GROUP BY course.academic_semester_id, course.subject_id
        HAVING COUNT(DISTINCT course.settings) > 1
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_042_COURSE_SNAPSHOT_AMBIGUOUS';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM activity_group_members member
        JOIN activity_groups activity_group ON activity_group.id = member.activity_group_id
        JOIN activity_slots slot ON slot.id = activity_group.slot_id
        JOIN academic_terms term ON term.id = slot.semester_id
        WHERE NOT EXISTS (
            SELECT 1
            FROM student_class_enrollments enrollment
            JOIN class_rooms homeroom ON homeroom.id = enrollment.class_room_id
            WHERE enrollment.student_id = member.student_id
              AND homeroom.academic_year_id = term.academic_year_id
        )
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_042_ACTIVITY_MEMBER_YEAR_UNRESOLVED';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM activity_groups activity_group
        JOIN activity_slots slot ON slot.id = activity_group.slot_id
        JOIN activity_versions activity ON activity.id = slot.activity_catalog_id
        WHERE activity.scheduling_mode = 'independent'
          AND activity_group.allowed_classroom_ids IS NULL
          AND (
              (SELECT COUNT(*) FROM activity_groups sibling WHERE sibling.slot_id = slot.id) <> 1
              OR (SELECT COUNT(*) FROM activity_slot_classroom_assignments assignment
                  WHERE assignment.slot_id = slot.id) <> 1
          )
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_042_ACTIVITY_GROUP_HOMEROOM_AMBIGUOUS';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM activity_groups activity_group
        WHERE activity_group.allowed_classroom_ids IS NOT NULL
          AND jsonb_typeof(activity_group.allowed_classroom_ids) <> 'array'
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_042_ACTIVITY_GROUP_HOMEROOM_INVALID';
    END IF;
END;
$$;

ALTER TABLE class_rooms RENAME TO homerooms;
ALTER TABLE homerooms RENAME COLUMN study_plan_version_id TO legacy_curriculum_version_id;
ALTER TABLE homerooms
    ADD COLUMN study_program_id UUID REFERENCES study_programs(id) ON DELETE RESTRICT,
    ADD COLUMN row_version BIGINT NOT NULL DEFAULT 1,
    ADD COLUMN migration_provenance JSONB NOT NULL DEFAULT '{}'::jsonb;

UPDATE homerooms homeroom
SET study_program_id = uuid_generate_v5(
        '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
        'program:' || homeroom.legacy_curriculum_version_id::text
    ),
    migration_provenance = jsonb_build_object(
        'migration', 42,
        'mappingAlgorithm', 'academic-core-v1',
        'legacyCurriculumVersionId', homeroom.legacy_curriculum_version_id
    );

ALTER TABLE homerooms
    ALTER COLUMN study_program_id SET NOT NULL,
    ADD CONSTRAINT homerooms_row_version_check CHECK (row_version > 0),
    ADD CONSTRAINT homerooms_id_year_key UNIQUE (id, academic_year_id);

ALTER TABLE classroom_advisors RENAME TO homeroom_advisors;
ALTER TABLE homeroom_advisors RENAME COLUMN classroom_id TO homeroom_id;

CREATE TABLE student_academic_years (
    id UUID PRIMARY KEY,
    student_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    academic_year_id UUID NOT NULL REFERENCES academic_years(id) ON DELETE RESTRICT,
    grade_level_id UUID NOT NULL REFERENCES grade_levels(id) ON DELETE RESTRICT,
    study_program_id UUID NOT NULL REFERENCES study_programs(id) ON DELETE RESTRICT,
    status TEXT NOT NULL
        CHECK (status IN ('planned', 'active', 'completed', 'withdrawn', 'graduated')),
    row_version BIGINT NOT NULL DEFAULT 1 CHECK (row_version > 0),
    migration_provenance JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT student_academic_years_student_year_key UNIQUE (student_id, academic_year_id),
    CONSTRAINT student_academic_years_id_year_key UNIQUE (id, academic_year_id),
    CONSTRAINT student_academic_years_id_year_student_key
        UNIQUE (id, academic_year_id, student_id)
);

WITH ranked AS (
    SELECT enrollment.student_id,
           homeroom.academic_year_id,
           homeroom.grade_level_id,
           homeroom.study_program_id,
           row_number() OVER (
               PARTITION BY enrollment.student_id, homeroom.academic_year_id
               ORDER BY enrollment.enrollment_date DESC, enrollment.created_at DESC, enrollment.id DESC
           ) AS choice_rank,
           bool_or(enrollment.status = 'active') OVER (
               PARTITION BY enrollment.student_id, homeroom.academic_year_id
           ) AS has_active,
           bool_or(enrollment.status = 'completed') OVER (
               PARTITION BY enrollment.student_id, homeroom.academic_year_id
           ) AS has_completed,
           bool_or(enrollment.status = 'dropped') OVER (
               PARTITION BY enrollment.student_id, homeroom.academic_year_id
           ) AS has_dropped,
           min(enrollment.created_at) OVER (
               PARTITION BY enrollment.student_id, homeroom.academic_year_id
           ) AS first_created_at,
           max(enrollment.updated_at) OVER (
               PARTITION BY enrollment.student_id, homeroom.academic_year_id
           ) AS last_updated_at
    FROM student_class_enrollments enrollment
    JOIN homerooms homeroom ON homeroom.id = enrollment.class_room_id
)
INSERT INTO student_academic_years (
    id, student_id, academic_year_id, grade_level_id, study_program_id, status,
    migration_provenance, created_at, updated_at
)
SELECT uuid_generate_v5(
           '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
           'student-year:' || ranked.student_id::text || ':' || ranked.academic_year_id::text
       ),
       ranked.student_id,
       ranked.academic_year_id,
       ranked.grade_level_id,
       ranked.study_program_id,
       CASE
           WHEN ranked.has_active AND year.status = 'active' THEN 'active'
           WHEN ranked.has_active AND year.status IN ('planning', 'ready') THEN 'planned'
           WHEN ranked.has_completed AND year.status IN ('closed', 'archived') THEN 'completed'
           WHEN ranked.has_dropped THEN 'withdrawn'
           ELSE 'withdrawn'
       END,
       jsonb_build_object(
           'migration', 42,
           'mappingAlgorithm', 'academic-core-v1',
           'source', 'student_class_enrollments'
       ),
       ranked.first_created_at,
       ranked.last_updated_at
FROM ranked
JOIN academic_years year ON year.id = ranked.academic_year_id
WHERE ranked.choice_rank = 1;

CREATE TABLE homeroom_placements (
    id UUID PRIMARY KEY,
    student_academic_year_id UUID NOT NULL,
    academic_year_id UUID NOT NULL,
    homeroom_id UUID NOT NULL,
    start_date DATE NOT NULL,
    end_date DATE,
    status TEXT NOT NULL CHECK (status IN ('planned', 'current', 'ended')),
    enrollment_type TEXT NOT NULL,
    class_number INTEGER,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    row_version BIGINT NOT NULL DEFAULT 1 CHECK (row_version > 0),
    migration_provenance JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT homeroom_placements_date_order_check
        CHECK (end_date IS NULL OR start_date <= end_date),
    CONSTRAINT homeroom_placements_student_year_fkey
        FOREIGN KEY (student_academic_year_id, academic_year_id)
        REFERENCES student_academic_years(id, academic_year_id) ON DELETE RESTRICT,
    CONSTRAINT homeroom_placements_homeroom_context_fkey
        FOREIGN KEY (homeroom_id, academic_year_id)
        REFERENCES homerooms(id, academic_year_id) ON DELETE RESTRICT,
    CONSTRAINT homeroom_placements_id_year_key UNIQUE (id, academic_year_id)
);

CREATE UNIQUE INDEX homeroom_placements_one_current_key
    ON homeroom_placements(student_academic_year_id)
    WHERE status = 'current';

WITH placement_intervals AS (
    SELECT enrollment.*,
           homeroom.academic_year_id,
           year.end_date AS academic_year_end,
           lead(enrollment.enrollment_date) OVER (
               PARTITION BY enrollment.student_id, homeroom.academic_year_id
               ORDER BY enrollment.enrollment_date, enrollment.created_at, enrollment.id
           ) AS next_start_date
    FROM student_class_enrollments enrollment
    JOIN homerooms homeroom ON homeroom.id = enrollment.class_room_id
    JOIN academic_years year ON year.id = homeroom.academic_year_id
)
INSERT INTO homeroom_placements (
    id, student_academic_year_id, academic_year_id, homeroom_id, start_date, end_date,
    status, enrollment_type, class_number, metadata, migration_provenance, created_at, updated_at
)
SELECT placement.id,
       uuid_generate_v5(
           '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
           'student-year:' || placement.student_id::text || ':' || placement.academic_year_id::text
       ),
       placement.academic_year_id,
       placement.class_room_id,
       placement.enrollment_date,
       CASE
           WHEN placement.status = 'active' THEN NULL
           WHEN placement.next_start_date IS NOT NULL THEN placement.next_start_date - 1
           ELSE placement.academic_year_end
       END,
       CASE WHEN placement.status = 'active' THEN 'current' ELSE 'ended' END,
       placement.enrollment_type,
       placement.class_number,
       COALESCE(placement.metadata, '{}'::jsonb),
       jsonb_build_object(
           'migration', 42,
           'mappingAlgorithm', 'academic-core-v1',
           'legacyStatus', placement.status
       ),
       placement.created_at,
       placement.updated_at
FROM placement_intervals placement;

CREATE TABLE academic_core_entity_map (
    source_table TEXT NOT NULL,
    source_id UUID NOT NULL,
    target_table TEXT NOT NULL,
    target_id UUID NOT NULL,
    mapping_rule TEXT NOT NULL,
    migration_version BIGINT NOT NULL DEFAULT 42 CHECK (migration_version >= 42),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (source_table, source_id, target_table, target_id)
);

CREATE TABLE learning_offerings (
    id UUID PRIMARY KEY,
    academic_term_id UUID NOT NULL,
    academic_year_id UUID NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('course', 'activity')),
    code_snapshot TEXT NOT NULL CHECK (btrim(code_snapshot) <> ''),
    name_snapshot TEXT NOT NULL CHECK (btrim(name_snapshot) <> ''),
    source_requirement_kind TEXT,
    source_requirement_id UUID,
    status TEXT NOT NULL CHECK (status IN ('draft', 'published', 'closed')),
    published_at TIMESTAMPTZ,
    owning_organization_unit_id UUID REFERENCES organization_units(id) ON DELETE RESTRICT,
    row_version BIGINT NOT NULL DEFAULT 1 CHECK (row_version > 0),
    migration_provenance JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT learning_offerings_term_context_fkey
        FOREIGN KEY (academic_term_id, academic_year_id)
        REFERENCES academic_terms(id, academic_year_id) ON DELETE RESTRICT,
    CONSTRAINT learning_offerings_id_term_year_key
        UNIQUE (id, academic_term_id, academic_year_id)
);

CREATE TABLE course_offering_details (
    learning_offering_id UUID PRIMARY KEY,
    academic_term_id UUID NOT NULL,
    academic_year_id UUID NOT NULL,
    subject_version_id UUID NOT NULL,
    subject_id UUID NOT NULL,
    curriculum_course_requirement_id UUID REFERENCES curriculum_course_requirements(id)
        ON DELETE RESTRICT,
    credit NUMERIC(8,2) NOT NULL CHECK (credit >= 0),
    hours NUMERIC(10,2) CHECK (hours IS NULL OR hours >= 0),
    grading_policy JSONB NOT NULL DEFAULT '{}'::jsonb,
    migration_provenance JSONB NOT NULL DEFAULT '{}'::jsonb,
    CONSTRAINT course_offering_details_offering_context_fkey
        FOREIGN KEY (learning_offering_id, academic_term_id, academic_year_id)
        REFERENCES learning_offerings(id, academic_term_id, academic_year_id) ON DELETE CASCADE,
    CONSTRAINT course_offering_details_subject_version_fkey
        FOREIGN KEY (subject_version_id, subject_id)
        REFERENCES subject_versions(id, subject_id) ON DELETE RESTRICT,
    CONSTRAINT course_offering_details_term_subject_key
        UNIQUE (academic_term_id, subject_version_id)
);

CREATE TABLE activity_offering_details (
    learning_offering_id UUID PRIMARY KEY,
    academic_term_id UUID NOT NULL,
    academic_year_id UUID NOT NULL,
    activity_version_id UUID NOT NULL,
    activity_id UUID NOT NULL,
    curriculum_activity_requirement_id UUID REFERENCES curriculum_activity_requirements(id)
        ON DELETE RESTRICT,
    registration_type TEXT NOT NULL CHECK (registration_type IN ('self', 'assigned')),
    scheduling_mode TEXT NOT NULL CHECK (scheduling_mode IN ('synchronized', 'independent')),
    hours NUMERIC(10,2) NOT NULL CHECK (hours >= 0),
    capacity INTEGER CHECK (capacity IS NULL OR capacity > 0),
    attendance_requirement JSONB NOT NULL DEFAULT '{}'::jsonb,
    pass_criteria JSONB NOT NULL DEFAULT '{}'::jsonb,
    migration_provenance JSONB NOT NULL DEFAULT '{}'::jsonb,
    CONSTRAINT activity_offering_details_offering_context_fkey
        FOREIGN KEY (learning_offering_id, academic_term_id, academic_year_id)
        REFERENCES learning_offerings(id, academic_term_id, academic_year_id) ON DELETE CASCADE,
    CONSTRAINT activity_offering_details_activity_version_fkey
        FOREIGN KEY (activity_version_id, activity_id)
        REFERENCES activity_versions(id, activity_id) ON DELETE RESTRICT
);

CREATE TABLE learning_offering_targets (
    id UUID PRIMARY KEY,
    learning_offering_id UUID NOT NULL,
    academic_term_id UUID NOT NULL,
    academic_year_id UUID NOT NULL,
    target_kind TEXT NOT NULL CHECK (target_kind IN ('homeroom', 'grade_program')),
    homeroom_id UUID,
    grade_level_id UUID NOT NULL REFERENCES grade_levels(id) ON DELETE RESTRICT,
    study_program_id UUID NOT NULL REFERENCES study_programs(id) ON DELETE RESTRICT,
    migration_provenance JSONB NOT NULL DEFAULT '{}'::jsonb,
    CONSTRAINT learning_offering_targets_offering_context_fkey
        FOREIGN KEY (learning_offering_id, academic_term_id, academic_year_id)
        REFERENCES learning_offerings(id, academic_term_id, academic_year_id) ON DELETE CASCADE,
    CONSTRAINT learning_offering_targets_homeroom_context_fkey
        FOREIGN KEY (homeroom_id, academic_year_id)
        REFERENCES homerooms(id, academic_year_id) ON DELETE RESTRICT,
    CONSTRAINT learning_offering_targets_homeroom_shape_check
        CHECK ((target_kind = 'homeroom' AND homeroom_id IS NOT NULL)
            OR (target_kind = 'grade_program' AND homeroom_id IS NULL)),
    CONSTRAINT learning_offering_targets_unique_key
        UNIQUE (learning_offering_id, target_kind, homeroom_id, grade_level_id, study_program_id)
);

WITH course_sources AS (
    SELECT course.academic_semester_id AS academic_term_id,
           term.academic_year_id,
           course.subject_id AS subject_version_id,
           version.subject_id,
           version.code,
           version.name_th,
           stable.owning_organization_unit_id,
           min(requirement.id::text)::uuid AS requirement_id,
           min(course.created_at) AS created_at,
           max(course.updated_at) AS updated_at
    FROM classroom_courses course
    JOIN homerooms homeroom ON homeroom.id = course.classroom_id
    JOIN academic_terms term ON term.id = course.academic_semester_id
    JOIN subject_versions version ON version.id = course.subject_id
    JOIN subjects stable ON stable.id = version.subject_id
    LEFT JOIN curriculum_course_requirements requirement
      ON requirement.subject_version_id = course.subject_id
     AND requirement.study_program_id = homeroom.study_program_id
     AND requirement.grade_level_id = homeroom.grade_level_id
     AND academic_normalize_identity(requirement.recommended_term_code)
         = academic_normalize_identity(term.legacy_term)
    GROUP BY course.academic_semester_id, term.academic_year_id, course.subject_id,
             version.subject_id, version.code, version.name_th,
             stable.owning_organization_unit_id
)
INSERT INTO learning_offerings (
    id, academic_term_id, academic_year_id, kind, code_snapshot, name_snapshot,
    source_requirement_kind, source_requirement_id, status, published_at,
    owning_organization_unit_id, migration_provenance, created_at, updated_at
)
SELECT uuid_generate_v5(
           '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
           'course-offering:' || source.academic_term_id::text || ':' || source.subject_version_id::text
       ),
       source.academic_term_id,
       source.academic_year_id,
       'course',
       source.code,
       source.name_th,
       'curriculum_course_requirement',
       source.requirement_id,
       'published',
       source.updated_at,
       source.owning_organization_unit_id,
       jsonb_build_object(
           'migration', 42,
           'mappingAlgorithm', 'academic-core-v1',
           'source', 'classroom_courses'
       ),
       source.created_at,
       source.updated_at
FROM course_sources source;

INSERT INTO course_offering_details (
    learning_offering_id, academic_term_id, academic_year_id, subject_version_id,
    subject_id, curriculum_course_requirement_id, credit, hours, grading_policy,
    migration_provenance
)
SELECT offering.id,
       offering.academic_term_id,
       offering.academic_year_id,
       version.id,
       version.subject_id,
       offering.source_requirement_id,
       version.credit,
       version.hours_per_semester::numeric(10,2),
       min(course.settings::text)::jsonb,
       jsonb_build_object('migration', 42, 'mappingAlgorithm', 'academic-core-v1')
FROM learning_offerings offering
JOIN classroom_courses course
  ON course.academic_semester_id = offering.academic_term_id
JOIN subject_versions version
  ON version.id = course.subject_id
 AND version.id = (
     SELECT detail_course.subject_id
     FROM classroom_courses detail_course
     WHERE detail_course.academic_semester_id = offering.academic_term_id
       AND detail_course.subject_id = course.subject_id
     LIMIT 1
 )
WHERE offering.kind = 'course'
  AND offering.id = uuid_generate_v5(
      '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
      'course-offering:' || course.academic_semester_id::text || ':' || course.subject_id::text
  )
GROUP BY offering.id, offering.academic_term_id, offering.academic_year_id,
         version.id, version.subject_id, offering.source_requirement_id,
         version.credit, version.hours_per_semester;

WITH activity_sources AS (
    SELECT slot.id,
           slot.semester_id AS academic_term_id,
           term.academic_year_id,
           slot.activity_catalog_id AS activity_version_id,
           version.activity_id,
           stable.code,
           version.name,
           stable.owning_organization_unit_id,
           min(requirement.id::text)::uuid AS requirement_id,
           slot.registration_type,
           version.scheduling_mode,
           version.hours_per_week,
           slot.created_at,
           slot.updated_at
    FROM activity_slots slot
    JOIN academic_terms term ON term.id = slot.semester_id
    JOIN activity_versions version ON version.id = slot.activity_catalog_id
    JOIN activities stable ON stable.id = version.activity_id
    LEFT JOIN activity_slot_classrooms slot_homeroom ON slot_homeroom.slot_id = slot.id
    LEFT JOIN homerooms homeroom ON homeroom.id = slot_homeroom.classroom_id
    LEFT JOIN curriculum_activity_requirements requirement
      ON requirement.activity_version_id = slot.activity_catalog_id
     AND requirement.study_program_id = homeroom.study_program_id
     AND requirement.grade_level_id = homeroom.grade_level_id
     AND academic_normalize_identity(requirement.recommended_term_code)
         = academic_normalize_identity(term.legacy_term)
    GROUP BY slot.id, slot.semester_id, term.academic_year_id, slot.activity_catalog_id,
             version.activity_id, stable.code, version.name,
             stable.owning_organization_unit_id, slot.registration_type,
             version.scheduling_mode, version.hours_per_week, slot.created_at, slot.updated_at
)
INSERT INTO learning_offerings (
    id, academic_term_id, academic_year_id, kind, code_snapshot, name_snapshot,
    source_requirement_kind, source_requirement_id, status, published_at,
    owning_organization_unit_id, migration_provenance, created_at, updated_at
)
SELECT source.id,
       source.academic_term_id,
       source.academic_year_id,
       'activity',
       source.code,
       source.name,
       CASE WHEN source.requirement_id IS NULL THEN NULL ELSE 'curriculum_activity_requirement' END,
       source.requirement_id,
       'published',
       source.updated_at,
       source.owning_organization_unit_id,
       jsonb_build_object(
           'migration', 42,
           'mappingAlgorithm', 'academic-core-v1',
           'source', 'activity_slots'
       ),
       source.created_at,
       source.updated_at
FROM activity_sources source;

INSERT INTO activity_offering_details (
    learning_offering_id, academic_term_id, academic_year_id, activity_version_id,
    activity_id, curriculum_activity_requirement_id, registration_type,
    scheduling_mode, hours, attendance_requirement, pass_criteria, migration_provenance
)
SELECT offering.id,
       offering.academic_term_id,
       offering.academic_year_id,
       version.id,
       version.activity_id,
       offering.source_requirement_id,
       slot.registration_type,
       version.scheduling_mode,
       version.hours_per_week,
       '{}'::jsonb,
       jsonb_build_object('outcomes', jsonb_build_array('pass', 'fail')),
       jsonb_build_object('migration', 42, 'mappingAlgorithm', 'academic-core-v1')
FROM learning_offerings offering
JOIN activity_slots slot ON slot.id = offering.id
JOIN activity_versions version ON version.id = slot.activity_catalog_id
WHERE offering.kind = 'activity';

INSERT INTO learning_offering_targets (
    id, learning_offering_id, academic_term_id, academic_year_id, target_kind,
    homeroom_id, grade_level_id, study_program_id, migration_provenance
)
SELECT uuid_generate_v5(
           '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
           'offering-target:' || offering.id::text || ':' || homeroom.id::text
       ),
       offering.id,
       offering.academic_term_id,
       offering.academic_year_id,
       'homeroom',
       homeroom.id,
       homeroom.grade_level_id,
       homeroom.study_program_id,
       jsonb_build_object('migration', 42, 'mappingAlgorithm', 'academic-core-v1')
FROM classroom_courses course
JOIN homerooms homeroom ON homeroom.id = course.classroom_id
JOIN learning_offerings offering
  ON offering.kind = 'course'
 AND offering.id = uuid_generate_v5(
     '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
     'course-offering:' || course.academic_semester_id::text || ':' || course.subject_id::text
 )
ON CONFLICT (learning_offering_id, target_kind, homeroom_id, grade_level_id, study_program_id)
DO NOTHING;

INSERT INTO learning_offering_targets (
    id, learning_offering_id, academic_term_id, academic_year_id, target_kind,
    homeroom_id, grade_level_id, study_program_id, migration_provenance
)
SELECT uuid_generate_v5(
           '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
           'offering-target:' || slot.id::text || ':' || homeroom.id::text
       ),
       slot.id,
       term.id,
       term.academic_year_id,
       'homeroom',
       homeroom.id,
       homeroom.grade_level_id,
       homeroom.study_program_id,
       jsonb_build_object('migration', 42, 'mappingAlgorithm', 'academic-core-v1')
FROM activity_slot_classrooms coverage
JOIN activity_slots slot ON slot.id = coverage.slot_id
JOIN academic_terms term ON term.id = slot.semester_id
JOIN homerooms homeroom ON homeroom.id = coverage.classroom_id
ON CONFLICT (learning_offering_id, target_kind, homeroom_id, grade_level_id, study_program_id)
DO NOTHING;

CREATE TABLE learning_groups (
    id UUID PRIMARY KEY,
    learning_offering_id UUID NOT NULL,
    academic_term_id UUID NOT NULL,
    academic_year_id UUID NOT NULL,
    code TEXT NOT NULL CHECK (btrim(code) <> ''),
    name TEXT NOT NULL CHECK (btrim(name) <> ''),
    description TEXT,
    capacity INTEGER CHECK (capacity IS NULL OR capacity > 0),
    status TEXT NOT NULL CHECK (status IN ('draft', 'published', 'closed')),
    roster_status TEXT NOT NULL CHECK (roster_status IN ('draft', 'published', 'closed')),
    roster_published_at TIMESTAMPTZ,
    row_version BIGINT NOT NULL DEFAULT 1 CHECK (row_version > 0),
    migration_provenance JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT learning_groups_offering_context_fkey
        FOREIGN KEY (learning_offering_id, academic_term_id, academic_year_id)
        REFERENCES learning_offerings(id, academic_term_id, academic_year_id) ON DELETE RESTRICT,
    CONSTRAINT learning_groups_id_term_year_key
        UNIQUE (id, academic_term_id, academic_year_id)
);

CREATE TABLE learning_group_homerooms (
    id UUID PRIMARY KEY,
    learning_group_id UUID NOT NULL,
    academic_term_id UUID NOT NULL,
    academic_year_id UUID NOT NULL,
    homeroom_id UUID NOT NULL,
    coverage_source TEXT NOT NULL,
    migration_provenance JSONB NOT NULL DEFAULT '{}'::jsonb,
    CONSTRAINT learning_group_homerooms_group_context_fkey
        FOREIGN KEY (learning_group_id, academic_term_id, academic_year_id)
        REFERENCES learning_groups(id, academic_term_id, academic_year_id) ON DELETE CASCADE,
    CONSTRAINT learning_group_homerooms_homeroom_context_fkey
        FOREIGN KEY (homeroom_id, academic_year_id)
        REFERENCES homerooms(id, academic_year_id) ON DELETE RESTRICT,
    CONSTRAINT learning_group_homerooms_unique_key UNIQUE (learning_group_id, homeroom_id)
);

CREATE TABLE learning_group_teachers (
    id UUID PRIMARY KEY,
    learning_group_id UUID NOT NULL,
    academic_term_id UUID NOT NULL,
    academic_year_id UUID NOT NULL,
    teacher_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    role TEXT NOT NULL CHECK (role IN ('primary', 'secondary', 'assistant')),
    migration_provenance JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT learning_group_teachers_group_context_fkey
        FOREIGN KEY (learning_group_id, academic_term_id, academic_year_id)
        REFERENCES learning_groups(id, academic_term_id, academic_year_id) ON DELETE CASCADE,
    CONSTRAINT learning_group_teachers_unique_key UNIQUE (learning_group_id, teacher_id)
);

CREATE TABLE learning_group_students (
    id UUID PRIMARY KEY,
    learning_group_id UUID NOT NULL,
    academic_term_id UUID NOT NULL,
    academic_year_id UUID NOT NULL,
    student_academic_year_id UUID NOT NULL,
    student_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    membership_status TEXT NOT NULL CHECK (membership_status IN ('active', 'ended', 'removed')),
    roster_source TEXT NOT NULL,
    joined_at DATE NOT NULL,
    left_at DATE,
    published_at TIMESTAMPTZ,
    row_version BIGINT NOT NULL DEFAULT 1 CHECK (row_version > 0),
    migration_provenance JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT learning_group_students_date_order_check
        CHECK (left_at IS NULL OR joined_at <= left_at),
    CONSTRAINT learning_group_students_group_context_fkey
        FOREIGN KEY (learning_group_id, academic_term_id, academic_year_id)
        REFERENCES learning_groups(id, academic_term_id, academic_year_id) ON DELETE CASCADE,
    CONSTRAINT learning_group_students_student_year_context_fkey
        FOREIGN KEY (student_academic_year_id, academic_year_id, student_id)
        REFERENCES student_academic_years(id, academic_year_id, student_id) ON DELETE RESTRICT
);

CREATE UNIQUE INDEX learning_group_students_one_active_key
    ON learning_group_students(learning_group_id, student_id)
    WHERE membership_status = 'active';

CREATE TABLE learning_group_preferred_rooms (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    learning_group_id UUID NOT NULL,
    academic_term_id UUID NOT NULL,
    academic_year_id UUID NOT NULL,
    room_id UUID NOT NULL REFERENCES rooms(id) ON DELETE RESTRICT,
    rank INTEGER NOT NULL DEFAULT 1 CHECK (rank > 0),
    is_required BOOLEAN NOT NULL DEFAULT false,
    migration_provenance JSONB NOT NULL DEFAULT '{}'::jsonb,
    CONSTRAINT learning_group_preferred_rooms_group_context_fkey
        FOREIGN KEY (learning_group_id, academic_term_id, academic_year_id)
        REFERENCES learning_groups(id, academic_term_id, academic_year_id) ON DELETE CASCADE,
    CONSTRAINT learning_group_preferred_rooms_unique_key UNIQUE (learning_group_id, room_id)
);

INSERT INTO learning_groups (
    id, learning_offering_id, academic_term_id, academic_year_id, code, name,
    capacity, status, roster_status, roster_published_at, migration_provenance,
    created_at, updated_at
)
SELECT course.id,
       offering.id,
       term.id,
       term.academic_year_id,
       homeroom.code || '-' || version.code,
       homeroom.name || ' · ' || version.name_th,
       homeroom.capacity,
       'published',
       'published',
       course.updated_at,
       jsonb_build_object(
           'migration', 42,
           'mappingAlgorithm', 'academic-core-v1',
           'source', 'classroom_courses'
       ),
       course.created_at,
       course.updated_at
FROM classroom_courses course
JOIN homerooms homeroom ON homeroom.id = course.classroom_id
JOIN subject_versions version ON version.id = course.subject_id
JOIN academic_terms term ON term.id = course.academic_semester_id
JOIN learning_offerings offering
  ON offering.id = uuid_generate_v5(
      '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
      'course-offering:' || course.academic_semester_id::text || ':' || course.subject_id::text
  );

INSERT INTO learning_group_homerooms (
    id, learning_group_id, academic_term_id, academic_year_id, homeroom_id,
    coverage_source, migration_provenance
)
SELECT uuid_generate_v5(
           '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
           'group-homeroom:' || course.id::text || ':' || homeroom.id::text
       ),
       course.id,
       term.id,
       term.academic_year_id,
       homeroom.id,
       'legacy_classroom_course',
       jsonb_build_object('migration', 42, 'mappingAlgorithm', 'academic-core-v1')
FROM classroom_courses course
JOIN homerooms homeroom ON homeroom.id = course.classroom_id
JOIN academic_terms term ON term.id = course.academic_semester_id;

INSERT INTO learning_group_teachers (
    id, learning_group_id, academic_term_id, academic_year_id, teacher_id,
    role, migration_provenance, created_at
)
SELECT instructor.id,
       instructor.classroom_course_id,
       term.id,
       term.academic_year_id,
       instructor.instructor_id,
       CASE WHEN instructor.role = 'primary' THEN 'primary' ELSE 'secondary' END,
       jsonb_build_object('migration', 42, 'mappingAlgorithm', 'academic-core-v1'),
       instructor.created_at
FROM classroom_course_instructors instructor
JOIN classroom_courses course ON course.id = instructor.classroom_course_id
JOIN academic_terms term ON term.id = course.academic_semester_id
ON CONFLICT (learning_group_id, teacher_id) DO NOTHING;

INSERT INTO learning_group_students (
    id, learning_group_id, academic_term_id, academic_year_id,
    student_academic_year_id, student_id, membership_status, roster_source,
    joined_at, left_at, published_at, migration_provenance, created_at, updated_at
)
SELECT uuid_generate_v5(
           '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
           'course-roster:' || course.id::text || ':' || placement.id::text
       ),
       course.id,
       term.id,
       term.academic_year_id,
       placement.student_academic_year_id,
       student_year.student_id,
       CASE WHEN term.status IN ('closed', 'cancelled') THEN 'ended' ELSE 'active' END,
       'migration_homeroom_snapshot',
       GREATEST(placement.start_date, term.start_date),
       CASE WHEN term.status IN ('closed', 'cancelled')
            THEN LEAST(COALESCE(placement.end_date, term.end_date), term.end_date)
            ELSE NULL END,
       course.updated_at,
       jsonb_build_object(
           'migration', 42,
           'mappingAlgorithm', 'academic-core-v1',
           'sourcePlacementId', placement.id
       ),
       placement.created_at,
       GREATEST(placement.updated_at, course.updated_at)
FROM classroom_courses course
JOIN academic_terms term ON term.id = course.academic_semester_id
JOIN homeroom_placements placement ON placement.homeroom_id = course.classroom_id
JOIN student_academic_years student_year ON student_year.id = placement.student_academic_year_id
WHERE placement.start_date <= term.end_date
  AND COALESCE(placement.end_date, term.end_date) >= term.start_date
  AND (placement.migration_provenance->>'legacyStatus') IN ('active', 'completed', 'transferred');

INSERT INTO learning_groups (
    id, learning_offering_id, academic_term_id, academic_year_id, code, name,
    description, capacity, status, roster_status, roster_published_at,
    migration_provenance, created_at, updated_at
)
SELECT activity_group.id,
       slot.id,
       term.id,
       term.academic_year_id,
       'ACT-' || upper(substr(replace(activity_group.id::text, '-', ''), 1, 12)),
       activity_group.name,
       activity_group.description,
       activity_group.max_capacity,
       CASE WHEN activity_group.is_active THEN 'published' ELSE 'closed' END,
       CASE WHEN activity_group.is_active THEN 'published' ELSE 'closed' END,
       activity_group.updated_at,
       jsonb_build_object(
           'migration', 42,
           'mappingAlgorithm', 'academic-core-v1',
           'source', 'activity_groups'
       ),
       activity_group.created_at,
       activity_group.updated_at
FROM activity_groups activity_group
JOIN activity_slots slot ON slot.id = activity_group.slot_id
JOIN academic_terms term ON term.id = slot.semester_id;

WITH uncovered AS (
    SELECT assignment.id AS assignment_id,
           assignment.slot_id,
           assignment.classroom_id,
           assignment.instructor_id,
           assignment.created_at,
           homeroom.name AS homeroom_name,
           homeroom.capacity,
           term.id AS academic_term_id,
           term.academic_year_id
    FROM activity_slot_classroom_assignments assignment
    JOIN activity_slots slot ON slot.id = assignment.slot_id
    JOIN activity_versions activity ON activity.id = slot.activity_catalog_id
    JOIN academic_terms term ON term.id = slot.semester_id
    JOIN homerooms homeroom ON homeroom.id = assignment.classroom_id
    WHERE activity.scheduling_mode = 'independent'
      AND NOT EXISTS (
          SELECT 1
          FROM activity_groups activity_group
          WHERE activity_group.slot_id = assignment.slot_id
            AND (
                (activity_group.allowed_classroom_ids IS NOT NULL
                 AND activity_group.allowed_classroom_ids ? assignment.classroom_id::text)
                OR (
                    activity_group.allowed_classroom_ids IS NULL
                    AND (SELECT COUNT(*) FROM activity_groups sibling
                         WHERE sibling.slot_id = assignment.slot_id) = 1
                    AND (SELECT COUNT(*) FROM activity_slot_classroom_assignments sibling_assignment
                         WHERE sibling_assignment.slot_id = assignment.slot_id) = 1
                )
            )
      )
)
INSERT INTO learning_groups (
    id, learning_offering_id, academic_term_id, academic_year_id, code, name,
    capacity, status, roster_status, roster_published_at, migration_provenance,
    created_at, updated_at
)
SELECT uuid_generate_v5(
           '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
           'activity-group:' || uncovered.slot_id::text || ':' || uncovered.classroom_id::text
       ),
       uncovered.slot_id,
       uncovered.academic_term_id,
       uncovered.academic_year_id,
       'ACT-' || upper(substr(replace(uncovered.slot_id::text, '-', ''), 1, 6))
           || '-' || upper(substr(replace(uncovered.classroom_id::text, '-', ''), 1, 6)),
       'กิจกรรม · ' || uncovered.homeroom_name,
       uncovered.capacity,
       'published',
       'published',
       uncovered.created_at,
       jsonb_build_object(
           'migration', 42,
           'mappingAlgorithm', 'academic-core-v1',
           'source', 'activity_slot_classroom_assignments',
           'generated', true
       ),
       uncovered.created_at,
       uncovered.created_at
FROM uncovered;

INSERT INTO learning_group_homerooms (
    id, learning_group_id, academic_term_id, academic_year_id, homeroom_id,
    coverage_source, migration_provenance
)
SELECT coverage.id,
       activity_group.id,
       term.id,
       term.academic_year_id,
       coverage.classroom_id,
       'legacy_activity_slot',
       jsonb_build_object('migration', 42, 'mappingAlgorithm', 'academic-core-v1')
FROM activity_groups activity_group
JOIN activity_slots slot ON slot.id = activity_group.slot_id
JOIN activity_versions activity ON activity.id = slot.activity_catalog_id
JOIN academic_terms term ON term.id = slot.semester_id
JOIN activity_slot_classrooms coverage ON coverage.slot_id = slot.id
WHERE activity.scheduling_mode = 'synchronized'
  AND (activity_group.allowed_classroom_ids IS NULL
       OR activity_group.allowed_classroom_ids ? coverage.classroom_id::text)
ON CONFLICT (learning_group_id, homeroom_id) DO NOTHING;

INSERT INTO learning_group_homerooms (
    id, learning_group_id, academic_term_id, academic_year_id, homeroom_id,
    coverage_source, migration_provenance
)
SELECT uuid_generate_v5(
           '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
           'activity-group-homeroom:' || activity_group.id::text || ':' || assignment.classroom_id::text
       ),
       activity_group.id,
       term.id,
       term.academic_year_id,
       assignment.classroom_id,
       'legacy_activity_assignment',
       jsonb_build_object('migration', 42, 'mappingAlgorithm', 'academic-core-v1')
FROM activity_groups activity_group
JOIN activity_slots slot ON slot.id = activity_group.slot_id
JOIN activity_versions activity ON activity.id = slot.activity_catalog_id
JOIN academic_terms term ON term.id = slot.semester_id
JOIN activity_slot_classroom_assignments assignment ON assignment.slot_id = slot.id
WHERE activity.scheduling_mode = 'independent'
  AND (
      (activity_group.allowed_classroom_ids IS NOT NULL
       AND activity_group.allowed_classroom_ids ? assignment.classroom_id::text)
      OR (
          activity_group.allowed_classroom_ids IS NULL
          AND (SELECT COUNT(*) FROM activity_groups sibling
               WHERE sibling.slot_id = slot.id) = 1
          AND (SELECT COUNT(*) FROM activity_slot_classroom_assignments sibling_assignment
               WHERE sibling_assignment.slot_id = slot.id) = 1
      )
  )
ON CONFLICT (learning_group_id, homeroom_id) DO NOTHING;

INSERT INTO learning_group_homerooms (
    id, learning_group_id, academic_term_id, academic_year_id, homeroom_id,
    coverage_source, migration_provenance
)
SELECT uuid_generate_v5(
           '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
           'activity-group-homeroom:' || generated.id::text || ':' || assignment.classroom_id::text
       ),
       generated.id,
       term.id,
       term.academic_year_id,
       assignment.classroom_id,
       'generated_independent_assignment',
       jsonb_build_object('migration', 42, 'mappingAlgorithm', 'academic-core-v1')
FROM activity_slot_classroom_assignments assignment
JOIN activity_slots slot ON slot.id = assignment.slot_id
JOIN academic_terms term ON term.id = slot.semester_id
JOIN learning_groups generated
  ON generated.id = uuid_generate_v5(
      '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
      'activity-group:' || assignment.slot_id::text || ':' || assignment.classroom_id::text
  );

INSERT INTO learning_group_teachers (
    id, learning_group_id, academic_term_id, academic_year_id, teacher_id,
    role, migration_provenance, created_at
)
SELECT instructor.id,
       instructor.activity_group_id,
       term.id,
       term.academic_year_id,
       instructor.instructor_id,
       instructor.role,
       jsonb_build_object('migration', 42, 'mappingAlgorithm', 'academic-core-v1'),
       now()
FROM activity_group_instructors instructor
JOIN activity_groups activity_group ON activity_group.id = instructor.activity_group_id
JOIN activity_slots slot ON slot.id = activity_group.slot_id
JOIN academic_terms term ON term.id = slot.semester_id
ON CONFLICT (learning_group_id, teacher_id) DO NOTHING;

INSERT INTO learning_group_teachers (
    id, learning_group_id, academic_term_id, academic_year_id, teacher_id,
    role, migration_provenance, created_at
)
SELECT uuid_generate_v5(
           '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
           'activity-group-teacher:' || activity_group.id::text || ':' || activity_group.instructor_id::text
       ),
       activity_group.id,
       term.id,
       term.academic_year_id,
       activity_group.instructor_id,
       'primary',
       jsonb_build_object('migration', 42, 'mappingAlgorithm', 'academic-core-v1'),
       activity_group.created_at
FROM activity_groups activity_group
JOIN activity_slots slot ON slot.id = activity_group.slot_id
JOIN academic_terms term ON term.id = slot.semester_id
WHERE activity_group.instructor_id IS NOT NULL
ON CONFLICT (learning_group_id, teacher_id) DO NOTHING;

INSERT INTO learning_group_teachers (
    id, learning_group_id, academic_term_id, academic_year_id, teacher_id,
    role, migration_provenance, created_at
)
SELECT uuid_generate_v5(
           '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
           'activity-slot-teacher:' || instructor.id::text || ':' || activity_group.id::text
       ),
       activity_group.id,
       term.id,
       term.academic_year_id,
       instructor.user_id,
       'assistant',
       jsonb_build_object(
           'migration', 42,
           'mappingAlgorithm', 'academic-core-v1',
           'source', 'activity_slot_instructors'
       ),
       instructor.created_at
FROM activity_slot_instructors instructor
JOIN activity_slots slot ON slot.id = instructor.slot_id
JOIN academic_terms term ON term.id = slot.semester_id
JOIN learning_groups activity_group ON activity_group.learning_offering_id = slot.id
ON CONFLICT (learning_group_id, teacher_id) DO NOTHING;

INSERT INTO learning_group_teachers (
    id, learning_group_id, academic_term_id, academic_year_id, teacher_id,
    role, migration_provenance, created_at
)
SELECT assignment.id,
       coverage.learning_group_id,
       term.id,
       term.academic_year_id,
       assignment.instructor_id,
       'primary',
       jsonb_build_object('migration', 42, 'mappingAlgorithm', 'academic-core-v1'),
       assignment.created_at
FROM activity_slot_classroom_assignments assignment
JOIN activity_slots slot ON slot.id = assignment.slot_id
JOIN academic_terms term ON term.id = slot.semester_id
JOIN learning_group_homerooms coverage
  ON coverage.homeroom_id = assignment.classroom_id
JOIN learning_groups activity_group
  ON activity_group.id = coverage.learning_group_id
 AND activity_group.learning_offering_id = assignment.slot_id
ON CONFLICT (learning_group_id, teacher_id) DO NOTHING;

INSERT INTO learning_group_students (
    id, learning_group_id, academic_term_id, academic_year_id,
    student_academic_year_id, student_id, membership_status, roster_source,
    joined_at, left_at, published_at, migration_provenance, created_at, updated_at
)
SELECT member.id,
       member.activity_group_id,
       term.id,
       term.academic_year_id,
       student_year.id,
       member.student_id,
       CASE WHEN term.status IN ('closed', 'cancelled') THEN 'ended' ELSE 'active' END,
       'legacy_activity_member',
       GREATEST(member.enrolled_at::date, term.start_date),
       CASE WHEN term.status IN ('closed', 'cancelled') THEN term.end_date ELSE NULL END,
       activity_group.updated_at,
       jsonb_build_object('migration', 42, 'mappingAlgorithm', 'academic-core-v1'),
       member.enrolled_at,
       activity_group.updated_at
FROM activity_group_members member
JOIN activity_groups activity_group ON activity_group.id = member.activity_group_id
JOIN activity_slots slot ON slot.id = activity_group.slot_id
JOIN academic_terms term ON term.id = slot.semester_id
JOIN student_academic_years student_year
  ON student_year.student_id = member.student_id
 AND student_year.academic_year_id = term.academic_year_id;

CREATE TABLE learning_results (
    id UUID PRIMARY KEY,
    learning_offering_id UUID NOT NULL,
    learning_group_id UUID NOT NULL,
    academic_term_id UUID NOT NULL,
    academic_year_id UUID NOT NULL,
    student_academic_year_id UUID NOT NULL,
    student_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    kind TEXT NOT NULL CHECK (kind IN ('course', 'activity')),
    status TEXT NOT NULL CHECK (status IN ('recorded', 'voided')),
    row_version BIGINT NOT NULL DEFAULT 1 CHECK (row_version > 0),
    migration_provenance JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT learning_results_group_context_fkey
        FOREIGN KEY (learning_group_id, academic_term_id, academic_year_id)
        REFERENCES learning_groups(id, academic_term_id, academic_year_id) ON DELETE RESTRICT,
    CONSTRAINT learning_results_student_year_context_fkey
        FOREIGN KEY (student_academic_year_id, academic_year_id, student_id)
        REFERENCES student_academic_years(id, academic_year_id, student_id) ON DELETE RESTRICT,
    CONSTRAINT learning_results_group_student_key UNIQUE (learning_group_id, student_id)
);

CREATE TABLE activity_result_details (
    learning_result_id UUID PRIMARY KEY REFERENCES learning_results(id) ON DELETE CASCADE,
    outcome TEXT NOT NULL CHECK (outcome IN ('pass', 'fail')),
    evidence JSONB NOT NULL DEFAULT '{}'::jsonb,
    migration_provenance JSONB NOT NULL DEFAULT '{}'::jsonb
);

INSERT INTO learning_results (
    id, learning_offering_id, learning_group_id, academic_term_id, academic_year_id,
    student_academic_year_id, student_id, kind, status, migration_provenance,
    created_at, updated_at
)
SELECT uuid_generate_v5(
           '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
           'activity-result:' || member.id::text
       ),
       slot.id,
       activity_group.id,
       term.id,
       term.academic_year_id,
       student_year.id,
       member.student_id,
       'activity',
       'recorded',
       jsonb_build_object(
           'migration', 42,
           'mappingAlgorithm', 'academic-core-v1',
           'source', 'activity_group_members'
       ),
       member.enrolled_at,
       activity_group.updated_at
FROM activity_group_members member
JOIN activity_groups activity_group ON activity_group.id = member.activity_group_id
JOIN activity_slots slot ON slot.id = activity_group.slot_id
JOIN academic_terms term ON term.id = slot.semester_id
JOIN student_academic_years student_year
  ON student_year.student_id = member.student_id
 AND student_year.academic_year_id = term.academic_year_id
WHERE member.result IS NOT NULL;

INSERT INTO activity_result_details (
    learning_result_id, outcome, migration_provenance
)
SELECT uuid_generate_v5(
           '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
           'activity-result:' || member.id::text
       ),
       member.result,
       jsonb_build_object('migration', 42, 'mappingAlgorithm', 'academic-core-v1')
FROM activity_group_members member
WHERE member.result IS NOT NULL;

INSERT INTO academic_core_entity_map (
    source_table, source_id, target_table, target_id, mapping_rule
)
SELECT 'class_rooms', id, 'homerooms', id, 'rename-preserve-id' FROM homerooms
UNION ALL
SELECT 'student_class_enrollments', enrollment.id, 'student_academic_years',
       student_year.id, 'split-deterministic-student-year'
FROM student_class_enrollments enrollment
JOIN homerooms homeroom ON homeroom.id = enrollment.class_room_id
JOIN student_academic_years student_year
  ON student_year.student_id = enrollment.student_id
 AND student_year.academic_year_id = homeroom.academic_year_id
UNION ALL
SELECT 'student_class_enrollments', id, 'homeroom_placements', id, 'split-preserve-placement-id'
FROM student_class_enrollments
UNION ALL
SELECT 'classroom_courses', course.id, 'learning_offerings', offering.id,
       'merge-term-subject-version'
FROM classroom_courses course
JOIN learning_offerings offering
  ON offering.id = uuid_generate_v5(
      '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
      'course-offering:' || course.academic_semester_id::text || ':' || course.subject_id::text
  )
UNION ALL
SELECT 'classroom_courses', id, 'learning_groups', id, 'preserve-course-group-id'
FROM classroom_courses
UNION ALL
SELECT 'activity_slots', id, 'learning_offerings', id, 'preserve-activity-offering-id'
FROM activity_slots
UNION ALL
SELECT 'activity_groups', id, 'learning_groups', id, 'preserve-activity-group-id'
FROM activity_groups
UNION ALL
SELECT 'activity_group_members', id, 'learning_group_students', id,
       'preserve-activity-roster-id'
FROM activity_group_members
UNION ALL
SELECT 'activity_group_members', member.id, 'learning_results', result.id,
       'split-deterministic-activity-result'
FROM activity_group_members member
JOIN learning_results result
  ON result.id = uuid_generate_v5(
      '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
      'activity-result:' || member.id::text
  );

INSERT INTO academic_core_entity_map (
    source_table, source_id, target_table, target_id, mapping_rule
)
SELECT 'activity_slot_classroom_assignments', assignment.id, 'learning_groups', generated.id,
       'generate-missing-independent-group'
FROM activity_slot_classroom_assignments assignment
JOIN learning_groups generated
  ON generated.id = uuid_generate_v5(
      '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
      'activity-group:' || assignment.slot_id::text || ':' || assignment.classroom_id::text
  );

INSERT INTO academic_core_entity_map (
    source_table, source_id, target_table, target_id, mapping_rule
)
SELECT 'classroom_advisors', id, 'homeroom_advisors', id, 'rename-preserve-id'
FROM homeroom_advisors
UNION ALL
SELECT 'classroom_course_instructors', instructor.id, 'learning_group_teachers', teacher.id,
       'preserve-course-teacher-id'
FROM classroom_course_instructors instructor
JOIN learning_group_teachers teacher ON teacher.id = instructor.id
UNION ALL
SELECT 'activity_group_instructors', instructor.id, 'learning_group_teachers', teacher.id,
       'preserve-activity-group-teacher-id'
FROM activity_group_instructors instructor
JOIN learning_group_teachers teacher ON teacher.id = instructor.id
UNION ALL
SELECT 'activity_slot_instructors', instructor.id, 'learning_group_teachers', teacher.id,
       'expand-slot-teacher-to-groups'
FROM activity_slot_instructors instructor
JOIN learning_groups activity_group ON activity_group.learning_offering_id = instructor.slot_id
JOIN learning_group_teachers teacher
  ON teacher.learning_group_id = activity_group.id
 AND teacher.teacher_id = instructor.user_id
UNION ALL
SELECT 'activity_slot_classrooms', coverage.id, 'learning_offering_targets', target.id,
       'map-slot-homeroom-target'
FROM activity_slot_classrooms coverage
JOIN learning_offering_targets target
  ON target.learning_offering_id = coverage.slot_id
 AND target.homeroom_id = coverage.classroom_id
UNION ALL
SELECT 'activity_slot_classroom_assignments', assignment.id, 'learning_group_homerooms',
       coverage.id, 'map-independent-homeroom-coverage'
FROM activity_slot_classroom_assignments assignment
JOIN learning_groups activity_group
  ON activity_group.learning_offering_id = assignment.slot_id
JOIN learning_group_homerooms coverage
  ON coverage.learning_group_id = activity_group.id
 AND coverage.homeroom_id = assignment.classroom_id
ON CONFLICT DO NOTHING;

CREATE OR REPLACE FUNCTION academic_assert_offering_subtype()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    affected_offering_id UUID;
    offering_kind TEXT;
    course_count INTEGER;
    activity_count INTEGER;
BEGIN
    affected_offering_id := COALESCE(
        (to_jsonb(NEW)->>'learning_offering_id')::uuid,
        (to_jsonb(OLD)->>'learning_offering_id')::uuid,
        (to_jsonb(NEW)->>'id')::uuid,
        (to_jsonb(OLD)->>'id')::uuid
    );

    SELECT kind INTO offering_kind
    FROM learning_offerings
    WHERE id = affected_offering_id;

    IF offering_kind IS NULL THEN
        RETURN NULL;
    END IF;

    SELECT COUNT(*) INTO course_count
    FROM course_offering_details
    WHERE learning_offering_id = affected_offering_id;

    SELECT COUNT(*) INTO activity_count
    FROM activity_offering_details
    WHERE learning_offering_id = affected_offering_id;

    IF (offering_kind = 'course' AND (course_count <> 1 OR activity_count <> 0))
       OR (offering_kind = 'activity' AND (course_count <> 0 OR activity_count <> 1)) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_OFFERING_SUBTYPE_MISMATCH:%', affected_offering_id;
    END IF;

    RETURN NULL;
END;
$$;

CREATE CONSTRAINT TRIGGER learning_offerings_exact_subtype
AFTER INSERT OR UPDATE ON learning_offerings
DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW EXECUTE FUNCTION academic_assert_offering_subtype();

CREATE CONSTRAINT TRIGGER course_offering_details_exact_subtype
AFTER INSERT OR UPDATE OR DELETE ON course_offering_details
DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW EXECUTE FUNCTION academic_assert_offering_subtype();

CREATE CONSTRAINT TRIGGER activity_offering_details_exact_subtype
AFTER INSERT OR UPDATE OR DELETE ON activity_offering_details
DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW EXECUTE FUNCTION academic_assert_offering_subtype();

CREATE OR REPLACE FUNCTION academic_protect_published_offering()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    IF OLD.status IN ('published', 'closed') AND (
        TG_OP = 'DELETE'
        OR NEW.academic_term_id IS DISTINCT FROM OLD.academic_term_id
        OR NEW.academic_year_id IS DISTINCT FROM OLD.academic_year_id
        OR NEW.kind IS DISTINCT FROM OLD.kind
        OR NEW.code_snapshot IS DISTINCT FROM OLD.code_snapshot
        OR NEW.name_snapshot IS DISTINCT FROM OLD.name_snapshot
        OR NEW.source_requirement_kind IS DISTINCT FROM OLD.source_requirement_kind
        OR NEW.source_requirement_id IS DISTINCT FROM OLD.source_requirement_id
        OR NEW.owning_organization_unit_id IS DISTINCT FROM OLD.owning_organization_unit_id
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_PUBLISHED_OFFERING_IMMUTABLE:%', OLD.id;
    END IF;
    RETURN CASE WHEN TG_OP = 'DELETE' THEN OLD ELSE NEW END;
END;
$$;

CREATE TRIGGER learning_offerings_published_immutable
BEFORE UPDATE OR DELETE ON learning_offerings
FOR EACH ROW EXECUTE FUNCTION academic_protect_published_offering();

CREATE OR REPLACE FUNCTION academic_protect_published_offering_child()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    affected_offering_id UUID;
BEGIN
    affected_offering_id := COALESCE(NEW.learning_offering_id, OLD.learning_offering_id);
    IF EXISTS (
        SELECT 1 FROM learning_offerings
        WHERE id = affected_offering_id AND status IN ('published', 'closed')
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_PUBLISHED_OFFERING_IMMUTABLE:%', affected_offering_id;
    END IF;
    RETURN CASE WHEN TG_OP = 'DELETE' THEN OLD ELSE NEW END;
END;
$$;

CREATE TRIGGER course_offering_details_published_immutable
BEFORE INSERT OR UPDATE OR DELETE ON course_offering_details
FOR EACH ROW EXECUTE FUNCTION academic_protect_published_offering_child();

CREATE TRIGGER activity_offering_details_published_immutable
BEFORE INSERT OR UPDATE OR DELETE ON activity_offering_details
FOR EACH ROW EXECUTE FUNCTION academic_protect_published_offering_child();

CREATE TRIGGER learning_offering_targets_published_immutable
BEFORE INSERT OR UPDATE OR DELETE ON learning_offering_targets
FOR EACH ROW EXECUTE FUNCTION academic_protect_published_offering_child();

DO $$
DECLARE
    source_enrollments BIGINT;
    source_courses BIGINT;
    source_slots BIGINT;
    source_groups BIGINT;
    source_members BIGINT;
    expected_student_years BIGINT;
    expected_course_offerings BIGINT;
    target_student_years BIGINT;
    target_placements BIGINT;
    target_course_offerings BIGINT;
    target_activity_offerings BIGINT;
    target_groups BIGINT;
    target_group_students BIGINT;
BEGIN
    SELECT COUNT(*) INTO source_enrollments FROM student_class_enrollments;
    SELECT COUNT(*) INTO source_courses FROM classroom_courses;
    SELECT COUNT(*) INTO source_slots FROM activity_slots;
    SELECT COUNT(*) INTO source_groups FROM activity_groups;
    SELECT COUNT(*) INTO source_members FROM activity_group_members;
    SELECT COUNT(DISTINCT (enrollment.student_id, homeroom.academic_year_id))
      INTO expected_student_years
    FROM student_class_enrollments enrollment
    JOIN homerooms homeroom ON homeroom.id = enrollment.class_room_id;
    SELECT COUNT(DISTINCT (academic_semester_id, subject_id))
      INTO expected_course_offerings
    FROM classroom_courses;

    SELECT COUNT(*) INTO target_student_years FROM student_academic_years;
    SELECT COUNT(*) INTO target_placements FROM homeroom_placements;
    SELECT COUNT(*) INTO target_course_offerings FROM learning_offerings WHERE kind = 'course';
    SELECT COUNT(*) INTO target_activity_offerings FROM learning_offerings WHERE kind = 'activity';
    SELECT COUNT(*) INTO target_groups FROM learning_groups;
    SELECT COUNT(*) INTO target_group_students FROM learning_group_students;

    IF target_student_years <> expected_student_years
       OR target_placements <> source_enrollments
       OR target_course_offerings <> expected_course_offerings
       OR target_activity_offerings <> source_slots
       OR target_groups < source_courses + source_groups
       OR target_group_students <> (
           SELECT COUNT(*)
           FROM (
               SELECT course.id AS group_id, enrollment.student_id
               FROM classroom_courses course
               JOIN student_class_enrollments enrollment
                 ON enrollment.class_room_id = course.classroom_id
                AND enrollment.status IN ('active', 'completed', 'transferred')
               UNION
               SELECT activity_group_id, student_id
               FROM activity_group_members
           ) roster
       )
       OR EXISTS (
           SELECT 1 FROM student_class_enrollments source
           WHERE NOT EXISTS (
               SELECT 1 FROM academic_core_entity_map entity_map
               WHERE entity_map.source_table = 'student_class_enrollments'
                 AND entity_map.source_id = source.id
                 AND entity_map.target_table = 'homeroom_placements'
           )
       )
       OR EXISTS (
           SELECT 1 FROM classroom_courses source
           WHERE NOT EXISTS (
               SELECT 1 FROM academic_core_entity_map entity_map
               WHERE entity_map.source_table = 'classroom_courses'
                 AND entity_map.source_id = source.id
                 AND entity_map.target_table = 'learning_groups'
           )
       )
       OR EXISTS (
           SELECT 1 FROM activity_slots source
           WHERE NOT EXISTS (
               SELECT 1 FROM academic_core_entity_map entity_map
               WHERE entity_map.source_table = 'activity_slots'
                 AND entity_map.source_id = source.id
                 AND entity_map.target_table = 'learning_offerings'
           )
       )
       OR EXISTS (
           SELECT 1 FROM activity_groups source
           WHERE NOT EXISTS (
               SELECT 1 FROM academic_core_entity_map entity_map
               WHERE entity_map.source_table = 'activity_groups'
                 AND entity_map.source_id = source.id
                 AND entity_map.target_table = 'learning_groups'
           )
       )
       OR EXISTS (
           SELECT 1 FROM activity_group_members source
           WHERE NOT EXISTS (
               SELECT 1 FROM academic_core_entity_map entity_map
               WHERE entity_map.source_table = 'activity_group_members'
                 AND entity_map.source_id = source.id
                 AND entity_map.target_table = 'learning_group_students'
           )
       )
       OR EXISTS (
           SELECT 1
           FROM (
               SELECT 'classroom_advisors'::text AS source_table, id AS source_id
               FROM homeroom_advisors
               UNION ALL
               SELECT 'classroom_course_instructors', id
               FROM classroom_course_instructors
               UNION ALL
               SELECT 'activity_group_instructors', id
               FROM activity_group_instructors
               UNION ALL
               SELECT 'activity_slot_instructors', id
               FROM activity_slot_instructors
               UNION ALL
               SELECT 'activity_slot_classrooms', id
               FROM activity_slot_classrooms
               UNION ALL
               SELECT 'activity_slot_classroom_assignments', id
               FROM activity_slot_classroom_assignments
           ) source
           WHERE NOT EXISTS (
               SELECT 1
               FROM academic_core_entity_map entity_map
               WHERE entity_map.source_table = source.source_table
                 AND entity_map.source_id = source.source_id
           )
       ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_042_RECONCILIATION_FAILED';
    END IF;

    INSERT INTO academic_core_cutover_audits (
        migration_version, mapping_algorithm_version,
        source_counts, target_counts, source_checksum, target_checksum
    )
    VALUES (
        42,
        'academic-core-v1',
        jsonb_build_object(
            'enrollments', source_enrollments,
            'classroomCourses', source_courses,
            'activitySlots', source_slots,
            'activityGroups', source_groups,
            'activityMembers', source_members
        ),
        jsonb_build_object(
            'studentAcademicYears', target_student_years,
            'homeroomPlacements', target_placements,
            'courseOfferings', target_course_offerings,
            'activityOfferings', target_activity_offerings,
            'learningGroups', target_groups,
            'groupStudents', target_group_students,
            'activityResults', (SELECT COUNT(*) FROM activity_result_details)
        ),
        encode(sha256(convert_to(
            (SELECT COALESCE(string_agg(id::text, ',' ORDER BY id), '')
             FROM student_class_enrollments)
            || '|'
            || (SELECT COALESCE(string_agg(id::text, ',' ORDER BY id), '')
                FROM classroom_courses)
            || '|'
            || (SELECT COALESCE(string_agg(id::text, ',' ORDER BY id), '')
                FROM activity_slots)
            || '|'
            || (SELECT COALESCE(string_agg(id::text, ',' ORDER BY id), '')
                FROM activity_groups)
            || '|'
            || (SELECT COALESCE(string_agg(id::text, ',' ORDER BY id), '')
                FROM activity_group_members),
            'UTF8'
        )), 'hex'),
        encode(sha256(convert_to(
            (SELECT COALESCE(string_agg(id::text, ',' ORDER BY id), '') FROM student_academic_years)
            || '|'
            || (SELECT COALESCE(string_agg(id::text, ',' ORDER BY id), '') FROM homeroom_placements)
            || '|'
            || (SELECT COALESCE(string_agg(id::text, ',' ORDER BY id), '') FROM learning_offerings)
            || '|'
            || (SELECT COALESCE(string_agg(id::text, ',' ORDER BY id), '') FROM learning_groups),
            'UTF8'
        )), 'hex')
    );
END;
$$;
