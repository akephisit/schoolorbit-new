ALTER TABLE study_plan_version_activities
    ADD COLUMN IF NOT EXISTS term VARCHAR(20);

COMMENT ON COLUMN study_plan_version_activities.term IS 'เทอมที่จัดกิจกรรม (NULL = ทุกเทอม). ตรงกับ study_plan_subjects.term';
