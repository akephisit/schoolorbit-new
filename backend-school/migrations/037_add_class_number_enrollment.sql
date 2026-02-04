-- Add class_number into student_class_enrollments table
-- This allows defining student number per specific classroom enrollment (e.g. No. 1 in P.1/1)

ALTER TABLE student_class_enrollments
ADD COLUMN IF NOT EXISTS class_number INTEGER;

-- Create an index for efficiently sorting students by class number within a classroom
CREATE INDEX IF NOT EXISTS idx_student_class_enrollments_number ON student_class_enrollments(class_room_id, class_number);
