-- ============================================
-- Subject default instructors (team teaching at catalog level)
--   - admin sets default instructors once per subject
--   - when subject is assigned to a classroom, defaults are auto-copied to classroom_course_instructors
--   - per-classroom team can still be overridden afterward
-- ============================================

CREATE TABLE IF NOT EXISTS subject_default_instructors (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    subject_id UUID NOT NULL REFERENCES subjects(id) ON DELETE CASCADE,
    instructor_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role VARCHAR(20) NOT NULL DEFAULT 'primary' CHECK (role IN ('primary', 'secondary')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT unique_subject_instructor UNIQUE (subject_id, instructor_id)
);

CREATE INDEX IF NOT EXISTS idx_sdi_subject ON subject_default_instructors(subject_id);
CREATE INDEX IF NOT EXISTS idx_sdi_instructor ON subject_default_instructors(instructor_id);

COMMENT ON TABLE subject_default_instructors IS 'Default team teaching per subject catalog entry — copied into classroom_course_instructors when a classroom picks up the subject';

-- Seed from existing subjects.default_instructor_id
INSERT INTO subject_default_instructors (subject_id, instructor_id, role)
SELECT id, default_instructor_id, 'primary'
FROM subjects
WHERE default_instructor_id IS NOT NULL
ON CONFLICT DO NOTHING;

-- ============================================
-- Sync: subjects.default_instructor_id ↔ subject_default_instructors
-- Mirrors the classroom_courses ↔ classroom_course_instructors pattern from migration 078.
-- ============================================

-- Trigger 1: When junction changes, refresh subjects.default_instructor_id to the primary
CREATE OR REPLACE FUNCTION refresh_subject_default_instructor(p_subject_id UUID)
RETURNS VOID AS $$
BEGIN
    UPDATE subjects
    SET default_instructor_id = (
        SELECT instructor_id
        FROM subject_default_instructors
        WHERE subject_id = p_subject_id
        ORDER BY (role = 'primary') DESC, created_at ASC
        LIMIT 1
    )
    WHERE id = p_subject_id;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION trg_sdi_sync_primary()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        PERFORM refresh_subject_default_instructor(OLD.subject_id);
        RETURN OLD;
    ELSE
        IF NEW.role = 'primary' THEN
            UPDATE subject_default_instructors
            SET role = 'secondary'
            WHERE subject_id = NEW.subject_id
              AND instructor_id <> NEW.instructor_id
              AND role = 'primary';
        END IF;
        PERFORM refresh_subject_default_instructor(NEW.subject_id);
        RETURN NEW;
    END IF;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS sdi_sync_primary ON subject_default_instructors;
CREATE TRIGGER sdi_sync_primary
AFTER INSERT OR UPDATE OR DELETE ON subject_default_instructors
FOR EACH ROW EXECUTE FUNCTION trg_sdi_sync_primary();

-- Trigger 2: When subjects.default_instructor_id is written directly, upsert into junction
CREATE OR REPLACE FUNCTION trg_subject_sync_junction()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.default_instructor_id IS NOT NULL
       AND (TG_OP = 'INSERT' OR NEW.default_instructor_id IS DISTINCT FROM OLD.default_instructor_id)
    THEN
        INSERT INTO subject_default_instructors (subject_id, instructor_id, role)
        VALUES (NEW.id, NEW.default_instructor_id, 'primary')
        ON CONFLICT (subject_id, instructor_id)
        DO UPDATE SET role = 'primary';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS subject_sync_junction ON subjects;
CREATE TRIGGER subject_sync_junction
AFTER INSERT OR UPDATE OF default_instructor_id ON subjects
FOR EACH ROW EXECUTE FUNCTION trg_subject_sync_junction();

COMMENT ON FUNCTION refresh_subject_default_instructor IS 'Recompute subjects.default_instructor_id from junction';
COMMENT ON FUNCTION trg_sdi_sync_primary IS 'Keep subjects.default_instructor_id in sync with junction mutations';
COMMENT ON FUNCTION trg_subject_sync_junction IS 'When default_instructor_id is written directly, mirror into junction';
