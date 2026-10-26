-- Empty all-grades intents become concrete save-time snapshots. Published history
-- and every pinned consumer remain immutable; corrections append at each date boundary.
LOCK TABLE grade_levels, subjects, activities, subject_versions, activity_versions,
    subject_version_grade_levels IN ACCESS EXCLUSIVE MODE;

DO $$
BEGIN
    IF (SELECT count(*) FROM pg_trigger WHERE
        (tgrelid='subject_versions'::regclass AND tgname IN ('subject_versions_published_immutable','update_subjects_updated_at')
         OR tgrelid='activity_versions'::regclass AND tgname='activity_versions_published_immutable')
        AND tgenabled='O') <> 3 THEN
        RAISE EXCEPTION 'CATALOG_141_GUARD_STATE_INVALID';
    END IF;
    IF EXISTS (
        SELECT 1 FROM activity_versions v JOIN activities a ON a.id=v.activity_id
        WHERE a.archived_at IS NULL AND (v.status='published' OR v.status='draft' AND v.published_at IS NULL)
          AND v.grade_level_ids IS NOT NULL AND jsonb_typeof(v.grade_level_ids) IS DISTINCT FROM 'array'
    ) THEN
        RAISE EXCEPTION 'CATALOG_141_ACTIVITY_SCOPE_UNMAPPABLE';
    END IF;
    IF EXISTS (
        SELECT 1 FROM activity_versions v JOIN activities a ON a.id=v.activity_id
        CROSS JOIN LATERAL jsonb_array_elements(CASE WHEN jsonb_typeof(v.grade_level_ids)='array' THEN v.grade_level_ids ELSE '[]'::jsonb END) element
        WHERE a.archived_at IS NULL AND (v.status='published' OR v.status='draft' AND v.published_at IS NULL)
          AND (jsonb_typeof(element) IS DISTINCT FROM 'string' OR NOT pg_input_is_valid(element #>> '{}','uuid'))
    ) THEN
        RAISE EXCEPTION 'CATALOG_141_ACTIVITY_SCOPE_UNMAPPABLE';
    END IF;
END $$;

CREATE TEMP TABLE catalog_141_grades ON COMMIT DROP AS
SELECT id FROM grade_levels WHERE is_active IS TRUE ORDER BY id;
CREATE TEMP TABLE catalog_141_versions ON COMMIT DROP AS
SELECT 'subject_versions'::text AS table_name,id,to_jsonb(v) AS facts FROM subject_versions v
UNION ALL SELECT 'activity_versions',id,to_jsonb(v) FROM activity_versions v;
CREATE TEMP TABLE catalog_141_links ON COMMIT DROP AS SELECT * FROM subject_version_grade_levels;
CREATE TEMP TABLE catalog_141_references (
    table_name text PRIMARY KEY,row_count bigint NOT NULL,content_checksum text NOT NULL
) ON COMMIT DROP;

-- Include immutable publication snapshots and actual FK/column owners. The grade
-- links are reconciled separately because only new correction/draft links are added.
DO $$
DECLARE owner record; evidence record;
BEGIN
    FOR owner IN
        SELECT DISTINCT c.oid,c.relname FROM pg_class c
        JOIN pg_namespace n ON n.oid=c.relnamespace
        WHERE n.nspname=current_schema() AND c.relkind IN ('r','p')
          AND c.relname NOT IN ('subject_versions','activity_versions','subject_version_grade_levels')
          AND (c.relname IN ('subjects','activities','subject_default_instructors','activity_default_instructors',
              'academic_delivery_versions','learning_delivery_apply_runs','academic_term_change_sets','academic_timetable_versions','learning_offering_curriculum_sources') OR EXISTS (
              SELECT 1 FROM pg_attribute a WHERE a.attrelid=c.oid AND a.attnum>0 AND NOT a.attisdropped
                AND a.attname IN ('subject_version_id','activity_version_id','catalog_version_id')
          ) OR EXISTS (
              SELECT 1 FROM pg_constraint fk WHERE fk.contype='f' AND fk.conrelid=c.oid
                AND fk.confrelid IN ('subject_versions'::regclass,'activity_versions'::regclass)
          )) ORDER BY c.relname
    LOOP
        EXECUTE format('LOCK TABLE %I IN ACCESS EXCLUSIVE MODE',owner.relname);
        EXECUTE format('SELECT count(*) AS row_count,md5(COALESCE(string_agg(md5(to_jsonb(r)::text),'''' ORDER BY to_jsonb(r)::text),'''')) AS content_checksum FROM %I r',owner.relname) INTO evidence;
        INSERT INTO catalog_141_references VALUES(owner.relname,evidence.row_count,evidence.content_checksum);
    END LOOP;
END $$;

CREATE TEMP TABLE catalog_141_boundaries ON COMMIT DROP AS
SELECT DISTINCT ON (table_name,catalog_id,effective_from) * FROM (
    SELECT 'subject_versions'::text AS table_name,v.subject_id AS catalog_id,v.id,v.version_no,v.effective_from,
           NOT EXISTS(SELECT 1 FROM subject_version_grade_levels g WHERE g.subject_id=v.id) AS empty_scope
    FROM subject_versions v JOIN subjects s ON s.id=v.subject_id
    WHERE v.status='published' AND s.archived_at IS NULL
    UNION ALL
    SELECT 'activity_versions',v.activity_id,v.id,v.version_no,v.effective_from,
           jsonb_array_length(COALESCE(v.grade_level_ids,'[]'::jsonb))=0
    FROM activity_versions v JOIN activities a ON a.id=v.activity_id
    WHERE v.status='published' AND a.archived_at IS NULL
) versions ORDER BY table_name,catalog_id,effective_from,version_no DESC;

CREATE TEMP TABLE catalog_141_drafts ON COMMIT DROP AS
SELECT 'subject_versions'::text AS table_name,v.id FROM subject_versions v JOIN subjects s ON s.id=v.subject_id
WHERE v.status='draft' AND v.published_at IS NULL AND s.archived_at IS NULL
  AND NOT EXISTS(SELECT 1 FROM subject_version_grade_levels g WHERE g.subject_id=v.id)
UNION ALL
SELECT 'activity_versions',v.id FROM activity_versions v JOIN activities a ON a.id=v.activity_id
WHERE v.status='draft' AND v.published_at IS NULL AND a.archived_at IS NULL
  AND jsonb_array_length(COALESCE(v.grade_level_ids,'[]'::jsonb))=0;

CREATE TEMP TABLE catalog_141_corrections ON COMMIT DROP AS
SELECT boundary.*,uuid_generate_v5(boundary.id,'catalog-grade-snapshot-141') AS target_id,
       (SELECT max((facts->>'version_no')::bigint) FROM catalog_141_versions old
        WHERE old.table_name=boundary.table_name AND COALESCE(old.facts->>'subject_id',old.facts->>'activity_id')=boundary.catalog_id::text)
       + row_number() OVER(PARTITION BY boundary.table_name,boundary.catalog_id ORDER BY boundary.effective_from,boundary.version_no,boundary.id) AS target_version_no
FROM catalog_141_boundaries boundary WHERE empty_scope;

DO $$
BEGIN
    IF (EXISTS(SELECT 1 FROM catalog_141_drafts) OR EXISTS(SELECT 1 FROM catalog_141_corrections))
       AND NOT EXISTS(SELECT 1 FROM catalog_141_grades) THEN
        RAISE EXCEPTION 'CATALOG_141_NO_ACTIVE_GRADES';
    END IF;
    IF EXISTS(SELECT 1 FROM catalog_141_versions old WHERE
        (EXISTS(SELECT 1 FROM catalog_141_drafts d WHERE d.table_name=old.table_name AND d.id=old.id)
         OR EXISTS(SELECT 1 FROM catalog_141_corrections c WHERE c.table_name=old.table_name AND c.id=old.id))
        AND (jsonb_typeof(facts->'migration_provenance') IS DISTINCT FROM 'object'
             OR (facts->'migration_provenance') ? 'catalogGradeSnapshot141'))
       OR EXISTS(SELECT 1 FROM academic_audit_events WHERE event_code='catalog.grade_snapshots_reconciled') THEN
        RAISE EXCEPTION 'CATALOG_141_PROVENANCE_UNMAPPABLE';
    END IF;
    IF EXISTS(SELECT 1 FROM catalog_141_corrections WHERE target_version_no>2147483647)
       OR EXISTS(SELECT 1 FROM catalog_141_drafts d JOIN catalog_141_versions v USING(table_name,id)
                 WHERE (v.facts->>'row_version')::bigint=9223372036854775807) THEN
        RAISE EXCEPTION 'CATALOG_141_REVISION_OVERFLOW';
    END IF;
    IF EXISTS(SELECT 1 FROM catalog_141_corrections c JOIN catalog_141_versions v ON v.id=c.target_id)
       OR EXISTS(SELECT target_id FROM catalog_141_corrections GROUP BY target_id HAVING count(*)>1) THEN
        RAISE EXCEPTION 'CATALOG_141_TARGET_ID_COLLISION';
    END IF;
END $$;

CREATE TEMP TABLE catalog_141_expected ON COMMIT DROP AS SELECT * FROM catalog_141_versions;
UPDATE catalog_141_expected expected SET facts=expected.facts || jsonb_build_object(
    'row_version',(facts->>'row_version')::bigint+1,'updated_at',now(),
    'migration_provenance',(facts->'migration_provenance') || jsonb_build_object('catalogGradeSnapshot141',jsonb_build_object(
        'action','draft_snapshot','sourceVersionId',expected.id,'gradeLevelIds',(SELECT jsonb_agg(id ORDER BY id) FROM catalog_141_grades),'capturedAt',now())))
    || CASE WHEN expected.table_name='activity_versions' THEN jsonb_build_object('grade_level_ids',(SELECT jsonb_agg(id ORDER BY id) FROM catalog_141_grades)) ELSE '{}'::jsonb END
WHERE EXISTS(SELECT 1 FROM catalog_141_drafts d WHERE d.table_name=expected.table_name AND d.id=expected.id);
INSERT INTO catalog_141_expected
SELECT c.table_name,c.target_id,v.facts || jsonb_build_object(
    'id',c.target_id,'version_no',c.target_version_no,'row_version',2,'created_at',now(),'updated_at',now(),'published_at',now(),
    'migration_provenance',(v.facts->'migration_provenance') || jsonb_build_object('catalogGradeSnapshot141',jsonb_build_object(
        'action','published_correction','sourceVersionId',c.id,'gradeLevelIds',(SELECT jsonb_agg(id ORDER BY id) FROM catalog_141_grades),'capturedAt',now())))
    || CASE WHEN c.table_name='activity_versions' THEN jsonb_build_object('grade_level_ids',(SELECT jsonb_agg(id ORDER BY id) FROM catalog_141_grades)) ELSE '{}'::jsonb END
FROM catalog_141_corrections c JOIN catalog_141_versions v ON v.table_name=c.table_name AND v.id=c.id;

UPDATE subject_versions v SET row_version=v.row_version+1,updated_at=now(),migration_provenance=e.facts->'migration_provenance'
FROM catalog_141_expected e JOIN catalog_141_drafts d USING(table_name,id)
WHERE e.table_name='subject_versions' AND v.id=e.id;
UPDATE activity_versions v SET grade_level_ids=e.facts->'grade_level_ids',row_version=v.row_version+1,updated_at=now(),migration_provenance=e.facts->'migration_provenance'
FROM catalog_141_expected e JOIN catalog_141_drafts d USING(table_name,id)
WHERE e.table_name='activity_versions' AND v.id=e.id;

-- Append as drafts, attach grades, then publish with the existing guards enabled.
INSERT INTO subject_versions
SELECT (jsonb_populate_record(NULL::subject_versions,e.facts || jsonb_build_object('status','draft','published_at',NULL,'row_version',1))).*
FROM catalog_141_expected e JOIN catalog_141_corrections c ON c.table_name=e.table_name AND c.target_id=e.id
WHERE e.table_name='subject_versions';
INSERT INTO activity_versions
SELECT (jsonb_populate_record(NULL::activity_versions,e.facts || jsonb_build_object('status','draft','published_at',NULL,'row_version',1))).*
FROM catalog_141_expected e JOIN catalog_141_corrections c ON c.table_name=e.table_name AND c.target_id=e.id
WHERE e.table_name='activity_versions';
INSERT INTO subject_version_grade_levels(subject_id,grade_level_id,created_at)
SELECT target.id,grade.id,now() FROM (
    SELECT id FROM catalog_141_drafts WHERE table_name='subject_versions'
    UNION ALL SELECT target_id FROM catalog_141_corrections WHERE table_name='subject_versions'
) target CROSS JOIN catalog_141_grades grade;
UPDATE subject_versions v SET status='published',published_at=now(),row_version=2,updated_at=now()
WHERE EXISTS(SELECT 1 FROM catalog_141_corrections c WHERE c.table_name='subject_versions' AND c.target_id=v.id);
UPDATE activity_versions v SET status='published',published_at=now(),row_version=2,updated_at=now()
WHERE EXISTS(SELECT 1 FROM catalog_141_corrections c WHERE c.table_name='activity_versions' AND c.target_id=v.id);

DO $$
DECLARE previous record; evidence record;
BEGIN
    IF EXISTS(
        (SELECT table_name,id,facts FROM catalog_141_expected EXCEPT ALL
         (SELECT 'subject_versions',id,to_jsonb(v) FROM subject_versions v UNION ALL SELECT 'activity_versions',id,to_jsonb(v) FROM activity_versions v))
        UNION ALL
        ((SELECT 'subject_versions',id,to_jsonb(v) FROM subject_versions v UNION ALL SELECT 'activity_versions',id,to_jsonb(v) FROM activity_versions v)
         EXCEPT ALL SELECT table_name,id,facts FROM catalog_141_expected)
    ) THEN
        RAISE EXCEPTION 'CATALOG_141_VERSION_PRESERVATION_FAILED';
    END IF;
    IF EXISTS(
        WITH expected AS (
            SELECT * FROM catalog_141_links UNION ALL
            SELECT target.id,grade.id,now() FROM (
                SELECT id FROM catalog_141_drafts WHERE table_name='subject_versions'
                UNION ALL SELECT target_id FROM catalog_141_corrections WHERE table_name='subject_versions'
            ) target CROSS JOIN catalog_141_grades grade
        )
        (SELECT * FROM expected EXCEPT ALL SELECT * FROM subject_version_grade_levels)
        UNION ALL (SELECT * FROM subject_version_grade_levels EXCEPT ALL SELECT * FROM expected)
    ) THEN
        RAISE EXCEPTION 'CATALOG_141_LINK_PRESERVATION_FAILED';
    END IF;
    IF EXISTS(
        SELECT 1 FROM catalog_141_boundaries old LEFT JOIN catalog_141_corrections c ON c.table_name=old.table_name AND c.id=old.id
        LEFT JOIN LATERAL (
            SELECT id FROM (
                SELECT v.id,v.version_no FROM subject_versions v WHERE old.table_name='subject_versions' AND v.subject_id=old.catalog_id AND v.effective_from=old.effective_from AND v.status='published'
                UNION ALL SELECT v.id,v.version_no FROM activity_versions v WHERE old.table_name='activity_versions' AND v.activity_id=old.catalog_id AND v.effective_from=old.effective_from AND v.status='published'
            ) candidates ORDER BY version_no DESC LIMIT 1
        ) current ON true
        WHERE current.id IS DISTINCT FROM COALESCE(c.target_id,old.id)
    ) THEN
        RAISE EXCEPTION 'CATALOG_141_SELECTION_RECONCILIATION_FAILED';
    END IF;
    FOR previous IN SELECT * FROM catalog_141_references LOOP
        EXECUTE format('SELECT count(*) AS row_count,md5(COALESCE(string_agg(md5(to_jsonb(r)::text),'''' ORDER BY to_jsonb(r)::text),'''')) AS content_checksum FROM %I r',previous.table_name) INTO evidence;
        IF previous.row_count IS DISTINCT FROM evidence.row_count OR previous.content_checksum IS DISTINCT FROM evidence.content_checksum THEN
            RAISE EXCEPTION 'CATALOG_141_REFERENCE_PRESERVATION_FAILED';
        END IF;
    END LOOP;
    IF (SELECT count(*) FROM pg_trigger WHERE
        (tgrelid='subject_versions'::regclass AND tgname IN ('subject_versions_published_immutable','update_subjects_updated_at')
         OR tgrelid='activity_versions'::regclass AND tgname='activity_versions_published_immutable')
        AND tgenabled='O') <> 3 THEN
        RAISE EXCEPTION 'CATALOG_141_GUARD_STATE_INVALID';
    END IF;
END $$;

INSERT INTO academic_audit_events(event_code,entity_type,entity_id,payload)
SELECT 'catalog.grade_snapshots_reconciled','catalog',NULL,jsonb_build_object(
    'migration',141,'passed',true,'activeGradeCount',(SELECT count(*) FROM catalog_141_grades),
    'draftsRepaired',(SELECT count(*) FROM catalog_141_drafts),'publishedCorrections',(SELECT count(*) FROM catalog_141_corrections),
    'archivedVersionsPreserved',(SELECT count(*) FROM catalog_141_versions WHERE facts->>'status'='archived'),
    'archivedCatalogsPreserved',(SELECT count(*) FROM subjects WHERE archived_at IS NOT NULL)+(SELECT count(*) FROM activities WHERE archived_at IS NOT NULL),
    'sourceVersions',(SELECT count(*) FROM catalog_141_versions),
    'sourceChecksum',(SELECT md5(COALESCE(string_agg(md5(facts::text),'' ORDER BY table_name,id),'')) FROM catalog_141_versions),
    'mappingChecksum',(SELECT md5(COALESCE(string_agg(md5(table_name||id::text||target_id::text||target_version_no::text),'' ORDER BY table_name,id),'')) FROM catalog_141_corrections),
    'references',(SELECT jsonb_object_agg(table_name,jsonb_build_object('count',row_count,'contentChecksum',content_checksum)) FROM catalog_141_references));
