-- ============================================
-- Drop legacy tables: classes, enrollments, teaching_assignments
-- ============================================
-- แทนที่ด้วยระบบใหม่:
--   classes              → class_rooms (mig 016)
--   enrollments          → student_class_enrollments (mig 016)
--   teaching_assignments → classroom_courses + classroom_course_instructors (mig 024/076)
--                          + classroom_advisors (mig 099) สำหรับครูที่ปรึกษา
--
-- ไม่มี write path ที่ใช้ตารางเก่าใน code ปัจจุบัน (audit แล้ว) — ข้อมูลเก่าอยู่ในฐาน
-- ตาราง class_rooms/classroom_courses/classroom_advisors ครอบคลุมทุก use case
-- ============================================

DROP TABLE IF EXISTS teaching_assignments CASCADE;
DROP TABLE IF EXISTS enrollments CASCADE;
DROP TABLE IF EXISTS classes CASCADE;
