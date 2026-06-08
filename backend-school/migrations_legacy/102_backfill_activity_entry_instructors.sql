-- ============================================
-- Backfill timetable_entry_instructors สำหรับ entry ที่เป็น ACTIVITY
-- ============================================
-- ก่อน migration นี้ handlers ของ activity slot (add_slot_instructor,
-- batch_upsert_slot_classroom_assignments, ฯลฯ) ไม่ได้ propagate การเปลี่ยนแปลง
-- instructor ไปยัง timetable_entry_instructors ของ entry ที่มีอยู่แล้ว ทำให้
-- ตารางไม่แสดงชื่อครูใน cell กิจกรรม แม้ว่าจะ assign ครูถูกต้องใน source
--
-- migration นี้ sync ย้อนหลังครั้งเดียวให้ตรงกับ source-of-truth:
--   • Independent → activity_slot_classroom_assignments (1 ครู/ห้อง)
--   • Synchronized → activity_slot_instructors (ครูทุกคนของ slot → ทุก cell)
--
-- idempotent ด้วย ON CONFLICT DO NOTHING รันซ้ำได้ไม่เกิด duplicate
-- ============================================

-- Independent activity entries
INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
SELECT DISTINCT te.id, asca.instructor_id, 'primary'
FROM academic_timetable_entries te
JOIN activity_slots s ON s.id = te.activity_slot_id
JOIN activity_catalog ac ON ac.id = s.activity_catalog_id
JOIN activity_slot_classroom_assignments asca
     ON asca.slot_id = te.activity_slot_id
    AND asca.classroom_id = te.classroom_id
WHERE te.entry_type = 'ACTIVITY'
  AND ac.scheduling_mode = 'independent'
  AND te.is_active = true
ON CONFLICT (entry_id, instructor_id) DO NOTHING;

-- Synchronized activity entries
INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
SELECT DISTINCT te.id, asi.user_id, 'primary'
FROM academic_timetable_entries te
JOIN activity_slots s ON s.id = te.activity_slot_id
JOIN activity_catalog ac ON ac.id = s.activity_catalog_id
JOIN activity_slot_instructors asi ON asi.slot_id = te.activity_slot_id
WHERE te.entry_type = 'ACTIVITY'
  AND ac.scheduling_mode = 'synchronized'
  AND te.is_active = true
ON CONFLICT (entry_id, instructor_id) DO NOTHING;
