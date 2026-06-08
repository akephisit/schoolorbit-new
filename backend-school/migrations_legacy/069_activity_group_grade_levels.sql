-- เพิ่ม allowed_grade_level_ids ใน activity_groups
-- เพื่อให้แต่ละกิจกรรมกำหนดชั้นที่รับได้เอง (override slot ถ้าต้องการ)
-- NULL = ใช้ค่าจาก slot

ALTER TABLE activity_groups
    ADD COLUMN IF NOT EXISTS allowed_grade_level_ids JSONB;

COMMENT ON COLUMN activity_groups.allowed_grade_level_ids IS 'ชั้นที่รับ (override slot), NULL = ใช้ค่าจาก slot';
