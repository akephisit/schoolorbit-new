-- Fresh reconciliation is mandatory before switching the canonical owners.
-- This runs with the coordinated release in maintenance; old binaries cannot
-- serve this schema. Recovery after contraction is roll-forward.
LOCK TABLE academic_delivery_versions, academic_delivery_version_migration_audit,
    academic_timetable_versions, academic_timetable_version_targets,
    academic_term_change_sets, academic_term_change_items, learning_offerings,
    learning_groups, learning_group_teachers, learning_group_homerooms,
    learning_offering_targets, academic_timetable_blocks,
    academic_timetable_block_groups, academic_timetable_block_group_instructors,
    academic_timetable_block_homerooms, academic_timetable_block_teachers,
    academic_timetable_block_group_sync, learning_group_students,
    academic_teacher_handoff_runs IN ACCESS EXCLUSIVE MODE;

DO $$
DECLARE evidence academic_delivery_version_migration_audit%ROWTYPE; bad_count BIGINT;
BEGIN
    SELECT * INTO evidence FROM academic_delivery_version_migration_audit WHERE migration_version=87;
    IF evidence.migration_version IS NULL OR NOT evidence.passed OR evidence.cutover_completed
        OR jsonb_array_length(evidence.checks)<>15
        OR EXISTS(SELECT 1 FROM academic_delivery_legacy_fingerprints() current_state
            FULL JOIN jsonb_array_elements(evidence.checks) recorded
                ON recorded->>'code'=upper(current_state.resource)||'_PRESERVED'
            WHERE current_state.resource IS NULL OR recorded IS NULL
                OR NOT COALESCE((recorded->>'passed')::BOOLEAN,false)
                OR current_state.count IS DISTINCT FROM (recorded->>'count')::BIGINT
                OR current_state.fingerprint IS DISTINCT FROM recorded->>'fingerprint')
    THEN RAISE EXCEPTION 'DELIVERY_VERSION_FRESH_RECONCILIATION_REQUIRED'; END IF;

    SELECT count(*) INTO bad_count FROM academic_timetable_versions version
    LEFT JOIN academic_delivery_versions delivery ON delivery.id=version.delivery_version_id
    WHERE delivery.id IS NULL OR delivery.status<>'published'
        OR delivery.academic_term_id<>version.academic_term_id OR delivery.academic_year_id<>version.academic_year_id;
    IF bad_count<>0 THEN RAISE EXCEPTION 'DELIVERY_VERSION_SOURCE_INVALID count=%',bad_count; END IF;

    SELECT count(*) INTO bad_count FROM academic_timetable_versions version
    JOIN academic_delivery_versions delivery ON delivery.id=version.delivery_version_id
    WHERE version.status='published' AND COALESCE((SELECT jsonb_agg(jsonb_build_array(
        target.learning_offering_id,target.weekly_period_target) ORDER BY target.learning_offering_id)
        FROM academic_timetable_version_targets target WHERE target.timetable_version_id=version.id),'[]'::JSONB)
        IS DISTINCT FROM COALESCE((SELECT jsonb_agg(jsonb_build_array((offering->>'id')::UUID,
            (offering->>'weeklyPeriodTarget')::INTEGER) ORDER BY (offering->>'id')::UUID)
            FROM jsonb_array_elements(delivery.snapshot->'offerings') offering),'[]'::JSONB);
    IF bad_count<>0 THEN RAISE EXCEPTION 'DELIVERY_VERSION_TARGET_RECONCILIATION_FAILED count=%',bad_count; END IF;

    SELECT count(*) INTO bad_count FROM academic_term_change_sets revision
    WHERE NOT EXISTS(SELECT 1 FROM academic_term_change_items item WHERE item.change_set_id=revision.id)
        AND (EXISTS(SELECT 1 FROM learning_group_teachers teacher WHERE teacher.started_by_change_set_id=revision.id OR teacher.ended_by_change_set_id=revision.id)
            OR EXISTS(SELECT 1 FROM learning_offerings offering WHERE offering.stop_change_set_id=revision.id)
            OR EXISTS(SELECT 1 FROM academic_teacher_handoff_runs receipt WHERE receipt.change_set_id=revision.id));
    IF bad_count<>0 THEN RAISE EXCEPTION 'DELIVERY_VERSION_EMPTY_JOURNAL_REFERENCED count=%',bad_count; END IF;
END $$;

-- Materialize canonical inclusion commands from preserved explicit target IDs.
-- No legacy row is rewritten or removed before the fresh 087 gate above passes.
-- Existing journals with commands are untouched. Deterministic IDs and the
-- lifecycle audit below retain the original table/date/reason as provenance.
ALTER TABLE academic_term_change_items DISABLE TRIGGER academic_term_change_items_immutable;
INSERT INTO academic_term_change_items(id,change_set_id,academic_term_id,academic_year_id,
    action_kind,learning_offering_id,weekly_period_target,created_by,created_at,updated_at)
SELECT uuid_generate_v5(revision.id,'legacy-included-offering:'||target.learning_offering_id::TEXT),
    revision.id,revision.academic_term_id,revision.academic_year_id,'add_offering',
    target.learning_offering_id,target.weekly_period_target,revision.created_by,revision.created_at,revision.updated_at
FROM academic_term_change_sets revision
JOIN academic_timetable_version_targets target ON target.timetable_version_id=revision.target_timetable_version_id
WHERE revision.status IN ('draft','cancelled') AND revision.target_delivery_version_id IS NOT NULL
    AND NOT EXISTS(SELECT 1 FROM academic_term_change_items item WHERE item.change_set_id=revision.id)
    AND NOT EXISTS(SELECT 1 FROM academic_timetable_version_targets original
        WHERE original.timetable_version_id=revision.base_timetable_version_id
            AND original.learning_offering_id=target.learning_offering_id);
ALTER TABLE academic_term_change_items ENABLE TRIGGER academic_term_change_items_immutable;

CREATE FUNCTION academic_delivery_cutover_fingerprints()
RETURNS TABLE(resource TEXT,count BIGINT,fingerprint TEXT) LANGUAGE SQL STABLE AS $$
SELECT 'timetables'::TEXT,count(*),md5(COALESCE(jsonb_agg(to_jsonb(row)-'change_set_id'-'effective_from'-'publication_idempotency_key'-'publication_request_hash' ORDER BY id)::TEXT,'[]')) FROM academic_timetable_versions row
UNION ALL SELECT 'revisions'::TEXT,count(*),md5(COALESCE(jsonb_agg(to_jsonb(row)-'base_timetable_version_id'-'target_timetable_version_id' ORDER BY id)::TEXT,'[]')) FROM academic_term_change_sets row WHERE EXISTS(SELECT 1 FROM academic_term_change_items item WHERE item.change_set_id=row.id)
UNION ALL SELECT 'delivery_versions'::TEXT,count(*),md5(COALESCE(jsonb_agg(to_jsonb(row) ORDER BY id)::TEXT,'[]')) FROM academic_delivery_versions row
UNION ALL SELECT 'blocks'::TEXT,count(*),md5(COALESCE(jsonb_agg(to_jsonb(row) ORDER BY id)::TEXT,'[]')) FROM academic_timetable_blocks row
UNION ALL SELECT 'block_groups'::TEXT,count(*),md5(COALESCE(jsonb_agg(to_jsonb(row)-'homeroom_ids' ORDER BY id)::TEXT,'[]')) FROM academic_timetable_block_groups row
UNION ALL SELECT 'instructors'::TEXT,count(*),md5(COALESCE(jsonb_agg(to_jsonb(row) ORDER BY id)::TEXT,'[]')) FROM academic_timetable_block_group_instructors row
UNION ALL SELECT 'offerings'::TEXT,count(*),md5(COALESCE(jsonb_agg(to_jsonb(row) ORDER BY id)::TEXT,'[]')) FROM learning_offerings row
UNION ALL SELECT 'groups'::TEXT,count(*),md5(COALESCE(jsonb_agg(to_jsonb(row) ORDER BY id)::TEXT,'[]')) FROM learning_groups row
UNION ALL SELECT 'teachers'::TEXT,count(*),md5(COALESCE(jsonb_agg(to_jsonb(row) ORDER BY id)::TEXT,'[]')) FROM learning_group_teachers row
UNION ALL SELECT 'handoffs'::TEXT,count(*),md5(COALESCE(jsonb_agg(to_jsonb(row) ORDER BY id)::TEXT,'[]')) FROM academic_teacher_handoff_runs row
UNION ALL SELECT 'revision_items'::TEXT,count(*),md5(COALESCE(jsonb_agg(to_jsonb(row) ORDER BY id)::TEXT,'[]')) FROM academic_term_change_items row
UNION ALL SELECT 'homeroom_placements'::TEXT,count(*),md5(COALESCE(jsonb_agg(to_jsonb(row) ORDER BY id)::TEXT,'[]')) FROM academic_timetable_block_homerooms row
UNION ALL SELECT 'teacher_placements'::TEXT,count(*),md5(COALESCE(jsonb_agg(to_jsonb(row) ORDER BY id)::TEXT,'[]')) FROM academic_timetable_block_teachers row
UNION ALL SELECT 'sync_states'::TEXT,count(*),md5(COALESCE(jsonb_agg(to_jsonb(row) ORDER BY id)::TEXT,'[]')) FROM academic_timetable_block_group_sync row
UNION ALL SELECT 'rosters'::TEXT,count(*),md5(COALESCE(jsonb_agg(to_jsonb(row) ORDER BY id)::TEXT,'[]')) FROM learning_group_students row;
$$;
CREATE TEMP TABLE academic_088_before ON COMMIT DROP AS SELECT * FROM academic_delivery_cutover_fingerprints();

-- Preserve lifecycle provenance as canonical audit events, including original
-- reasons, dates and identifiers. It no longer controls timetable publication.
INSERT INTO academic_audit_events(id,event_code,entity_type,entity_id,academic_year_id,academic_term_id,actor_user_id,payload,created_at)
SELECT uuid_generate_v5(revision.id,'independent-version-cutover'),
    'academic_revision.lifecycle_migrated','academic_term_change_set',revision.id,
    revision.academic_year_id,revision.academic_term_id,revision.created_by,
    to_jsonb(revision)-'base_delivery_version_id'-'target_delivery_version_id',revision.created_at
FROM academic_term_change_sets revision;

INSERT INTO academic_audit_events(id,event_code,entity_type,entity_id,academic_year_id,academic_term_id,actor_user_id,payload,created_at)
SELECT uuid_generate_v5(version.id,'independent-version-cutover'),
    'academic_timetable_version.lifecycle_migrated','academic_timetable_version',version.id,
    version.academic_year_id,version.academic_term_id,COALESCE(version.created_by,version.published_by),
    jsonb_build_object('effectiveFrom',version.effective_from,'changeSetId',version.change_set_id,
        'legacyTargets',COALESCE((SELECT jsonb_agg(to_jsonb(target) ORDER BY target.learning_offering_id)
            FROM academic_timetable_version_targets target WHERE target.timetable_version_id=version.id),'[]'::JSONB)),
    version.created_at
FROM academic_timetable_versions version;

-- Actual placement coverage is stable even when its intended delivery group
-- later changes. Operational drafts use their recorded draft graph for coverage.
ALTER TABLE academic_timetable_block_groups ADD COLUMN homeroom_ids UUID[];
ALTER TABLE academic_timetable_blocks DISABLE TRIGGER academic_timetable_blocks_version_immutable;
-- The group immutable guard resolves through its parent block.
ALTER TABLE academic_timetable_block_groups DISABLE TRIGGER academic_timetable_block_groups_version_immutable;
CREATE TEMP TABLE academic_088_coverage ON COMMIT DROP AS
SELECT placed.id, source_group->'homeroomIds' AS homeroom_ids
FROM academic_timetable_block_groups placed
JOIN academic_timetable_blocks block ON block.id=placed.block_id
JOIN academic_timetable_versions version ON version.id=block.timetable_version_id
LEFT JOIN academic_term_change_sets revision ON revision.id=version.change_set_id
LEFT JOIN LATERAL (
    SELECT candidate.source_group FROM (
        SELECT source_group,1 AS priority FROM academic_delivery_versions delivery,
            jsonb_array_elements(delivery.snapshot->'offerings') offering,
            jsonb_array_elements(offering->'groups') source_group
        WHERE delivery.id=revision.target_delivery_version_id
            AND (offering->>'id')::UUID=placed.learning_offering_id AND (source_group->>'id')::UUID=placed.learning_group_id
        UNION ALL
        SELECT source_group,2 AS priority FROM academic_delivery_versions delivery,
            jsonb_array_elements(delivery.snapshot->'offerings') offering,
            jsonb_array_elements(offering->'groups') source_group
        WHERE delivery.id=version.delivery_version_id
            AND (offering->>'id')::UUID=placed.learning_offering_id AND (source_group->>'id')::UUID=placed.learning_group_id
    ) candidate ORDER BY priority LIMIT 1
) evidence ON true;
DO $$ DECLARE bad_count BIGINT; BEGIN
    SELECT count(*) INTO bad_count FROM academic_088_coverage WHERE homeroom_ids IS NULL;
    IF bad_count<>0 THEN RAISE EXCEPTION 'DELIVERY_PLACEMENT_COVERAGE_UNMAPPABLE count=%',bad_count; END IF;
END $$;
UPDATE academic_timetable_block_groups placed SET homeroom_ids=ARRAY(
    SELECT (element.value#>>'{}')::UUID FROM jsonb_array_elements(coverage.homeroom_ids) AS element(value) ORDER BY (element.value#>>'{}')::UUID
) FROM academic_088_coverage coverage WHERE coverage.id=placed.id;
ALTER TABLE academic_timetable_block_groups ALTER COLUMN homeroom_ids SET NOT NULL,
    ADD CONSTRAINT academic_timetable_block_groups_coverage_shape_check CHECK(array_position(homeroom_ids,NULL) IS NULL);
ALTER TABLE academic_timetable_blocks ENABLE TRIGGER academic_timetable_blocks_version_immutable;
ALTER TABLE academic_timetable_block_groups ENABLE TRIGGER academic_timetable_block_groups_version_immutable;

-- Immutable handoff receipts keep every original ID and item relationship;
-- their timetable context no longer depends on a joint revision lifecycle.
ALTER TABLE academic_teacher_handoff_runs DROP CONSTRAINT academic_teacher_handoff_runs_version_context_fkey;
ALTER TABLE academic_teacher_handoff_runs ADD CONSTRAINT academic_teacher_handoff_runs_version_context_fkey
    FOREIGN KEY(timetable_version_id,academic_term_id,academic_year_id)
    REFERENCES academic_timetable_versions(id,academic_term_id,academic_year_id) ON DELETE RESTRICT;

ALTER TABLE academic_term_change_sets
    DROP CONSTRAINT academic_term_change_sets_base_version_context_fkey,
    DROP CONSTRAINT academic_term_change_sets_target_version_context_fkey;
ALTER TABLE academic_timetable_versions
    DROP CONSTRAINT academic_timetable_versions_change_set_context_fkey,
    DROP CONSTRAINT academic_timetable_versions_change_set_key,
    DROP CONSTRAINT academic_timetable_versions_change_set_item_context_key;
ALTER TABLE academic_timetable_versions DISABLE TRIGGER academic_timetable_versions_published_immutable;
ALTER TABLE academic_timetable_versions
    DROP COLUMN change_set_id,
    ALTER COLUMN delivery_version_id SET NOT NULL,
    ALTER COLUMN effective_from DROP NOT NULL,
    ADD CONSTRAINT academic_timetable_versions_published_date_check CHECK(status<>'published' OR effective_from IS NOT NULL),
    ADD COLUMN publication_idempotency_key UUID,
    ADD COLUMN publication_request_hash TEXT;
UPDATE academic_timetable_versions SET effective_from=NULL WHERE status='draft';
ALTER TABLE academic_timetable_versions ENABLE TRIGGER academic_timetable_versions_published_immutable;
DROP INDEX academic_timetable_versions_live_effective_key;
CREATE UNIQUE INDEX academic_timetable_versions_published_effective_key
    ON academic_timetable_versions(academic_term_id,effective_from) WHERE status='published';
CREATE UNIQUE INDEX academic_timetable_versions_publication_idempotency_key
    ON academic_timetable_versions(publication_idempotency_key) WHERE publication_idempotency_key IS NOT NULL;
ALTER TABLE academic_timetable_versions ADD CONSTRAINT academic_timetable_versions_publication_receipt_check CHECK(
    (publication_idempotency_key IS NULL AND publication_request_hash IS NULL)
    OR (status='published' AND publication_idempotency_key IS NOT NULL AND publication_request_hash ~ '^[0-9a-f]{64}$')
);

ALTER TABLE academic_term_change_sets DISABLE TRIGGER academic_term_change_sets_immutable;
DELETE FROM academic_term_change_sets revision WHERE NOT EXISTS(
    SELECT 1 FROM academic_term_change_items item WHERE item.change_set_id=revision.id);
ALTER TABLE academic_term_change_sets
    DROP COLUMN base_timetable_version_id,
    DROP COLUMN target_timetable_version_id,
    ALTER COLUMN target_delivery_version_id SET NOT NULL;
ALTER TABLE academic_term_change_sets ENABLE TRIGGER academic_term_change_sets_immutable;

CREATE FUNCTION academic_validate_timetable_delivery_source() RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE source academic_delivery_versions%ROWTYPE; last_date DATE;
BEGIN
    SELECT * INTO source FROM academic_delivery_versions WHERE id=NEW.delivery_version_id FOR SHARE;
    IF source.id IS NULL OR source.status<>'published' OR source.academic_term_id<>NEW.academic_term_id
        OR source.academic_year_id<>NEW.academic_year_id
    THEN RAISE EXCEPTION 'ACADEMIC_TIMETABLE_DELIVERY_SOURCE_INVALID' USING ERRCODE='check_violation'; END IF;
    IF NEW.status='published' THEN
        SELECT min(effective_from)-1 INTO last_date FROM academic_delivery_versions
            WHERE academic_term_id=source.academic_term_id AND status='published' AND effective_from>source.effective_from;
        IF NEW.effective_from IS NULL OR NEW.effective_from<source.effective_from
            OR (last_date IS NOT NULL AND NEW.effective_from>last_date)
        THEN RAISE EXCEPTION 'ACADEMIC_TIMETABLE_DELIVERY_DATE_MISMATCH' USING ERRCODE='check_violation'; END IF;
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER academic_timetable_versions_delivery_source_guard
    BEFORE INSERT OR UPDATE OF delivery_version_id,status,effective_from ON academic_timetable_versions
    FOR EACH ROW EXECUTE FUNCTION academic_validate_timetable_delivery_source();

CREATE OR REPLACE FUNCTION assert_timetable_block_conflict_free(target_block_id UUID)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    target_block academic_timetable_blocks%ROWTYPE;
    slot_lock_key BIGINT;
BEGIN
    SELECT * INTO target_block
    FROM academic_timetable_blocks
    WHERE id = target_block_id;

    IF target_block.id IS NULL OR NOT target_block.is_active THEN
        RETURN;
    END IF;

    slot_lock_key := hashtextextended(
        target_block.timetable_version_id::text || ':' || target_block.day_of_week || ':'
            || target_block.bell_schedule_period_id::text,
        0
    );
    PERFORM pg_advisory_xact_lock(slot_lock_key);

    IF EXISTS (
        SELECT 1
        FROM academic_timetable_block_groups candidate
        JOIN academic_timetable_block_groups occupied
          ON occupied.learning_group_id = candidate.learning_group_id
         AND occupied.block_id <> candidate.block_id
         AND occupied.is_active
        JOIN academic_timetable_blocks occupied_block ON occupied_block.id = occupied.block_id
        WHERE candidate.block_id = target_block_id
          AND candidate.is_active
          AND occupied_block.is_active
          AND occupied_block.timetable_version_id = target_block.timetable_version_id
          AND occupied_block.day_of_week = target_block.day_of_week
          AND occupied_block.bell_schedule_period_id = target_block.bell_schedule_period_id
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_TIMETABLE_GROUP_CONFLICT'
            USING ERRCODE = 'check_violation';
    END IF;

    IF EXISTS (
        WITH candidate_homerooms AS (
            SELECT homeroom_id
            FROM academic_timetable_block_homerooms
            WHERE block_id = target_block_id AND is_active
            UNION
            SELECT coverage.homeroom_id
            FROM academic_timetable_block_groups block_group
            CROSS JOIN LATERAL unnest(block_group.homeroom_ids) AS coverage(homeroom_id)
            WHERE block_group.block_id = target_block_id AND block_group.is_active
        ), occupied_homerooms AS (
            SELECT homeroom.homeroom_id, homeroom.block_id
            FROM academic_timetable_block_homerooms homeroom
            JOIN academic_timetable_blocks block ON block.id = homeroom.block_id
            WHERE homeroom.block_id <> target_block_id
              AND homeroom.is_active AND block.is_active
              AND block.timetable_version_id = target_block.timetable_version_id
              AND block.day_of_week = target_block.day_of_week
              AND block.bell_schedule_period_id = target_block.bell_schedule_period_id
            UNION
            SELECT coverage.homeroom_id, block_group.block_id
            FROM academic_timetable_block_groups block_group
            JOIN academic_timetable_blocks block ON block.id = block_group.block_id
            CROSS JOIN LATERAL unnest(block_group.homeroom_ids) AS coverage(homeroom_id)
            WHERE block_group.block_id <> target_block_id
              AND block_group.is_active AND block.is_active
              AND block.timetable_version_id = target_block.timetable_version_id
              AND block.day_of_week = target_block.day_of_week
              AND block.bell_schedule_period_id = target_block.bell_schedule_period_id
        )
        SELECT 1
        FROM candidate_homerooms candidate
        JOIN occupied_homerooms occupied USING (homeroom_id)
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_TIMETABLE_HOMEROOM_CONFLICT'
            USING ERRCODE = 'check_violation';
    END IF;

    IF EXISTS (
        WITH candidate_teachers AS (
            SELECT teacher_id
            FROM academic_timetable_block_teachers
            WHERE block_id = target_block_id AND is_active
            UNION
            SELECT instructor.instructor_id
            FROM academic_timetable_block_groups block_group
            JOIN academic_timetable_block_group_instructors instructor
              ON instructor.block_group_id = block_group.id
            WHERE block_group.block_id = target_block_id AND block_group.is_active
        ), occupied_teachers AS (
            SELECT teacher.teacher_id, teacher.block_id
            FROM academic_timetable_block_teachers teacher
            JOIN academic_timetable_blocks block ON block.id = teacher.block_id
            WHERE teacher.block_id <> target_block_id
              AND teacher.is_active AND block.is_active
              AND block.timetable_version_id = target_block.timetable_version_id
              AND block.day_of_week = target_block.day_of_week
              AND block.bell_schedule_period_id = target_block.bell_schedule_period_id
            UNION
            SELECT instructor.instructor_id, block_group.block_id
            FROM academic_timetable_block_groups block_group
            JOIN academic_timetable_blocks block ON block.id = block_group.block_id
            JOIN academic_timetable_block_group_instructors instructor
              ON instructor.block_group_id = block_group.id
            WHERE block_group.block_id <> target_block_id
              AND block_group.is_active AND block.is_active
              AND block.timetable_version_id = target_block.timetable_version_id
              AND block.day_of_week = target_block.day_of_week
              AND block.bell_schedule_period_id = target_block.bell_schedule_period_id
        )
        SELECT 1
        FROM candidate_teachers candidate
        JOIN occupied_teachers occupied USING (teacher_id)
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_TIMETABLE_INSTRUCTOR_DOUBLE_BOOKED'
            USING ERRCODE = 'check_violation';
    END IF;

    IF EXISTS (
        WITH candidate_rooms AS (
            SELECT room_id
            FROM academic_timetable_block_groups
            WHERE block_id = target_block_id AND is_active AND room_id IS NOT NULL
            UNION
            SELECT room_id
            FROM academic_timetable_block_homerooms
            WHERE block_id = target_block_id AND is_active AND room_id IS NOT NULL
        ), occupied_rooms AS (
            SELECT block_group.room_id, block_group.block_id
            FROM academic_timetable_block_groups block_group
            JOIN academic_timetable_blocks block ON block.id = block_group.block_id
            WHERE block_group.block_id <> target_block_id
              AND block_group.is_active AND block.is_active
              AND block_group.room_id IS NOT NULL
              AND block.timetable_version_id = target_block.timetable_version_id
              AND block.day_of_week = target_block.day_of_week
              AND block.bell_schedule_period_id = target_block.bell_schedule_period_id
            UNION
            SELECT homeroom.room_id, homeroom.block_id
            FROM academic_timetable_block_homerooms homeroom
            JOIN academic_timetable_blocks block ON block.id = homeroom.block_id
            WHERE homeroom.block_id <> target_block_id
              AND homeroom.is_active AND block.is_active
              AND homeroom.room_id IS NOT NULL
              AND block.timetable_version_id = target_block.timetable_version_id
              AND block.day_of_week = target_block.day_of_week
              AND block.bell_schedule_period_id = target_block.bell_schedule_period_id
        )
        SELECT 1
        FROM candidate_rooms candidate
        JOIN occupied_rooms occupied USING (room_id)
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_TIMETABLE_ROOM_CONFLICT'
            USING ERRCODE = 'check_violation';
    END IF;
END;
$$;


-- Placement coverage, rather than mutable delivery registry coverage, controls
-- conflict checks for both current and historical pinned schedules.
DROP TRIGGER academic_timetable_block_groups_conflict_guard ON academic_timetable_block_groups;
CREATE TRIGGER academic_timetable_block_groups_conflict_guard
    AFTER INSERT OR UPDATE OF block_id,learning_group_id,room_id,is_active,homeroom_ids
    ON academic_timetable_block_groups FOR EACH ROW
    EXECUTE FUNCTION academic_validate_timetable_block_child_change();

CREATE FUNCTION academic_touch_timetable_content_revision() RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE version_id UUID; parent_block_id UUID; child_group_id UUID;
BEGIN
    IF TG_TABLE_NAME='academic_timetable_blocks' THEN
        version_id := CASE WHEN TG_OP='DELETE' THEN OLD.timetable_version_id ELSE NEW.timetable_version_id END;
    ELSE
        IF TG_TABLE_NAME='academic_timetable_block_group_instructors' THEN
            child_group_id := CASE WHEN TG_OP='DELETE' THEN OLD.block_group_id ELSE NEW.block_group_id END;
            SELECT block_id INTO parent_block_id FROM academic_timetable_block_groups WHERE id=child_group_id;
        ELSE
            parent_block_id := CASE WHEN TG_OP='DELETE' THEN OLD.block_id ELSE NEW.block_id END;
        END IF;
        SELECT timetable_version_id INTO version_id FROM academic_timetable_blocks WHERE id=parent_block_id;
    END IF;
    UPDATE academic_timetable_versions SET row_version=row_version+1,updated_at=now() WHERE id=version_id AND status='draft';
    RETURN CASE WHEN TG_OP='DELETE' THEN OLD ELSE NEW END;
END $$;
CREATE TRIGGER academic_timetable_blocks_content_revision AFTER INSERT OR UPDATE OR DELETE ON academic_timetable_blocks
    FOR EACH ROW EXECUTE FUNCTION academic_touch_timetable_content_revision();
CREATE TRIGGER academic_timetable_block_groups_content_revision AFTER INSERT OR UPDATE OR DELETE ON academic_timetable_block_groups
    FOR EACH ROW EXECUTE FUNCTION academic_touch_timetable_content_revision();
CREATE TRIGGER academic_timetable_block_group_instructors_content_revision AFTER INSERT OR UPDATE OR DELETE ON academic_timetable_block_group_instructors
    FOR EACH ROW EXECUTE FUNCTION academic_touch_timetable_content_revision();
CREATE TRIGGER academic_timetable_block_homerooms_content_revision AFTER INSERT OR UPDATE OR DELETE ON academic_timetable_block_homerooms
    FOR EACH ROW EXECUTE FUNCTION academic_touch_timetable_content_revision();
CREATE TRIGGER academic_timetable_block_teachers_content_revision AFTER INSERT OR UPDATE OR DELETE ON academic_timetable_block_teachers
    FOR EACH ROW EXECUTE FUNCTION academic_touch_timetable_content_revision();
CREATE TRIGGER academic_timetable_block_group_sync_content_revision AFTER INSERT OR UPDATE OR DELETE ON academic_timetable_block_group_sync
    FOR EACH ROW EXECUTE FUNCTION academic_touch_timetable_content_revision();

DROP TABLE academic_timetable_version_targets;
DROP FUNCTION academic_delivery_legacy_fingerprints();
CREATE TEMP TABLE academic_088_after ON COMMIT DROP AS SELECT * FROM academic_delivery_cutover_fingerprints();
DO $$ BEGIN
    IF EXISTS(SELECT 1 FROM academic_088_before b FULL JOIN academic_088_after a USING(resource)
        WHERE b.count IS DISTINCT FROM a.count OR b.fingerprint IS DISTINCT FROM a.fingerprint)
        OR EXISTS(SELECT 1 FROM academic_088_coverage expected JOIN academic_timetable_block_groups actual ON actual.id=expected.id
            WHERE actual.homeroom_ids IS DISTINCT FROM ARRAY(SELECT (element.value#>>'{}')::UUID FROM jsonb_array_elements(expected.homeroom_ids) AS element(value) ORDER BY (element.value#>>'{}')::UUID))
        OR EXISTS(SELECT 1 FROM academic_timetable_versions version JOIN academic_delivery_versions delivery ON delivery.id=version.delivery_version_id
            WHERE delivery.status<>'published' OR delivery.academic_term_id<>version.academic_term_id OR delivery.academic_year_id<>version.academic_year_id)
    THEN RAISE EXCEPTION 'DELIVERY_CANONICAL_RECONCILIATION_FAILED'; END IF;
END $$;
INSERT INTO academic_delivery_version_migration_audit(migration_version,checks,passed,cutover_completed)
SELECT 88,jsonb_agg(jsonb_build_object('code',upper(resource)||'_CANONICAL_PRESERVED','count',count,'fingerprint',fingerprint,'passed',true) ORDER BY resource),true,true FROM academic_088_after;
DROP FUNCTION academic_delivery_cutover_fingerprints();
UPDATE academic_delivery_version_migration_audit SET cutover_completed=true WHERE migration_version=87;
