-- Discard unpublished amendments only after restoring the complete released graph.
-- An unpublished, unreferenced identity is draft data, not retained history.
-- Published/actual identities remain protected by snapshot and delivery foreign keys.
CREATE FUNCTION curriculum_requirement_source_mutation_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP='DELETE' AND EXISTS(
  SELECT 1 FROM curriculum_levels l JOIN curriculum_editions e ON e.id=l.edition_id
  WHERE l.id=OLD.curriculum_level_id AND e.draft_id IS NOT NULL
 ) AND NOT EXISTS(SELECT 1 FROM curriculum_publication_courses WHERE id=OLD.id)
 AND NOT EXISTS(SELECT 1 FROM curriculum_publication_activities WHERE id=OLD.id)
 AND NOT EXISTS(SELECT 1 FROM curriculum_course_requirements WHERE id=OLD.id)
 AND NOT EXISTS(SELECT 1 FROM curriculum_activity_requirements WHERE id=OLD.id)
 THEN RETURN OLD; END IF;
 RAISE EXCEPTION 'CURRICULUM_PUBLICATION_IMMUTABLE';
END $$;
DROP TRIGGER curriculum_sources_immutable ON curriculum_requirement_sources;
CREATE TRIGGER curriculum_sources_immutable BEFORE UPDATE OR DELETE ON curriculum_requirement_sources
 FOR EACH ROW EXECUTE FUNCTION curriculum_requirement_source_mutation_guard();

CREATE FUNCTION curriculum_workspace_matches_publication(selected_edition uuid,selected_publication uuid)
RETURNS boolean LANGUAGE sql STABLE AS $$
 SELECT selected_publication IS NOT NULL AND
 (SELECT jsonb_agg(to_jsonb(r)-'row_version'-'updated_at' ORDER BY id) FROM curriculum_levels r WHERE edition_id=selected_edition)
 IS NOT DISTINCT FROM
 (SELECT jsonb_agg(to_jsonb(r)-'publication_id'-'row_version'-'updated_at' ORDER BY id) FROM curriculum_publication_levels r WHERE publication_id=selected_publication)
 AND
 (SELECT jsonb_agg(to_jsonb(r)-'row_version'-'updated_at' ORDER BY id) FROM study_programs r WHERE curriculum_level_id IN (SELECT id FROM curriculum_levels WHERE edition_id=selected_edition))
 IS NOT DISTINCT FROM
 (SELECT jsonb_agg(to_jsonb(r)-'publication_id'-'row_version'-'updated_at' ORDER BY id) FROM curriculum_publication_programs r WHERE publication_id=selected_publication)
 AND
 (SELECT jsonb_agg(to_jsonb(r)-'row_version'-'updated_at' ORDER BY id) FROM curriculum_term_slots r WHERE curriculum_level_id IN (SELECT id FROM curriculum_levels WHERE edition_id=selected_edition))
 IS NOT DISTINCT FROM
 (SELECT jsonb_agg(to_jsonb(r)-'publication_id'-'row_version'-'updated_at' ORDER BY id) FROM curriculum_publication_slots r WHERE publication_id=selected_publication)
 AND
 (SELECT jsonb_agg(to_jsonb(r)-'row_version'-'updated_at' ORDER BY id) FROM curriculum_course_requirements r WHERE curriculum_level_id IN (SELECT id FROM curriculum_levels WHERE edition_id=selected_edition))
 IS NOT DISTINCT FROM
 (SELECT jsonb_agg(to_jsonb(r)-'publication_id'-'row_version'-'updated_at' ORDER BY id) FROM curriculum_publication_courses r WHERE publication_id=selected_publication)
 AND
 (SELECT jsonb_agg(to_jsonb(r)-'row_version'-'updated_at' ORDER BY id) FROM curriculum_activity_requirements r WHERE curriculum_level_id IN (SELECT id FROM curriculum_levels WHERE edition_id=selected_edition))
 IS NOT DISTINCT FROM
 (SELECT jsonb_agg(to_jsonb(r)-'publication_id'-'row_version'-'updated_at' ORDER BY id) FROM curriculum_publication_activities r WHERE publication_id=selected_publication);
$$;

CREATE OR REPLACE FUNCTION curriculum_edition_mutation_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP='DELETE' AND OLD.current_publication_id IS NOT NULL THEN RAISE EXCEPTION 'CURRICULUM_PUBLICATION_IMMUTABLE'; END IF;
 IF TG_OP='UPDATE' AND OLD.current_publication_id IS NOT NULL AND
 (NEW.name IS DISTINCT FROM OLD.name OR NEW.revision_year IS DISTINCT FROM OLD.revision_year OR NEW.description IS DISTINCT FROM OLD.description)
 THEN RAISE EXCEPTION 'CURRICULUM_EDITION_METADATA_IMMUTABLE'; END IF;
 IF TG_OP='UPDATE' AND NEW.current_publication_id IS NOT DISTINCT FROM OLD.current_publication_id AND
 NEW.publication_count<>OLD.publication_count THEN RAISE EXCEPTION 'CURRICULUM_PUBLICATION_POINTER_INVALID'; END IF;
 IF TG_OP='UPDATE' AND OLD.draft_id IS NOT NULL AND NEW.draft_id IS DISTINCT FROM OLD.draft_id AND
 NEW.current_publication_id IS NOT DISTINCT FROM OLD.current_publication_id THEN
  IF NEW.draft_id IS NOT NULL OR NOT curriculum_workspace_matches_publication(NEW.id,NEW.current_publication_id)
  THEN RAISE EXCEPTION 'CURRICULUM_DRAFT_TOKEN_INVALID'; END IF;
 END IF;
 IF TG_OP='UPDATE' AND NEW.current_publication_id IS DISTINCT FROM OLD.current_publication_id AND
 NOT EXISTS(SELECT 1 FROM curriculum_publications WHERE id=NEW.current_publication_id AND edition_id=NEW.id
  AND publication_no=OLD.publication_count+1 AND previous_publication_id IS NOT DISTINCT FROM OLD.current_publication_id
  AND creation_xid=pg_current_xact_id()) THEN RAISE EXCEPTION 'CURRICULUM_PUBLICATION_POINTER_INVALID'; END IF;
 RETURN CASE WHEN TG_OP='DELETE' THEN OLD ELSE NEW END;
END $$;
