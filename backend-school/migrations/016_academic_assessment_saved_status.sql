ALTER TABLE academic_assessment_plans
    DROP CONSTRAINT IF EXISTS academic_assessment_plans_status_check,
    ADD CONSTRAINT academic_assessment_plans_status_check
        CHECK (status IN ('draft', 'saved', 'submitted', 'locked'));

UPDATE academic_assessment_plans ap
SET status = 'saved'
WHERE status = 'draft'
  AND EXISTS (
      SELECT 1
      FROM academic_assessment_categories c
      WHERE c.plan_id = ap.id
  );
