-- Academic Core cutover, phase A: years/terms, stable catalogs, curricula, and bell schedules.
-- This migration is intentionally additive or rename-based. Legacy delivery consumers are cut over
-- by migrations 042-043 and removed only by the separately gated migration 044.

CREATE OR REPLACE FUNCTION academic_normalize_identity(value TEXT)
RETURNS TEXT
LANGUAGE sql
IMMUTABLE
STRICT
PARALLEL SAFE
AS $$
    SELECT lower(regexp_replace(btrim(normalize(value, NFKC)), '\s+', ' ', 'g'))
$$;

DO $$
DECLARE
    active_year_id UUID;
    active_term_id UUID;
BEGIN
    IF (SELECT COUNT(*) FROM academic_years WHERE is_active IS TRUE) <> 1 THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_041_ACTIVE_YEAR_COUNT_INVALID';
    END IF;
    IF (SELECT COUNT(*) FROM academic_semesters WHERE is_active IS TRUE) <> 1 THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_041_ACTIVE_TERM_COUNT_INVALID';
    END IF;

    SELECT id INTO active_year_id FROM academic_years WHERE is_active IS TRUE;
    SELECT id INTO active_term_id FROM academic_semesters WHERE is_active IS TRUE;

    IF EXISTS (SELECT 1 FROM academic_years WHERE start_date > end_date) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_041_YEAR_DATE_RANGE_INVALID';
    END IF;
    IF EXISTS (SELECT 1 FROM academic_semesters WHERE start_date > end_date) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_041_TERM_DATE_RANGE_INVALID';
    END IF;
    IF EXISTS (
        SELECT 1
        FROM academic_semesters term
        JOIN academic_years year ON year.id = term.academic_year_id
        WHERE term.start_date < year.start_date OR term.end_date > year.end_date
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_041_TERM_OUTSIDE_YEAR';
    END IF;
    IF EXISTS (
        SELECT 1
        FROM academic_semesters term
        WHERE term.id = active_term_id AND term.academic_year_id <> active_year_id
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_041_ACTIVE_TERM_YEAR_MISMATCH';
    END IF;
    IF EXISTS (
        SELECT 1
        FROM academic_years inactive
        JOIN academic_years active ON active.id = active_year_id
          AND daterange(inactive.start_date, inactive.end_date, '[]')
              && daterange(active.start_date, active.end_date, '[]')
        WHERE inactive.id <> active.id
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_041_INACTIVE_CURRENT_YEAR_AMBIGUOUS';
    END IF;
    IF EXISTS (
        SELECT 1
        FROM academic_semesters inactive
        JOIN academic_semesters active ON active.id = active_term_id
          AND daterange(inactive.start_date, inactive.end_date, '[]')
              && daterange(active.start_date, active.end_date, '[]')
        WHERE inactive.id <> active.id
          AND inactive.academic_year_id = active.academic_year_id
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_041_INACTIVE_CURRENT_TERM_AMBIGUOUS';
    END IF;
    IF EXISTS (
        SELECT 1
        FROM academic_semesters
        GROUP BY academic_year_id, academic_normalize_identity(term)
        HAVING COUNT(*) > 1
    ) OR EXISTS (
        SELECT 1
        FROM academic_semesters
        GROUP BY academic_year_id, start_date, end_date
        HAVING COUNT(*) > 1
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_041_TERM_SEQUENCE_AMBIGUOUS';
    END IF;

    IF EXISTS (SELECT 1 FROM subjects WHERE academic_normalize_identity(code) = '') THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_041_SUBJECT_IDENTITY_BLANK';
    END IF;
    IF EXISTS (
        SELECT 1
        FROM subjects
        GROUP BY academic_normalize_identity(code), start_academic_year_id
        HAVING COUNT(*) > 1
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_041_SUBJECT_IDENTITY_CONFLICT';
    END IF;
    IF EXISTS (
        SELECT 1
        FROM (
            SELECT year.start_date,
                   lag(year.end_date) OVER (
                       PARTITION BY academic_normalize_identity(subject.code)
                       ORDER BY year.start_date, subject.id
                   ) AS previous_end
            FROM subjects subject
            JOIN academic_years year ON year.id = subject.start_academic_year_id
        ) ordered
        WHERE previous_end >= start_date
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_041_SUBJECT_VERSION_RANGE_OVERLAP';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM activity_catalog
        GROUP BY academic_normalize_identity(activity_type),
                 academic_normalize_identity(name),
                 start_academic_year_id
        HAVING COUNT(*) > 1
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_041_ACTIVITY_IDENTITY_CONFLICT';
    END IF;
    IF EXISTS (
        SELECT 1
        FROM (
            SELECT year.start_date,
                   lag(year.end_date) OVER (
                       PARTITION BY academic_normalize_identity(activity.activity_type),
                                    academic_normalize_identity(activity.name)
                       ORDER BY year.start_date, activity.id
                   ) AS previous_end
            FROM activity_catalog activity
            JOIN academic_years year ON year.id = activity.start_academic_year_id
        ) ordered
        WHERE previous_end >= start_date
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_041_ACTIVITY_VERSION_RANGE_OVERLAP';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM study_plan_versions version
        JOIN academic_years starts ON starts.id = version.start_academic_year_id
        JOIN academic_years ends ON ends.id = version.end_academic_year_id
        WHERE ends.start_date < starts.start_date
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_041_CURRICULUM_VERSION_UNRESOLVED';
    END IF;
    IF EXISTS (
        SELECT 1
        FROM class_rooms homeroom
        JOIN academic_years homeroom_year ON homeroom_year.id = homeroom.academic_year_id
        JOIN study_plan_versions version ON version.id = homeroom.study_plan_version_id
        JOIN academic_years starts ON starts.id = version.start_academic_year_id
        LEFT JOIN academic_years ends ON ends.id = version.end_academic_year_id
        WHERE starts.start_date > homeroom_year.start_date
           OR (ends.id IS NOT NULL AND ends.end_date < homeroom_year.end_date)
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_041_HOMEROOM_PROGRAM_UNRESOLVED';
    END IF;
    IF EXISTS (
        SELECT 1
        FROM classroom_courses course
        JOIN class_rooms homeroom ON homeroom.id = course.classroom_id
        JOIN academic_semesters term ON term.id = course.academic_semester_id
        WHERE homeroom.academic_year_id <> term.academic_year_id
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_041_COURSE_TERM_YEAR_MISMATCH';
    END IF;
    IF EXISTS (
        SELECT 1
        FROM student_class_enrollments enrollment
        JOIN class_rooms homeroom ON homeroom.id = enrollment.class_room_id
        WHERE enrollment.status = 'active'
        GROUP BY enrollment.student_id, homeroom.academic_year_id
        HAVING COUNT(*) > 1
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_041_ENROLLMENT_YEAR_CONFLICT';
    END IF;
    IF EXISTS (
        SELECT 1
        FROM activity_group_members
        GROUP BY activity_group_id, student_id
        HAVING COUNT(*) > 1
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_041_ACTIVITY_MEMBER_DUPLICATE';
    END IF;
END
$$;

ALTER TABLE academic_years
    ADD COLUMN status TEXT,
    ADD COLUMN row_version BIGINT NOT NULL DEFAULT 1,
    ADD COLUMN migration_provenance JSONB NOT NULL DEFAULT '{}'::jsonb;

WITH active_year AS (
    SELECT start_date, end_date FROM academic_years WHERE is_active IS TRUE
)
UPDATE academic_years year
SET status = CASE
        WHEN year.is_active IS TRUE THEN 'active'
        WHEN year.end_date < (SELECT start_date FROM active_year) THEN 'closed'
        ELSE 'planning'
    END,
    migration_provenance = jsonb_build_object(
        'migration', 41,
        'mappingAlgorithm', 'academic-core-v1',
        'legacyIsActive', year.is_active
    );

ALTER TABLE academic_years
    ALTER COLUMN status SET NOT NULL,
    ADD CONSTRAINT academic_years_status_check
        CHECK (status IN ('planning', 'ready', 'active', 'closing', 'closed', 'archived')),
    ADD CONSTRAINT academic_years_date_order_check CHECK (start_date <= end_date),
    ADD CONSTRAINT academic_years_row_version_check CHECK (row_version > 0);

CREATE UNIQUE INDEX academic_years_one_active_status
    ON academic_years (status)
    WHERE status = 'active';

ALTER TABLE academic_semesters RENAME TO academic_terms;
ALTER TABLE academic_terms RENAME COLUMN term TO legacy_term;

ALTER TABLE academic_terms
    ADD COLUMN sequence_no INTEGER,
    ADD COLUMN code TEXT,
    ADD COLUMN term_type TEXT,
    ADD COLUMN included_in_year_result BOOLEAN,
    ADD COLUMN blocks_year_closure BOOLEAN,
    ADD COLUMN status TEXT,
    ADD COLUMN row_version BIGINT NOT NULL DEFAULT 1,
    ADD COLUMN migration_provenance JSONB NOT NULL DEFAULT '{}'::jsonb;

WITH ordered AS (
    SELECT term.id,
           row_number() OVER (
               PARTITION BY term.academic_year_id
               ORDER BY term.start_date, term.end_date, term.id
           )::integer AS chronological_sequence
    FROM academic_terms term
), active_term AS (
    SELECT start_date, end_date FROM academic_terms WHERE is_active IS TRUE
)
UPDATE academic_terms term
SET sequence_no = CASE
        WHEN btrim(term.legacy_term) ~ '^[0-9]+$' THEN btrim(term.legacy_term)::integer
        ELSE ordered.chronological_sequence
    END,
    code = CASE
        WHEN upper(btrim(term.legacy_term)) ~ '^[A-Z][A-Z0-9_-]*$'
            THEN upper(btrim(term.legacy_term))
        ELSE 'TERM-' || ordered.chronological_sequence::text
    END,
    term_type = CASE
        WHEN academic_normalize_identity(term.legacy_term) IN ('summer', 'ภาคฤดูร้อน')
            THEN 'summer'
        ELSE 'regular'
    END,
    included_in_year_result = true,
    blocks_year_closure = true,
    status = CASE
        WHEN term.is_active IS TRUE THEN 'active'
        WHEN term.end_date < (SELECT start_date FROM active_term) THEN 'closed'
        ELSE 'planning'
    END,
    migration_provenance = jsonb_build_object(
        'migration', 41,
        'mappingAlgorithm', 'academic-core-v1',
        'legacyTerm', term.legacy_term,
        'legacyIsActive', term.is_active
    )
FROM ordered
WHERE ordered.id = term.id;

ALTER TABLE academic_terms
    ALTER COLUMN sequence_no SET NOT NULL,
    ALTER COLUMN code SET NOT NULL,
    ALTER COLUMN term_type SET NOT NULL,
    ALTER COLUMN included_in_year_result SET NOT NULL,
    ALTER COLUMN blocks_year_closure SET NOT NULL,
    ALTER COLUMN status SET NOT NULL,
    ADD CONSTRAINT academic_terms_sequence_positive_check CHECK (sequence_no > 0),
    ADD CONSTRAINT academic_terms_code_not_blank_check CHECK (btrim(code) <> ''),
    ADD CONSTRAINT academic_terms_type_check
        CHECK (term_type IN ('regular', 'summer', 'remedial', 'custom')),
    ADD CONSTRAINT academic_terms_status_check
        CHECK (status IN ('planning', 'ready', 'active', 'closing', 'closed', 'cancelled')),
    ADD CONSTRAINT academic_terms_date_order_check CHECK (start_date <= end_date),
    ADD CONSTRAINT academic_terms_row_version_check CHECK (row_version > 0),
    ADD CONSTRAINT academic_terms_year_sequence_key UNIQUE (academic_year_id, sequence_no),
    ADD CONSTRAINT academic_terms_year_code_key UNIQUE (academic_year_id, code),
    ADD CONSTRAINT academic_terms_id_year_key UNIQUE (id, academic_year_id);

CREATE UNIQUE INDEX academic_terms_one_active_status
    ON academic_terms (status)
    WHERE status = 'active';

CREATE TABLE bell_schedules (
    id UUID PRIMARY KEY,
    academic_year_id UUID NOT NULL REFERENCES academic_years(id) ON DELETE RESTRICT,
    code TEXT NOT NULL,
    name TEXT NOT NULL,
    is_default BOOLEAN NOT NULL DEFAULT false,
    status TEXT NOT NULL DEFAULT 'published'
        CHECK (status IN ('draft', 'published', 'archived')),
    owning_organization_unit_id UUID REFERENCES organization_units(id) ON DELETE RESTRICT,
    row_version BIGINT NOT NULL DEFAULT 1 CHECK (row_version > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (academic_year_id, code)
);

CREATE UNIQUE INDEX bell_schedules_one_default_per_year
    ON bell_schedules (academic_year_id)
    WHERE is_default;

INSERT INTO bell_schedules (id, academic_year_id, code, name, is_default)
SELECT uuid_generate_v5(
           '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
           'bell-schedule:' || year.id::text
       ),
       year.id,
       'DEFAULT',
       'ตารางคาบมาตรฐาน ' || year.name,
       true
FROM academic_years year;

ALTER TABLE academic_periods RENAME TO bell_schedule_periods;
ALTER TABLE bell_schedule_periods
    ADD COLUMN bell_schedule_id UUID REFERENCES bell_schedules(id) ON DELETE RESTRICT;

UPDATE bell_schedule_periods period
SET bell_schedule_id = uuid_generate_v5(
    '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
    'bell-schedule:' || period.academic_year_id::text
);

ALTER TABLE bell_schedule_periods
    ALTER COLUMN bell_schedule_id SET NOT NULL,
    ADD CONSTRAINT bell_schedule_periods_schedule_order_key
        UNIQUE (bell_schedule_id, order_index),
    ADD CONSTRAINT bell_schedule_periods_schedule_id_key UNIQUE (id, bell_schedule_id);

ALTER TABLE academic_terms
    ADD COLUMN bell_schedule_id UUID REFERENCES bell_schedules(id) ON DELETE RESTRICT;

UPDATE academic_terms term
SET bell_schedule_id = uuid_generate_v5(
    '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
    'bell-schedule:' || term.academic_year_id::text
);

ALTER TABLE academic_terms ALTER COLUMN bell_schedule_id SET NOT NULL;

ALTER TABLE subjects RENAME TO subject_versions;

CREATE TABLE subjects (
    id UUID PRIMARY KEY,
    code TEXT NOT NULL,
    identity_key TEXT NOT NULL UNIQUE,
    owning_organization_unit_id UUID REFERENCES organization_units(id) ON DELETE RESTRICT,
    row_version BIGINT NOT NULL DEFAULT 1 CHECK (row_version > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT subjects_code_not_blank_check CHECK (btrim(code) <> '')
);

INSERT INTO subjects (id, code, identity_key, created_at, updated_at)
SELECT uuid_generate_v5(
           '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
           'subject:' || academic_normalize_identity(version.code)
       ),
       (array_agg(upper(btrim(version.code)) ORDER BY year.start_date, version.id))[1],
       academic_normalize_identity(version.code),
       min(version.created_at),
       max(version.updated_at)
FROM subject_versions version
JOIN academic_years year ON year.id = version.start_academic_year_id
GROUP BY academic_normalize_identity(version.code);

ALTER TABLE subject_versions
    ADD COLUMN subject_id UUID REFERENCES subjects(id) ON DELETE RESTRICT,
    ADD COLUMN version_no INTEGER,
    ADD COLUMN effective_from DATE,
    ADD COLUMN effective_until DATE,
    ADD COLUMN status TEXT,
    ADD COLUMN published_at TIMESTAMPTZ,
    ADD COLUMN row_version BIGINT NOT NULL DEFAULT 1,
    ADD COLUMN migration_provenance JSONB NOT NULL DEFAULT '{}'::jsonb;

WITH mapped AS (
    SELECT version.id,
           uuid_generate_v5(
               '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
               'subject:' || academic_normalize_identity(version.code)
           ) AS stable_id,
           row_number() OVER (
               PARTITION BY academic_normalize_identity(version.code)
               ORDER BY year.start_date, version.id
           )::integer AS mapped_version_no,
           year.start_date AS mapped_from,
           lead(year.start_date) OVER (
               PARTITION BY academic_normalize_identity(version.code)
               ORDER BY year.start_date, version.id
           ) AS mapped_until
    FROM subject_versions version
    JOIN academic_years year ON year.id = version.start_academic_year_id
)
UPDATE subject_versions version
SET subject_id = mapped.stable_id,
    version_no = mapped.mapped_version_no,
    effective_from = mapped.mapped_from,
    effective_until = mapped.mapped_until,
    status = 'published',
    published_at = version.updated_at,
    migration_provenance = jsonb_build_object(
        'migration', 41,
        'mappingAlgorithm', 'academic-core-v1',
        'legacyStartAcademicYearId', version.start_academic_year_id
    )
FROM mapped
WHERE mapped.id = version.id;

ALTER TABLE subject_versions
    ALTER COLUMN credit TYPE NUMERIC(8,2) USING round(credit::numeric, 2),
    ALTER COLUMN subject_id SET NOT NULL,
    ALTER COLUMN version_no SET NOT NULL,
    ALTER COLUMN effective_from SET NOT NULL,
    ALTER COLUMN status SET NOT NULL,
    ADD CONSTRAINT subject_versions_version_positive_check CHECK (version_no > 0),
    ADD CONSTRAINT subject_versions_effective_range_check
        CHECK (effective_until IS NULL OR effective_from < effective_until),
    ADD CONSTRAINT subject_versions_status_check
        CHECK (status IN ('draft', 'published', 'archived')),
    ADD CONSTRAINT subject_versions_row_version_check CHECK (row_version > 0),
    ADD CONSTRAINT subject_versions_subject_version_key UNIQUE (subject_id, version_no),
    ADD CONSTRAINT subject_versions_id_subject_key UNIQUE (id, subject_id);

ALTER TABLE subject_grade_levels RENAME TO subject_version_grade_levels;

ALTER TABLE subject_default_instructors
    DROP CONSTRAINT subject_default_instructors_subject_id_fkey;

WITH ranked AS (
    SELECT instructor.id,
           version.subject_id AS stable_subject_id,
           row_number() OVER (
               PARTITION BY version.subject_id, instructor.instructor_id
               ORDER BY (instructor.role = 'primary') DESC, instructor.created_at, instructor.id
           ) AS duplicate_rank
    FROM subject_default_instructors instructor
    JOIN subject_versions version ON version.id = instructor.subject_id
)
DELETE FROM subject_default_instructors instructor
USING ranked
WHERE instructor.id = ranked.id AND ranked.duplicate_rank > 1;

UPDATE subject_default_instructors instructor
SET subject_id = version.subject_id
FROM subject_versions version
WHERE version.id = instructor.subject_id;

ALTER TABLE subject_default_instructors
    ADD CONSTRAINT subject_default_instructors_subject_id_fkey
        FOREIGN KEY (subject_id) REFERENCES subjects(id) ON DELETE CASCADE;

ALTER TABLE activity_catalog RENAME TO activity_versions;

CREATE TABLE activities (
    id UUID PRIMARY KEY,
    code TEXT NOT NULL,
    identity_key TEXT NOT NULL UNIQUE,
    activity_type TEXT NOT NULL,
    owning_organization_unit_id UUID REFERENCES organization_units(id) ON DELETE RESTRICT,
    row_version BIGINT NOT NULL DEFAULT 1 CHECK (row_version > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT activities_code_not_blank_check CHECK (btrim(code) <> '')
);

INSERT INTO activities (id, code, identity_key, activity_type, created_at, updated_at)
SELECT uuid_generate_v5(
           '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
           'activity:' || academic_normalize_identity(version.activity_type)
               || ':' || academic_normalize_identity(version.name)
       ),
       upper(academic_normalize_identity(version.activity_type)) || '-'
           || substr(encode(sha256(convert_to(academic_normalize_identity(version.name), 'UTF8')), 'hex'), 1, 12),
       academic_normalize_identity(version.activity_type)
           || ':' || academic_normalize_identity(version.name),
       academic_normalize_identity(version.activity_type),
       min(version.created_at),
       max(version.updated_at)
FROM activity_versions version
GROUP BY academic_normalize_identity(version.activity_type),
         academic_normalize_identity(version.name);

ALTER TABLE activity_versions
    ADD COLUMN activity_id UUID REFERENCES activities(id) ON DELETE RESTRICT,
    ADD COLUMN version_no INTEGER,
    ADD COLUMN effective_from DATE,
    ADD COLUMN effective_until DATE,
    ADD COLUMN hours_per_week NUMERIC(8,2),
    ADD COLUMN status TEXT,
    ADD COLUMN published_at TIMESTAMPTZ,
    ADD COLUMN row_version BIGINT NOT NULL DEFAULT 1,
    ADD COLUMN migration_provenance JSONB NOT NULL DEFAULT '{}'::jsonb;

WITH mapped AS (
    SELECT version.id,
           uuid_generate_v5(
               '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
               'activity:' || academic_normalize_identity(version.activity_type)
                   || ':' || academic_normalize_identity(version.name)
           ) AS stable_id,
           row_number() OVER (
               PARTITION BY academic_normalize_identity(version.activity_type),
                            academic_normalize_identity(version.name)
               ORDER BY year.start_date, version.id
           )::integer AS mapped_version_no,
           year.start_date AS mapped_from,
           lead(year.start_date) OVER (
               PARTITION BY academic_normalize_identity(version.activity_type),
                            academic_normalize_identity(version.name)
               ORDER BY year.start_date, version.id
           ) AS mapped_until
    FROM activity_versions version
    JOIN academic_years year ON year.id = version.start_academic_year_id
)
UPDATE activity_versions version
SET activity_id = mapped.stable_id,
    version_no = mapped.mapped_version_no,
    effective_from = mapped.mapped_from,
    effective_until = mapped.mapped_until,
    hours_per_week = version.periods_per_week::numeric(8,2),
    status = 'published',
    published_at = version.updated_at,
    migration_provenance = jsonb_build_object(
        'migration', 41,
        'mappingAlgorithm', 'academic-core-v1',
        'legacyStartAcademicYearId', version.start_academic_year_id
    )
FROM mapped
WHERE mapped.id = version.id;

ALTER TABLE activity_versions
    ALTER COLUMN activity_id SET NOT NULL,
    ALTER COLUMN version_no SET NOT NULL,
    ALTER COLUMN effective_from SET NOT NULL,
    ALTER COLUMN hours_per_week SET NOT NULL,
    ALTER COLUMN status SET NOT NULL,
    ADD CONSTRAINT activity_versions_version_positive_check CHECK (version_no > 0),
    ADD CONSTRAINT activity_versions_effective_range_check
        CHECK (effective_until IS NULL OR effective_from < effective_until),
    ADD CONSTRAINT activity_versions_status_check
        CHECK (status IN ('draft', 'published', 'archived')),
    ADD CONSTRAINT activity_versions_row_version_check CHECK (row_version > 0),
    ADD CONSTRAINT activity_versions_activity_version_key UNIQUE (activity_id, version_no),
    ADD CONSTRAINT activity_versions_id_activity_key UNIQUE (id, activity_id);

ALTER TABLE activity_catalog_default_instructors
    DROP CONSTRAINT activity_catalog_default_instructors_catalog_id_fkey;

WITH ranked AS (
    SELECT instructor.id,
           version.activity_id AS stable_activity_id,
           row_number() OVER (
               PARTITION BY version.activity_id, instructor.instructor_id
               ORDER BY (instructor.role = 'primary') DESC, instructor.created_at, instructor.id
           ) AS duplicate_rank
    FROM activity_catalog_default_instructors instructor
    JOIN activity_versions version ON version.id = instructor.catalog_id
)
DELETE FROM activity_catalog_default_instructors instructor
USING ranked
WHERE instructor.id = ranked.id AND ranked.duplicate_rank > 1;

UPDATE activity_catalog_default_instructors instructor
SET catalog_id = version.activity_id
FROM activity_versions version
WHERE version.id = instructor.catalog_id;

ALTER TABLE activity_catalog_default_instructors RENAME TO activity_default_instructors;
ALTER TABLE activity_default_instructors RENAME COLUMN catalog_id TO activity_id;
ALTER TABLE activity_default_instructors
    ADD CONSTRAINT activity_default_instructors_activity_id_fkey
        FOREIGN KEY (activity_id) REFERENCES activities(id) ON DELETE CASCADE;

ALTER TABLE study_plans RENAME TO curricula;
ALTER TABLE curricula
    ADD COLUMN identity_key TEXT,
    ADD COLUMN owning_organization_unit_id UUID REFERENCES organization_units(id) ON DELETE RESTRICT,
    ADD COLUMN row_version BIGINT NOT NULL DEFAULT 1;

UPDATE curricula
SET identity_key = academic_normalize_identity(code);

ALTER TABLE curricula
    ALTER COLUMN identity_key SET NOT NULL,
    ADD CONSTRAINT curricula_identity_key_key UNIQUE (identity_key),
    ADD CONSTRAINT curricula_row_version_check CHECK (row_version > 0);

ALTER TABLE study_plan_versions RENAME TO curriculum_versions;
ALTER TABLE curriculum_versions RENAME COLUMN study_plan_id TO curriculum_id;
ALTER TABLE curriculum_versions
    ADD COLUMN status TEXT,
    ADD COLUMN published_at TIMESTAMPTZ,
    ADD COLUMN row_version BIGINT NOT NULL DEFAULT 1,
    ADD COLUMN migration_provenance JSONB NOT NULL DEFAULT '{}'::jsonb;

UPDATE curriculum_versions
SET status = 'published',
    published_at = updated_at,
    migration_provenance = jsonb_build_object(
        'migration', 41,
        'mappingAlgorithm', 'academic-core-v1',
        'legacyIsActive', is_active
    );

ALTER TABLE curriculum_versions
    ALTER COLUMN status SET NOT NULL,
    ADD CONSTRAINT curriculum_versions_status_check
        CHECK (status IN ('draft', 'published', 'archived')),
    ADD CONSTRAINT curriculum_versions_row_version_check CHECK (row_version > 0),
    ADD CONSTRAINT curriculum_versions_id_curriculum_key UNIQUE (id, curriculum_id);

CREATE TABLE study_programs (
    id UUID PRIMARY KEY,
    curriculum_version_id UUID NOT NULL UNIQUE
        REFERENCES curriculum_versions(id) ON DELETE RESTRICT,
    code TEXT NOT NULL,
    name_th TEXT NOT NULL,
    name_en TEXT,
    is_default BOOLEAN NOT NULL DEFAULT false,
    status TEXT NOT NULL DEFAULT 'published'
        CHECK (status IN ('draft', 'published', 'archived')),
    owning_organization_unit_id UUID REFERENCES organization_units(id) ON DELETE RESTRICT,
    row_version BIGINT NOT NULL DEFAULT 1 CHECK (row_version > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (curriculum_version_id, code)
);

INSERT INTO study_programs (
    id, curriculum_version_id, code, name_th, name_en, is_default, created_at, updated_at
)
SELECT uuid_generate_v5(
           '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
           'program:' || version.id::text
       ),
       version.id,
       'DEFAULT',
       'แผนมาตรฐาน',
       'Default Program',
       true,
       version.created_at,
       version.updated_at
FROM curriculum_versions version;

ALTER TABLE study_plan_subjects RENAME TO curriculum_course_requirements;
ALTER TABLE curriculum_course_requirements
    RENAME COLUMN study_plan_version_id TO curriculum_version_id;
ALTER TABLE curriculum_course_requirements RENAME COLUMN subject_id TO subject_version_id;
ALTER TABLE curriculum_course_requirements RENAME COLUMN term TO recommended_term_code;
ALTER TABLE curriculum_course_requirements
    ADD COLUMN study_program_id UUID REFERENCES study_programs(id) ON DELETE RESTRICT,
    ADD COLUMN requirement_kind TEXT,
    ADD COLUMN credit NUMERIC(8,2),
    ADD COLUMN hours NUMERIC(10,2),
    ADD COLUMN row_version BIGINT NOT NULL DEFAULT 1;

UPDATE curriculum_course_requirements requirement
SET study_program_id = uuid_generate_v5(
        '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
        'program:' || requirement.curriculum_version_id::text
    ),
    requirement_kind = 'required',
    credit = version.credit,
    hours = version.hours_per_semester::numeric(10,2)
FROM subject_versions version
WHERE version.id = requirement.subject_version_id;

ALTER TABLE curriculum_course_requirements
    ALTER COLUMN study_program_id SET NOT NULL,
    ALTER COLUMN requirement_kind SET NOT NULL,
    ALTER COLUMN credit SET NOT NULL,
    ADD CONSTRAINT curriculum_course_requirements_kind_check
        CHECK (requirement_kind IN ('required', 'elective', 'optional')),
    ADD CONSTRAINT curriculum_course_requirements_credit_check CHECK (credit >= 0),
    ADD CONSTRAINT curriculum_course_requirements_hours_check CHECK (hours IS NULL OR hours >= 0),
    ADD CONSTRAINT curriculum_course_requirements_row_version_check CHECK (row_version > 0);

ALTER TABLE study_plan_version_activities RENAME TO curriculum_activity_requirements;
ALTER TABLE curriculum_activity_requirements
    RENAME COLUMN study_plan_version_id TO curriculum_version_id;
ALTER TABLE curriculum_activity_requirements
    RENAME COLUMN activity_catalog_id TO activity_version_id;
ALTER TABLE curriculum_activity_requirements RENAME COLUMN term TO recommended_term_code;
ALTER TABLE curriculum_activity_requirements
    ADD COLUMN study_program_id UUID REFERENCES study_programs(id) ON DELETE RESTRICT,
    ADD COLUMN requirement_kind TEXT,
    ADD COLUMN hours NUMERIC(10,2),
    ADD COLUMN row_version BIGINT NOT NULL DEFAULT 1;

UPDATE curriculum_activity_requirements requirement
SET study_program_id = uuid_generate_v5(
        '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
        'program:' || requirement.curriculum_version_id::text
    ),
    requirement_kind = 'required',
    hours = version.hours_per_week
FROM activity_versions version
WHERE version.id = requirement.activity_version_id;

ALTER TABLE curriculum_activity_requirements
    ALTER COLUMN study_program_id SET NOT NULL,
    ALTER COLUMN requirement_kind SET NOT NULL,
    ALTER COLUMN hours SET NOT NULL,
    ADD CONSTRAINT curriculum_activity_requirements_kind_check
        CHECK (requirement_kind IN ('required', 'elective', 'optional')),
    ADD CONSTRAINT curriculum_activity_requirements_hours_check CHECK (hours >= 0),
    ADD CONSTRAINT curriculum_activity_requirements_row_version_check CHECK (row_version > 0);

CREATE TABLE grade_level_progressions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    from_grade_level_id UUID NOT NULL REFERENCES grade_levels(id) ON DELETE RESTRICT,
    to_grade_level_id UUID REFERENCES grade_levels(id) ON DELETE RESTRICT,
    transition_kind TEXT NOT NULL
        CHECK (transition_kind IN ('promote', 'repeat', 'graduate', 'exception')),
    curriculum_id UUID REFERENCES curricula(id) ON DELETE RESTRICT,
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (from_grade_level_id, to_grade_level_id, transition_kind, curriculum_id)
);

INSERT INTO grade_level_progressions (
    from_grade_level_id, to_grade_level_id, transition_kind
)
SELECT id, next_grade_level_id, 'promote'
FROM grade_levels
WHERE next_grade_level_id IS NOT NULL;

CREATE TABLE academic_audit_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_code TEXT NOT NULL,
    entity_type TEXT NOT NULL,
    entity_id UUID,
    academic_year_id UUID REFERENCES academic_years(id) ON DELETE RESTRICT,
    academic_term_id UUID REFERENCES academic_terms(id) ON DELETE RESTRICT,
    actor_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE academic_core_cutover_audits (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    migration_version BIGINT NOT NULL UNIQUE,
    mapping_algorithm_version TEXT NOT NULL,
    source_counts JSONB NOT NULL,
    target_counts JSONB NOT NULL,
    source_checksum CHAR(64) NOT NULL,
    target_checksum CHAR(64) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE OR REPLACE FUNCTION academic_assert_term_within_year()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    owner_start DATE;
    owner_end DATE;
BEGIN
    SELECT start_date, end_date INTO owner_start, owner_end
    FROM academic_years
    WHERE id = NEW.academic_year_id
    FOR SHARE;

    IF NEW.start_date < owner_start OR NEW.end_date > owner_end THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_TERM_OUTSIDE_YEAR:%', NEW.id;
    END IF;
    RETURN NEW;
END
$$;

CREATE CONSTRAINT TRIGGER academic_terms_within_year
AFTER INSERT OR UPDATE ON academic_terms
DEFERRABLE INITIALLY IMMEDIATE
FOR EACH ROW EXECUTE FUNCTION academic_assert_term_within_year();

CREATE OR REPLACE FUNCTION academic_assert_version_range()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    IF TG_TABLE_NAME = 'subject_versions' THEN
        PERFORM 1 FROM subjects WHERE id = NEW.subject_id FOR UPDATE;
        IF EXISTS (
            SELECT 1 FROM subject_versions other
            WHERE other.subject_id = NEW.subject_id
              AND other.id <> NEW.id
              AND daterange(other.effective_from, other.effective_until, '[)')
                  && daterange(NEW.effective_from, NEW.effective_until, '[)')
        ) THEN
            RAISE EXCEPTION 'ACADEMIC_CORE_SUBJECT_VERSION_RANGE_OVERLAP:%', NEW.id;
        END IF;
    ELSIF TG_TABLE_NAME = 'activity_versions' THEN
        PERFORM 1 FROM activities WHERE id = NEW.activity_id FOR UPDATE;
        IF EXISTS (
            SELECT 1 FROM activity_versions other
            WHERE other.activity_id = NEW.activity_id
              AND other.id <> NEW.id
              AND daterange(other.effective_from, other.effective_until, '[)')
                  && daterange(NEW.effective_from, NEW.effective_until, '[)')
        ) THEN
            RAISE EXCEPTION 'ACADEMIC_CORE_ACTIVITY_VERSION_RANGE_OVERLAP:%', NEW.id;
        END IF;
    ELSE
        RAISE EXCEPTION 'ACADEMIC_CORE_VERSION_RANGE_TRIGGER_TARGET_INVALID';
    END IF;
    RETURN NEW;
END
$$;

CREATE CONSTRAINT TRIGGER subject_versions_no_overlap
AFTER INSERT OR UPDATE ON subject_versions
DEFERRABLE INITIALLY IMMEDIATE
FOR EACH ROW EXECUTE FUNCTION academic_assert_version_range();

CREATE CONSTRAINT TRIGGER activity_versions_no_overlap
AFTER INSERT OR UPDATE ON activity_versions
DEFERRABLE INITIALLY IMMEDIATE
FOR EACH ROW EXECUTE FUNCTION academic_assert_version_range();

CREATE OR REPLACE FUNCTION academic_prevent_published_version_mutation()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    IF TG_OP = 'DELETE' AND OLD.status = 'published' THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_PUBLISHED_VERSION_IMMUTABLE:%', OLD.id;
    END IF;
    IF TG_OP = 'UPDATE' AND OLD.status = 'published' AND NEW IS DISTINCT FROM OLD THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_PUBLISHED_VERSION_IMMUTABLE:%', OLD.id;
    END IF;
    RETURN CASE WHEN TG_OP = 'DELETE' THEN OLD ELSE NEW END;
END
$$;

CREATE TRIGGER subject_versions_published_immutable
BEFORE UPDATE OR DELETE ON subject_versions
FOR EACH ROW EXECUTE FUNCTION academic_prevent_published_version_mutation();

CREATE TRIGGER activity_versions_published_immutable
BEFORE UPDATE OR DELETE ON activity_versions
FOR EACH ROW EXECUTE FUNCTION academic_prevent_published_version_mutation();

CREATE TRIGGER curriculum_versions_published_immutable
BEFORE UPDATE OR DELETE ON curriculum_versions
FOR EACH ROW EXECUTE FUNCTION academic_prevent_published_version_mutation();

CREATE OR REPLACE FUNCTION academic_prevent_published_curriculum_child_mutation()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    owner_version_id UUID;
    owner_status TEXT;
BEGIN
    IF TG_OP = 'DELETE' THEN
        owner_version_id := OLD.curriculum_version_id;
    ELSE
        owner_version_id := NEW.curriculum_version_id;
    END IF;

    SELECT status INTO owner_status
    FROM curriculum_versions
    WHERE id = owner_version_id
    FOR SHARE;

    IF owner_status = 'published' THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_PUBLISHED_CURRICULUM_IMMUTABLE:%', owner_version_id;
    END IF;
    RETURN CASE WHEN TG_OP = 'DELETE' THEN OLD ELSE NEW END;
END
$$;

CREATE TRIGGER study_programs_published_curriculum_immutable
BEFORE INSERT OR UPDATE OR DELETE ON study_programs
FOR EACH ROW EXECUTE FUNCTION academic_prevent_published_curriculum_child_mutation();

CREATE TRIGGER curriculum_course_requirements_published_immutable
BEFORE INSERT OR UPDATE OR DELETE ON curriculum_course_requirements
FOR EACH ROW EXECUTE FUNCTION academic_prevent_published_curriculum_child_mutation();

CREATE TRIGGER curriculum_activity_requirements_published_immutable
BEFORE INSERT OR UPDATE OR DELETE ON curriculum_activity_requirements
FOR EACH ROW EXECUTE FUNCTION academic_prevent_published_curriculum_child_mutation();

INSERT INTO academic_core_cutover_audits (
    migration_version,
    mapping_algorithm_version,
    source_counts,
    target_counts,
    source_checksum,
    target_checksum
)
SELECT 41,
       'academic-core-v1',
       jsonb_build_object(
           'academicYears', (SELECT COUNT(*) FROM academic_years),
           'academicTerms', (SELECT COUNT(*) FROM academic_terms),
           'subjects', (SELECT COUNT(*) FROM subject_versions),
           'activities', (SELECT COUNT(*) FROM activity_versions),
           'curricula', (SELECT COUNT(*) FROM curricula),
           'curriculumVersions', (SELECT COUNT(*) FROM curriculum_versions),
           'courseRequirements', (SELECT COUNT(*) FROM curriculum_course_requirements),
           'activityRequirements', (SELECT COUNT(*) FROM curriculum_activity_requirements)
       ),
       jsonb_build_object(
           'academicYears', (SELECT COUNT(*) FROM academic_years),
           'academicTerms', (SELECT COUNT(*) FROM academic_terms),
           'stableSubjects', (SELECT COUNT(*) FROM subjects),
           'subjectVersions', (SELECT COUNT(*) FROM subject_versions),
           'stableActivities', (SELECT COUNT(*) FROM activities),
           'activityVersions', (SELECT COUNT(*) FROM activity_versions),
           'curricula', (SELECT COUNT(*) FROM curricula),
           'curriculumVersions', (SELECT COUNT(*) FROM curriculum_versions),
           'programs', (SELECT COUNT(*) FROM study_programs),
           'courseRequirements', (SELECT COUNT(*) FROM curriculum_course_requirements),
           'activityRequirements', (SELECT COUNT(*) FROM curriculum_activity_requirements)
       ),
       encode(sha256(convert_to(concat_ws('|',
           (SELECT COALESCE(string_agg(id::text, ',' ORDER BY id), '') FROM academic_years),
           (SELECT COALESCE(string_agg(id::text, ',' ORDER BY id), '') FROM academic_terms),
           (SELECT COALESCE(string_agg(id::text, ',' ORDER BY id), '') FROM subject_versions),
           (SELECT COALESCE(string_agg(id::text, ',' ORDER BY id), '') FROM activity_versions)
       ), 'UTF8')), 'hex'),
       encode(sha256(convert_to(concat_ws('|',
           (SELECT COALESCE(string_agg(id::text, ',' ORDER BY id), '') FROM subjects),
           (SELECT COALESCE(string_agg(id::text, ',' ORDER BY id), '') FROM activities),
           (SELECT COALESCE(string_agg(id::text, ',' ORDER BY id), '') FROM study_programs)
       ), 'UTF8')), 'hex');

DO $$
BEGIN
    IF (SELECT (source_counts ->> 'subjects')::bigint
        FROM academic_core_cutover_audits WHERE migration_version = 41)
       <> (SELECT COUNT(*) FROM subject_versions)
       OR (SELECT (source_counts ->> 'activities')::bigint
           FROM academic_core_cutover_audits WHERE migration_version = 41)
          <> (SELECT COUNT(*) FROM activity_versions)
       OR (SELECT (source_counts ->> 'academicTerms')::bigint
           FROM academic_core_cutover_audits WHERE migration_version = 41)
          <> (SELECT COUNT(*) FROM academic_terms)
       OR (SELECT COUNT(*) FROM subjects) > (SELECT COUNT(*) FROM subject_versions)
       OR (SELECT COUNT(*) FROM activities) > (SELECT COUNT(*) FROM activity_versions)
    THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_041_COUNT_RECONCILIATION_FAILED';
    END IF;
    IF EXISTS (SELECT 1 FROM subject_versions WHERE subject_id IS NULL)
       OR EXISTS (SELECT 1 FROM activity_versions WHERE activity_id IS NULL)
       OR EXISTS (SELECT 1 FROM academic_terms WHERE bell_schedule_id IS NULL)
       OR EXISTS (SELECT 1 FROM curriculum_course_requirements WHERE study_program_id IS NULL)
       OR EXISTS (SELECT 1 FROM curriculum_activity_requirements WHERE study_program_id IS NULL)
    THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_041_TARGET_REFERENCE_RECONCILIATION_FAILED';
    END IF;
END
$$;
