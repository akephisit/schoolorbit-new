-- ============================================
-- academic_timetable_entries: เพิ่ม 'ACADEMIC' ใน entry_type
-- ============================================
-- frontend Select dropdown มี option "วิชาการ" (value=ACADEMIC) อยู่
-- แต่ DB constraint เดิมจาก migration 029 อนุญาตแค่ COURSE/BREAK/ACTIVITY/HOMEROOM
-- → batch INSERT พังด้วย check constraint violation เมื่อ user เลือก "วิชาการ"
-- เพิ่ม ACADEMIC เข้า constraint (ไม่ทำลายข้อมูลเดิม)
-- ============================================

ALTER TABLE academic_timetable_entries
    DROP CONSTRAINT academic_timetable_entries_entry_type_check;

ALTER TABLE academic_timetable_entries
    ADD CONSTRAINT academic_timetable_entries_entry_type_check
    CHECK (entry_type IN ('COURSE', 'BREAK', 'ACTIVITY', 'HOMEROOM', 'ACADEMIC'));

COMMENT ON COLUMN academic_timetable_entries.entry_type IS
    'ประเภทรายการ: COURSE=วิชาเรียน, BREAK=พัก, ACTIVITY=กิจกรรม/ชุมนุม, HOMEROOM=โฮมรูม, ACADEMIC=วิชาการทั่วไป';
