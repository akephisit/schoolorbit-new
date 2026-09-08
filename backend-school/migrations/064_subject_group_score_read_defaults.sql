-- Migration 015 made assessment structures readable by all subject-group members.
-- Migration 060 copied those grants to scores and learner evaluations. Those two
-- defaults belong to heads, not all members. Match the original seed timestamp
-- and unchanged copied metadata so later school-owned grants remain untouched.
WITH generated_reads AS (
    DELETE FROM organization_permission_grants AS target
    USING permissions AS target_permission,
          organization_units AS unit,
          organization_permission_grants AS source,
          permissions AS source_permission,
          _sqlx_migrations AS seed
    WHERE target.permission_id = target_permission.id
      AND target_permission.code IN (
          'academic_gradebook.read.organization_unit',
          'academic_learner_evaluation.read.organization_unit'
      )
      AND target.organization_unit_id = unit.id
      AND unit.unit_type = 'subject_group'
      AND source.organization_unit_id = unit.id
      AND source.permission_id = source_permission.id
      AND source_permission.code = 'academic_assessment.read.organization_unit'
      AND source.position_code IS NULL
      AND source.created_by IS NULL
      AND seed.version = 15 AND seed.success
      AND source.created_at = seed.installed_on
      AND target.position_code IS NULL
      AND target.created_by IS NULL
      AND target.created_at = source.created_at
    RETURNING target.organization_unit_id, target.permission_id,
              target.created_at, target.created_by
)
INSERT INTO organization_permission_grants (
    organization_unit_id, permission_id, created_at, created_by, position_code
)
SELECT organization_unit_id, permission_id, created_at, created_by, 'head'
FROM generated_reads
ON CONFLICT DO NOTHING;
