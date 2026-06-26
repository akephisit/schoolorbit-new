-- Remove the legacy single-teacher column from subjects.
-- subject_default_instructors is the source of truth for subject teaching teams.

INSERT INTO subject_default_instructors (subject_id, instructor_id, role)
SELECT id, default_instructor_id, 'primary'
FROM subjects
WHERE default_instructor_id IS NOT NULL
ON CONFLICT (subject_id, instructor_id) DO UPDATE SET role = 'primary';

DROP TRIGGER IF EXISTS subject_sync_junction ON subjects;
DROP TRIGGER IF EXISTS sdi_sync_primary ON subject_default_instructors;

DROP FUNCTION IF EXISTS trg_subject_sync_junction();
DROP FUNCTION IF EXISTS trg_sdi_sync_primary();
DROP FUNCTION IF EXISTS refresh_subject_default_instructor(UUID);

ALTER TABLE subjects DROP COLUMN IF EXISTS default_instructor_id;

CREATE OR REPLACE FUNCTION trg_sdi_enforce_single_primary()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        RETURN OLD;
    END IF;

    IF NEW.role = 'primary' THEN
        UPDATE subject_default_instructors
        SET role = 'secondary'
        WHERE subject_id = NEW.subject_id
          AND instructor_id <> NEW.instructor_id
          AND role = 'primary';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER sdi_enforce_single_primary
AFTER INSERT OR UPDATE ON subject_default_instructors
FOR EACH ROW EXECUTE FUNCTION trg_sdi_enforce_single_primary();

COMMENT ON FUNCTION trg_sdi_enforce_single_primary() IS
    'Keep at most one primary default instructor per subject without syncing to subjects.';
