-- ============================================
-- academic_timetable_entries.classroom_id → nullable
-- ============================================
-- Teacher-only events (ประชุมครู, อบรมครู, เวร) ไม่ผูกกับห้องเรียนใด
-- entry ลักษณะนี้ classroom_id = NULL และมี tei (instructor junction) ปกติ
--
-- ประวัติ:
--   029 SET NOT NULL
--   059 DROP NOT NULL (activity entries เดิม)
--   066 SET NOT NULL (หลังย้าย activity ไป groups)
-- ============================================

ALTER TABLE academic_timetable_entries
    ALTER COLUMN classroom_id DROP NOT NULL;

COMMENT ON COLUMN academic_timetable_entries.classroom_id IS
    'NULL = teacher-only event (ไม่ผูกห้องเรียน) ลง tei เพื่อระบุครูที่เกี่ยวข้อง';
