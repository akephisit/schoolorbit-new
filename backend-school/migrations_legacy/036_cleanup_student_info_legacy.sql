-- ===================================================================
-- Migration: Clean up student_info to remove legacy string fields
-- Description: Remove grade_level and class_room text columns from student_info
--              as we now rely fully on relational student_class_enrollments.
-- ===================================================================

ALTER TABLE student_info 
DROP COLUMN IF EXISTS grade_level,
DROP COLUMN IF EXISTS class_room;
