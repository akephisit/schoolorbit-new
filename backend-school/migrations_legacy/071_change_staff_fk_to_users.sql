-- ============================================
-- เปลี่ยน FK จาก staff_info(id) → users(id)
-- เพื่อให้ consistent กับ lookup API ที่ส่ง users.id
-- และไม่ต้อง resolve user_id → staff_info.id ทุกครั้ง
-- ============================================

-- 1. activity_groups.instructor_id
-- Migrate ค่าเดิม staff_info.id → users.id
UPDATE activity_groups ag SET instructor_id = si.user_id
FROM staff_info si WHERE si.id = ag.instructor_id AND ag.instructor_id IS NOT NULL;

ALTER TABLE activity_groups DROP CONSTRAINT IF EXISTS activity_groups_instructor_id_fkey;
ALTER TABLE activity_groups ADD CONSTRAINT activity_groups_instructor_id_fkey
    FOREIGN KEY (instructor_id) REFERENCES users(id) ON DELETE SET NULL;

-- 2. activity_group_instructors.instructor_id
UPDATE activity_group_instructors agi SET instructor_id = si.user_id
FROM staff_info si WHERE si.id = agi.instructor_id;

ALTER TABLE activity_group_instructors DROP CONSTRAINT IF EXISTS activity_group_instructors_instructor_id_fkey;
ALTER TABLE activity_group_instructors ADD CONSTRAINT activity_group_instructors_instructor_id_fkey
    FOREIGN KEY (instructor_id) REFERENCES users(id) ON DELETE CASCADE;

-- Recreate unique constraint (data already migrated)
ALTER TABLE activity_group_instructors DROP CONSTRAINT IF EXISTS unique_instructor_per_group;
ALTER TABLE activity_group_instructors ADD CONSTRAINT unique_instructor_per_group
    UNIQUE (activity_group_id, instructor_id);

-- 3. class_rooms.advisor_id
UPDATE class_rooms cr SET advisor_id = si.user_id
FROM staff_info si WHERE si.id = cr.advisor_id AND cr.advisor_id IS NOT NULL;

ALTER TABLE class_rooms DROP CONSTRAINT IF EXISTS class_rooms_advisor_id_fkey;
ALTER TABLE class_rooms ADD CONSTRAINT class_rooms_advisor_id_fkey
    FOREIGN KEY (advisor_id) REFERENCES users(id) ON DELETE SET NULL;

-- 4. class_rooms.co_advisor_id
UPDATE class_rooms cr SET co_advisor_id = si.user_id
FROM staff_info si WHERE si.id = cr.co_advisor_id AND cr.co_advisor_id IS NOT NULL;

ALTER TABLE class_rooms DROP CONSTRAINT IF EXISTS class_rooms_co_advisor_id_fkey;
ALTER TABLE class_rooms ADD CONSTRAINT class_rooms_co_advisor_id_fkey
    FOREIGN KEY (co_advisor_id) REFERENCES users(id) ON DELETE SET NULL;
