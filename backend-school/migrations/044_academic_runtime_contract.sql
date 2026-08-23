-- Make the clean Academic Core API writable while the consumer modules are still
-- being ported. Destructive removal of the now-nullable legacy columns belongs to
-- the final cleanup migration after every runtime reader and writer is migrated.

ALTER TABLE academic_terms
    ALTER COLUMN legacy_term DROP NOT NULL;

ALTER TABLE activity_versions
    ALTER COLUMN scheduling_mode TYPE TEXT USING scheduling_mode::text;

-- Migration 042 retained the legacy free-form settings JSON. The clean delivery
-- API consumes exact tagged snapshots, so preserve the source value in provenance
-- and normalize the runtime payload once instead of carrying compatibility code.
CREATE OR REPLACE FUNCTION academic_protect_published_offering_child()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    RETURN CASE WHEN TG_OP = 'DELETE' THEN OLD ELSE NEW END;
END;
$$;

UPDATE course_offering_details
SET migration_provenance = migration_provenance || jsonb_build_object(
        'legacyGradingPolicy', grading_policy
    ),
    grading_policy = jsonb_build_object(
        'policyCode', 'legacy_migrated',
        'passingScore', NULL
    )
WHERE NOT grading_policy ? 'policyCode';

UPDATE activity_offering_details
SET migration_provenance = migration_provenance || jsonb_build_object(
        'legacyAttendanceRequirement', attendance_requirement,
        'legacyPassCriteria', pass_criteria
    ),
    attendance_requirement = jsonb_build_object(
        'minimumPercent', NULL,
        'requiredSessions', NULL
    ),
    pass_criteria = jsonb_build_object(
        'requireAttendance', false,
        'requireTeacherConfirmation', true,
        'outcomes', jsonb_build_array('pass', 'fail')
    )
WHERE NOT attendance_requirement ? 'minimumPercent'
   OR NOT pass_criteria ? 'requireAttendance'
   OR NOT pass_criteria ? 'requireTeacherConfirmation';

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

ALTER TABLE bell_schedule_periods
    ALTER COLUMN academic_year_id DROP NOT NULL;

ALTER TABLE subject_groups
    ADD COLUMN row_version BIGINT NOT NULL DEFAULT 1,
    ADD CONSTRAINT subject_groups_row_version_check CHECK (row_version > 0);

ALTER TABLE subjects
    ADD COLUMN archived_at TIMESTAMPTZ;

ALTER TABLE activities
    ADD COLUMN archived_at TIMESTAMPTZ;

ALTER TABLE study_programs
    DROP CONSTRAINT study_programs_curriculum_version_id_key;

CREATE UNIQUE INDEX study_programs_one_default_per_version
    ON study_programs (curriculum_version_id)
    WHERE is_default AND status <> 'archived';

-- Curriculum requirements are reusable across academic years, so their term
-- selector must use the canonical term code rather than the removed legacy
-- numeric label. Migration 041 intentionally retained the original value for
-- auditability; the runtime cutover converts it once before the clean API opens.
ALTER TABLE curriculum_course_requirements
    DISABLE TRIGGER curriculum_course_requirements_published_immutable;
ALTER TABLE curriculum_activity_requirements
    DISABLE TRIGGER curriculum_activity_requirements_published_immutable;

UPDATE curriculum_course_requirements
SET recommended_term_code = CASE
    WHEN btrim(recommended_term_code) ~ '^[0-9]+$'
        THEN 'TERM-' || btrim(recommended_term_code)::integer::text
    ELSE upper(btrim(recommended_term_code))
END
WHERE recommended_term_code IS NOT NULL;

UPDATE curriculum_activity_requirements
SET recommended_term_code = CASE
    WHEN btrim(recommended_term_code) ~ '^[0-9]+$'
        THEN 'TERM-' || btrim(recommended_term_code)::integer::text
    ELSE upper(btrim(recommended_term_code))
END
WHERE recommended_term_code IS NOT NULL;

ALTER TABLE curriculum_course_requirements
    ENABLE TRIGGER curriculum_course_requirements_published_immutable;
ALTER TABLE curriculum_activity_requirements
    ENABLE TRIGGER curriculum_activity_requirements_published_immutable;

ALTER TABLE curriculum_course_requirements
    DROP CONSTRAINT unique_plan_subject,
    ADD CONSTRAINT curriculum_course_requirements_program_resource_key
        UNIQUE (
            study_program_id, grade_level_id, recommended_term_code, subject_version_id
        );

ALTER TABLE curriculum_activity_requirements
    DROP CONSTRAINT unique_sva_plan_grade_term_catalog,
    ADD CONSTRAINT curriculum_activity_requirements_program_resource_key
        UNIQUE (
            study_program_id, grade_level_id, recommended_term_code, activity_version_id
        );

CREATE TABLE grade_level_progression_sets (
    id SMALLINT PRIMARY KEY DEFAULT 1 CHECK (id = 1),
    row_version BIGINT NOT NULL DEFAULT 1 CHECK (row_version > 0),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

INSERT INTO grade_level_progression_sets (id) VALUES (1);

ALTER TABLE academic_audit_events
    DROP CONSTRAINT academic_audit_events_academic_term_id_fkey,
    ADD CONSTRAINT academic_audit_events_academic_term_id_fkey
        FOREIGN KEY (academic_term_id) REFERENCES academic_terms(id) ON DELETE SET NULL;

ALTER TABLE learning_offerings
    ADD COLUMN publish_idempotency_key UUID;

CREATE UNIQUE INDEX learning_offerings_publish_idempotency_key
    ON learning_offerings (publish_idempotency_key)
    WHERE publish_idempotency_key IS NOT NULL;

CREATE UNIQUE INDEX learning_offering_targets_grade_program_key
    ON learning_offering_targets (
        learning_offering_id, grade_level_id, study_program_id
    )
    WHERE target_kind = 'grade_program' AND homeroom_id IS NULL;

ALTER TABLE learning_groups
    ADD COLUMN roster_source_hash CHAR(64),
    ADD COLUMN roster_publish_idempotency_key UUID,
    ADD CONSTRAINT learning_groups_roster_source_hash_check
        CHECK (roster_source_hash IS NULL OR roster_source_hash ~ '^[0-9a-f]{64}$');

CREATE UNIQUE INDEX learning_groups_roster_publish_idempotency_key
    ON learning_groups (roster_publish_idempotency_key)
    WHERE roster_publish_idempotency_key IS NOT NULL;

CREATE TABLE learning_delivery_apply_runs (
    idempotency_key UUID PRIMARY KEY,
    academic_term_id UUID NOT NULL REFERENCES academic_terms(id) ON DELETE RESTRICT,
    request_hash CHAR(64) NOT NULL CHECK (request_hash ~ '^[0-9a-f]{64}$'),
    source_hash CHAR(64) NOT NULL CHECK (source_hash ~ '^[0-9a-f]{64}$'),
    offering_ids UUID[] NOT NULL,
    actor_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
