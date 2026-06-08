ALTER TABLE activity_catalog
    ADD COLUMN IF NOT EXISTS term VARCHAR(20),
    ADD COLUMN IF NOT EXISTS grade_level_ids JSONB;

COMMENT ON COLUMN activity_catalog.term IS 'ภาคเรียน (NULL = ทุกเทอม) — ตรงกับ subjects.term';
COMMENT ON COLUMN activity_catalog.grade_level_ids IS 'ระดับชั้นที่ใช้กิจกรรมนี้ (JSONB array of grade_level UUIDs)';
