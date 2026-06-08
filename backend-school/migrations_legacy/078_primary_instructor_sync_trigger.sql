-- ============================================
-- Auto-sync classroom_courses.primary_instructor_id ↔ classroom_course_instructors
-- ============================================

-- Helper: recompute primary_instructor_id for a course from the junction.
CREATE OR REPLACE FUNCTION refresh_course_primary_instructor(p_course_id UUID)
RETURNS VOID AS $$
BEGIN
    UPDATE classroom_courses
    SET primary_instructor_id = (
        SELECT instructor_id
        FROM classroom_course_instructors
        WHERE classroom_course_id = p_course_id
        ORDER BY (role = 'primary') DESC, created_at ASC
        LIMIT 1
    )
    WHERE id = p_course_id;
END;
$$ LANGUAGE plpgsql;

-- Trigger 1: When junction changes, refresh classroom_courses cache
CREATE OR REPLACE FUNCTION trg_cci_sync_primary()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        PERFORM refresh_course_primary_instructor(OLD.classroom_course_id);
        RETURN OLD;
    ELSE
        -- INSERT or UPDATE: if role is 'primary', demote other primaries first
        IF NEW.role = 'primary' THEN
            UPDATE classroom_course_instructors
            SET role = 'secondary'
            WHERE classroom_course_id = NEW.classroom_course_id
              AND instructor_id <> NEW.instructor_id
              AND role = 'primary';
        END IF;
        PERFORM refresh_course_primary_instructor(NEW.classroom_course_id);
        RETURN NEW;
    END IF;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS cci_sync_primary ON classroom_course_instructors;
CREATE TRIGGER cci_sync_primary
AFTER INSERT OR UPDATE OR DELETE ON classroom_course_instructors
FOR EACH ROW EXECUTE FUNCTION trg_cci_sync_primary();

-- Trigger 2: When classroom_courses.primary_instructor_id is written directly, upsert junction
CREATE OR REPLACE FUNCTION trg_cc_sync_junction()
RETURNS TRIGGER AS $$
BEGIN
    -- Only act if primary_instructor_id actually changed to a non-null value
    IF NEW.primary_instructor_id IS NOT NULL
       AND (TG_OP = 'INSERT' OR NEW.primary_instructor_id IS DISTINCT FROM OLD.primary_instructor_id)
    THEN
        -- Upsert the new primary, demote others in one statement
        -- Use a function-scoped session variable to avoid re-entering the cci trigger's demote loop
        INSERT INTO classroom_course_instructors (classroom_course_id, instructor_id, role)
        VALUES (NEW.id, NEW.primary_instructor_id, 'primary')
        ON CONFLICT (classroom_course_id, instructor_id)
        DO UPDATE SET role = 'primary';
        -- (The cci trigger will demote any pre-existing primary)
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS cc_sync_junction ON classroom_courses;
CREATE TRIGGER cc_sync_junction
AFTER INSERT OR UPDATE OF primary_instructor_id ON classroom_courses
FOR EACH ROW EXECUTE FUNCTION trg_cc_sync_junction();

COMMENT ON FUNCTION refresh_course_primary_instructor IS 'Recompute classroom_courses.primary_instructor_id from junction';
COMMENT ON FUNCTION trg_cci_sync_primary IS 'Keep classroom_courses.primary_instructor_id in sync with junction mutations';
COMMENT ON FUNCTION trg_cc_sync_junction IS 'When primary_instructor_id is written directly, mirror into junction';
