-- 032_update_timetable_entries_for_activities.sql

-- 1. Add new columns
ALTER TABLE academic_timetable_entries
ADD COLUMN entry_type VARCHAR(20) NOT NULL DEFAULT 'COURSE' CHECK (entry_type IN ('COURSE', 'BREAK', 'ACTIVITY', 'HOMEROOM')),
ADD COLUMN title VARCHAR(200),
ADD COLUMN classroom_id UUID REFERENCES class_rooms(id) ON DELETE CASCADE,
ADD COLUMN academic_semester_id UUID REFERENCES academic_semesters(id) ON DELETE CASCADE;

-- 2. Migrate existing data (Backfill IDs from classroom_courses)
UPDATE academic_timetable_entries te
SET 
    classroom_id = cc.classroom_id,
    academic_semester_id = cc.academic_semester_id
FROM classroom_courses cc
WHERE te.classroom_course_id = cc.id;

-- 3. Make columns NOT NULL (Ensure data integrity)
ALTER TABLE academic_timetable_entries
ALTER COLUMN classroom_id SET NOT NULL,
ALTER COLUMN academic_semester_id SET NOT NULL;

-- 4. Make classroom_course_id NULLABLE (To allow non-course entries like Lunch Break)
ALTER TABLE academic_timetable_entries
ALTER COLUMN classroom_course_id DROP NOT NULL;

-- 5. Add Indexes for performance
CREATE INDEX idx_timetable_classroom_id ON academic_timetable_entries(classroom_id);
CREATE INDEX idx_timetable_semester_id ON academic_timetable_entries(academic_semester_id);
CREATE INDEX idx_timetable_activities ON academic_timetable_entries(academic_semester_id, entry_type) WHERE entry_type != 'COURSE';

-- 6. Update Constraints to reflect new structure
-- Remove old constraint dependent on classroom_course_id
ALTER TABLE academic_timetable_entries DROP CONSTRAINT IF EXISTS unique_entry_per_slot;

-- Add new constraint: A classroom cannot have multiple/overlapping entries in the same slot within a semester
-- (Using unique index with WHERE clause to allow soft deletes if is_active is used, or just strictly unique)
CREATE UNIQUE INDEX unique_classroom_slot ON academic_timetable_entries(classroom_id, academic_semester_id, day_of_week, period_id) WHERE is_active = true;

-- Update Comments
COMMENT ON COLUMN academic_timetable_entries.entry_type IS 'ประเภทรายการ: COURSE=วิชาเรียน, BREAK=พัก, ACTIVITY=กิจกรรม/ชุมนุม';
COMMENT ON COLUMN academic_timetable_entries.title IS 'ชื่อรายการ (กรณีไม่ใช่ Course) เช่น พักเที่ยง';
COMMENT ON COLUMN academic_timetable_entries.classroom_id IS 'ห้องเรียนเจ้าของตาราง';
COMMENT ON COLUMN academic_timetable_entries.academic_semester_id IS 'ภาคเรียนที่ใช้ตารางนี้';
