-- Routine delivery controls use the immutable accepted creator, never current
-- homeroom membership or a cached permission flag.
DO $$ BEGIN
 IF EXISTS(SELECT 1 FROM care_assignments WHERE document->>'createdBy' IS NOT NULL AND document->>'createdBy'<>created_by::text) THEN
  RAISE EXCEPTION 'care_assignment_creator_conflict';
 END IF;
END $$;
UPDATE care_assignments SET document=jsonb_set(document,'{createdBy}',to_jsonb(created_by))
WHERE document->>'createdBy' IS NULL;
ALTER TABLE care_assignments ADD CONSTRAINT care_assignment_creator_consistency
CHECK(document->>'createdBy' IS NOT NULL AND (document->>'createdBy')::uuid=created_by);
