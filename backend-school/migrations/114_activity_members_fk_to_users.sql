-- ============================================
-- เปลี่ยน FK activity_group_members.student_id
--   จาก student_info(id) → users(id)
--
-- เหตุผล: ทำให้สอดคล้องกับ M071 และ pattern หลักของระบบ
--   (student_class_enrollments, student_parents ก็ใช้ users.id)
--   หลัง migrate ไม่ต้อง resolve user_id → student_info.id ใน handler อีก
--
-- ผลพลอย: แก้ bug ที่ admin add_members และ get_activity_for_entry
--   (timetable.rs:1984) bind users.id ลง column ที่ FK ชี้ student_info.id
-- ============================================

-- 1. Migrate ค่าเดิม: ตอนนี้ student_id = student_info.id → แปลงเป็น user_id
UPDATE activity_group_members agm SET student_id = si.user_id
FROM student_info si WHERE si.id = agm.student_id;

-- 2. Drop FK เดิมที่ชี้ student_info(id)
ALTER TABLE activity_group_members DROP CONSTRAINT IF EXISTS activity_group_members_student_id_fkey;

-- 3. Add FK ใหม่ → users(id)
ALTER TABLE activity_group_members ADD CONSTRAINT activity_group_members_student_id_fkey
    FOREIGN KEY (student_id) REFERENCES users(id) ON DELETE CASCADE;

-- Note: UNIQUE(activity_group_id, student_id) และ idx_activity_members_student คงไว้เหมือนเดิม
--       — column name ไม่เปลี่ยน, semantics ของ uniqueness ยังถูกต้อง
