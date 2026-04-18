-- ============================================
-- Populate junction tables จากข้อมูลเดิม
-- ============================================

-- 1. classroom_courses.primary_instructor_id → classroom_course_instructors
INSERT INTO classroom_course_instructors (classroom_course_id, instructor_id, role)
SELECT id, primary_instructor_id, 'primary'
FROM classroom_courses
WHERE primary_instructor_id IS NOT NULL
ON CONFLICT DO NOTHING;

-- 2. Regular course entries → timetable_entry_instructors
INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
SELECT te.id, cc.primary_instructor_id, 'primary'
FROM academic_timetable_entries te
JOIN classroom_courses cc ON te.classroom_course_id = cc.id
WHERE cc.primary_instructor_id IS NOT NULL
  AND te.is_active = true
ON CONFLICT DO NOTHING;

-- 3. Synchronized activity entries (copy ทุก slot_instructor เข้าทุก entry)
INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
SELECT te.id, asi.user_id, 'primary'
FROM academic_timetable_entries te
JOIN activity_slots asl ON te.activity_slot_id = asl.id
JOIN activity_slot_instructors asi ON asi.slot_id = asl.id
WHERE asl.scheduling_mode = 'synchronized'
  AND te.is_active = true
ON CONFLICT DO NOTHING;

-- 4. Independent activity entries
INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
SELECT te.id, asca.instructor_id, 'primary'
FROM academic_timetable_entries te
JOIN activity_slot_classroom_assignments asca
  ON asca.slot_id = te.activity_slot_id AND asca.classroom_id = te.classroom_id
WHERE te.is_active = true
ON CONFLICT DO NOTHING;
