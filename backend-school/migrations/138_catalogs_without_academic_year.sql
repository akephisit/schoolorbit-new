-- Central catalogs use civil effective dates, independently of academic years.
-- The canonical runner executes this matched-release contraction atomically.
LOCK TABLE subject_versions, activity_versions IN ACCESS EXCLUSIVE MODE;

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM subject_versions WHERE jsonb_typeof(migration_provenance) IS DISTINCT FROM 'object' OR migration_provenance ? 'historicalYearBinding')
       OR EXISTS (SELECT 1 FROM activity_versions WHERE jsonb_typeof(migration_provenance) IS DISTINCT FROM 'object' OR migration_provenance ? 'historicalYearBinding') THEN
        RAISE EXCEPTION 'CATALOG_138_PROVENANCE_UNMAPPABLE';
    END IF;
    IF (SELECT count(*) FROM pg_trigger WHERE
          (tgrelid='subject_versions'::regclass AND tgname IN ('subject_versions_published_immutable','update_subjects_updated_at')
           OR tgrelid='activity_versions'::regclass AND tgname IN ('activity_versions_published_immutable','activity_versions_no_overlap'))
          AND tgenabled='O') <> 4 THEN
        RAISE EXCEPTION 'CATALOG_138_GUARD_STATE_INVALID';
    END IF;
END $$;

CREATE TEMP TABLE catalog_138_versions ON COMMIT DROP AS
SELECT 'subject_versions'::text AS table_name,id,to_jsonb(v) AS facts FROM subject_versions v
UNION ALL SELECT 'activity_versions',id,to_jsonb(v) FROM activity_versions v;
CREATE TEMP TABLE catalog_138_reconciliation (
    table_name text PRIMARY KEY,row_count bigint NOT NULL,content_checksum text NOT NULL
) ON COMMIT DROP;

-- Discover actual reference owners, including immutable publication copies whose
-- catalog references are protected by domain guards rather than direct FKs.
DO $$
DECLARE owner record; evidence record;
BEGIN
    FOR owner IN
        SELECT DISTINCT c.oid,c.relname FROM pg_class c
        JOIN pg_namespace n ON n.oid=c.relnamespace
        WHERE n.nspname=current_schema() AND c.relkind IN ('r','p')
          AND c.relname NOT IN ('subject_versions','activity_versions')
          AND (c.relname IN ('subjects','activities','subject_default_instructors','activity_default_instructors',
              'academic_delivery_versions','learning_delivery_apply_runs','academic_term_change_sets','academic_timetable_versions','offering_curriculum_sources') OR EXISTS (
              SELECT 1 FROM pg_attribute a WHERE a.attrelid=c.oid AND a.attnum>0 AND NOT a.attisdropped
                AND a.attname IN ('subject_version_id','activity_version_id','catalog_version_id')
          ) OR EXISTS (
              SELECT 1 FROM pg_constraint fk WHERE fk.contype='f' AND fk.conrelid=c.oid
                AND fk.confrelid IN ('subject_versions'::regclass,'activity_versions'::regclass)
          )) ORDER BY c.relname
    LOOP
        EXECUTE format('LOCK TABLE %I IN ACCESS EXCLUSIVE MODE',owner.relname);
        EXECUTE format('SELECT count(*) AS row_count,md5(COALESCE(string_agg(md5(to_jsonb(r)::text),'''' ORDER BY to_jsonb(r)::text),'''')) AS content_checksum FROM %I r',owner.relname) INTO evidence;
        INSERT INTO catalog_138_reconciliation VALUES(owner.relname,evidence.row_count,evidence.content_checksum);
    END LOOP;
END $$;

-- Only historical metadata changes. Disable the exact published guards and the
-- inherited subject timestamp trigger; never change the shared guard function.
ALTER TABLE subject_versions DISABLE TRIGGER subject_versions_published_immutable;
ALTER TABLE subject_versions DISABLE TRIGGER update_subjects_updated_at;
ALTER TABLE activity_versions DISABLE TRIGGER activity_versions_published_immutable;
UPDATE subject_versions SET migration_provenance=migration_provenance || jsonb_build_object(
    'historicalYearBinding',jsonb_build_object('migration',138,'startAcademicYearId',start_academic_year_id));
UPDATE activity_versions SET migration_provenance=migration_provenance || jsonb_build_object(
    'historicalYearBinding',jsonb_build_object('migration',138,'startAcademicYearId',start_academic_year_id));
ALTER TABLE subject_versions ENABLE TRIGGER subject_versions_published_immutable;
ALTER TABLE subject_versions ENABLE TRIGGER update_subjects_updated_at;
ALTER TABLE activity_versions ENABLE TRIGGER activity_versions_published_immutable;

-- RESTRICT refuses unknown dependencies instead of silently removing consumers.
ALTER TABLE subject_versions DROP CONSTRAINT subjects_start_academic_year_id_fkey;
ALTER TABLE activity_versions DROP CONSTRAINT activity_catalog_start_academic_year_id_fkey;
ALTER TABLE activity_versions DROP CONSTRAINT unique_catalog_name_per_year;
DROP INDEX idx_subjects_start_year RESTRICT;
DROP INDEX idx_activity_catalog_year RESTRICT;
ALTER TABLE subject_versions DROP COLUMN start_academic_year_id RESTRICT;
ALTER TABLE activity_versions DROP COLUMN start_academic_year_id RESTRICT;

DO $$
DECLARE previous record; evidence record;
BEGIN
    IF (SELECT count(*) FROM catalog_138_versions WHERE table_name='subject_versions') <> (SELECT count(*) FROM subject_versions)
       OR (SELECT count(*) FROM catalog_138_versions WHERE table_name='activity_versions') <> (SELECT count(*) FROM activity_versions) THEN
        RAISE EXCEPTION 'CATALOG_138_PRESERVATION_FAILED';
    END IF;
    IF EXISTS (
        SELECT table_name,id,(facts-'start_academic_year_id') || jsonb_build_object(
            'migration_provenance',(facts->'migration_provenance') || jsonb_build_object(
                'historicalYearBinding',jsonb_build_object('migration',138,'startAcademicYearId',facts->'start_academic_year_id')))
        FROM catalog_138_versions
        EXCEPT ALL
        (SELECT 'subject_versions',id,to_jsonb(v) FROM subject_versions v
         UNION ALL SELECT 'activity_versions',id,to_jsonb(v) FROM activity_versions v)
    ) THEN
        RAISE EXCEPTION 'CATALOG_138_PRESERVATION_FAILED';
    END IF;
    FOR previous IN SELECT * FROM catalog_138_reconciliation LOOP
        EXECUTE format('SELECT count(*) AS row_count,md5(COALESCE(string_agg(md5(to_jsonb(r)::text),'''' ORDER BY to_jsonb(r)::text),'''')) AS content_checksum FROM %I r',previous.table_name) INTO evidence;
        IF previous.row_count IS DISTINCT FROM evidence.row_count OR previous.content_checksum IS DISTINCT FROM evidence.content_checksum THEN
            RAISE EXCEPTION 'CATALOG_138_REFERENCE_PRESERVATION_FAILED';
        END IF;
    END LOOP;
    IF (SELECT count(*) FROM pg_trigger WHERE
          (tgrelid='subject_versions'::regclass AND tgname IN ('subject_versions_published_immutable','update_subjects_updated_at')
           OR tgrelid='activity_versions'::regclass AND tgname IN ('activity_versions_published_immutable','activity_versions_no_overlap'))
          AND tgenabled='O') <> 4 THEN
        RAISE EXCEPTION 'CATALOG_138_GUARD_RESTORE_FAILED';
    END IF;
END $$;

INSERT INTO academic_audit_events(event_code,entity_type,entity_id,payload)
SELECT 'catalog.year_binding_removed.reconciled','catalog',NULL,
    jsonb_build_object('migration',138,'passed',true,'versions',
        (SELECT jsonb_object_agg(table_name,jsonb_build_object('count',row_count,'contentChecksum',content_checksum)) FROM (
            SELECT table_name,count(*) AS row_count,md5(COALESCE(string_agg(md5(facts::text),'' ORDER BY id),'')) AS content_checksum
            FROM catalog_138_versions GROUP BY table_name
        ) version_evidence),'references',
        (SELECT jsonb_object_agg(table_name,jsonb_build_object('count',row_count,'contentChecksum',content_checksum)) FROM catalog_138_reconciliation));
