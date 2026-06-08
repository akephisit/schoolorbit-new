-- ============================================
-- academic_timetable_entries: batch_id
-- ============================================
-- entries ที่ถูกสร้างพร้อมกันจาก /timetable/batch จะมี batch_id ค่าเดียวกัน
-- ใช้สำหรับ "ลบทั้งกลุ่ม" (delete by batch_id) แบบถามผู้ใช้ก่อน
-- ============================================

ALTER TABLE academic_timetable_entries
    ADD COLUMN IF NOT EXISTS batch_id UUID;

CREATE INDEX IF NOT EXISTS idx_entries_batch_id
    ON academic_timetable_entries(batch_id)
    WHERE batch_id IS NOT NULL;

COMMENT ON COLUMN academic_timetable_entries.batch_id IS
    'UUID ร่วมของ entries ที่สร้างพร้อมกันจาก batch call; NULL = entry สร้างแยก (drag/create)';
