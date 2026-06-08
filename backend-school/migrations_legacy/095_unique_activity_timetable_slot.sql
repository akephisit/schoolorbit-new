-- ============================================
-- Prevent activity entries from occupying the same (classroom, day, period) twice
-- ============================================
-- เดิม (028): UNIQUE (classroom_course_id, day_of_week, period_id) — เฉพาะ COURSE
-- ACTIVITY entry (classroom_course_id = NULL) หลุด constraint นี้
-- → admin สามารถลาก 2 กิจกรรมลงห้องเดียวกันคาบเดียวกันได้
--
-- เพิ่ม partial unique index สำหรับ ACTIVITY:
-- ห้องเดียวกัน + วันเดียวกัน + คาบเดียวกัน + is_active = true
-- (partial → ไม่กระทบ COURSE entries)
-- ============================================

CREATE UNIQUE INDEX IF NOT EXISTS unique_activity_entry_per_classroom_slot
ON academic_timetable_entries (classroom_id, day_of_week, period_id)
WHERE classroom_course_id IS NULL
  AND activity_slot_id IS NOT NULL
  AND is_active = true;

COMMENT ON INDEX unique_activity_entry_per_classroom_slot IS
    'ห้ามวาง activity 2 ตัวในคาบเดียวกันห้องเดียวกัน (complement UNIQUE unique_entry_per_slot ที่ cover เฉพาะ COURSE)';
