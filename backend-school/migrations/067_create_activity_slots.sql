-- ============================================
-- Activity Slots (ช่องกิจกรรม)
--
-- Admin สร้าง "ช่อง" ก่อนเปิดเทอม เช่น
--   "ชุมนุม ม.ต้น เทอม 1/2569" → พุธ คาบ 6-7, ม.1-ม.3
--   "ลูกเสือ ม.ต้น เทอม 1/2569" → จันทร์ คาบ 6, ม.1-ม.3
--
-- จากนั้นครูสร้างชุมนุม/กิจกรรมจริงภายใต้ "ช่อง" นี้
-- ============================================

-- 1. สร้างตาราง activity_slots
CREATE TABLE IF NOT EXISTS activity_slots (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),

    -- ข้อมูลช่อง
    name VARCHAR(200) NOT NULL,              -- เช่น "ชุมนุม ม.ต้น"
    description TEXT,
    activity_type VARCHAR(20) NOT NULL CHECK (activity_type IN ('scout', 'club', 'guidance', 'social', 'other')),

    -- ขอบเขต
    semester_id UUID NOT NULL REFERENCES academic_semesters(id) ON DELETE CASCADE,
    allowed_grade_level_ids JSONB,           -- ชั้นที่รับ, NULL = ทุกชั้น

    -- ตารางเวลา
    day_of_week VARCHAR(3),                  -- MON, TUE, ...
    period_ids JSONB,                        -- ["period_uuid1", "period_uuid2"]

    -- การลงทะเบียน
    registration_type VARCHAR(10) NOT NULL DEFAULT 'assigned' CHECK (registration_type IN ('self', 'assigned')),
    teacher_reg_open BOOLEAN NOT NULL DEFAULT false,     -- เปิดให้ครูสร้างกิจกรรม
    student_reg_open BOOLEAN NOT NULL DEFAULT false,     -- เปิดให้นักเรียนเลือก
    student_reg_start TIMESTAMPTZ,           -- เริ่มลงทะเบียนนักเรียน (optional)
    student_reg_end TIMESTAMPTZ,             -- ปิดลงทะเบียนนักเรียน (optional)

    -- Metadata
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_activity_slots_semester ON activity_slots(semester_id);
CREATE INDEX idx_activity_slots_type ON activity_slots(activity_type);

CREATE TRIGGER update_activity_slots_updated_at
    BEFORE UPDATE ON activity_slots
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

COMMENT ON TABLE activity_slots IS 'ช่องกิจกรรม: Admin สร้างก่อนเปิดเทอม กำหนดวัน/คาบ/ชั้น ให้ครูสร้างชุมนุมภายใต้ช่องนี้';
COMMENT ON COLUMN activity_slots.teacher_reg_open IS 'เปิดให้ครูลงทะเบียนสร้างกิจกรรมในช่องนี้';
COMMENT ON COLUMN activity_slots.student_reg_open IS 'เปิดให้นักเรียนลงทะเบียนเลือกกิจกรรม';

-- 2. เพิ่ม slot_id ใน activity_groups
ALTER TABLE activity_groups ADD COLUMN slot_id UUID REFERENCES activity_slots(id) ON DELETE SET NULL;
CREATE INDEX idx_activity_groups_slot ON activity_groups(slot_id) WHERE slot_id IS NOT NULL;

-- 3. Migrate ข้อมูล: สร้าง slot จาก activity_groups ที่มีอยู่
-- จัดกลุ่มตาม (activity_type, semester_id, day_of_week) → สร้าง slot ให้แต่ละกลุ่ม
INSERT INTO activity_slots (
    id, name, activity_type, semester_id, allowed_grade_level_ids,
    day_of_week, period_ids, registration_type, teacher_reg_open, student_reg_open,
    created_by
)
SELECT
    uuid_generate_v4(),
    CASE ag.activity_type
        WHEN 'scout' THEN 'ลูกเสือ/เนตรนารี'
        WHEN 'club' THEN 'ชุมนุม'
        WHEN 'guidance' THEN 'แนะแนว'
        WHEN 'social' THEN 'กิจกรรมเพื่อสังคม'
        ELSE 'กิจกรรมอื่นๆ'
    END || ' ' || sem.name AS name,
    ag.activity_type,
    ag.semester_id,
    ag.allowed_grade_level_ids,
    ag.day_of_week,
    ag.period_ids,
    ag.registration_type,
    false,  -- teacher_reg_open
    false,  -- student_reg_open
    ag.created_by
FROM activity_groups ag
JOIN academic_semesters sem ON sem.id = ag.semester_id
WHERE ag.is_active = true
GROUP BY ag.activity_type, ag.semester_id, ag.day_of_week, ag.period_ids,
         ag.allowed_grade_level_ids, ag.registration_type, ag.created_by, sem.name
ON CONFLICT DO NOTHING;

-- 4. ผูก activity_groups กับ slot ที่สร้าง
UPDATE activity_groups ag SET slot_id = s.id
FROM activity_slots s
WHERE s.activity_type = ag.activity_type
  AND s.semester_id = ag.semester_id
  AND (s.day_of_week = ag.day_of_week OR (s.day_of_week IS NULL AND ag.day_of_week IS NULL));

-- 5. ลบ fields ที่ซ้ำออกจาก activity_groups (ย้ายไป slot แล้ว)
ALTER TABLE activity_groups
    DROP COLUMN IF EXISTS semester_id CASCADE,
    DROP COLUMN IF EXISTS activity_type,
    DROP COLUMN IF EXISTS allowed_grade_level_ids,
    DROP COLUMN IF EXISTS day_of_week,
    DROP COLUMN IF EXISTS period_ids,
    DROP COLUMN IF EXISTS registration_type;

-- 6. ลบ indexes เก่าที่อ้าง columns ที่ถูกลบ
DROP INDEX IF EXISTS idx_activity_groups_semester;
DROP INDEX IF EXISTS idx_activity_groups_type;
DROP INDEX IF EXISTS idx_activity_groups_day;
