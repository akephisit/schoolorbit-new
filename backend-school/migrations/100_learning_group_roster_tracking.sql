-- Existing groups retain explicit manual membership until an authorized owner opts in.
CREATE TABLE learning_group_roster_tracking (
    learning_group_id UUID PRIMARY KEY REFERENCES learning_groups(id) ON DELETE CASCADE,
    mode TEXT NOT NULL CHECK (mode IN ('manual', 'homeroom')),
    effective_from DATE,
    CHECK ((mode = 'homeroom' AND effective_from IS NOT NULL) OR (mode = 'manual' AND effective_from IS NULL))
);

-- A removed episode was cancelled before it started, so it occupies no teaching dates.
-- Keep its identity and related evidence while allowing a later valid episode.
CREATE OR REPLACE FUNCTION academic_validate_roster_membership_interval()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    new_lock_key BIGINT;
    old_lock_key BIGINT;
BEGIN
    new_lock_key := hashtextextended(
        NEW.learning_group_id::TEXT || ':' || NEW.student_id::TEXT,
        0
    );

    IF TG_OP = 'UPDATE' THEN
        old_lock_key := hashtextextended(
            OLD.learning_group_id::TEXT || ':' || OLD.student_id::TEXT,
            0
        );
        PERFORM pg_advisory_xact_lock(LEAST(new_lock_key, old_lock_key));
        IF new_lock_key <> old_lock_key THEN
            PERFORM pg_advisory_xact_lock(GREATEST(new_lock_key, old_lock_key));
        END IF;
    ELSE
        PERFORM pg_advisory_xact_lock(new_lock_key);
    END IF;

    PERFORM 1
    FROM learning_group_students membership
    WHERE membership.learning_group_id = NEW.learning_group_id
      AND membership.student_id = NEW.student_id
      AND membership.id <> NEW.id
    ORDER BY membership.id
    FOR UPDATE;

    IF NEW.membership_status <> 'removed' AND (NEW.left_at IS NULL OR NEW.left_at >= NEW.joined_at) THEN
        IF EXISTS (
            SELECT 1
            FROM learning_group_students membership
            WHERE membership.learning_group_id = NEW.learning_group_id
              AND membership.student_id = NEW.student_id
              AND membership.id <> NEW.id
              AND membership.membership_status <> 'removed'
              AND daterange(
                      membership.joined_at,
                      membership.left_at,
                      '[]'
                  ) && daterange(NEW.joined_at, NEW.left_at, '[]')
        ) THEN
            RAISE EXCEPTION 'ACADEMIC_ROSTER_MEMBERSHIP_INTERVAL_OVERLAP'
                USING ERRCODE = 'check_violation';
        END IF;
    END IF;

    RETURN NEW;
END;
$$;

DROP TRIGGER learning_group_students_interval_guard ON learning_group_students;
CREATE TRIGGER learning_group_students_interval_guard
BEFORE INSERT OR UPDATE OF learning_group_id, student_id, joined_at, left_at, membership_status
ON learning_group_students
FOR EACH ROW EXECUTE FUNCTION academic_validate_roster_membership_interval();
