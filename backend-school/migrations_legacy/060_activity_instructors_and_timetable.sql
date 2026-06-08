-- ============================================
-- 1. Activity Group Instructors (ครูหลายคนต่อกลุ่ม)
-- ============================================

CREATE TABLE IF NOT EXISTS activity_group_instructors (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    activity_group_id UUID NOT NULL REFERENCES activity_groups(id) ON DELETE CASCADE,
    instructor_id UUID NOT NULL REFERENCES staff_info(id) ON DELETE CASCADE,
    role VARCHAR(20) NOT NULL DEFAULT 'assistant' CHECK (role IN ('primary', 'assistant')),
    CONSTRAINT unique_instructor_per_group UNIQUE (activity_group_id, instructor_id)
);

CREATE INDEX idx_agi_group ON activity_group_instructors(activity_group_id);
CREATE INDEX idx_agi_instructor ON activity_group_instructors(instructor_id);

-- ย้าย instructor_id เดิมใน activity_groups ไปเป็น primary ใน join table
INSERT INTO activity_group_instructors (activity_group_id, instructor_id, role)
SELECT id, instructor_id, 'primary'
FROM activity_groups
WHERE instructor_id IS NOT NULL
ON CONFLICT DO NOTHING;

COMMENT ON TABLE activity_group_instructors IS 'ครูที่ดูแลกลุ่มกิจกรรม (รองรับหลายคน)';
COMMENT ON COLUMN activity_group_instructors.role IS 'primary=ครูหลัก, assistant=ครูผู้ช่วย';
