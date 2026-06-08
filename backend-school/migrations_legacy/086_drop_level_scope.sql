-- ============================================
-- Migrate level_scope → grade_level_ids / junction, then drop level_scope columns
-- level_scope values: 'all', 'kindergarten', 'primary', 'secondary'
-- grade_levels.level_type: 'kindergarten', 'primary', 'secondary'
-- ============================================

-- ----- study_plans (has grade_level_ids JSONB column from migration 085) -----
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

-- ----- subjects (uses subject_grade_levels junction table, NOT a JSONB column) -----
-- For each subject with level_scope set, insert rows into subject_grade_levels
-- for every grade_level matching that scope (skip if junction already has entries).
INSERT INTO subject_grade_levels (subject_id, grade_level_id)
SELECT s.id, gl.id
FROM subjects s
CROSS JOIN grade_levels gl
WHERE s.level_scope IS NOT NULL
  AND ((s.level_scope = 'all' AND gl.is_active = true)
       OR gl.level_type = s.level_scope)
  AND NOT EXISTS (
      SELECT 1 FROM subject_grade_levels sgl
      WHERE sgl.subject_id = s.id
  )
ON CONFLICT DO NOTHING;

ALTER TABLE subjects DROP COLUMN IF EXISTS level_scope;
