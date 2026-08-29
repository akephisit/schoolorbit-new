-- Academic operational change foundation and effective-from timetable versions.
--
-- This is a hard cutover. Existing timetable rows are attached to one
-- deterministic published version per populated term, operational weekly targets
-- move to that version, and the obsolete runtime owners are removed.

ALTER TABLE academic_terms
    RENAME COLUMN end_date TO planned_end_date;

ALTER TABLE academic_terms
    ALTER COLUMN planned_end_date DROP NOT NULL,
    ADD COLUMN closed_on DATE;

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

    IF NEW.start_date < owner_start
       OR NEW.start_date > owner_end
       OR NEW.planned_end_date > owner_end
       OR NEW.closed_on > owner_end
       OR NEW.closed_on < NEW.start_date
    THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_TERM_OUTSIDE_YEAR:%', NEW.id;
    END IF;
    RETURN NEW;
END
$$;

UPDATE academic_terms
SET closed_on = planned_end_date
WHERE status = 'closed';

ALTER TABLE learning_offerings
    ADD COLUMN starts_on DATE,
    ADD COLUMN ends_on DATE,
    ADD COLUMN stop_reason TEXT,
    ADD COLUMN stopped_at TIMESTAMPTZ,
    ADD COLUMN stopped_by UUID REFERENCES users(id) ON DELETE RESTRICT,
    ADD COLUMN stop_change_set_id UUID;

UPDATE learning_offerings offering
SET starts_on = term.start_date
FROM academic_terms term
WHERE term.id = offering.academic_term_id;

SET CONSTRAINTS ALL IMMEDIATE;

ALTER TABLE learning_offerings
    ALTER COLUMN starts_on SET NOT NULL,
    ADD CONSTRAINT learning_offerings_availability_order_check
        CHECK (ends_on IS NULL OR starts_on <= ends_on),
    ADD CONSTRAINT learning_offerings_stop_metadata_shape_check
        CHECK (
            (ends_on IS NULL
             AND stop_reason IS NULL
             AND stopped_at IS NULL
             AND stopped_by IS NULL
             AND stop_change_set_id IS NULL)
            OR
            (ends_on IS NOT NULL
             AND stop_reason IS NOT NULL
             AND btrim(stop_reason) <> ''
             AND stopped_at IS NOT NULL
             AND stopped_by IS NOT NULL
             AND stop_change_set_id IS NOT NULL)
        );

ALTER TABLE learning_offerings
    DROP CONSTRAINT learning_offerings_status_check,
    ADD CONSTRAINT learning_offerings_status_check
        CHECK (status IN ('draft', 'published', 'cancelled', 'closed'));

CREATE TABLE academic_term_change_sets (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    academic_term_id UUID NOT NULL,
    academic_year_id UUID NOT NULL,
    effective_from DATE NOT NULL,
    reason TEXT NOT NULL CHECK (btrim(reason) <> ''),
    status TEXT NOT NULL DEFAULT 'draft'
        CHECK (status IN ('draft', 'published', 'cancelled')),
    base_timetable_version_id UUID,
    target_timetable_version_id UUID,
    idempotency_key TEXT NOT NULL CHECK (btrim(idempotency_key) <> ''),
    row_version BIGINT NOT NULL DEFAULT 1 CHECK (row_version > 0),
    created_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    published_by UUID REFERENCES users(id) ON DELETE RESTRICT,
    published_at TIMESTAMPTZ,
    cancelled_by UUID REFERENCES users(id) ON DELETE RESTRICT,
    cancelled_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT academic_term_change_sets_term_context_fkey
        FOREIGN KEY (academic_term_id, academic_year_id)
        REFERENCES academic_terms(id, academic_year_id) ON DELETE RESTRICT,
    CONSTRAINT academic_term_change_sets_id_context_key
        UNIQUE (id, academic_term_id, academic_year_id),
    CONSTRAINT academic_term_change_sets_idempotency_key
        UNIQUE (academic_term_id, idempotency_key),
    CONSTRAINT academic_term_change_sets_status_metadata_check CHECK (
        (status = 'draft'
         AND published_by IS NULL AND published_at IS NULL
         AND cancelled_by IS NULL AND cancelled_at IS NULL)
        OR
        (status = 'published'
         AND published_by IS NOT NULL AND published_at IS NOT NULL
         AND cancelled_by IS NULL AND cancelled_at IS NULL)
        OR
        (status = 'cancelled'
         AND published_by IS NULL AND published_at IS NULL
         AND cancelled_by IS NOT NULL AND cancelled_at IS NOT NULL)
    )
);

CREATE TABLE academic_term_change_items (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    change_set_id UUID NOT NULL,
    academic_term_id UUID NOT NULL,
    academic_year_id UUID NOT NULL,
    action_kind TEXT NOT NULL CHECK (
        action_kind IN ('add_offering', 'stop_offering', 'adjust_weekly_period_target')
    ),
    learning_offering_id UUID NOT NULL,
    weekly_period_target INTEGER CHECK (weekly_period_target > 0),
    row_version BIGINT NOT NULL DEFAULT 1 CHECK (row_version > 0),
    created_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT academic_term_change_items_change_set_context_fkey
        FOREIGN KEY (change_set_id, academic_term_id, academic_year_id)
        REFERENCES academic_term_change_sets(id, academic_term_id, academic_year_id)
        ON DELETE CASCADE,
    CONSTRAINT academic_term_change_items_offering_context_fkey
        FOREIGN KEY (learning_offering_id, academic_term_id, academic_year_id)
        REFERENCES learning_offerings(id, academic_term_id, academic_year_id)
        ON DELETE RESTRICT,
    CONSTRAINT academic_term_change_items_action_shape_check CHECK (
        (action_kind = 'stop_offering' AND weekly_period_target IS NULL)
        OR
        (action_kind IN ('add_offering', 'adjust_weekly_period_target')
         AND weekly_period_target IS NOT NULL)
    ),
    CONSTRAINT academic_term_change_items_action_offering_key
        UNIQUE (change_set_id, action_kind, learning_offering_id)
);

CREATE TABLE academic_timetable_versions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    academic_term_id UUID NOT NULL,
    academic_year_id UUID NOT NULL,
    effective_from DATE NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('draft', 'published', 'cancelled')),
    source_version_id UUID,
    change_set_id UUID,
    bell_schedule_id UUID NOT NULL REFERENCES bell_schedules(id) ON DELETE RESTRICT,
    row_version BIGINT NOT NULL DEFAULT 1 CHECK (row_version > 0),
    created_by UUID REFERENCES users(id) ON DELETE RESTRICT,
    published_by UUID REFERENCES users(id) ON DELETE RESTRICT,
    published_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT academic_timetable_versions_term_context_fkey
        FOREIGN KEY (academic_term_id, academic_year_id)
        REFERENCES academic_terms(id, academic_year_id) ON DELETE RESTRICT,
    CONSTRAINT academic_timetable_versions_term_schedule_fkey
        FOREIGN KEY (academic_term_id, bell_schedule_id)
        REFERENCES academic_terms(id, bell_schedule_id) ON DELETE RESTRICT,
    CONSTRAINT academic_timetable_versions_id_context_key
        UNIQUE (id, academic_term_id, academic_year_id),
    CONSTRAINT academic_timetable_versions_change_set_context_fkey
        FOREIGN KEY (change_set_id, academic_term_id, academic_year_id)
        REFERENCES academic_term_change_sets(id, academic_term_id, academic_year_id)
        ON DELETE RESTRICT,
    CONSTRAINT academic_timetable_versions_change_set_key UNIQUE (change_set_id),
    CONSTRAINT academic_timetable_versions_publication_shape_check CHECK (
        (status = 'published' AND published_by IS NOT NULL AND published_at IS NOT NULL)
        OR
        (status IN ('draft', 'cancelled') AND published_by IS NULL AND published_at IS NULL)
    )
);

ALTER TABLE academic_timetable_versions
    ADD CONSTRAINT academic_timetable_versions_source_context_fkey
        FOREIGN KEY (source_version_id, academic_term_id, academic_year_id)
        REFERENCES academic_timetable_versions(id, academic_term_id, academic_year_id)
        ON DELETE RESTRICT;

CREATE UNIQUE INDEX academic_timetable_versions_live_effective_key
    ON academic_timetable_versions(academic_term_id, effective_from)
    WHERE status IN ('draft', 'published');

CREATE INDEX academic_timetable_versions_term_effective_idx
    ON academic_timetable_versions(academic_term_id, effective_from DESC, id);

CREATE TABLE academic_timetable_version_targets (
    timetable_version_id UUID NOT NULL,
    learning_offering_id UUID NOT NULL,
    academic_term_id UUID NOT NULL,
    academic_year_id UUID NOT NULL,
    weekly_period_target INTEGER NOT NULL CHECK (weekly_period_target > 0),
    migration_provenance JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (timetable_version_id, learning_offering_id),
    CONSTRAINT academic_timetable_version_targets_version_context_fkey
        FOREIGN KEY (timetable_version_id, academic_term_id, academic_year_id)
        REFERENCES academic_timetable_versions(id, academic_term_id, academic_year_id)
        ON DELETE CASCADE,
    CONSTRAINT academic_timetable_version_targets_offering_context_fkey
        FOREIGN KEY (learning_offering_id, academic_term_id, academic_year_id)
        REFERENCES learning_offerings(id, academic_term_id, academic_year_id)
        ON DELETE RESTRICT
);

ALTER TABLE academic_term_change_sets
    ADD CONSTRAINT academic_term_change_sets_base_version_context_fkey
        FOREIGN KEY (base_timetable_version_id, academic_term_id, academic_year_id)
        REFERENCES academic_timetable_versions(id, academic_term_id, academic_year_id)
        ON DELETE RESTRICT,
    ADD CONSTRAINT academic_term_change_sets_target_version_context_fkey
        FOREIGN KEY (target_timetable_version_id, academic_term_id, academic_year_id)
        REFERENCES academic_timetable_versions(id, academic_term_id, academic_year_id)
        ON DELETE RESTRICT;

ALTER TABLE learning_offerings
    ADD CONSTRAINT learning_offerings_stop_change_set_context_fkey
        FOREIGN KEY (stop_change_set_id, academic_term_id, academic_year_id)
        REFERENCES academic_term_change_sets(id, academic_term_id, academic_year_id)
        ON DELETE RESTRICT;

CREATE OR REPLACE FUNCTION academic_validate_offering_availability()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    year_start DATE;
    year_end DATE;
    term_start DATE;
BEGIN
    SELECT year.start_date, year.end_date, term.start_date
      INTO year_start, year_end, term_start
    FROM academic_years year
    JOIN academic_terms term ON term.academic_year_id = year.id
    WHERE term.id = NEW.academic_term_id
      AND term.academic_year_id = NEW.academic_year_id;

    IF year_start IS NULL THEN
        RAISE EXCEPTION 'ACADEMIC_OFFERING_AVAILABILITY_CONTEXT_MISMATCH'
            USING ERRCODE = 'foreign_key_violation';
    END IF;

    NEW.starts_on := COALESCE(NEW.starts_on, term_start);

    IF NEW.starts_on < year_start
       OR NEW.starts_on > year_end
       OR NEW.ends_on < year_start
       OR NEW.ends_on > year_end
    THEN
        RAISE EXCEPTION 'ACADEMIC_OFFERING_AVAILABILITY_OUTSIDE_YEAR'
            USING ERRCODE = 'check_violation';
    END IF;

    RETURN NEW;
END;
$$;

CREATE TRIGGER learning_offerings_availability_guard
BEFORE INSERT OR UPDATE OF academic_term_id, academic_year_id, starts_on, ends_on
ON learning_offerings
FOR EACH ROW EXECUTE FUNCTION academic_validate_offering_availability();

INSERT INTO academic_timetable_versions (
    id, academic_term_id, academic_year_id, effective_from, status,
    bell_schedule_id, published_by, published_at, created_at, updated_at
)
SELECT uuid_generate_v5(
           'f291607b-fef7-56f8-a679-ad9d37e3bc75'::uuid,
           'initial-timetable-version:' || term.id::text
       ),
       term.id,
       term.academic_year_id,
       term.start_date,
       'published',
       term.bell_schedule_id,
       COALESCE(
           (
               SELECT entry.created_by
               FROM academic_timetable_entries entry
               WHERE entry.academic_term_id = term.id
                 AND entry.created_by IS NOT NULL
               ORDER BY entry.created_at, entry.id
               LIMIT 1
           ),
           (
               SELECT offering_user.id
               FROM users offering_user
               WHERE offering_user.user_type = 'staff'
                 AND offering_user.status = 'active'
               ORDER BY offering_user.id
               LIMIT 1
           )
       ),
       COALESCE(
           (
               SELECT min(entry.created_at)
               FROM academic_timetable_entries entry
               WHERE entry.academic_term_id = term.id
           ),
           term.created_at
       ),
       term.created_at,
       term.updated_at
FROM academic_terms term
WHERE EXISTS (
          SELECT 1 FROM learning_offerings offering
          WHERE offering.academic_term_id = term.id
      )
   OR EXISTS (
          SELECT 1 FROM academic_timetable_entries entry
          WHERE entry.academic_term_id = term.id
      );

DO $$
DECLARE
    ambiguous_offering UUID;
BEGIN
    WITH group_counts AS (
        SELECT entry.learning_offering_id,
               entry.learning_group_id,
               count(DISTINCT (entry.day_of_week, entry.bell_schedule_period_id))::integer
                   AS weekly_period_count
        FROM academic_timetable_entries entry
        JOIN learning_offerings offering ON offering.id = entry.learning_offering_id
        WHERE offering.kind = 'activity'
          AND entry.entry_type = 'ACTIVITY'
          AND entry.is_active
        GROUP BY entry.learning_offering_id, entry.learning_group_id
    ), ambiguous AS (
        SELECT learning_offering_id
        FROM group_counts
        GROUP BY learning_offering_id
        HAVING min(weekly_period_count) <= 0
            OR min(weekly_period_count) <> max(weekly_period_count)
    )
    SELECT learning_offering_id
    INTO ambiguous_offering
    FROM ambiguous
    ORDER BY learning_offering_id
    LIMIT 1;

    IF ambiguous_offering IS NOT NULL THEN
        RAISE EXCEPTION 'ACADEMIC_052_ACTIVITY_TARGET_AMBIGUOUS:%', ambiguous_offering
            USING ERRCODE = 'check_violation';
    END IF;
END
$$;

INSERT INTO academic_timetable_version_targets (
    timetable_version_id, learning_offering_id, academic_term_id,
    academic_year_id, weekly_period_target, migration_provenance
)
SELECT version.id,
       detail.learning_offering_id,
       detail.academic_term_id,
       detail.academic_year_id,
       detail.weekly_period_target,
       jsonb_build_object(
           'migration', 52,
           'source', 'course_offering_details.weekly_period_target'
       )
FROM course_offering_details detail
JOIN academic_timetable_versions version
  ON version.academic_term_id = detail.academic_term_id
 AND version.status = 'published';

WITH group_counts AS (
    SELECT entry.learning_offering_id,
           entry.academic_term_id,
           entry.academic_year_id,
           entry.learning_group_id,
           count(DISTINCT (entry.day_of_week, entry.bell_schedule_period_id))::integer
               AS weekly_period_count
    FROM academic_timetable_entries entry
    JOIN learning_offerings offering ON offering.id = entry.learning_offering_id
    WHERE offering.kind = 'activity'
      AND entry.entry_type = 'ACTIVITY'
      AND entry.is_active
    GROUP BY entry.learning_offering_id, entry.academic_term_id,
             entry.academic_year_id, entry.learning_group_id
), activity_targets AS (
    SELECT learning_offering_id,
           academic_term_id,
           academic_year_id,
           max(weekly_period_count)::integer AS weekly_period_target
    FROM group_counts
    GROUP BY learning_offering_id, academic_term_id, academic_year_id
)
INSERT INTO academic_timetable_version_targets (
    timetable_version_id, learning_offering_id, academic_term_id,
    academic_year_id, weekly_period_target, migration_provenance
)
SELECT version.id,
       target.learning_offering_id,
       target.academic_term_id,
       target.academic_year_id,
       target.weekly_period_target,
       jsonb_build_object(
           'migration', 52,
           'source', 'distinct-active-activity-slots'
       )
FROM activity_targets target
JOIN academic_timetable_versions version
  ON version.academic_term_id = target.academic_term_id
 AND version.status = 'published';

ALTER TABLE academic_timetable_entries
    ADD COLUMN timetable_version_id UUID;

UPDATE academic_timetable_entries entry
SET timetable_version_id = version.id
FROM academic_timetable_versions version
WHERE version.academic_term_id = entry.academic_term_id
  AND version.status = 'published';

ALTER TABLE academic_timetable_entries
    ALTER COLUMN timetable_version_id SET NOT NULL,
    ADD CONSTRAINT academic_timetable_entries_version_context_fkey
        FOREIGN KEY (timetable_version_id, academic_term_id, academic_year_id)
        REFERENCES academic_timetable_versions(id, academic_term_id, academic_year_id)
        ON DELETE RESTRICT;

DROP INDEX IF EXISTS unique_activity_entry_per_classroom_slot;
DROP INDEX IF EXISTS unique_classroom_slot;
DROP INDEX IF EXISTS idx_timetable_activities;
DROP INDEX IF EXISTS idx_timetable_day_period;
DROP INDEX IF EXISTS idx_timetable_lookup_class_sem;
DROP INDEX IF EXISTS idx_timetable_room;
DROP INDEX IF EXISTS idx_timetable_room_conflict;
DROP INDEX IF EXISTS academic_timetable_entries_term_group_offering_idx;

CREATE UNIQUE INDEX academic_timetable_entries_version_homeroom_slot_key
    ON academic_timetable_entries(
        timetable_version_id, homeroom_id, day_of_week, bell_schedule_period_id
    )
    WHERE is_active AND homeroom_id IS NOT NULL;

CREATE INDEX academic_timetable_entries_version_type_idx
    ON academic_timetable_entries(timetable_version_id, entry_type)
    WHERE entry_type <> 'COURSE';

CREATE INDEX academic_timetable_entries_version_day_period_idx
    ON academic_timetable_entries(
        timetable_version_id, day_of_week, bell_schedule_period_id
    );

CREATE INDEX academic_timetable_entries_version_group_offering_idx
    ON academic_timetable_entries(
        timetable_version_id, learning_group_id, learning_offering_id
    );

CREATE INDEX academic_timetable_entries_version_room_slot_idx
    ON academic_timetable_entries(
        timetable_version_id, room_id, day_of_week, bell_schedule_period_id
    )
    WHERE is_active AND room_id IS NOT NULL;

CREATE OR REPLACE FUNCTION check_entry_move_no_instructor_conflict()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    has_conflict BOOLEAN;
BEGIN
    IF OLD.timetable_version_id = NEW.timetable_version_id
       AND OLD.day_of_week = NEW.day_of_week
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
          AND other_entry.timetable_version_id = NEW.timetable_version_id
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
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    entry_version UUID;
    entry_day VARCHAR(10);
    entry_period UUID;
    entry_active BOOLEAN;
    entry_group UUID;
    entry_batch UUID;
    entry_kind VARCHAR(50);
    has_conflict BOOLEAN;
BEGIN
    SELECT timetable_version_id,
           day_of_week,
           bell_schedule_period_id,
           is_active,
           learning_group_id,
           batch_id,
           entry_type
      INTO entry_version,
           entry_day,
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
          AND other_entry.timetable_version_id = entry_version
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

DROP TRIGGER ate_prevent_move_conflict ON academic_timetable_entries;
CREATE TRIGGER ate_prevent_move_conflict
BEFORE UPDATE OF timetable_version_id, day_of_week, bell_schedule_period_id, is_active
ON academic_timetable_entries
FOR EACH ROW EXECUTE FUNCTION check_entry_move_no_instructor_conflict();

CREATE OR REPLACE FUNCTION academic_protect_timetable_version()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    IF TG_OP = 'DELETE' AND OLD.status = 'published' THEN
        RAISE EXCEPTION 'ACADEMIC_PUBLISHED_TIMETABLE_VERSION_IMMUTABLE'
            USING ERRCODE = 'check_violation';
    END IF;

    IF TG_OP = 'UPDATE' AND OLD.status = 'published' THEN
        RAISE EXCEPTION 'ACADEMIC_PUBLISHED_TIMETABLE_VERSION_IMMUTABLE'
            USING ERRCODE = 'check_violation';
    END IF;

    RETURN CASE WHEN TG_OP = 'DELETE' THEN OLD ELSE NEW END;
END;
$$;

CREATE TRIGGER academic_timetable_versions_published_immutable
BEFORE UPDATE OR DELETE ON academic_timetable_versions
FOR EACH ROW EXECUTE FUNCTION academic_protect_timetable_version();

CREATE OR REPLACE FUNCTION academic_protect_timetable_version_child()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    old_version_id UUID;
    new_version_id UUID;
BEGIN
    old_version_id := CASE WHEN TG_OP = 'INSERT' THEN NULL ELSE OLD.timetable_version_id END;
    new_version_id := CASE WHEN TG_OP = 'DELETE' THEN NULL ELSE NEW.timetable_version_id END;

    IF (old_version_id IS NOT NULL AND EXISTS (
            SELECT 1 FROM academic_timetable_versions version
            WHERE version.id = old_version_id AND version.status = 'published'
        ))
       OR (new_version_id IS NOT NULL AND EXISTS (
            SELECT 1 FROM academic_timetable_versions version
            WHERE version.id = new_version_id AND version.status = 'published'
        ))
    THEN
        RAISE EXCEPTION 'ACADEMIC_PUBLISHED_TIMETABLE_VERSION_CHILD_IMMUTABLE'
            USING ERRCODE = 'check_violation';
    END IF;

    RETURN CASE WHEN TG_OP = 'DELETE' THEN OLD ELSE NEW END;
END;
$$;

CREATE TRIGGER academic_timetable_entries_version_immutable
BEFORE INSERT OR UPDATE OR DELETE ON academic_timetable_entries
FOR EACH ROW EXECUTE FUNCTION academic_protect_timetable_version_child();

CREATE TRIGGER academic_timetable_version_targets_immutable
BEFORE INSERT OR UPDATE OR DELETE ON academic_timetable_version_targets
FOR EACH ROW EXECUTE FUNCTION academic_protect_timetable_version_child();

CREATE OR REPLACE FUNCTION academic_protect_published_group_teachers()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    affected_group_id UUID;
BEGIN
    affected_group_id := CASE
        WHEN TG_OP = 'INSERT' THEN NEW.learning_group_id
        ELSE OLD.learning_group_id
    END;

    IF EXISTS (
        SELECT 1
        FROM learning_groups learning_group
        WHERE learning_group.id = affected_group_id
          AND learning_group.status IN ('published', 'closed')
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_PUBLISHED_GROUP_TEACHERS_IMMUTABLE'
            USING ERRCODE = 'check_violation';
    END IF;

    IF TG_OP = 'UPDATE'
       AND NEW.learning_group_id <> OLD.learning_group_id
       AND EXISTS (
           SELECT 1
           FROM learning_groups learning_group
           WHERE learning_group.id = NEW.learning_group_id
             AND learning_group.status IN ('published', 'closed')
       )
    THEN
        RAISE EXCEPTION 'ACADEMIC_PUBLISHED_GROUP_TEACHERS_IMMUTABLE'
            USING ERRCODE = 'check_violation';
    END IF;

    RETURN CASE WHEN TG_OP = 'DELETE' THEN OLD ELSE NEW END;
END;
$$;

CREATE TRIGGER learning_group_teachers_published_immutable
BEFORE INSERT OR UPDATE OR DELETE ON learning_group_teachers
FOR EACH ROW EXECUTE FUNCTION academic_protect_published_group_teachers();

CREATE OR REPLACE FUNCTION academic_protect_change_set()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    IF OLD.status IN ('published', 'cancelled') THEN
        RAISE EXCEPTION 'ACADEMIC_TERM_CHANGE_SET_IMMUTABLE'
            USING ERRCODE = 'check_violation';
    END IF;

    RETURN CASE WHEN TG_OP = 'DELETE' THEN OLD ELSE NEW END;
END;
$$;

CREATE TRIGGER academic_term_change_sets_immutable
BEFORE UPDATE OR DELETE ON academic_term_change_sets
FOR EACH ROW EXECUTE FUNCTION academic_protect_change_set();

CREATE OR REPLACE FUNCTION academic_protect_change_set_item()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    affected_change_set_id UUID;
BEGIN
    affected_change_set_id := CASE
        WHEN TG_OP = 'INSERT' THEN NEW.change_set_id
        ELSE OLD.change_set_id
    END;

    IF EXISTS (
        SELECT 1 FROM academic_term_change_sets change_set
        WHERE change_set.id = affected_change_set_id
          AND change_set.status <> 'draft'
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_TERM_CHANGE_SET_ITEMS_IMMUTABLE'
            USING ERRCODE = 'check_violation';
    END IF;

    IF TG_OP = 'UPDATE'
       AND NEW.change_set_id <> OLD.change_set_id
       AND EXISTS (
           SELECT 1 FROM academic_term_change_sets change_set
           WHERE change_set.id = NEW.change_set_id
             AND change_set.status <> 'draft'
       )
    THEN
        RAISE EXCEPTION 'ACADEMIC_TERM_CHANGE_SET_ITEMS_IMMUTABLE'
            USING ERRCODE = 'check_violation';
    END IF;

    RETURN CASE WHEN TG_OP = 'DELETE' THEN OLD ELSE NEW END;
END;
$$;

CREATE TRIGGER academic_term_change_items_immutable
BEFORE INSERT OR UPDATE OR DELETE ON academic_term_change_items
FOR EACH ROW EXECUTE FUNCTION academic_protect_change_set_item();

DO $$
DECLARE
    enabled_trigger_count INTEGER;
BEGIN
    IF EXISTS (
        SELECT 1
        FROM academic_timetable_entries
        WHERE timetable_version_id IS NULL
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_052_TIMETABLE_VERSION_LINK_INCOMPLETE';
    END IF;

    IF (
        SELECT count(*) FROM academic_timetable_version_targets target
        JOIN learning_offerings offering ON offering.id = target.learning_offering_id
        WHERE offering.kind = 'course'
    ) <> (SELECT count(*) FROM course_offering_details) THEN
        RAISE EXCEPTION 'ACADEMIC_052_COURSE_TARGET_COUNT_MISMATCH';
    END IF;

    IF EXISTS (
        SELECT term.id
        FROM academic_terms term
        WHERE EXISTS (
                  SELECT 1 FROM learning_offerings offering
                  WHERE offering.academic_term_id = term.id
              )
           OR EXISTS (
                  SELECT 1 FROM academic_timetable_entries entry
                  WHERE entry.academic_term_id = term.id
              )
        GROUP BY term.id
        HAVING (
            SELECT count(*)
            FROM academic_timetable_versions version
            WHERE version.academic_term_id = term.id
              AND version.status = 'published'
        ) <> 1
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_052_INITIAL_VERSION_COUNT_MISMATCH';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM learning_offerings offering
        JOIN academic_years year ON year.id = offering.academic_year_id
        WHERE offering.starts_on < year.start_date
           OR offering.starts_on > year.end_date
           OR offering.ends_on < year.start_date
           OR offering.ends_on > year.end_date
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_052_OFFERING_AVAILABILITY_INVALID';
    END IF;

    SELECT count(*)::integer
    INTO enabled_trigger_count
    FROM pg_trigger trigger_record
    JOIN pg_class relation ON relation.oid = trigger_record.tgrelid
    JOIN pg_namespace namespace ON namespace.oid = relation.relnamespace
    WHERE namespace.nspname = current_schema()
      AND trigger_record.tgname = ANY(ARRAY[
          'learning_offerings_availability_guard',
          'academic_timetable_versions_published_immutable',
          'academic_timetable_entries_version_immutable',
          'academic_timetable_version_targets_immutable',
          'learning_group_teachers_published_immutable',
          'academic_term_change_sets_immutable',
          'academic_term_change_items_immutable'
      ])
      AND trigger_record.tgenabled = 'O';

    IF enabled_trigger_count <> 7 THEN
        RAISE EXCEPTION 'ACADEMIC_052_TRIGGER_ENABLEMENT_FAILED';
    END IF;
END
$$;

ALTER TABLE course_offering_details
    DROP COLUMN weekly_period_target;
