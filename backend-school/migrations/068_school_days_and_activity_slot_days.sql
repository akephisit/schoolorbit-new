-- ============================================
-- 1. เพิ่ม school_days ใน academic_years
--    ค่ากลางบอกว่าปีนี้เรียนวันอะไรบ้าง
--    default = จ-ศ, โรงเรียนสอนเสาร์ก็เปลี่ยนเป็น จ-ส ได้
-- ============================================

ALTER TABLE academic_years
    ADD COLUMN IF NOT EXISTS school_days VARCHAR(50) NOT NULL DEFAULT 'MON,TUE,WED,THU,FRI';

COMMENT ON COLUMN academic_years.school_days IS 'วันที่เรียน เช่น MON,TUE,WED,THU,FRI หรือ MON,TUE,WED,THU,FRI,SAT';

-- ============================================
-- 2. Rename activity_slots.day_of_week → days_of_week
--    รองรับกิจกรรมที่สอนหลายวัน (comma-separated)
-- ============================================

ALTER TABLE activity_slots RENAME COLUMN day_of_week TO days_of_week;

COMMENT ON COLUMN activity_slots.days_of_week IS 'วันที่สอน เช่น MON หรือ MON,WED (หลายวันได้)';
