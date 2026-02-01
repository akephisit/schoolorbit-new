-- ===================================================================
-- Migration 017: Performance Tuning & Advanced Search
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

-- 2.1 Name Search (Check if columns exist just in case)
CREATE INDEX IF NOT EXISTS trgm_users_first_name ON users USING GIN (first_name gin_trgm_ops);
CREATE INDEX IF NOT EXISTS trgm_users_last_name ON users USING GIN (last_name gin_trgm_ops);

-- 2.2 Combined Name Index
CREATE INDEX IF NOT EXISTS trgm_users_full_name ON users USING GIN ((first_name || ' ' || last_name) gin_trgm_ops);

-- 2.3 Email & Phone Search
CREATE INDEX IF NOT EXISTS trgm_users_email ON users USING GIN (email gin_trgm_ops);
CREATE INDEX IF NOT EXISTS trgm_users_phone ON users USING GIN (phone gin_trgm_ops);

-- 2.4 User Nickname (Check if column exists)
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'users' AND column_name = 'nickname') THEN
        CREATE INDEX IF NOT EXISTS trgm_users_nickname ON users USING GIN (nickname gin_trgm_ops);
    END IF;
END $$;

-- ===================================================================
-- 3. Student Search Optimization
-- ===================================================================

-- 3.1 Student ID Search
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'student_info' AND column_name = 'student_id') THEN
        CREATE INDEX IF NOT EXISTS trgm_student_info_student_id ON student_info USING GIN (student_id gin_trgm_ops);
    END IF;
END $$;

-- 3.2 Optimize Filtering by Grade & Class
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'student_info' AND column_name = 'grade_level' AND column_name = 'class_room') THEN
        CREATE INDEX IF NOT EXISTS idx_student_info_grade_class ON student_info (grade_level, class_room);
    END IF;
END $$;

-- ===================================================================
-- 4. Staff Search Optimization
-- ===================================================================

-- 4.1 Employee ID Search (Removed as it may not be used/exist)
-- DO $$ ... END $$;

-- ===================================================================
-- 5. Status & User Type Optimization (Composite Indexes)
-- ===================================================================
-- These columns are standard, should exist
CREATE INDEX IF NOT EXISTS idx_users_type_status ON users (user_type, status);
CREATE INDEX IF NOT EXISTS idx_user_roles_check ON user_roles (user_id, role_id) WHERE ended_at IS NULL;

-- ===================================================================
-- 6. Achievements Search (Optimization for Migration 017)
-- ===================================================================
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = 'staff_achievements') THEN
        CREATE INDEX IF NOT EXISTS trgm_staff_achievements_title ON staff_achievements USING GIN (title gin_trgm_ops);
    END IF;
END $$;

-- ===================================================================
-- NOTES:
-- ===================================================================
-- - GIN indexes with gin_trgm_ops are larger than B-Tree but enable
--   super fast LIKE '%...%' queries which B-Tree cannot handle effectively.
-- - Without these, searching "Somchai" in a 100k user table requires scanning every row.
--   With GIN, it's instant.
-- ===================================================================
