-- Make the clean Academic Core API writable while the consumer modules are still
-- being ported. Destructive removal of the now-nullable legacy columns belongs to
-- the final cleanup migration after every runtime reader and writer is migrated.

ALTER TABLE academic_terms
    ALTER COLUMN legacy_term DROP NOT NULL;

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
