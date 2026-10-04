-- Publication dates belong to publication; draft preparation uses its own stable date.
-- No resource or version is deleted by this schema migration.
ALTER TABLE academic_delivery_versions ADD COLUMN reference_date DATE;
ALTER TABLE academic_term_change_sets ADD COLUMN reference_date DATE;
ALTER TABLE academic_delivery_versions ALTER COLUMN effective_from DROP NOT NULL;
ALTER TABLE academic_term_change_sets ALTER COLUMN effective_from DROP NOT NULL;
UPDATE academic_delivery_versions SET reference_date=effective_from,effective_from=NULL WHERE status='draft';
UPDATE academic_term_change_sets SET reference_date=effective_from,effective_from=NULL WHERE status='draft';
ALTER TABLE academic_delivery_versions ADD CONSTRAINT academic_delivery_versions_date_shape CHECK (
    (status='draft' AND effective_from IS NULL AND reference_date IS NOT NULL)
    OR (status='published' AND effective_from IS NOT NULL) OR status='cancelled');
ALTER TABLE academic_term_change_sets ADD CONSTRAINT academic_term_change_sets_date_shape CHECK (
    (status='draft' AND effective_from IS NULL AND reference_date IS NOT NULL)
    OR (status='published' AND effective_from IS NOT NULL) OR status='cancelled');

-- Cancelled data can be deleted, but cannot be edited or revived.
CREATE OR REPLACE FUNCTION academic_protect_delivery_version() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    IF OLD.status='published' OR (OLD.status='cancelled' AND TG_OP<>'DELETE') THEN
        RAISE EXCEPTION 'ACADEMIC_DELIVERY_VERSION_IMMUTABLE' USING ERRCODE='check_violation';
    END IF;
    RETURN CASE WHEN TG_OP='DELETE' THEN OLD ELSE NEW END;
END $$;
CREATE OR REPLACE FUNCTION academic_protect_change_set() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    IF OLD.status='published' OR (OLD.status='cancelled' AND TG_OP<>'DELETE') THEN
        RAISE EXCEPTION 'ACADEMIC_TERM_CHANGE_SET_IMMUTABLE' USING ERRCODE='check_violation';
    END IF;
    RETURN CASE WHEN TG_OP='DELETE' THEN OLD ELSE NEW END;
END $$;
CREATE OR REPLACE FUNCTION academic_protect_change_set_item() RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE parent_status TEXT;
BEGIN
    SELECT status INTO parent_status FROM academic_term_change_sets
      WHERE id=CASE WHEN TG_OP='INSERT' THEN NEW.change_set_id ELSE OLD.change_set_id END;
    IF parent_status='published' OR (parent_status='cancelled' AND TG_OP<>'DELETE') THEN
        RAISE EXCEPTION 'ACADEMIC_TERM_CHANGE_SET_ITEMS_IMMUTABLE' USING ERRCODE='check_violation';
    END IF;
    IF TG_OP='UPDATE' AND NEW.change_set_id<>OLD.change_set_id AND EXISTS(
        SELECT 1 FROM academic_term_change_sets WHERE id=NEW.change_set_id AND status<>'draft') THEN
        RAISE EXCEPTION 'ACADEMIC_TERM_CHANGE_SET_ITEMS_IMMUTABLE' USING ERRCODE='check_violation';
    END IF;
    RETURN CASE WHEN TG_OP='DELETE' THEN OLD ELSE NEW END;
END $$;

DROP FUNCTION academic_delivery_revision_snapshot(UUID);
CREATE OR REPLACE FUNCTION academic_delivery_revision_snapshot(revision UUID, candidate_date DATE DEFAULT NULL)
RETURNS JSONB LANGUAGE SQL STABLE AS $$
    WITH context AS (
        SELECT c.*,COALESCE(candidate_date,c.effective_from,c.reference_date) AS evaluation_date,base.snapshot AS base_snapshot FROM academic_term_change_sets c
        LEFT JOIN academic_delivery_versions base ON base.id=c.base_delivery_version_id
        WHERE c.id=revision
    ), base_targets AS (
        SELECT (offering->>'id')::UUID AS id,(offering->>'weeklyPeriodTarget')::INTEGER AS weekly_period_target
        FROM context,jsonb_array_elements(base_snapshot->'offerings') offering
        UNION ALL
        SELECT offering.id,COALESCE(subject_version.periods_per_week,activity_version.periods_per_week)
        FROM context JOIN learning_offerings offering ON offering.academic_term_id=context.academic_term_id
        LEFT JOIN course_offering_details course ON course.learning_offering_id=offering.id
        LEFT JOIN subject_versions subject_version ON subject_version.id=course.subject_version_id
        LEFT JOIN activity_offering_details activity ON activity.learning_offering_id=offering.id
        LEFT JOIN activity_versions activity_version ON activity_version.id=activity.activity_version_id
        WHERE context.base_delivery_version_id IS NULL AND offering.status IN ('draft','published')
            AND offering.starts_on<=context.evaluation_date
            AND (offering.ends_on IS NULL OR offering.ends_on>=context.evaluation_date)
            AND NOT EXISTS(SELECT 1 FROM academic_term_change_items item
                WHERE item.learning_offering_id=offering.id AND item.action_kind='add_offering'
                )
    ), selected_targets AS (
        SELECT base.id,COALESCE(adjustment.weekly_period_target,base.weekly_period_target) AS weekly_period_target
        FROM base_targets base LEFT JOIN academic_term_change_items adjustment
            ON adjustment.change_set_id=revision AND adjustment.learning_offering_id=base.id
                AND adjustment.action_kind='adjust_weekly_period_target'
        WHERE NOT EXISTS(SELECT 1 FROM academic_term_change_items stopped
            WHERE stopped.change_set_id=revision AND stopped.learning_offering_id=base.id
                AND stopped.action_kind='stop_offering')
        UNION ALL
        SELECT learning_offering_id,weekly_period_target FROM academic_term_change_items
        WHERE change_set_id=revision AND action_kind='add_offering'
    )
    SELECT academic_project_delivery_teacher_changes(academic_capture_delivery_snapshot(
        context.academic_term_id,context.evaluation_date,
        COALESCE((SELECT jsonb_agg(jsonb_build_object('id',id,'weekly_period_target',weekly_period_target) ORDER BY id)
            FROM selected_targets),'[]'::JSONB)),revision) FROM context;
$$;

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
           OR NEW.starts_on IS DISTINCT FROM COALESCE(publishing_change_set.effective_from,publishing_change_set.reference_date)
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
       OR NEW.ends_on IS DISTINCT FROM COALESCE(publishing_change_set.effective_from,publishing_change_set.reference_date) - 1
       OR COALESCE(publishing_change_set.effective_from,publishing_change_set.reference_date) <= OLD.starts_on
       OR (OLD.ends_on IS NOT NULL AND OLD.ends_on < COALESCE(publishing_change_set.effective_from,publishing_change_set.reference_date))
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

-- Timetable cancellation is likewise DELETE-only; placements remain immutable until deletion.
CREATE OR REPLACE FUNCTION academic_protect_timetable_version() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    IF OLD.status='published' OR (OLD.status='cancelled' AND TG_OP<>'DELETE') THEN
        RAISE EXCEPTION 'ACADEMIC_PUBLISHED_TIMETABLE_VERSION_IMMUTABLE' USING ERRCODE='check_violation';
    END IF;
    RETURN CASE WHEN TG_OP='DELETE' THEN OLD ELSE NEW END;
END $$;

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
            WHERE version.id = old_version_id AND (version.status = 'published' OR (version.status='cancelled' AND TG_OP<>'DELETE'))
        ))
       OR (new_version_id IS NOT NULL AND EXISTS (
            SELECT 1 FROM academic_timetable_versions version
            WHERE version.id = new_version_id AND (version.status = 'published' OR (version.status='cancelled' AND TG_OP<>'DELETE'))
        ))
    THEN
        RAISE EXCEPTION 'ACADEMIC_PUBLISHED_TIMETABLE_VERSION_CHILD_IMMUTABLE'
            USING ERRCODE = 'check_violation';
    END IF;

    RETURN CASE WHEN TG_OP = 'DELETE' THEN OLD ELSE NEW END;
END;
$$;

CREATE OR REPLACE FUNCTION academic_protect_timetable_block_child()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    old_block_id UUID;
    new_block_id UUID;
BEGIN
    IF TG_TABLE_NAME = 'academic_timetable_block_group_instructors' THEN
        IF TG_OP <> 'INSERT' THEN
            SELECT block_id INTO old_block_id
            FROM academic_timetable_block_groups
            WHERE id = OLD.block_group_id;
        END IF;
        IF TG_OP <> 'DELETE' THEN
            SELECT block_id INTO new_block_id
            FROM academic_timetable_block_groups
            WHERE id = NEW.block_group_id;
        END IF;
    ELSE
        old_block_id := CASE WHEN TG_OP = 'INSERT' THEN NULL ELSE OLD.block_id END;
        new_block_id := CASE WHEN TG_OP = 'DELETE' THEN NULL ELSE NEW.block_id END;
    END IF;

    IF TG_OP<>'DELETE' AND EXISTS (
        SELECT 1 FROM academic_timetable_blocks block JOIN academic_timetable_versions version ON version.id=block.timetable_version_id
        WHERE block.id IN(old_block_id,new_block_id) AND version.status='cancelled'
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CANCELLED_TIMETABLE_CHILD_IMMUTABLE' USING ERRCODE='check_violation';
    END IF;

    IF old_block_id IS NOT NULL THEN
        PERFORM assert_timetable_block_mutable(old_block_id);
    END IF;
    IF new_block_id IS NOT NULL AND new_block_id IS DISTINCT FROM old_block_id THEN
        PERFORM assert_timetable_block_mutable(new_block_id);
    END IF;

    RETURN CASE WHEN TG_OP = 'DELETE' THEN OLD ELSE NEW END;
END;
$$;
