-- ============================================
-- Migrate level_scope → grade_level_ids, then drop level_scope columns
-- level_scope has values: 'all', 'kindergarten', 'primary', 'secondary'
-- grade_levels.level_type has values: 'kindergarten', 'primary', 'secondary'
-- ============================================

-- ----- study_plans -----
-- Where grade_level_ids is missing but level_scope is set, derive from level_scope
UPDATE study_plans
SET grade_level_ids = (
    SELECT COALESCE(jsonb_agg(gl.id), '[]'::jsonb)
    FROM grade_levels gl
    WHERE (study_plans.level_scope = 'all' AND gl.is_active = true)
       OR gl.level_type = study_plans.level_scope
)
WHERE (grade_level_ids IS NULL OR grade_level_ids = '[]'::jsonb)
  AND level_scope IS NOT NULL;

ALTER TABLE study_plans DROP COLUMN IF EXISTS level_scope;

-- ----- subjects -----
-- Same pattern
UPDATE subjects
SET grade_level_ids = (
    SELECT COALESCE(jsonb_agg(gl.id), '[]'::jsonb)
    FROM grade_levels gl
    WHERE (subjects.level_scope = 'all' AND gl.is_active = true)
       OR gl.level_type = subjects.level_scope
)
WHERE (grade_level_ids IS NULL OR grade_level_ids = '[]'::jsonb)
  AND level_scope IS NOT NULL;

ALTER TABLE subjects DROP COLUMN IF EXISTS level_scope;
