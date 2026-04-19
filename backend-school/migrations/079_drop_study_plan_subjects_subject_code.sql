-- Drop denormalized subject_code column from study_plan_subjects
-- Code is now resolved via JOIN through subjects.id

DROP INDEX IF EXISTS idx_plan_subjects_code;

ALTER TABLE study_plan_subjects DROP COLUMN IF EXISTS subject_code;
