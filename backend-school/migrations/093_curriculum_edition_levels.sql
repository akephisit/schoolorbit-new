-- A school edition is the single owner of revision metadata and publication.
-- Existing version UUIDs become educational-level UUIDs; placements keep program UUIDs.
LOCK TABLE curricula, curriculum_versions, study_programs, curriculum_term_slots,
    curriculum_course_requirements, curriculum_activity_requirements,
    grade_level_progressions, homerooms, student_academic_years,
    learning_offering_targets IN SHARE ROW EXCLUSIVE MODE;

CREATE TEMP TABLE curriculum_093_evidence ON COMMIT DROP AS SELECT
    (SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM curriculum_course_requirements r) AS courses,
    (SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM curriculum_activity_requirements r) AS activities,
    (SELECT jsonb_agg(jsonb_build_array(id,study_program_id) ORDER BY id) FROM homerooms) AS rooms,
    (SELECT jsonb_agg(jsonb_build_array(id,study_program_id) ORDER BY id) FROM student_academic_years) AS students,
    (SELECT jsonb_agg(jsonb_build_array(id,study_program_id) ORDER BY id) FROM learning_offering_targets) AS targets,
    (SELECT jsonb_agg(id ORDER BY id) FROM study_programs) AS programs;

CREATE TEMP TABLE curriculum_093_mapping ON COMMIT DROP AS
SELECT c.id AS old_root_id, v.id AS level_id,
       uuid_generate_v5('f4b23449-155b-4a3b-944c-54c7e081c930'::uuid,
           CASE WHEN v.revision_year IS NULL THEN 'unknown:'||v.id::text
                ELSE jsonb_build_array(v.revision_year,btrim(v.version_name),v.status)::text END) AS edition_id,
       btrim(v.version_name) AS edition_name, v.revision_year, v.status,
       c.code,c.name_th,c.name_en,c.description AS level_description,c.grade_level_ids,
       c.is_active AS level_active,to_jsonb(c) AS original_root,to_jsonb(v) AS original_version
FROM curricula c JOIN curriculum_versions v ON v.curriculum_id=c.id;

DO $$ BEGIN
    IF EXISTS (SELECT 1 FROM curriculum_093_mapping GROUP BY edition_id,lower(btrim(name_th)) HAVING count(*)>1)
       OR EXISTS (SELECT 1 FROM curriculum_093_mapping GROUP BY edition_id,lower(code) HAVING count(*)>1)
       OR EXISTS (SELECT 1 FROM curriculum_093_mapping WHERE jsonb_typeof(grade_level_ids)<>'array')
    THEN RAISE EXCEPTION 'CURRICULUM_093_AMBIGUOUS_EDITION_LEVEL'; END IF;
END $$;

CREATE TABLE curriculum_editions (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    name text NOT NULL CHECK (btrim(name)<>''),
    revision_year integer CHECK (revision_year BETWEEN 2400 AND 2999),
    description text,
    status text NOT NULL DEFAULT 'draft' CHECK(status IN ('draft','published','archived')),
    is_active boolean NOT NULL DEFAULT true,
    published_at timestamptz,
    row_version bigint NOT NULL DEFAULT 1 CHECK(row_version>0),
    migration_provenance jsonb NOT NULL DEFAULT '{}'::jsonb,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX curriculum_editions_revision_status_idx ON curriculum_editions(revision_year DESC,status,id);

INSERT INTO curriculum_editions(id,name,revision_year,description,status,published_at,migration_provenance,created_at,updated_at)
SELECT edition_id,min(edition_name),min(revision_year),
       min(original_version->>'description'),min(status),
       max((original_version->>'published_at')::timestamptz),
       jsonb_build_object('migration',93,'sources',jsonb_agg(original_version ORDER BY level_id)),
       min((original_version->>'created_at')::timestamptz),max((original_version->>'updated_at')::timestamptz)
FROM curriculum_093_mapping GROUP BY edition_id;

-- Retain roots which had no edition without guessing their revision year.
INSERT INTO curriculum_editions(id,name,revision_year,status,migration_provenance,created_at,updated_at)
SELECT uuid_generate_v5(c.id,'unassigned-edition'),'ร่างหลักสูตร — '||c.name_th,NULL,'draft',
       jsonb_build_object('migration',93,'unassignedRevision',true,'originalRoot',to_jsonb(c)),c.created_at,c.updated_at
FROM curricula c WHERE NOT EXISTS (SELECT 1 FROM curriculum_versions v WHERE v.curriculum_id=c.id);

ALTER TABLE curriculum_versions DISABLE TRIGGER curriculum_versions_published_immutable;
ALTER TABLE curriculum_versions ADD COLUMN edition_id uuid REFERENCES curriculum_editions(id) ON DELETE RESTRICT,
    ADD COLUMN code text, ADD COLUMN name_th text, ADD COLUMN name_en text,
    ADD COLUMN grade_level_ids jsonb, ADD COLUMN level_active boolean;
UPDATE curriculum_versions v SET edition_id=m.edition_id,code=m.code,name_th=m.name_th,name_en=m.name_en,
    grade_level_ids=m.grade_level_ids,level_active=COALESCE(m.level_active,true),description=m.level_description,
    migration_provenance=v.migration_provenance||jsonb_build_object('editionHierarchy',jsonb_build_object(
        'migration',93,'originalRoot',m.original_root,'originalVersion',m.original_version))
FROM curriculum_093_mapping m WHERE m.level_id=v.id;

INSERT INTO curriculum_versions(id,curriculum_id,version_name,status,edition_id,code,name_th,name_en,
    grade_level_ids,level_active,description,migration_provenance,created_at,updated_at)
SELECT uuid_generate_v5(c.id,'unassigned-level'),c.id,'ร่างหลักสูตร — '||c.name_th,'draft',
       uuid_generate_v5(c.id,'unassigned-edition'),c.code,c.name_th,c.name_en,c.grade_level_ids,
       COALESCE(c.is_active,true),c.description,jsonb_build_object('editionHierarchy',jsonb_build_object(
           'migration',93,'originalRoot',to_jsonb(c),'unassignedRevision',true)),c.created_at,c.updated_at
FROM curricula c WHERE NOT EXISTS (SELECT 1 FROM curriculum_versions v WHERE v.curriculum_id=c.id);

-- Preserve curriculum-specific progression rules in every corresponding level.
CREATE TEMP TABLE curriculum_093_progressions ON COMMIT DROP AS
SELECT p.*,v.id AS level_id,row_number() OVER(PARTITION BY p.id ORDER BY v.id) AS member
FROM grade_level_progressions p JOIN curriculum_versions v ON v.curriculum_id=p.curriculum_id;
ALTER TABLE grade_level_progressions DROP CONSTRAINT grade_level_progressions_curriculum_id_fkey;
DELETE FROM grade_level_progressions WHERE curriculum_id IS NOT NULL;
INSERT INTO grade_level_progressions(id,from_grade_level_id,to_grade_level_id,transition_kind,curriculum_id,is_active,created_at,updated_at)
SELECT CASE WHEN member=1 THEN id ELSE uuid_generate_v5(id,'level:'||level_id::text) END,
       from_grade_level_id,to_grade_level_id,transition_kind,level_id,is_active,created_at,updated_at
FROM curriculum_093_progressions;
ALTER TABLE grade_level_progressions RENAME COLUMN curriculum_id TO curriculum_level_id;

ALTER TABLE curriculum_versions DROP COLUMN curriculum_id,
    DROP COLUMN version_name,DROP COLUMN revision_year,DROP COLUMN status,DROP COLUMN published_at,
    DROP COLUMN is_active;
ALTER TABLE curriculum_versions RENAME TO curriculum_levels;
ALTER TABLE curriculum_levels RENAME COLUMN level_active TO is_active;
ALTER TABLE curriculum_levels ALTER COLUMN edition_id SET NOT NULL,
    ALTER COLUMN code SET NOT NULL,ALTER COLUMN name_th SET NOT NULL,
    ALTER COLUMN grade_level_ids SET NOT NULL,ALTER COLUMN is_active SET NOT NULL;
ALTER TABLE curriculum_levels ADD CONSTRAINT curriculum_levels_edition_code_key UNIQUE(edition_id,code),
    ADD CONSTRAINT curriculum_levels_nonempty_name CHECK(btrim(name_th)<>''),
    ADD CONSTRAINT curriculum_levels_grade_array CHECK(jsonb_typeof(grade_level_ids)='array');
ALTER TABLE grade_level_progressions ADD CONSTRAINT grade_level_progressions_level_fkey
    FOREIGN KEY(curriculum_level_id) REFERENCES curriculum_levels(id) ON DELETE RESTRICT;

ALTER TABLE study_programs RENAME COLUMN curriculum_version_id TO curriculum_level_id;
ALTER TABLE curriculum_term_slots RENAME COLUMN curriculum_version_id TO curriculum_level_id;
ALTER TABLE curriculum_course_requirements RENAME COLUMN curriculum_version_id TO curriculum_level_id;
ALTER TABLE curriculum_activity_requirements RENAME COLUMN curriculum_version_id TO curriculum_level_id;
ALTER TABLE study_programs ADD COLUMN IF NOT EXISTS migration_provenance jsonb NOT NULL DEFAULT '{}'::jsonb;
ALTER TABLE study_programs DISABLE TRIGGER study_programs_published_curriculum_immutable;
UPDATE study_programs SET migration_provenance=migration_provenance||jsonb_build_object(
    'editionHierarchy',jsonb_build_object('migration',93,'originalOwningOrganizationUnitId',owning_organization_unit_id));
ALTER TABLE study_programs DROP COLUMN owning_organization_unit_id;
DROP TABLE curricula;

DROP TRIGGER curriculum_versions_published_immutable ON curriculum_levels;
CREATE TRIGGER curriculum_editions_published_immutable BEFORE UPDATE OR DELETE ON curriculum_editions
FOR EACH ROW EXECUTE FUNCTION academic_prevent_published_version_mutation();

CREATE OR REPLACE FUNCTION academic_prevent_published_curriculum_level_mutation() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE owner_status text;
BEGIN
    IF TG_OP<>'INSERT' THEN
        SELECT status INTO owner_status FROM curriculum_editions WHERE id=OLD.edition_id FOR SHARE;
        IF owner_status='published' THEN RAISE EXCEPTION 'ACADEMIC_CORE_PUBLISHED_CURRICULUM_IMMUTABLE:%',OLD.edition_id; END IF;
    END IF;
    IF TG_OP<>'DELETE' THEN
        SELECT status INTO owner_status FROM curriculum_editions WHERE id=NEW.edition_id FOR SHARE;
        IF owner_status='published' THEN RAISE EXCEPTION 'ACADEMIC_CORE_PUBLISHED_CURRICULUM_IMMUTABLE:%',NEW.edition_id; END IF;
    END IF;
    RETURN CASE WHEN TG_OP='DELETE' THEN OLD ELSE NEW END;
END $$;
CREATE TRIGGER curriculum_levels_published_immutable BEFORE INSERT OR UPDATE OR DELETE ON curriculum_levels
FOR EACH ROW EXECUTE FUNCTION academic_prevent_published_curriculum_level_mutation();

CREATE OR REPLACE FUNCTION academic_prevent_published_curriculum_child_mutation() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE owner_status text;
BEGIN
    IF TG_OP<>'INSERT' THEN
        SELECT e.status INTO owner_status FROM curriculum_levels l JOIN curriculum_editions e ON e.id=l.edition_id
        WHERE l.id=OLD.curriculum_level_id FOR SHARE OF e;
        IF owner_status='published' THEN RAISE EXCEPTION 'ACADEMIC_CORE_PUBLISHED_CURRICULUM_IMMUTABLE:%',OLD.curriculum_level_id; END IF;
    END IF;
    IF TG_OP<>'DELETE' THEN
        SELECT e.status INTO owner_status FROM curriculum_levels l JOIN curriculum_editions e ON e.id=l.edition_id
        WHERE l.id=NEW.curriculum_level_id FOR SHARE OF e;
        IF owner_status='published' THEN RAISE EXCEPTION 'ACADEMIC_CORE_PUBLISHED_CURRICULUM_IMMUTABLE:%',NEW.curriculum_level_id; END IF;
    END IF;
    RETURN CASE WHEN TG_OP='DELETE' THEN OLD ELSE NEW END;
END $$;
CREATE OR REPLACE FUNCTION check_admission_track_program_context() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM admission_rounds r JOIN study_programs p ON p.id=NEW.study_program_id
        JOIN curriculum_levels l ON l.id=p.curriculum_level_id JOIN curriculum_editions e ON e.id=l.edition_id
        WHERE r.id=NEW.admission_round_id AND r.academic_year_id=NEW.academic_year_id
          AND p.status='published' AND e.status='published' AND e.is_active AND l.is_active)
    THEN RAISE EXCEPTION 'ACADEMIC_ADMISSION_TRACK_PROGRAM_CONTEXT_MISMATCH' USING ERRCODE='check_violation'; END IF;
    RETURN NEW;
END $$;

-- Child triggers are restored before accepting any post-cutover write.
ALTER TABLE study_programs ENABLE TRIGGER study_programs_published_curriculum_immutable;
ALTER TABLE curriculum_term_slots ENABLE TRIGGER curriculum_term_slots_published_immutable;
ALTER TABLE curriculum_course_requirements ENABLE TRIGGER curriculum_course_requirements_published_immutable;
ALTER TABLE curriculum_activity_requirements ENABLE TRIGGER curriculum_activity_requirements_published_immutable;

DO $$ BEGIN
    IF EXISTS(SELECT 1 FROM curriculum_093_evidence e WHERE
        e.courses IS DISTINCT FROM (SELECT jsonb_agg((to_jsonb(r)-'curriculum_level_id')||jsonb_build_object('curriculum_version_id',r.curriculum_level_id) ORDER BY id) FROM curriculum_course_requirements r)
        OR e.activities IS DISTINCT FROM (SELECT jsonb_agg((to_jsonb(r)-'curriculum_level_id')||jsonb_build_object('curriculum_version_id',r.curriculum_level_id) ORDER BY id) FROM curriculum_activity_requirements r)
        OR e.rooms IS DISTINCT FROM (SELECT jsonb_agg(jsonb_build_array(id,study_program_id) ORDER BY id) FROM homerooms)
        OR e.students IS DISTINCT FROM (SELECT jsonb_agg(jsonb_build_array(id,study_program_id) ORDER BY id) FROM student_academic_years)
        OR e.targets IS DISTINCT FROM (SELECT jsonb_agg(jsonb_build_array(id,study_program_id) ORDER BY id) FROM learning_offering_targets)
        OR e.programs IS DISTINCT FROM (SELECT jsonb_agg(id ORDER BY id) FROM study_programs))
    THEN RAISE EXCEPTION 'CURRICULUM_093_PRESERVATION_FAILED'; END IF;
END $$;

INSERT INTO academic_audit_events(event_code,entity_type,entity_id,payload)
SELECT 'curriculum.edition_hierarchy.reconciled','curriculum_edition',e.id,
       jsonb_build_object('migration',93,'passed',true,'levels',count(l.id))
FROM curriculum_editions e LEFT JOIN curriculum_levels l ON l.edition_id=e.id GROUP BY e.id;
