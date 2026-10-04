-- Migrate and reconcile before any consumer or legacy owner is removed.
LOCK TABLE academic_timetable_versions, academic_timetable_version_targets,
    academic_term_change_sets, academic_term_change_items, learning_offerings,
    learning_groups, learning_group_teachers, learning_group_homerooms,
    learning_offering_targets, academic_timetable_blocks,
    academic_timetable_block_groups, academic_timetable_block_group_instructors,
    academic_timetable_block_homerooms, academic_timetable_block_teachers,
    academic_timetable_block_group_sync, learning_group_students,
    academic_teacher_handoff_runs IN ACCESS EXCLUSIVE MODE;

CREATE FUNCTION academic_delivery_legacy_fingerprints()
RETURNS TABLE(resource TEXT,count BIGINT,fingerprint TEXT) LANGUAGE SQL STABLE AS $$
SELECT 'timetables'::TEXT,count(*),md5(COALESCE(jsonb_agg(to_jsonb(row)-'delivery_version_id' ORDER BY id)::TEXT,'[]')) FROM academic_timetable_versions row
UNION ALL SELECT 'blocks'::TEXT,count(*),md5(COALESCE(jsonb_agg(to_jsonb(row) ORDER BY id)::TEXT,'[]')) FROM academic_timetable_blocks row
UNION ALL SELECT 'block_groups'::TEXT,count(*),md5(COALESCE(jsonb_agg(to_jsonb(row) ORDER BY id)::TEXT,'[]')) FROM academic_timetable_block_groups row
UNION ALL SELECT 'instructors'::TEXT,count(*),md5(COALESCE(jsonb_agg(to_jsonb(row) ORDER BY id)::TEXT,'[]')) FROM academic_timetable_block_group_instructors row
UNION ALL SELECT 'offerings'::TEXT,count(*),md5(COALESCE(jsonb_agg(to_jsonb(row) ORDER BY id)::TEXT,'[]')) FROM learning_offerings row
UNION ALL SELECT 'groups'::TEXT,count(*),md5(COALESCE(jsonb_agg(to_jsonb(row) ORDER BY id)::TEXT,'[]')) FROM learning_groups row
UNION ALL SELECT 'teachers'::TEXT,count(*),md5(COALESCE(jsonb_agg(to_jsonb(row) ORDER BY id)::TEXT,'[]')) FROM learning_group_teachers row
UNION ALL SELECT 'handoffs'::TEXT,count(*),md5(COALESCE(jsonb_agg(to_jsonb(row) ORDER BY id)::TEXT,'[]')) FROM academic_teacher_handoff_runs row
UNION ALL SELECT 'timetable_targets'::TEXT,count(*),md5(COALESCE(jsonb_agg(to_jsonb(row) ORDER BY timetable_version_id,learning_offering_id)::TEXT,'[]')) FROM academic_timetable_version_targets row
UNION ALL SELECT 'revisions'::TEXT,count(*),md5(COALESCE(jsonb_agg(to_jsonb(row)-'base_delivery_version_id'-'target_delivery_version_id' ORDER BY id)::TEXT,'[]')) FROM academic_term_change_sets row
UNION ALL SELECT 'revision_items'::TEXT,count(*),md5(COALESCE(jsonb_agg(to_jsonb(row) ORDER BY id)::TEXT,'[]')) FROM academic_term_change_items row
UNION ALL SELECT 'homeroom_placements'::TEXT,count(*),md5(COALESCE(jsonb_agg(to_jsonb(row) ORDER BY id)::TEXT,'[]')) FROM academic_timetable_block_homerooms row
UNION ALL SELECT 'teacher_placements'::TEXT,count(*),md5(COALESCE(jsonb_agg(to_jsonb(row) ORDER BY id)::TEXT,'[]')) FROM academic_timetable_block_teachers row
UNION ALL SELECT 'sync_states'::TEXT,count(*),md5(COALESCE(jsonb_agg(to_jsonb(row) ORDER BY id)::TEXT,'[]')) FROM academic_timetable_block_group_sync row
UNION ALL SELECT 'rosters'::TEXT,count(*),md5(COALESCE(jsonb_agg(to_jsonb(row) ORDER BY id)::TEXT,'[]')) FROM learning_group_students row;
$$;

CREATE TEMP TABLE academic_087_before ON COMMIT DROP AS
SELECT * FROM academic_delivery_legacy_fingerprints();

-- The retired direct-inclusion API wrote exact offering IDs/targets without
-- edit items. Recognize only additive targets on the journal's recorded source;
-- removals, altered targets, missing journals and conflicting sources still stop.
CREATE TEMP TABLE academic_087_direct_inclusions ON COMMIT DROP AS
SELECT revision.id AS revision_id
FROM academic_term_change_sets revision
JOIN academic_timetable_versions draft ON draft.id=revision.target_timetable_version_id
JOIN academic_timetable_versions source ON source.id=revision.base_timetable_version_id
WHERE revision.status IN ('draft','cancelled') AND draft.status=revision.status
    AND draft.change_set_id=revision.id AND draft.source_version_id=source.id
    AND source.status='published'
    AND draft.academic_term_id=revision.academic_term_id
    AND draft.academic_year_id=revision.academic_year_id
    AND source.academic_term_id=draft.academic_term_id
    AND source.academic_year_id=draft.academic_year_id
    AND NOT EXISTS(SELECT 1 FROM academic_term_change_items item WHERE item.change_set_id=revision.id)
    AND NOT EXISTS(SELECT 1 FROM academic_timetable_version_targets original
        LEFT JOIN academic_timetable_version_targets target
            ON target.timetable_version_id=draft.id AND target.learning_offering_id=original.learning_offering_id
        WHERE original.timetable_version_id=source.id
            AND target.weekly_period_target IS DISTINCT FROM original.weekly_period_target)
    AND EXISTS(SELECT 1 FROM academic_timetable_version_targets target
        WHERE target.timetable_version_id=draft.id AND NOT EXISTS(
            SELECT 1 FROM academic_timetable_version_targets original
            WHERE original.timetable_version_id=source.id
                AND original.learning_offering_id=target.learning_offering_id));

DO $$
DECLARE bad_count BIGINT;
BEGIN
    SELECT count(*) INTO bad_count FROM academic_timetable_versions v
    LEFT JOIN academic_term_change_sets c ON c.id=v.change_set_id
    LEFT JOIN academic_timetable_versions source ON source.id=COALESCE(v.source_version_id,c.base_timetable_version_id)
    WHERE v.status<>'published' AND (source.id IS NULL OR source.status<>'published'
        OR source.academic_term_id<>v.academic_term_id OR source.academic_year_id<>v.academic_year_id);
    IF bad_count<>0 THEN
        RAISE EXCEPTION 'DELIVERY_VERSION_SOURCE_UNMAPPABLE count=%',bad_count;
    END IF;
    SELECT count(*) INTO bad_count FROM academic_term_change_sets c
    LEFT JOIN academic_timetable_versions base ON base.id=c.base_timetable_version_id
    LEFT JOIN academic_timetable_versions target ON target.id=c.target_timetable_version_id
    WHERE base.id IS NULL OR target.id IS NULL OR base.status<>'published'
        OR base.academic_term_id<>c.academic_term_id OR target.academic_term_id<>c.academic_term_id;
    IF bad_count<>0 THEN
        RAISE EXCEPTION 'DELIVERY_REVISION_SOURCE_UNMAPPABLE count=%',bad_count;
    END IF;
    SELECT count(*) INTO bad_count FROM academic_timetable_versions draft
    LEFT JOIN academic_term_change_sets revision ON revision.id=draft.change_set_id
    JOIN academic_timetable_versions source ON source.id=COALESCE(draft.source_version_id,revision.base_timetable_version_id)
    WHERE draft.status<>'published'
      AND NOT EXISTS(SELECT 1 FROM academic_term_change_items item WHERE item.change_set_id=revision.id)
      AND NOT EXISTS(SELECT 1 FROM academic_087_direct_inclusions evidence WHERE evidence.revision_id=revision.id)
      AND COALESCE((SELECT jsonb_agg(jsonb_build_array(target.learning_offering_id,target.weekly_period_target)
            ORDER BY target.learning_offering_id) FROM academic_timetable_version_targets target
            WHERE target.timetable_version_id=draft.id),'[]'::JSONB)
        IS DISTINCT FROM COALESCE((SELECT jsonb_agg(jsonb_build_array(target.learning_offering_id,target.weekly_period_target)
            ORDER BY target.learning_offering_id) FROM academic_timetable_version_targets target
            WHERE target.timetable_version_id=source.id),'[]'::JSONB);
    IF bad_count<>0 THEN
        RAISE EXCEPTION 'DELIVERY_DRAFT_TARGETS_UNMAPPABLE count=%',bad_count;
    END IF;
END $$;

-- Import timestamps do not define the start of a recorded teaching assignment.
-- A later-created group is historical only when its exact placed instructors
-- each have one dated assignment (same group, teacher and role) at this boundary.
-- Every active placement of that group must agree; no name/teacher inference.
CREATE TEMP TABLE academic_087_dated_group_evidence ON COMMIT DROP AS
SELECT DISTINCT version.id AS timetable_version_id, placed.learning_offering_id,
    placed.learning_group_id
FROM academic_timetable_versions version
JOIN academic_timetable_blocks block ON block.timetable_version_id=version.id AND block.is_active
JOIN academic_timetable_block_groups placed ON placed.block_id=block.id AND placed.is_active
JOIN learning_groups learning_group ON learning_group.id=placed.learning_group_id
    AND learning_group.learning_offering_id=placed.learning_offering_id
WHERE version.status='published' AND learning_group.status<>'cancelled'
    AND EXISTS(SELECT 1 FROM academic_timetable_block_group_instructors instructor
        WHERE instructor.block_group_id=placed.id)
    AND NOT EXISTS(SELECT 1 FROM academic_timetable_blocks other_block
        JOIN academic_timetable_block_groups other_placement ON other_placement.block_id=other_block.id
        WHERE other_block.timetable_version_id=version.id AND other_block.is_active
            AND other_placement.is_active AND other_placement.learning_group_id=placed.learning_group_id
            AND (other_placement.learning_offering_id<>placed.learning_offering_id
                OR NOT EXISTS(SELECT 1 FROM academic_timetable_block_group_instructors instructor
                    WHERE instructor.block_group_id=other_placement.id)
                OR EXISTS(SELECT 1 FROM academic_timetable_block_group_instructors instructor
                    WHERE instructor.block_group_id=other_placement.id
                        AND (SELECT count(*) FROM learning_group_teachers assignment
                            WHERE assignment.learning_group_id=placed.learning_group_id
                                AND assignment.teacher_id=instructor.instructor_id AND assignment.role=instructor.role
                                AND assignment.starts_on<=version.effective_from
                                AND (assignment.ends_on IS NULL OR assignment.ends_on>=version.effective_from))<>1)));

CREATE TEMP TABLE academic_087_graphs ON COMMIT DROP AS
WITH captured AS (
    SELECT v.id AS timetable_version_id, v.academic_term_id, v.academic_year_id,
        v.effective_from, v.created_by, v.published_by, v.published_at, v.created_at, v.updated_at,
        academic_capture_delivery_snapshot(v.academic_term_id,v.effective_from,
            COALESCE((SELECT jsonb_agg(jsonb_build_object('id',t.learning_offering_id,
                'weekly_period_target',t.weekly_period_target) ORDER BY t.learning_offering_id)
                FROM academic_timetable_version_targets t WHERE t.timetable_version_id=v.id),'[]'::JSONB)) AS snapshot
    FROM academic_timetable_versions v WHERE v.status='published'
)
SELECT captured.timetable_version_id,captured.academic_term_id,captured.academic_year_id,
    captured.effective_from,captured.created_by,captured.published_by,captured.published_at,
    captured.created_at,captured.updated_at,
    jsonb_set(captured.snapshot,'{offerings}',COALESCE((SELECT jsonb_agg(offering ||
        jsonb_build_object('groups',COALESCE((SELECT jsonb_agg(source_group ORDER BY source_group->>'id')
            FROM jsonb_array_elements(offering->'groups') source_group
            JOIN learning_groups learning_group ON learning_group.id=(source_group->>'id')::UUID
            WHERE learning_group.created_at<=GREATEST(captured.published_at,
                    captured.effective_from::TIMESTAMP AT TIME ZONE 'Asia/Bangkok')
                OR EXISTS(SELECT 1 FROM academic_087_dated_group_evidence evidence
                    WHERE evidence.timetable_version_id=captured.timetable_version_id
                        AND evidence.learning_offering_id=(offering->>'id')::UUID
                        AND evidence.learning_group_id=learning_group.id)),'[]'::JSONB)) ORDER BY offering->>'id')
        FROM jsonb_array_elements(captured.snapshot->'offerings') offering),'[]'::JSONB)) AS snapshot
FROM captured;

DO $$
DECLARE bad_count BIGINT;
BEGIN
    SELECT count(*) INTO bad_count FROM academic_087_graphs g
    WHERE jsonb_array_length(g.snapshot->'offerings')<>(SELECT count(*) FROM academic_timetable_version_targets t
        WHERE t.timetable_version_id=g.timetable_version_id);
    IF bad_count<>0 THEN
        RAISE EXCEPTION 'DELIVERY_VERSION_TARGET_UNMAPPABLE count=%',bad_count;
    END IF;
    -- A placed historical group must have evidence in the graph at publication.
    SELECT count(*) INTO bad_count FROM academic_timetable_block_groups placed
    JOIN academic_timetable_blocks b ON b.id=placed.block_id
    JOIN academic_087_graphs g ON g.timetable_version_id=b.timetable_version_id
    WHERE placed.is_active AND b.is_active AND NOT EXISTS (
        SELECT 1 FROM jsonb_array_elements(g.snapshot->'offerings') o,
            jsonb_array_elements(o->'groups') source_group
        WHERE (o->>'id')::UUID=placed.learning_offering_id
            AND (source_group->>'id')::UUID=placed.learning_group_id
    );
    IF bad_count<>0 THEN
        RAISE EXCEPTION 'DELIVERY_VERSION_HISTORICAL_GROUP_UNMAPPABLE count=%',bad_count;
    END IF;
    SELECT count(*) INTO bad_count FROM academic_timetable_block_group_instructors instructor
    JOIN academic_timetable_block_groups placed ON placed.id=instructor.block_group_id
    JOIN academic_timetable_blocks block ON block.id=placed.block_id
    JOIN academic_087_graphs graph ON graph.timetable_version_id=block.timetable_version_id
    WHERE placed.is_active AND block.is_active AND (SELECT count(*)
        FROM jsonb_array_elements(graph.snapshot->'offerings') offering,
            jsonb_array_elements(offering->'groups') source_group,
            jsonb_array_elements(source_group->'teachers') assignment
        WHERE (offering->>'id')::UUID=placed.learning_offering_id
            AND (source_group->>'id')::UUID=placed.learning_group_id
            AND (assignment->>'teacherId')::UUID=instructor.instructor_id
            AND assignment->>'role'=instructor.role)<>1;
    IF bad_count<>0 THEN
        RAISE EXCEPTION 'DELIVERY_VERSION_HISTORICAL_INSTRUCTOR_UNMAPPABLE count=%',bad_count;
    END IF;
END $$;

-- Consecutive identical delivery graphs share one source, even when timetable
-- placements differ. A later A->B->A transition remains a new dated revision.
CREATE TEMP TABLE academic_087_mapping ON COMMIT DROP AS
WITH boundaries AS (
    SELECT g.*, CASE WHEN snapshot IS NOT DISTINCT FROM lag(snapshot) OVER (
        PARTITION BY academic_term_id ORDER BY effective_from,timetable_version_id
    ) THEN 0 ELSE 1 END AS boundary FROM academic_087_graphs g
), islands AS (
    SELECT boundaries.*, sum(boundary) OVER (PARTITION BY academic_term_id
        ORDER BY effective_from,timetable_version_id) AS island FROM boundaries
)
SELECT islands.*, first_value(timetable_version_id) OVER (PARTITION BY academic_term_id,island
    ORDER BY effective_from,timetable_version_id) AS first_timetable_id,
    uuid_generate_v5('f291607b-fef7-56f8-a679-ad9d37e3bc75'::UUID,
        'delivery-version:' || first_value(timetable_version_id) OVER (PARTITION BY academic_term_id,island
            ORDER BY effective_from,timetable_version_id)::TEXT) AS delivery_version_id
FROM islands;

INSERT INTO academic_delivery_versions (id,academic_term_id,academic_year_id,source_version_id,effective_from,status,
    snapshot,created_by,published_by,published_at,publication_idempotency_key,
    publication_request_hash,created_at,updated_at)
SELECT delivery_version_id,academic_term_id,academic_year_id,
    lag(delivery_version_id) OVER (PARTITION BY academic_term_id ORDER BY effective_from,timetable_version_id),
    effective_from,'published',snapshot,
    created_by,published_by,published_at,
    uuid_generate_v5(delivery_version_id,'legacy-publication'),
    encode(sha256(convert_to(snapshot::TEXT,'UTF8')),'hex'),created_at,updated_at
FROM academic_087_mapping WHERE timetable_version_id=first_timetable_id;

-- Scoped trigger suspension is confined to this locked atomic migration.
ALTER TABLE academic_timetable_versions DISABLE TRIGGER academic_timetable_versions_published_immutable;
ALTER TABLE academic_term_change_sets DISABLE TRIGGER academic_term_change_sets_immutable;
UPDATE academic_timetable_versions v SET delivery_version_id=m.delivery_version_id
FROM academic_087_mapping m WHERE m.timetable_version_id=v.id;
UPDATE academic_timetable_versions v SET delivery_version_id=source.delivery_version_id
FROM academic_timetable_versions source, academic_term_change_sets c
WHERE v.status<>'published' AND c.id=v.change_set_id
    AND source.id=COALESCE(v.source_version_id,c.base_timetable_version_id);
UPDATE academic_timetable_versions v SET delivery_version_id=source.delivery_version_id
FROM academic_timetable_versions source
WHERE v.status<>'published' AND v.delivery_version_id IS NULL AND source.id=v.source_version_id;

-- Operational drafts alone become delivery drafts; pure placement drafts retain
-- their timetable and acquire no artificial pending delivery revision.
INSERT INTO academic_delivery_versions (id,academic_term_id,academic_year_id,source_version_id,
    effective_from,status,snapshot,created_by,created_at,updated_at)
SELECT uuid_generate_v5(c.id,'delivery-draft'),c.academic_term_id,c.academic_year_id,
    base.delivery_version_id,c.effective_from,c.status,
    academic_project_delivery_teacher_changes(academic_capture_delivery_snapshot(c.academic_term_id,c.effective_from,
        COALESCE((SELECT jsonb_agg(jsonb_build_object('id',t.learning_offering_id,
            'weekly_period_target',t.weekly_period_target) ORDER BY t.learning_offering_id)
            FROM academic_timetable_version_targets t WHERE t.timetable_version_id=c.target_timetable_version_id),'[]'::JSONB)),c.id),
    c.created_by,c.created_at,c.updated_at
FROM academic_term_change_sets c JOIN academic_timetable_versions base ON base.id=c.base_timetable_version_id
WHERE c.status IN ('draft','cancelled') AND (
    EXISTS(SELECT 1 FROM academic_term_change_items i WHERE i.change_set_id=c.id)
    OR EXISTS(SELECT 1 FROM academic_087_direct_inclusions evidence WHERE evidence.revision_id=c.id));

UPDATE academic_term_change_sets c SET base_delivery_version_id=base.delivery_version_id,
    target_delivery_version_id=CASE WHEN c.status IN ('draft','cancelled') THEN uuid_generate_v5(c.id,'delivery-draft')
        ELSE target.delivery_version_id END
FROM academic_timetable_versions base,academic_timetable_versions target
WHERE base.id=c.base_timetable_version_id AND target.id=c.target_timetable_version_id
    AND (EXISTS(SELECT 1 FROM academic_term_change_items i WHERE i.change_set_id=c.id)
        OR EXISTS(SELECT 1 FROM academic_087_direct_inclusions evidence WHERE evidence.revision_id=c.id));

DO $$
BEGIN
    IF EXISTS(SELECT 1 FROM academic_term_change_sets revision
        JOIN academic_delivery_versions draft ON draft.id=revision.target_delivery_version_id
        WHERE revision.status IN ('draft','cancelled')
            AND jsonb_array_length(draft.snapshot->'offerings')<>(SELECT count(*)
                FROM academic_timetable_version_targets target WHERE target.timetable_version_id=revision.target_timetable_version_id))
    THEN RAISE EXCEPTION 'DELIVERY_DRAFT_TARGETS_UNMAPPABLE'; END IF;
END $$;

ALTER TABLE academic_timetable_versions ENABLE TRIGGER academic_timetable_versions_published_immutable;
ALTER TABLE academic_term_change_sets ENABLE TRIGGER academic_term_change_sets_immutable;

CREATE TABLE academic_delivery_version_migration_audit (
    migration_version INTEGER PRIMARY KEY,
    checked_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    checks JSONB NOT NULL CHECK(jsonb_typeof(checks)='array'),
    passed BOOLEAN NOT NULL,
    cutover_completed BOOLEAN NOT NULL DEFAULT false
);

CREATE TEMP TABLE academic_087_after ON COMMIT DROP AS
SELECT * FROM academic_delivery_legacy_fingerprints();

DO $$
BEGIN
    IF EXISTS(SELECT 1 FROM academic_087_before b FULL JOIN academic_087_after a USING(resource)
        WHERE b.count IS DISTINCT FROM a.count OR b.fingerprint IS DISTINCT FROM a.fingerprint)
        OR EXISTS(SELECT 1 FROM academic_timetable_versions v LEFT JOIN academic_delivery_versions d ON d.id=v.delivery_version_id
            WHERE d.id IS NULL OR d.status<>'published' OR d.academic_term_id<>v.academic_term_id)
        OR EXISTS(SELECT 1 FROM academic_087_mapping m JOIN academic_delivery_versions d ON d.id=m.delivery_version_id
            WHERE m.snapshot IS DISTINCT FROM d.snapshot)
    THEN RAISE EXCEPTION 'DELIVERY_VERSION_RECONCILIATION_FAILED'; END IF;
END $$;
INSERT INTO academic_delivery_version_migration_audit(migration_version,checks,passed)
SELECT 87,jsonb_agg(jsonb_build_object('code',upper(resource)||'_PRESERVED','count',count,
    'fingerprint',fingerprint,'passed',true) ORDER BY resource),true FROM academic_087_after;
