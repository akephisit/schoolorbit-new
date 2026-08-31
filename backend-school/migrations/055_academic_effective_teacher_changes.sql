-- Add typed, effective-dated teacher changes and immutable timetable-handoff
-- receipts. This migration is forward-only and never repairs tenant data.

CREATE TEMP TABLE academic_055_preflight_counts ON COMMIT DROP AS
SELECT (SELECT count(*) FROM learning_offerings) AS offering_count,
       (SELECT count(*) FROM learning_groups) AS group_count,
       (SELECT count(*) FROM learning_group_teachers) AS teacher_episode_count,
       (SELECT count(*) FROM academic_timetable_versions) AS version_count,
       (SELECT count(*) FROM academic_timetable_entries) AS entry_count,
       (SELECT count(*) FROM timetable_entry_instructors) AS instructor_count,
       (SELECT count(*) FROM academic_term_change_sets) AS change_set_count,
       (SELECT count(*) FROM academic_term_change_items) AS change_item_count;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM academic_term_change_items item
        WHERE item.action_kind NOT IN (
            'add_offering',
            'stop_offering',
            'adjust_weekly_period_target'
        )
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_055_UNKNOWN_CHANGE_ACTION'
            USING ERRCODE = 'check_violation';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM learning_group_teachers teacher
        JOIN learning_groups learning_group ON learning_group.id = teacher.learning_group_id
        JOIN users user_account ON user_account.id = teacher.teacher_id
        WHERE teacher.starts_on IS NULL
           OR teacher.ends_on < teacher.starts_on
           OR teacher.academic_term_id <> learning_group.academic_term_id
           OR teacher.academic_year_id <> learning_group.academic_year_id
           OR user_account.user_type <> 'staff'
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_055_INVALID_TEACHER_EPISODE'
            USING ERRCODE = 'check_violation';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM learning_group_teachers left_episode
        JOIN learning_group_teachers right_episode
          ON right_episode.learning_group_id = left_episode.learning_group_id
         AND right_episode.teacher_id = left_episode.teacher_id
         AND right_episode.id > left_episode.id
         AND daterange(right_episode.starts_on, right_episode.ends_on, '[]')
             && daterange(left_episode.starts_on, left_episode.ends_on, '[]')
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_055_OVERLAPPING_TEACHER_EPISODE'
            USING ERRCODE = 'check_violation';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM academic_term_change_items item
        WHERE item.learning_offering_id IS NULL
           OR (item.action_kind = 'stop_offering' AND item.weekly_period_target IS NOT NULL)
           OR (item.action_kind IN ('add_offering', 'adjust_weekly_period_target')
               AND item.weekly_period_target IS NULL)
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_055_UNMAPPABLE_CHANGE_ITEM'
            USING ERRCODE = 'check_violation';
    END IF;
END;
$$;

ALTER TABLE academic_term_change_items
    DROP CONSTRAINT academic_term_change_items_action_kind_check,
    DROP CONSTRAINT academic_term_change_items_action_shape_check,
    DROP CONSTRAINT academic_term_change_items_action_offering_key,
    ALTER COLUMN learning_offering_id DROP NOT NULL,
    ADD COLUMN learning_group_id UUID,
    ADD COLUMN learning_group_teacher_id UUID,
    ADD COLUMN teacher_id UUID,
    ADD COLUMN teacher_role TEXT;

ALTER TABLE learning_group_teachers
    ADD CONSTRAINT learning_group_teachers_item_context_key
        UNIQUE (id, learning_group_id, teacher_id, academic_term_id, academic_year_id);

ALTER TABLE academic_term_change_items
    ADD CONSTRAINT academic_term_change_items_action_kind_check CHECK (
        action_kind IN (
            'add_offering',
            'stop_offering',
            'adjust_weekly_period_target',
            'add_group_teacher',
            'adjust_group_teacher_role',
            'stop_group_teacher'
        )
    ),
    ADD CONSTRAINT academic_term_change_items_teacher_role_check CHECK (
        teacher_role IS NULL OR teacher_role IN ('primary', 'secondary', 'assistant')
    ),
    ADD CONSTRAINT academic_term_change_items_action_shape_check CHECK (
        (
            action_kind = 'stop_offering'
            AND learning_offering_id IS NOT NULL
            AND weekly_period_target IS NULL
            AND learning_group_id IS NULL
            AND learning_group_teacher_id IS NULL
            AND teacher_id IS NULL
            AND teacher_role IS NULL
        )
        OR
        (
            action_kind IN ('add_offering', 'adjust_weekly_period_target')
            AND learning_offering_id IS NOT NULL
            AND weekly_period_target IS NOT NULL
            AND learning_group_id IS NULL
            AND learning_group_teacher_id IS NULL
            AND teacher_id IS NULL
            AND teacher_role IS NULL
        )
        OR
        (
            action_kind = 'add_group_teacher'
            AND learning_offering_id IS NULL
            AND weekly_period_target IS NULL
            AND learning_group_id IS NOT NULL
            AND learning_group_teacher_id IS NULL
            AND teacher_id IS NOT NULL
            AND teacher_role IS NOT NULL
        )
        OR
        (
            action_kind = 'adjust_group_teacher_role'
            AND learning_offering_id IS NULL
            AND weekly_period_target IS NULL
            AND learning_group_id IS NOT NULL
            AND learning_group_teacher_id IS NOT NULL
            AND teacher_id IS NOT NULL
            AND teacher_role IS NOT NULL
        )
        OR
        (
            action_kind = 'stop_group_teacher'
            AND learning_offering_id IS NULL
            AND weekly_period_target IS NULL
            AND learning_group_id IS NOT NULL
            AND learning_group_teacher_id IS NOT NULL
            AND teacher_id IS NOT NULL
            AND teacher_role IS NULL
        )
    ),
    ADD CONSTRAINT academic_term_change_items_group_context_fkey
        FOREIGN KEY (learning_group_id, academic_term_id, academic_year_id)
        REFERENCES learning_groups(id, academic_term_id, academic_year_id)
        ON DELETE RESTRICT,
    ADD CONSTRAINT academic_term_change_items_teacher_fkey
        FOREIGN KEY (teacher_id) REFERENCES users(id) ON DELETE RESTRICT,
    ADD CONSTRAINT academic_term_change_items_teacher_episode_context_fkey
        FOREIGN KEY (
            learning_group_teacher_id,
            learning_group_id,
            teacher_id,
            academic_term_id,
            academic_year_id
        )
        REFERENCES learning_group_teachers(
            id,
            learning_group_id,
            teacher_id,
            academic_term_id,
            academic_year_id
        )
        ON DELETE RESTRICT,
    ADD CONSTRAINT academic_term_change_items_id_context_key
        UNIQUE (id, change_set_id, academic_term_id, academic_year_id);

CREATE UNIQUE INDEX academic_term_change_items_offering_action_key
    ON academic_term_change_items(change_set_id, action_kind, learning_offering_id)
    WHERE action_kind IN ('add_offering', 'stop_offering', 'adjust_weekly_period_target');

CREATE UNIQUE INDEX academic_term_change_items_teacher_add_key
    ON academic_term_change_items(change_set_id, learning_group_id, teacher_id)
    WHERE action_kind = 'add_group_teacher';

CREATE UNIQUE INDEX academic_term_change_items_teacher_role_adjust_key
    ON academic_term_change_items(change_set_id, learning_group_teacher_id)
    WHERE action_kind = 'adjust_group_teacher_role';

CREATE UNIQUE INDEX academic_term_change_items_teacher_stop_key
    ON academic_term_change_items(change_set_id, learning_group_teacher_id)
    WHERE action_kind = 'stop_group_teacher';

ALTER TABLE academic_timetable_versions
    ADD CONSTRAINT academic_timetable_versions_change_set_item_context_key
        UNIQUE (id, change_set_id, academic_term_id, academic_year_id);

CREATE TABLE academic_teacher_handoff_runs (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    idempotency_key UUID NOT NULL UNIQUE,
    change_set_id UUID NOT NULL,
    teacher_change_item_id UUID NOT NULL,
    timetable_version_id UUID NOT NULL,
    academic_term_id UUID NOT NULL,
    academic_year_id UUID NOT NULL,
    request_hash CHAR(64) NOT NULL CHECK (request_hash ~ '^[0-9a-f]{64}$'),
    selected_entry_ids UUID[] NOT NULL CHECK (cardinality(selected_entry_ids) > 0),
    response_snapshot JSONB NOT NULL CHECK (jsonb_typeof(response_snapshot) = 'object'),
    applied_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    applied_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT academic_teacher_handoff_runs_change_set_context_fkey
        FOREIGN KEY (change_set_id, academic_term_id, academic_year_id)
        REFERENCES academic_term_change_sets(id, academic_term_id, academic_year_id)
        ON DELETE RESTRICT,
    CONSTRAINT academic_teacher_handoff_runs_item_context_fkey
        FOREIGN KEY (
            teacher_change_item_id,
            change_set_id,
            academic_term_id,
            academic_year_id
        )
        REFERENCES academic_term_change_items(
            id,
            change_set_id,
            academic_term_id,
            academic_year_id
        )
        ON DELETE RESTRICT,
    CONSTRAINT academic_teacher_handoff_runs_version_context_fkey
        FOREIGN KEY (
            timetable_version_id,
            change_set_id,
            academic_term_id,
            academic_year_id
        )
        REFERENCES academic_timetable_versions(
            id,
            change_set_id,
            academic_term_id,
            academic_year_id
        )
        ON DELETE RESTRICT
);

CREATE OR REPLACE FUNCTION academic_protect_teacher_handoff_receipt()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    RAISE EXCEPTION 'ACADEMIC_TEACHER_HANDOFF_RECEIPT_IMMUTABLE'
        USING ERRCODE = 'check_violation';
END;
$$;

CREATE TRIGGER academic_teacher_handoff_runs_immutable
BEFORE UPDATE OR DELETE ON academic_teacher_handoff_runs
FOR EACH ROW EXECUTE FUNCTION academic_protect_teacher_handoff_receipt();

CREATE OR REPLACE FUNCTION academic_protect_published_group_teachers()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    affected_group_id UUID;
    publishing_change_set_id UUID;
    publishing_change_set academic_term_change_sets%ROWTYPE;
BEGIN
    affected_group_id := CASE
        WHEN TG_OP = 'INSERT' THEN NEW.learning_group_id
        ELSE OLD.learning_group_id
    END;

    IF NOT EXISTS (
        SELECT 1
        FROM learning_groups learning_group
        WHERE learning_group.id = affected_group_id
          AND learning_group.status IN ('published', 'closed')
    ) THEN
        RETURN CASE WHEN TG_OP = 'DELETE' THEN OLD ELSE NEW END;
    END IF;

    publishing_change_set_id := NULLIF(
        current_setting('schoolorbit.academic_change_set_id', true),
        ''
    )::UUID;

    IF publishing_change_set_id IS NULL OR TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'ACADEMIC_PUBLISHED_GROUP_TEACHERS_IMMUTABLE'
            USING ERRCODE = 'check_violation';
    END IF;

    SELECT * INTO publishing_change_set
    FROM academic_term_change_sets change_set
    WHERE change_set.id = publishing_change_set_id
      AND change_set.status = 'draft'
    FOR SHARE;

    IF NOT FOUND THEN
        RAISE EXCEPTION 'ACADEMIC_TEACHER_CHANGE_PROVENANCE_INVALID'
            USING ERRCODE = 'check_violation';
    END IF;

    IF TG_OP = 'INSERT' THEN
        IF NEW.started_by_change_set_id IS DISTINCT FROM publishing_change_set.id
           OR NEW.ended_by_change_set_id IS NOT NULL
           OR NEW.academic_term_id IS DISTINCT FROM publishing_change_set.academic_term_id
           OR NEW.academic_year_id IS DISTINCT FROM publishing_change_set.academic_year_id
           OR NEW.starts_on IS DISTINCT FROM publishing_change_set.effective_from
           OR NEW.ends_on IS NOT NULL
           OR NEW.created_by IS NULL
           OR NEW.updated_by IS NULL
        THEN
            RAISE EXCEPTION 'ACADEMIC_TEACHER_CHANGE_PROVENANCE_INVALID'
                USING ERRCODE = 'check_violation';
        END IF;

        RETURN NEW;
    END IF;

    IF NEW.learning_group_id IS DISTINCT FROM OLD.learning_group_id
       OR NEW.academic_term_id IS DISTINCT FROM OLD.academic_term_id
       OR NEW.academic_year_id IS DISTINCT FROM OLD.academic_year_id
       OR NEW.teacher_id IS DISTINCT FROM OLD.teacher_id
       OR NEW.role IS DISTINCT FROM OLD.role
       OR NEW.starts_on IS DISTINCT FROM OLD.starts_on
       OR NEW.started_by_change_set_id IS DISTINCT FROM OLD.started_by_change_set_id
       OR NEW.created_by IS DISTINCT FROM OLD.created_by
       OR NEW.created_at IS DISTINCT FROM OLD.created_at
       OR NEW.migration_provenance IS DISTINCT FROM OLD.migration_provenance
       OR OLD.ended_by_change_set_id IS NOT NULL
       OR NEW.ended_by_change_set_id IS DISTINCT FROM publishing_change_set.id
       OR NEW.ends_on IS DISTINCT FROM publishing_change_set.effective_from - 1
       OR publishing_change_set.effective_from <= OLD.starts_on
       OR (OLD.ends_on IS NOT NULL AND OLD.ends_on < publishing_change_set.effective_from)
       OR NEW.row_version IS DISTINCT FROM OLD.row_version + 1
       OR NEW.updated_by IS NULL
       OR NEW.updated_at < OLD.updated_at
    THEN
        RAISE EXCEPTION 'ACADEMIC_TEACHER_CHANGE_PROVENANCE_INVALID'
            USING ERRCODE = 'check_violation';
    END IF;

    RETURN NEW;
END;
$$;

DO $$
DECLARE
    before_counts RECORD;
    after_counts RECORD;
    required_trigger_count INTEGER;
BEGIN
    SELECT * INTO before_counts FROM academic_055_preflight_counts;
    SELECT (SELECT count(*) FROM learning_offerings) AS offering_count,
           (SELECT count(*) FROM learning_groups) AS group_count,
           (SELECT count(*) FROM learning_group_teachers) AS teacher_episode_count,
           (SELECT count(*) FROM academic_timetable_versions) AS version_count,
           (SELECT count(*) FROM academic_timetable_entries) AS entry_count,
           (SELECT count(*) FROM timetable_entry_instructors) AS instructor_count,
           (SELECT count(*) FROM academic_term_change_sets) AS change_set_count,
           (SELECT count(*) FROM academic_term_change_items) AS change_item_count
      INTO after_counts;

    IF before_counts.offering_count <> after_counts.offering_count
       OR before_counts.group_count <> after_counts.group_count
       OR before_counts.teacher_episode_count <> after_counts.teacher_episode_count
       OR before_counts.version_count <> after_counts.version_count
       OR before_counts.entry_count <> after_counts.entry_count
       OR before_counts.instructor_count <> after_counts.instructor_count
       OR before_counts.change_set_count <> after_counts.change_set_count
       OR before_counts.change_item_count <> after_counts.change_item_count
    THEN
        RAISE EXCEPTION 'ACADEMIC_055_ROW_PRESERVATION_FAILED';
    END IF;

    IF EXISTS (SELECT 1 FROM academic_teacher_handoff_runs) THEN
        RAISE EXCEPTION 'ACADEMIC_055_HANDOFF_RECEIPT_NOT_EMPTY';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM learning_group_teachers left_episode
        JOIN learning_group_teachers right_episode
          ON right_episode.learning_group_id = left_episode.learning_group_id
         AND right_episode.teacher_id = left_episode.teacher_id
         AND right_episode.id > left_episode.id
         AND daterange(right_episode.starts_on, right_episode.ends_on, '[]')
             && daterange(left_episode.starts_on, left_episode.ends_on, '[]')
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_055_OVERLAPPING_TEACHER_EPISODE';
    END IF;

    SELECT count(*) INTO required_trigger_count
    FROM pg_trigger trigger_row
    JOIN pg_class target_table ON target_table.oid = trigger_row.tgrelid
    JOIN pg_namespace target_schema ON target_schema.oid = target_table.relnamespace
    WHERE target_schema.nspname = current_schema()
      AND trigger_row.tgname IN (
          'learning_group_teachers_published_immutable',
          'learning_group_teachers_interval_guard',
          'academic_teacher_handoff_runs_immutable'
      )
      AND trigger_row.tgenabled = 'O';

    IF required_trigger_count <> 3 THEN
        RAISE EXCEPTION 'ACADEMIC_055_REQUIRED_TRIGGER_MISSING';
    END IF;
END;
$$;
