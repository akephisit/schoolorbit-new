-- ===================================================================
-- Migration 018: Performance Tuning & Advanced Search
-- Description: Add pg_trgm extension and indexes for high-performance text search
--              Optimizes queries for users, students, and staff
-- Date: 2026-01-11
-- ===================================================================

-- ===================================================================
-- 1. Enable Extensions
-- ===================================================================
-- pg_trgm (Trigram) is essential for fast LIKE/ILIKE searches (e.g. '%query%')
CREATE EXTENSION IF NOT EXISTS pg_trgm;

-- ===================================================================
-- 2. Users Table Search Optimization
-- ===================================================================

-- 2.1 Name Search (Firstname & Lastname)
-- Allows extremely fast case-insensitive partial search: WHERE first_name ILIKE '%som%'
CREATE INDEX IF NOT EXISTS trgm_users_first_name ON users USING GIN (first_name gin_trgm_ops);
CREATE INDEX IF NOT EXISTS trgm_users_last_name ON users USING GIN (last_name gin_trgm_ops);

-- 2.2 Combined Name Index (Optional but useful for full name search concat)
-- Useful if searching "First Last" string
CREATE INDEX IF NOT EXISTS trgm_users_full_name ON users USING GIN ((first_name || ' ' || last_name) gin_trgm_ops);

-- 2.3 Email & Phone Search
CREATE INDEX IF NOT EXISTS trgm_users_email ON users USING GIN (email gin_trgm_ops);
CREATE INDEX IF NOT EXISTS trgm_users_phone ON users USING GIN (phone gin_trgm_ops);

-- 2.4 User Nickname (Often used for searching students)
CREATE INDEX IF NOT EXISTS trgm_users_nickname ON users USING GIN (nickname gin_trgm_ops);

-- ===================================================================
-- 3. Student Search Optimization
-- ===================================================================

-- 3.1 Student ID Search (Fast lookup by student ID partial or full)
CREATE INDEX IF NOT EXISTS trgm_student_info_student_id ON student_info USING GIN (student_id gin_trgm_ops);

-- 3.2 Optimize Filtering by Grade & Class
-- Composite index for frequent filtering: "Show all students in M.1/2"
CREATE INDEX IF NOT EXISTS idx_student_info_grade_class ON student_info (grade_level, class_room);

-- ===================================================================
-- 4. Staff Search Optimization
-- ===================================================================

-- 4.1 Employee ID Search
CREATE INDEX IF NOT EXISTS trgm_staff_info_employee_id ON staff_info USING GIN (employee_id gin_trgm_ops);

-- ===================================================================
-- 5. Status & User Type Optimization (Composite Indexes)
-- ===================================================================

-- 5.1 Active Users by Type
-- Extremely common query: "Get all active students" or "Get all suspended staff"
-- The existing indexes are on single columns. Composite is faster for AND conditions.
CREATE INDEX IF NOT EXISTS idx_users_type_status ON users (user_type, status);

-- 5.2 Role Assignments
-- Quick lookup for: "Is this user active in this role?"
-- Existing indexes cover FKs, but this composite covers the condition check
CREATE INDEX IF NOT EXISTS idx_user_roles_check ON user_roles (user_id, role_id) WHERE ended_at IS NULL;

-- ===================================================================
-- 6. Achievements Search (Optimization for Migration 017)
-- ===================================================================
-- Ensure searching achievements by title is fast
CREATE INDEX IF NOT EXISTS trgm_staff_achievements_title ON staff_achievements USING GIN (title gin_trgm_ops);

-- ===================================================================
-- NOTES:
-- ===================================================================
-- - GIN indexes with gin_trgm_ops are larger than B-Tree but enable
--   super fast LIKE '%...%' queries which B-Tree cannot handle effectively.
-- - Without these, searching "Somchai" in a 100k user table requires scanning every row.
--   With GIN, it's instant.
-- ===================================================================
