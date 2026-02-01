-- Migration 033: Optimize Timetable Queries
-- Date: 2026-01-24
-- Description: Add missing indexes to improve performance of timetable conflict checks and listing

-- 1. Add index for Instructor lookup in Course Planning
-- Crucial for: Checking if an instructor is busy (INSTRUCTOR_CONFLICT check)
-- This allows the database to instantly find all courses taught by an instructor
-- instead of scanning the entire table.
CREATE INDEX IF NOT EXISTS idx_cc_instructor ON classroom_courses(primary_instructor_id);

-- 2. Add Composite Index for Timetable Retrieval
-- Crucial for: Fetching the entire timetable for a specific classroom in a specific semester.
-- This is the most common query on the timetable page.
-- Replaces usage of separate indexes on classroom_id and academic_semester_id for this specific query pattern.
CREATE INDEX IF NOT EXISTS idx_timetable_lookup_class_sem 
ON academic_timetable_entries(classroom_id, academic_semester_id);

-- 3. Add Composite Index for Room Conflict Check
-- Crucial for: Checking if a room is busy.
-- Creating a composite index on (room_id, day_of_week, period_id) makes the conflict check index-only (mostly)
-- and very exclusive.
CREATE INDEX IF NOT EXISTS idx_timetable_room_conflict 
ON academic_timetable_entries(room_id, day_of_week, period_id) 
WHERE is_active = true AND room_id IS NOT NULL;
