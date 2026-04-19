ALTER TABLE study_plans
    ADD COLUMN IF NOT EXISTS grade_level_ids JSONB;

COMMENT ON COLUMN study_plans.grade_level_ids IS 'ระดับชั้นเฉพาะที่หลักสูตรนี้ใช้ (array of grade_level UUIDs). NULL/empty = ทุกระดับในขอบเขต level_scope';
