-- ============================================
-- 1. ลบ days_of_week, period_ids ออกจาก activity_slots
--    วัน/คาบ จะกำหนดตอนจัดตาราง (timetable_entries) แทน
-- ============================================

DROP INDEX IF EXISTS idx_activity_groups_day;
ALTER TABLE activity_slots DROP COLUMN IF EXISTS days_of_week;
ALTER TABLE activity_slots DROP COLUMN IF EXISTS period_ids;

-- ============================================
-- 2. สร้างตาราง activity_slot_instructors
--    กำหนดว่าครูคนไหนสอนใน slot นี้บ้าง
--    ใช้เพื่อ: เช็ค conflict ตอนจัดตาราง + จำกัดสิทธิ์สร้างกิจกรรม
-- ============================================

CREATE TABLE IF NOT EXISTS activity_slot_instructors (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    slot_id UUID NOT NULL REFERENCES activity_slots(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT unique_slot_instructor UNIQUE (slot_id, user_id)
);

CREATE INDEX idx_slot_instructors_slot ON activity_slot_instructors(slot_id);
CREATE INDEX idx_slot_instructors_user ON activity_slot_instructors(user_id);

COMMENT ON TABLE activity_slot_instructors IS 'ครูที่สอนใน slot กิจกรรม — ใช้เช็ค conflict + จำกัดสิทธิ์สร้างกิจกรรม';
