-- A curriculum edition is selected explicitly, independently of the academic year.
CREATE OR REPLACE FUNCTION check_admission_track_program_context()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM admission_rounds round
        JOIN study_programs program ON program.id = NEW.study_program_id
        JOIN curriculum_versions version ON version.id = program.curriculum_version_id
        JOIN curricula curriculum ON curriculum.id = version.curriculum_id
        WHERE round.id = NEW.admission_round_id
          AND round.academic_year_id = NEW.academic_year_id
          AND program.status = 'published' AND version.status = 'published'
          AND curriculum.is_active IS TRUE
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_ADMISSION_TRACK_PROGRAM_CONTEXT_MISMATCH'
            USING ERRCODE = 'check_violation';
    END IF;
    RETURN NEW;
END $$;

ALTER TABLE curriculum_versions
    ADD COLUMN revision_year INTEGER,
    ADD CONSTRAINT curriculum_versions_revision_year_check
        CHECK (revision_year IS NULL OR revision_year BETWEEN 2400 AND 2999);

ALTER TABLE curriculum_versions DISABLE TRIGGER curriculum_versions_published_immutable;

UPDATE curriculum_versions version
SET revision_year = CASE
        WHEN btrim(version.version_name) ~ '^2[4-9][0-9]{2}$'
            THEN btrim(version.version_name)::integer
        WHEN btrim(version.version_name) ~ '^ฉบับปรับปรุง (พุทธศักราช|พ\.ศ\.) 2[4-9][0-9]{2}$'
            THEN right(btrim(version.version_name), 4)::integer
        ELSE NULL
    END,
    migration_provenance = version.migration_provenance || jsonb_build_object(
        'revisionSelection', jsonb_build_object(
            'migration', 91,
            'legacyStartAcademicYearId', version.start_academic_year_id,
            'legacyEndAcademicYearId', version.end_academic_year_id
        )
    );

ALTER TABLE curriculum_versions
    DROP COLUMN start_academic_year_id,
    DROP COLUMN end_academic_year_id;

ALTER TABLE curriculum_versions ENABLE TRIGGER curriculum_versions_published_immutable;

DO $$ BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_trigger
        WHERE tgrelid = 'curriculum_versions'::regclass
          AND tgname = 'curriculum_versions_published_immutable' AND tgenabled = 'O'
    ) THEN
        RAISE EXCEPTION 'CURRICULUM_091_IMMUTABILITY_NOT_RESTORED';
    END IF;
END $$;
