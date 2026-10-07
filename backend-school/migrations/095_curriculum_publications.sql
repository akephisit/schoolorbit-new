-- One mutable amendment workspace; released whole-edition graphs are immutable.
LOCK TABLE curriculum_editions,curriculum_levels,study_programs,curriculum_term_slots,
 curriculum_course_requirements,curriculum_activity_requirements,course_offering_details,
 activity_offering_details IN SHARE ROW EXCLUSIVE MODE;
CREATE TEMP TABLE curriculum_095_evidence ON COMMIT DROP AS SELECT
 (SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM curriculum_course_requirements r) courses,
 (SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM curriculum_activity_requirements r) activities,
 (SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM homerooms r) rooms,
 (SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM student_academic_years r) students,
 (SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM learning_offering_targets r) targets,
 (SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM academic_delivery_versions r) deliveries;
DROP TRIGGER curriculum_editions_published_immutable ON curriculum_editions;
ALTER TABLE curriculum_editions ADD COLUMN current_publication_id uuid,
 ADD COLUMN publication_count integer NOT NULL DEFAULT 0 CHECK(publication_count>=0),
 ADD COLUMN draft_id uuid DEFAULT gen_random_uuid(),
 ADD CONSTRAINT curriculum_editions_draft_id_key UNIQUE(draft_id);
UPDATE curriculum_editions SET draft_id=NULL WHERE status<>'draft';
CREATE TABLE curriculum_publications (
 id uuid PRIMARY KEY DEFAULT gen_random_uuid(), edition_id uuid NOT NULL REFERENCES curriculum_editions(id),
 publication_no integer NOT NULL CHECK(publication_no>0),
 previous_publication_id uuid, name text NOT NULL,revision_year integer,description text,
 published_by uuid REFERENCES users(id),published_at timestamptz,
 captured_at timestamptz NOT NULL DEFAULT now(),change_note text NOT NULL CHECK(btrim(change_note)<>''),
 is_baseline boolean NOT NULL DEFAULT false,creation_xid xid8 NOT NULL DEFAULT pg_current_xact_id(),
 level_count bigint NOT NULL,program_count bigint NOT NULL,course_count bigint NOT NULL,activity_count bigint NOT NULL,slot_count bigint NOT NULL,
 UNIQUE(edition_id,publication_no),UNIQUE(id,edition_id),
 FOREIGN KEY(previous_publication_id,edition_id) REFERENCES curriculum_publications(id,edition_id)
);
ALTER TABLE curriculum_editions ADD CONSTRAINT curriculum_editions_publication_fkey
 FOREIGN KEY(current_publication_id,id) REFERENCES curriculum_publications(id,edition_id);

-- Durable source identities survive removal from a draft; existing actual references stay valid.
CREATE TABLE curriculum_requirement_sources (
 id uuid PRIMARY KEY,resource_kind text NOT NULL CHECK(resource_kind IN ('course','activity')),
 curriculum_level_id uuid NOT NULL REFERENCES curriculum_levels(id),
 study_program_id uuid NOT NULL REFERENCES study_programs(id),catalog_version_id uuid NOT NULL,
 grade_level_id uuid NOT NULL REFERENCES grade_levels(id),term_slot_id uuid NOT NULL
);
INSERT INTO curriculum_requirement_sources
 SELECT id,'course',curriculum_level_id,study_program_id,subject_version_id,grade_level_id,term_slot_id FROM curriculum_course_requirements
 UNION ALL SELECT id,'activity',curriculum_level_id,study_program_id,activity_version_id,grade_level_id,term_slot_id FROM curriculum_activity_requirements;
DO $$ DECLARE detail text; requirement text; fk_name text; matches integer;
BEGIN
 FOR detail,requirement IN SELECT * FROM (VALUES('course_offering_details','curriculum_course_requirements'),('activity_offering_details','curriculum_activity_requirements')) AS sources(detail,requirement)
 LOOP
  SELECT count(*),min(conname) INTO matches,fk_name FROM pg_constraint
   WHERE conrelid=detail::regclass AND confrelid=requirement::regclass AND contype='f';
  IF matches<>1 THEN RAISE EXCEPTION 'CURRICULUM_SOURCE_FOREIGN_KEY_AMBIGUOUS:%',detail; END IF;
  EXECUTE format('ALTER TABLE %I DROP CONSTRAINT %I',detail,fk_name);
 END LOOP;
END $$;
ALTER TABLE course_offering_details ADD CONSTRAINT course_offering_details_curriculum_source_fkey FOREIGN KEY(curriculum_course_requirement_id) REFERENCES curriculum_requirement_sources(id);
ALTER TABLE activity_offering_details ADD CONSTRAINT activity_offering_details_curriculum_source_fkey FOREIGN KEY(curriculum_activity_requirement_id) REFERENCES curriculum_requirement_sources(id);
CREATE FUNCTION curriculum_register_requirement_source() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE kind text; catalog uuid;
BEGIN
 kind:=CASE WHEN TG_TABLE_NAME IN ('curriculum_course_requirements','curriculum_publication_courses') THEN 'course' ELSE 'activity' END;
 catalog:=CASE WHEN kind='course' THEN (to_jsonb(NEW)->>'subject_version_id')::uuid ELSE (to_jsonb(NEW)->>'activity_version_id')::uuid END;
 INSERT INTO curriculum_requirement_sources VALUES(NEW.id,kind,NEW.curriculum_level_id,NEW.study_program_id,catalog,NEW.grade_level_id,NEW.term_slot_id)
 ON CONFLICT(id) DO NOTHING;
 IF NOT EXISTS(SELECT 1 FROM curriculum_requirement_sources WHERE id=NEW.id AND resource_kind=kind
  AND curriculum_level_id=NEW.curriculum_level_id AND study_program_id=NEW.study_program_id
  AND catalog_version_id=catalog AND grade_level_id=NEW.grade_level_id AND term_slot_id=NEW.term_slot_id)
 THEN RAISE EXCEPTION 'CURRICULUM_SOURCE_IDENTITY_MISMATCH'; END IF;
 RETURN NEW;
END $$;
CREATE FUNCTION curriculum_history_immutable() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN RAISE EXCEPTION 'CURRICULUM_PUBLICATION_IMMUTABLE'; END $$;
CREATE TRIGGER curriculum_sources_immutable BEFORE UPDATE OR DELETE ON curriculum_requirement_sources FOR EACH ROW EXECUTE FUNCTION curriculum_history_immutable();
CREATE TRIGGER curriculum_publications_immutable BEFORE UPDATE OR DELETE ON curriculum_publications FOR EACH ROW EXECUTE FUNCTION curriculum_history_immutable();
CREATE FUNCTION curriculum_publication_insert_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF NOT EXISTS(SELECT 1 FROM curriculum_publications WHERE id=NEW.publication_id AND creation_xid=pg_current_xact_id())
 THEN RAISE EXCEPTION 'CURRICULUM_PUBLICATION_SEALED'; END IF;
 RETURN NEW;
END $$;
CREATE TABLE curriculum_publication_levels (LIKE curriculum_levels INCLUDING DEFAULTS INCLUDING CONSTRAINTS,publication_id uuid NOT NULL REFERENCES curriculum_publications(id),PRIMARY KEY(publication_id,id));
CREATE TRIGGER curriculum_publication_levels_immutable BEFORE UPDATE OR DELETE ON curriculum_publication_levels FOR EACH ROW EXECUTE FUNCTION curriculum_history_immutable();
CREATE TRIGGER curriculum_publication_levels_insert_guard BEFORE INSERT ON curriculum_publication_levels FOR EACH ROW EXECUTE FUNCTION curriculum_publication_insert_guard();
CREATE TABLE curriculum_publication_programs (LIKE study_programs INCLUDING DEFAULTS INCLUDING CONSTRAINTS,publication_id uuid NOT NULL REFERENCES curriculum_publications(id),PRIMARY KEY(publication_id,id));
CREATE TRIGGER curriculum_publication_programs_immutable BEFORE UPDATE OR DELETE ON curriculum_publication_programs FOR EACH ROW EXECUTE FUNCTION curriculum_history_immutable();
CREATE TRIGGER curriculum_publication_programs_insert_guard BEFORE INSERT ON curriculum_publication_programs FOR EACH ROW EXECUTE FUNCTION curriculum_publication_insert_guard();
CREATE TABLE curriculum_publication_slots (LIKE curriculum_term_slots INCLUDING DEFAULTS INCLUDING CONSTRAINTS,publication_id uuid NOT NULL REFERENCES curriculum_publications(id),PRIMARY KEY(publication_id,id));
CREATE TRIGGER curriculum_publication_slots_immutable BEFORE UPDATE OR DELETE ON curriculum_publication_slots FOR EACH ROW EXECUTE FUNCTION curriculum_history_immutable();
CREATE TRIGGER curriculum_publication_slots_insert_guard BEFORE INSERT ON curriculum_publication_slots FOR EACH ROW EXECUTE FUNCTION curriculum_publication_insert_guard();
CREATE TABLE curriculum_publication_courses (LIKE curriculum_course_requirements INCLUDING DEFAULTS INCLUDING CONSTRAINTS,publication_id uuid NOT NULL REFERENCES curriculum_publications(id),PRIMARY KEY(publication_id,id));
CREATE TRIGGER curriculum_publication_courses_immutable BEFORE UPDATE OR DELETE ON curriculum_publication_courses FOR EACH ROW EXECUTE FUNCTION curriculum_history_immutable();
CREATE TRIGGER curriculum_publication_courses_insert_guard BEFORE INSERT ON curriculum_publication_courses FOR EACH ROW EXECUTE FUNCTION curriculum_publication_insert_guard();
CREATE TABLE curriculum_publication_activities (LIKE curriculum_activity_requirements INCLUDING DEFAULTS INCLUDING CONSTRAINTS,publication_id uuid NOT NULL REFERENCES curriculum_publications(id),PRIMARY KEY(publication_id,id));
CREATE TRIGGER curriculum_publication_activities_immutable BEFORE UPDATE OR DELETE ON curriculum_publication_activities FOR EACH ROW EXECUTE FUNCTION curriculum_history_immutable();
CREATE TRIGGER curriculum_publication_activities_insert_guard BEFORE INSERT ON curriculum_publication_activities FOR EACH ROW EXECUTE FUNCTION curriculum_publication_insert_guard();
CREATE TRIGGER curriculum_publication_course_source BEFORE INSERT ON curriculum_publication_courses FOR EACH ROW EXECUTE FUNCTION curriculum_register_requirement_source();
CREATE TRIGGER curriculum_publication_activity_source BEFORE INSERT ON curriculum_publication_activities FOR EACH ROW EXECUTE FUNCTION curriculum_register_requirement_source();
ALTER TABLE curriculum_publication_levels ADD CONSTRAINT publication_level_owner_fkey FOREIGN KEY(publication_id,edition_id) REFERENCES curriculum_publications(id,edition_id);
ALTER TABLE curriculum_publication_programs ADD CONSTRAINT publication_program_level_key UNIQUE(publication_id,id,curriculum_level_id),
 ADD CONSTRAINT publication_program_level_fkey FOREIGN KEY(publication_id,curriculum_level_id) REFERENCES curriculum_publication_levels(publication_id,id);
ALTER TABLE curriculum_publication_slots ADD CONSTRAINT publication_slot_level_key UNIQUE(publication_id,id,curriculum_level_id),
 ADD CONSTRAINT publication_slot_level_fkey FOREIGN KEY(publication_id,curriculum_level_id) REFERENCES curriculum_publication_levels(publication_id,id);
ALTER TABLE curriculum_publication_courses
 ADD CONSTRAINT publication_courses_level_fkey FOREIGN KEY(publication_id,curriculum_level_id) REFERENCES curriculum_publication_levels(publication_id,id),
 ADD CONSTRAINT publication_courses_program_fkey FOREIGN KEY(publication_id,study_program_id,curriculum_level_id) REFERENCES curriculum_publication_programs(publication_id,id,curriculum_level_id),
 ADD CONSTRAINT publication_courses_slot_fkey FOREIGN KEY(publication_id,term_slot_id,curriculum_level_id) REFERENCES curriculum_publication_slots(publication_id,id,curriculum_level_id),
 ADD CONSTRAINT publication_courses_source_fkey FOREIGN KEY(id) REFERENCES curriculum_requirement_sources(id);
ALTER TABLE curriculum_publication_activities
 ADD CONSTRAINT publication_activities_level_fkey FOREIGN KEY(publication_id,curriculum_level_id) REFERENCES curriculum_publication_levels(publication_id,id),
 ADD CONSTRAINT publication_activities_program_fkey FOREIGN KEY(publication_id,study_program_id,curriculum_level_id) REFERENCES curriculum_publication_programs(publication_id,id,curriculum_level_id),
 ADD CONSTRAINT publication_activities_slot_fkey FOREIGN KEY(publication_id,term_slot_id,curriculum_level_id) REFERENCES curriculum_publication_slots(publication_id,id,curriculum_level_id),
 ADD CONSTRAINT publication_activities_source_fkey FOREIGN KEY(id) REFERENCES curriculum_requirement_sources(id);
CREATE FUNCTION curriculum_publication_complete() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF NOT EXISTS(SELECT 1 FROM curriculum_editions WHERE id=NEW.edition_id AND current_publication_id=NEW.id AND publication_count=NEW.publication_no AND draft_id IS NULL)
 OR NEW.level_count<>(SELECT count(*) FROM curriculum_publication_levels WHERE publication_id=NEW.id)
 OR NEW.program_count<>(SELECT count(*) FROM curriculum_publication_programs WHERE publication_id=NEW.id)
 OR NEW.slot_count<>(SELECT count(*) FROM curriculum_publication_slots WHERE publication_id=NEW.id)
 OR NEW.course_count<>(SELECT count(*) FROM curriculum_publication_courses WHERE publication_id=NEW.id)
 OR NEW.activity_count<>(SELECT count(*) FROM curriculum_publication_activities WHERE publication_id=NEW.id)
 THEN RAISE EXCEPTION 'CURRICULUM_PUBLICATION_INCOMPLETE'; END IF;
 RETURN NULL;
END $$;
CREATE CONSTRAINT TRIGGER curriculum_publication_complete AFTER INSERT ON curriculum_publications DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION curriculum_publication_complete();
CREATE FUNCTION capture_curriculum_publication(selected_edition uuid,actor uuid,note text,baseline boolean DEFAULT false) RETURNS uuid LANGUAGE plpgsql AS $$
DECLARE edition curriculum_editions; publication uuid:=gen_random_uuid();
BEGIN
 SELECT * INTO STRICT edition FROM curriculum_editions WHERE id=selected_edition FOR UPDATE;
 IF NOT baseline AND edition.draft_id IS NULL THEN RAISE EXCEPTION 'CURRICULUM_DRAFT_REQUIRED'; END IF;
 IF baseline AND (edition.publication_count<>0 OR edition.status<>'published') THEN RAISE EXCEPTION 'CURRICULUM_INVALID_BASELINE'; END IF;
 INSERT INTO curriculum_publications(id,edition_id,publication_no,previous_publication_id,name,revision_year,description,published_by,published_at,change_note,is_baseline,level_count,program_count,slot_count,course_count,activity_count)
 SELECT publication,edition.id,edition.publication_count+1,edition.current_publication_id,edition.name,edition.revision_year,edition.description,actor,
 CASE WHEN baseline THEN edition.published_at ELSE now() END,note,baseline,
 (SELECT count(*) FROM curriculum_levels WHERE edition_id=edition.id),
 (SELECT count(*) FROM study_programs WHERE curriculum_level_id IN(SELECT id FROM curriculum_levels WHERE edition_id=edition.id)),
 (SELECT count(*) FROM curriculum_term_slots WHERE curriculum_level_id IN(SELECT id FROM curriculum_levels WHERE edition_id=edition.id)),
 (SELECT count(*) FROM curriculum_course_requirements WHERE curriculum_level_id IN(SELECT id FROM curriculum_levels WHERE edition_id=edition.id)),
 (SELECT count(*) FROM curriculum_activity_requirements WHERE curriculum_level_id IN(SELECT id FROM curriculum_levels WHERE edition_id=edition.id));
 INSERT INTO curriculum_publication_levels SELECT r.*,publication FROM curriculum_levels r WHERE edition_id=edition.id;
 INSERT INTO curriculum_publication_programs SELECT r.*,publication FROM study_programs r WHERE curriculum_level_id IN(SELECT id FROM curriculum_levels WHERE edition_id=edition.id);
 INSERT INTO curriculum_publication_slots SELECT r.*,publication FROM curriculum_term_slots r WHERE curriculum_level_id IN(SELECT id FROM curriculum_levels WHERE edition_id=edition.id);
 INSERT INTO curriculum_publication_courses SELECT r.*,publication FROM curriculum_course_requirements r WHERE curriculum_level_id IN(SELECT id FROM curriculum_levels WHERE edition_id=edition.id);
 INSERT INTO curriculum_publication_activities SELECT r.*,publication FROM curriculum_activity_requirements r WHERE curriculum_level_id IN(SELECT id FROM curriculum_levels WHERE edition_id=edition.id);
 UPDATE curriculum_editions SET current_publication_id=publication,publication_count=publication_count+1,draft_id=NULL,status='published',
  published_at=CASE WHEN baseline THEN edition.published_at ELSE now() END,
  row_version=row_version+CASE WHEN baseline THEN 0 ELSE 1 END,
  updated_at=CASE WHEN baseline THEN edition.updated_at ELSE now() END WHERE id=edition.id;
 RETURN publication;
END $$;
SELECT capture_curriculum_publication(id,NULL,'ข้อมูลหลักสูตรที่เผยแพร่ก่อนเริ่มประวัติการแก้ไข',true) FROM curriculum_editions WHERE status='published';
CREATE VIEW published_curriculum_levels AS SELECT r.* FROM curriculum_publication_levels r JOIN curriculum_editions e ON e.current_publication_id=r.publication_id;
CREATE VIEW published_study_programs AS SELECT r.* FROM curriculum_publication_programs r JOIN curriculum_editions e ON e.current_publication_id=r.publication_id;
CREATE VIEW published_curriculum_term_slots AS SELECT r.* FROM curriculum_publication_slots r JOIN curriculum_editions e ON e.current_publication_id=r.publication_id;
CREATE VIEW published_curriculum_course_requirements AS SELECT r.* FROM curriculum_publication_courses r JOIN curriculum_editions e ON e.current_publication_id=r.publication_id;
CREATE VIEW published_curriculum_activity_requirements AS SELECT r.* FROM curriculum_publication_activities r JOIN curriculum_editions e ON e.current_publication_id=r.publication_id;
CREATE FUNCTION curriculum_edition_mutation_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP='DELETE' AND OLD.current_publication_id IS NOT NULL THEN RAISE EXCEPTION 'CURRICULUM_PUBLICATION_IMMUTABLE'; END IF;
 IF TG_OP='UPDATE' AND OLD.current_publication_id IS NOT NULL AND
 (NEW.name IS DISTINCT FROM OLD.name OR NEW.revision_year IS DISTINCT FROM OLD.revision_year OR NEW.description IS DISTINCT FROM OLD.description)
 THEN RAISE EXCEPTION 'CURRICULUM_EDITION_METADATA_IMMUTABLE'; END IF;
 IF TG_OP='UPDATE' AND NEW.current_publication_id IS NOT DISTINCT FROM OLD.current_publication_id AND
 NEW.publication_count<>OLD.publication_count THEN RAISE EXCEPTION 'CURRICULUM_PUBLICATION_POINTER_INVALID'; END IF;
 IF TG_OP='UPDATE' AND OLD.draft_id IS NOT NULL AND NEW.draft_id IS DISTINCT FROM OLD.draft_id AND
 NEW.current_publication_id IS NOT DISTINCT FROM OLD.current_publication_id THEN RAISE EXCEPTION 'CURRICULUM_DRAFT_TOKEN_INVALID'; END IF;
 IF TG_OP='UPDATE' AND NEW.current_publication_id IS DISTINCT FROM OLD.current_publication_id AND
 NOT EXISTS(SELECT 1 FROM curriculum_publications WHERE id=NEW.current_publication_id AND edition_id=NEW.id
  AND publication_no=OLD.publication_count+1 AND previous_publication_id IS NOT DISTINCT FROM OLD.current_publication_id
  AND creation_xid=pg_current_xact_id()) THEN RAISE EXCEPTION 'CURRICULUM_PUBLICATION_POINTER_INVALID'; END IF;
 RETURN CASE WHEN TG_OP='DELETE' THEN OLD ELSE NEW END;
END $$;
CREATE TRIGGER curriculum_editions_publication_guard BEFORE UPDATE OR DELETE ON curriculum_editions FOR EACH ROW EXECUTE FUNCTION curriculum_edition_mutation_guard();
CREATE OR REPLACE FUNCTION academic_prevent_published_curriculum_level_mutation() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE token uuid;
BEGIN
 IF TG_OP<>'INSERT' THEN
  SELECT draft_id INTO token FROM curriculum_editions WHERE id=OLD.edition_id FOR SHARE;
  IF token IS NULL THEN RAISE EXCEPTION 'CURRICULUM_DRAFT_REQUIRED'; END IF;
 END IF;
 IF TG_OP<>'DELETE' THEN
  SELECT draft_id INTO token FROM curriculum_editions WHERE id=NEW.edition_id FOR SHARE;
  IF token IS NULL THEN RAISE EXCEPTION 'CURRICULUM_DRAFT_REQUIRED'; END IF;
 END IF;
 RETURN CASE WHEN TG_OP='DELETE' THEN OLD ELSE NEW END;
END $$;
CREATE OR REPLACE FUNCTION academic_prevent_published_curriculum_child_mutation() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE token uuid;
BEGIN
 IF TG_OP<>'INSERT' THEN
  SELECT e.draft_id INTO token FROM curriculum_levels l JOIN curriculum_editions e ON e.id=l.edition_id WHERE l.id=OLD.curriculum_level_id FOR SHARE OF e;
  IF token IS NULL THEN RAISE EXCEPTION 'CURRICULUM_DRAFT_REQUIRED'; END IF;
 END IF;
 IF TG_OP<>'DELETE' THEN
  SELECT e.draft_id INTO token FROM curriculum_levels l JOIN curriculum_editions e ON e.id=l.edition_id WHERE l.id=NEW.curriculum_level_id FOR SHARE OF e;
  IF token IS NULL THEN RAISE EXCEPTION 'CURRICULUM_DRAFT_REQUIRED'; END IF;
 END IF;
 RETURN CASE WHEN TG_OP='DELETE' THEN OLD ELSE NEW END;
END $$;
CREATE OR REPLACE FUNCTION check_admission_track_program_context() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF NOT EXISTS(SELECT 1 FROM admission_rounds r JOIN published_study_programs p ON p.id=NEW.study_program_id
 JOIN published_curriculum_levels l ON l.id=p.curriculum_level_id JOIN curriculum_editions e ON e.id=l.edition_id
 WHERE r.id=NEW.admission_round_id AND r.academic_year_id=NEW.academic_year_id AND p.status='published' AND e.status='published' AND e.is_active AND l.is_active)
 THEN RAISE EXCEPTION 'ACADEMIC_ADMISSION_TRACK_PROGRAM_CONTEXT_MISMATCH' USING ERRCODE='check_violation'; END IF;
 RETURN NEW;
END $$;
DO $$ BEGIN
 IF EXISTS(SELECT 1 FROM curriculum_095_evidence e WHERE
 e.courses IS DISTINCT FROM(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM curriculum_course_requirements r)
 OR e.activities IS DISTINCT FROM(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM curriculum_activity_requirements r)
 OR e.rooms IS DISTINCT FROM(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM homerooms r)
 OR e.students IS DISTINCT FROM(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM student_academic_years r)
 OR e.targets IS DISTINCT FROM(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM learning_offering_targets r)
 OR e.deliveries IS DISTINCT FROM(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM academic_delivery_versions r))
 THEN RAISE EXCEPTION 'CURRICULUM_095_PRESERVATION_FAILED'; END IF;
 IF EXISTS(SELECT 1 FROM curriculum_editions e WHERE current_publication_id IS NOT NULL AND
 (SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM curriculum_levels r WHERE r.edition_id=e.id) IS DISTINCT FROM
 (SELECT jsonb_agg(to_jsonb(s)-'publication_id' ORDER BY id) FROM curriculum_publication_levels s WHERE publication_id=e.current_publication_id))
 THEN RAISE EXCEPTION 'CURRICULUM_095_LEVELS_SNAPSHOT_MISMATCH'; END IF;
 IF EXISTS(SELECT 1 FROM curriculum_editions e WHERE current_publication_id IS NOT NULL AND
 (SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM study_programs r WHERE r.curriculum_level_id IN(SELECT id FROM curriculum_levels WHERE edition_id=e.id)) IS DISTINCT FROM
 (SELECT jsonb_agg(to_jsonb(s)-'publication_id' ORDER BY id) FROM curriculum_publication_programs s WHERE publication_id=e.current_publication_id))
 THEN RAISE EXCEPTION 'CURRICULUM_095_PROGRAMS_SNAPSHOT_MISMATCH'; END IF;
 IF EXISTS(SELECT 1 FROM curriculum_editions e WHERE current_publication_id IS NOT NULL AND
 (SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM curriculum_term_slots r WHERE r.curriculum_level_id IN(SELECT id FROM curriculum_levels WHERE edition_id=e.id)) IS DISTINCT FROM
 (SELECT jsonb_agg(to_jsonb(s)-'publication_id' ORDER BY id) FROM curriculum_publication_slots s WHERE publication_id=e.current_publication_id))
 THEN RAISE EXCEPTION 'CURRICULUM_095_SLOTS_SNAPSHOT_MISMATCH'; END IF;
 IF EXISTS(SELECT 1 FROM curriculum_editions e WHERE current_publication_id IS NOT NULL AND
 (SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM curriculum_course_requirements r WHERE r.curriculum_level_id IN(SELECT id FROM curriculum_levels WHERE edition_id=e.id)) IS DISTINCT FROM
 (SELECT jsonb_agg(to_jsonb(s)-'publication_id' ORDER BY id) FROM curriculum_publication_courses s WHERE publication_id=e.current_publication_id))
 THEN RAISE EXCEPTION 'CURRICULUM_095_COURSES_SNAPSHOT_MISMATCH'; END IF;
 IF EXISTS(SELECT 1 FROM curriculum_editions e WHERE current_publication_id IS NOT NULL AND
 (SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM curriculum_activity_requirements r WHERE r.curriculum_level_id IN(SELECT id FROM curriculum_levels WHERE edition_id=e.id)) IS DISTINCT FROM
 (SELECT jsonb_agg(to_jsonb(s)-'publication_id' ORDER BY id) FROM curriculum_publication_activities s WHERE publication_id=e.current_publication_id))
 THEN RAISE EXCEPTION 'CURRICULUM_095_ACTIVITIES_SNAPSHOT_MISMATCH'; END IF;
END $$;
INSERT INTO academic_audit_events(event_code,entity_type,entity_id,payload)
 SELECT 'curriculum.publication_baseline.reconciled','curriculum_edition',id,jsonb_build_object('migration',95,'passed',true,'publicationId',current_publication_id)
 FROM curriculum_editions WHERE current_publication_id IS NOT NULL;
